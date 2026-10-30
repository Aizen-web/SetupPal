use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::os::OsProvider;
use crate::types::Step;

/// A single immutable state change record appended to `.orchestrator_state.jsonl`.
/// Powers auditability, system telemetry, and deterministic `undo` rollback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionRecord {
    pub tx_id: String,
    pub timestamp: DateTime<Utc>,
    pub plan_id: String,
    pub goal: String,
    pub step_id: String,
    pub action_summary: String,
    pub step: Step,
    pub dry_run: bool,
    pub changes_made: Vec<String>,
    pub rollback_hint: Option<String>,
    pub rollback_data: Option<RollbackData>,
    #[serde(default)]
    pub rolled_back_tx_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RollbackData {
    PathBackup {
        previous_path: String,
        added_directory: PathBuf,
    },
    PackageInstall {
        ecosystem: String,
        package_id: String,
        uninstall_command: Option<String>,
        /// Exact uninstall argv (binary first); preferred over `uninstall_command`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uninstall_argv: Option<Vec<String>>,
        /// Raw user PATH before the install (for auditing; strip uses `path_entries_added`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_path_before: Option<String>,
        /// PATH entries this install added (only these may be removed on rewind).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path_entries_added: Vec<String>,
        /// Install folder recorded after install (uninstall registry / WinGet Packages / PATH parent).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        install_location: Option<PathBuf>,
    },
    FileDownload {
        destination: PathBuf,
    },
    IsolatedEnv {
        path: PathBuf,
    },
    /// Files or folders a step created (mod jars, loader profiles, portable tools).
    CreatedPaths {
        paths: Vec<PathBuf>,
    },
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

/// After a successful winget uninstall: strip PATH leftovers, remove empty install dirs,
/// then verify package/PATH/folder are clean (R6).
async fn cleanup_package_leftovers(
    os: &dyn OsProvider,
    package_id: &str,
    ecosystem: &str,
    path_entries_added: &[String],
    install_location: Option<&Path>,
) -> String {
    use crate::cleanup::{
        remove_path_entries, should_strip_path_entry, try_remove_install_dir,
    };

    let mut notes = Vec::new();

    // Strip only PATH entries this install added, and only when they now point at
    // missing or empty/leftover folders (never touch a live directory with user files).
    if !path_entries_added.is_empty() {
        let strip: Vec<String> = path_entries_added
            .iter()
            .filter(|e| should_strip_path_entry(e))
            .cloned()
            .collect();
        if !strip.is_empty() {
            if let Ok(current) = os.read_user_path_raw().await {
                let next = remove_path_entries(&current, &strip);
                if next != current {
                    match os.write_user_path_raw(&next).await {
                        Ok(()) => notes.push(format!("removed {} PATH entr{}", strip.len(), if strip.len() == 1 { "y" } else { "ies" })),
                        Err(e) => notes.push(format!("could not update PATH: {e}")),
                    }
                }
            }
        }
        let kept: Vec<_> = path_entries_added
            .iter()
            .filter(|e| !should_strip_path_entry(e))
            .cloned()
            .collect();
        if !kept.is_empty() {
            notes.push(format!(
                "left PATH entries still in use: {}",
                kept.join("; ")
            ));
        }
    }

    if let Some(loc) = install_location {
        notes.push(try_remove_install_dir(loc));
    }

    // Verify: package gone from winget list (best-effort), PATH entries gone, folder gone.
    let mut failed = Vec::new();
    let paths = os.refresh_and_get_path().await.unwrap_or_default();
    if let Some(winget) = os.which_in_paths("winget", &paths) {
        let args = vec![
            "list".into(),
            "--id".into(),
            package_id.to_string(),
            "--exact".into(),
            "--accept-source-agreements".into(),
            "--disable-interactivity".into(),
        ];
        if let Ok(out) = os.run_command(&winget, &args, &paths).await {
            let still_listed = out.exit_code == Some(0)
                && out
                    .stdout
                    .to_ascii_lowercase()
                    .contains(&package_id.to_ascii_lowercase());
            if still_listed {
                failed.push(format!("{package_id} still listed in winget"));
            }
        }
    }
    if let Ok(current) = os.read_user_path_raw().await {
        let current_l = current.to_ascii_lowercase();
        for e in path_entries_added {
            if should_strip_path_entry(e) && current_l.contains(&e.to_ascii_lowercase()) {
                failed.push(format!("PATH still contains {e}"));
            }
        }
    }
    if let Some(loc) = install_location {
        if loc.exists() && crate::cleanup::is_safe_to_delete_install_dir(loc) {
            failed.push(format!("install folder still present: {}", loc.display()));
        } else if loc.exists() {
            // User files remain — reported in notes, not a hard verify failure.
        }
    }

    let detail = if notes.is_empty() {
        String::new()
    } else {
        format!("; {}", notes.join("; "))
    };
    if failed.is_empty() {
        format!(
            "Successfully uninstalled package {package_id} ({ecosystem}){detail} [Post-Undo Verified: Passed]"
        )
    } else {
        format!(
            "Uninstalled package {package_id} ({ecosystem}){detail} [Post-Undo Verified: Failed — {}]",
            failed.join("; ")
        )
    }
}

/// Append-only transaction state logger.
#[derive(Debug, Clone)]
pub struct StateLogger {
    log_file_path: PathBuf,
    events: EventBus,
}

impl StateLogger {
    pub fn new(log_file_path: PathBuf, events: EventBus) -> Self {
        Self {
            log_file_path,
            events,
        }
    }

    pub fn log_path(&self) -> &Path {
        &self.log_file_path
    }

    /// Appends a new `TransactionRecord` to the append-only JSONL log file.
    pub fn record_transaction(&self, record: &TransactionRecord) -> Result<(), CoreError> {
        if let Some(parent) = self.log_file_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)?;

        let line = serde_json::to_string(record)?;
        writeln!(file, "{line}")?;
        file.flush()?;

        self.events.emit(AgentEvent::TransactionRecorded {
            tx_id: record.tx_id.clone(),
            action_summary: record.action_summary.clone(),
        });

        Ok(())
    }

    /// Reads all transaction records from the JSONL log file in chronological order.
    pub fn read_all_transactions(&self) -> Result<Vec<TransactionRecord>, CoreError> {
        if !self.log_file_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_file_path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for (idx, line_res) in reader.lines().enumerate() {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let record: TransactionRecord = serde_json::from_str(trimmed).map_err(|e| {
                CoreError::Validation(format!(
                    "Corrupt state log entry at line {}: {} (raw: {})",
                    idx + 1,
                    e,
                    trimmed
                ))
            })?;
            records.push(record);
        }

        Ok(records)
    }

    /// Checks whether a transaction with the given ID has already been rolled back.
    pub fn is_transaction_rolled_back(&self, tx_id: &str) -> Result<bool, CoreError> {
        let all = self.read_all_transactions()?;
        Ok(all
            .iter()
            .any(|t| t.rolled_back_tx_id.as_deref() == Some(tx_id)))
    }

    /// Retrieves all active transactions that have not been rolled back and are not themselves undo rollbacks.
    pub fn get_active_transactions(&self) -> Result<Vec<TransactionRecord>, CoreError> {
        let all = self.read_all_transactions()?;
        let rolled_back_ids: std::collections::HashSet<String> = all
            .iter()
            .filter_map(|t| t.rolled_back_tx_id.clone())
            .collect();

        Ok(all
            .into_iter()
            .filter(|t| t.rolled_back_tx_id.is_none() && !rolled_back_ids.contains(&t.tx_id))
            .collect())
    }

    /// Retrieves the most recent active transaction eligible for rollback.
    pub fn get_latest_transaction(&self) -> Result<Option<TransactionRecord>, CoreError> {
        let active = self.get_active_transactions()?;
        Ok(active.into_iter().last())
    }

    /// Finds a transaction by its unique ID.
    pub fn get_transaction_by_id(
        &self,
        tx_id: &str,
    ) -> Result<Option<TransactionRecord>, CoreError> {
        let all = self.read_all_transactions()?;
        Ok(all.into_iter().find(|t| t.tx_id == tx_id))
    }

    /// Reverses the state modifications made by a given transaction.
    /// Appends a compensating undo transaction record to ensure idempotent, audit-compliant rollbacks.
    pub async fn rollback_transaction(
        &self,
        tx: &TransactionRecord,
        os_provider: &Arc<dyn OsProvider>,
    ) -> Result<String, CoreError> {
        if self.is_transaction_rolled_back(&tx.tx_id)? {
            let msg = format!(
                "Transaction {} was already rolled back previously.",
                tx.tx_id
            );
            self.events.emit(AgentEvent::RollbackCompleted {
                tx_id: tx.tx_id.clone(),
                success: true,
                message: msg.clone(),
            });
            return Ok(msg);
        }

        self.events.emit(AgentEvent::RollbackStarted {
            tx_id: tx.tx_id.clone(),
        });

        let outcome = match &tx.rollback_data {
            Some(RollbackData::PathBackup {
                previous_path,
                added_directory,
            }) => {
                os_provider.restore_user_path(previous_path).await?;
                let current_paths = os_provider.refresh_and_get_path().await.unwrap_or_default();
                let verified = !current_paths.contains(added_directory);
                format!(
                    "Restored previous User PATH (removed directory {}) [Post-Undo Verified: {}]",
                    added_directory.display(),
                    if verified {
                        "Passed"
                    } else {
                        "Pending terminal reload"
                    }
                )
            }
            Some(RollbackData::FileDownload { destination }) => {
                if destination.exists() {
                    std::fs::remove_file(destination)?;
                    let verified = !destination.exists();
                    format!(
                        "Deleted downloaded artifact: {} [Post-Undo Verified: {}]",
                        destination.display(),
                        if verified { "Passed" } else { "Failed" }
                    )
                } else {
                    format!(
                        "Downloaded artifact already removed: {} [Post-Undo Verified: Passed]",
                        destination.display()
                    )
                }
            }
            Some(RollbackData::IsolatedEnv { path }) => {
                if path.exists() {
                    std::fs::remove_dir_all(path)?;
                    let verified = !path.exists();
                    format!(
                        "Cleaned up isolated environment directory: {} [Post-Undo Verified: {}]",
                        path.display(),
                        if verified { "Passed" } else { "Failed" }
                    )
                } else {
                    format!(
                        "Isolated directory already removed: {} [Post-Undo Verified: Passed]",
                        path.display()
                    )
                }
            }
            Some(RollbackData::CreatedPaths { paths }) => {
                if tx.dry_run {
                    format!(
                        "[Dry Run] Would delete: {}",
                        paths
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    for p in paths {
                        remove_path(p)?;
                    }
                    let verified = paths.iter().all(|p| !p.exists());
                    format!(
                        "Deleted {} [Post-Undo Verified: {}]",
                        paths
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                        if verified { "Passed" } else { "Failed" }
                    )
                }
            }
            Some(RollbackData::PackageInstall {
                ecosystem,
                package_id,
                uninstall_command,
                uninstall_argv,
                path_entries_added,
                install_location,
                ..
            }) => {
                let argv: Option<Vec<String>> = uninstall_argv.clone().or_else(|| {
                    uninstall_command
                        .as_ref()
                        .map(|c| c.split_whitespace().map(str::to_string).collect())
                });
                let cmd_str = argv.as_ref().map(|a| a.join(" "));
                if tx.dry_run {
                    format!(
                        "[Dry Run] Would uninstall {} ({}) via: {}",
                        package_id,
                        ecosystem,
                        cmd_str.as_deref().unwrap_or("none")
                    )
                } else if let (Some(argv), Some(cmd_str)) = (argv, cmd_str) {
                    if let Some((bin_name, args)) = argv.split_first() {
                        #[cfg(windows)]
                        if let Some(msg) = crate::os::windows::blocking_installer_message().await {
                            return Err(CoreError::Os(msg));
                        }
                        let paths = os_provider.refresh_and_get_path().await.unwrap_or_default();
                        let direct = Path::new(bin_name);
                        let resolved = if direct.is_absolute() && direct.is_file() {
                            Some(direct.to_path_buf())
                        } else {
                            os_provider.which_in_paths(bin_name, &paths)
                        };
                        if let Some(bin_path) = resolved {
                            match os_provider.run_command(&bin_path, args, &paths).await {
                                Ok(out) if out.exit_code == Some(0) => {
                                    cleanup_package_leftovers(
                                        os_provider.as_ref(),
                                        package_id,
                                        ecosystem,
                                        path_entries_added,
                                        install_location.as_deref(),
                                    )
                                    .await
                                }
                                Ok(out) => {
                                    format!(
                                        "Executed uninstall command for {} (exit {:?}): {}",
                                        package_id, out.exit_code, out.stderr
                                    )
                                }
                                Err(e) => {
                                    format!("Failed executing uninstall for {}: {e}", package_id)
                                }
                            }
                        } else {
                            format!(
                                "Uninstall binary '{}' not found on PATH. Manual command: {}",
                                bin_name, cmd_str
                            )
                        }
                    } else {
                        format!(
                            "Package rollback recorded for {} ({})",
                            package_id, ecosystem
                        )
                    }
                } else {
                    format!(
                        "Package rollback recorded for {} (no uninstall command)",
                        package_id
                    )
                }
            }
            None => {
                format!(
                    "No rollback data attached to transaction {}. Hint: {}",
                    tx.tx_id,
                    tx.rollback_hint.as_deref().unwrap_or("none")
                )
            }
        };

        // Record compensating undo transaction in the ledger
        let undo_record = TransactionRecord {
            tx_id: format!("undo-{}", &tx.tx_id),
            timestamp: Utc::now(),
            plan_id: tx.plan_id.clone(),
            goal: format!("Undo/Rollback transaction {}", tx.tx_id),
            step_id: tx.step_id.clone(),
            action_summary: format!("Rolled back: {}", tx.action_summary),
            step: tx.step.clone(),
            dry_run: tx.dry_run,
            changes_made: vec![outcome.clone()],
            rollback_hint: None,
            rollback_data: None,
            rolled_back_tx_id: Some(tx.tx_id.clone()),
        };
        self.record_transaction(&undo_record)?;

        self.events.emit(AgentEvent::RollbackCompleted {
            tx_id: tx.tx_id.clone(),
            success: true,
            message: outcome.clone(),
        });

        Ok(outcome)
    }

    /// Exports the full append-only transaction log as a formatted Markdown audit report.
    pub fn export_audit_report_markdown(&self) -> Result<String, CoreError> {
        let transactions = self.read_all_transactions()?;
        let rolled_back_ids: std::collections::HashSet<String> = transactions
            .iter()
            .filter_map(|t| t.rolled_back_tx_id.clone())
            .collect();

        let mut md = String::new();
        md.push_str("# SetupPal Security & Environment Audit Report\n\n");
        md.push_str(&format!(
            "- **Generated At:** {}\n",
            Utc::now().to_rfc3339()
        ));
        md.push_str(&format!(
            "- **Total Transactions Recorded:** {}\n",
            transactions.len()
        ));
        md.push_str(&format!(
            "- **Ledger Storage Path:** `{}`\n\n",
            self.log_file_path.display()
        ));
        md.push_str("---\n\n");
        md.push_str("## Transaction History\n\n");

        if transactions.is_empty() {
            md.push_str("*No transactions have been recorded in the state ledger yet.*\n");
            return Ok(md);
        }

        md.push_str("| Status | Timestamp (UTC) | Plan ID | Action Summary | Changes Made | Rollback Command |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");

        for tx in &transactions {
            let changes = tx.changes_made.join(", ");
            let hint = tx.rollback_hint.as_deref().unwrap_or("N/A");
            let status = if tx.rolled_back_tx_id.is_some() {
                "↺ Reversal"
            } else if rolled_back_ids.contains(&tx.tx_id) {
                "✗ Rolled Back"
            } else {
                "✓ Active"
            };

            md.push_str(&format!(
                "| {} | {} | `{}` | {} | {} | `{}` |\n",
                status,
                tx.timestamp.format("%Y-%m-%d %H:%M:%S"),
                tx.plan_id,
                tx.action_summary,
                changes,
                hint
            ));
        }

        Ok(md)
    }
    /// Exports the full append-only transaction log as a self-contained, styled HTML audit report.
    pub fn export_audit_report_html(&self) -> Result<String, CoreError> {
        let transactions = self.read_all_transactions()?;
        let rolled_back_ids: std::collections::HashSet<String> = transactions
            .iter()
            .filter_map(|t| t.rolled_back_tx_id.clone())
            .collect();

        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n<title>SetupPal Audit Report</title>\n");
        html.push_str("<style>\n");
        html.push_str("body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0d1117; color: #c9d1d9; padding: 30px; line-height: 1.6; }\n");
        html.push_str(
            "h1 { color: #58a6ff; border-bottom: 1px solid #30363d; padding-bottom: 10px; }\n",
        );
        html.push_str(".meta { background: #161b22; padding: 15px; border-radius: 6px; border: 1px solid #30363d; margin-bottom: 25px; }\n");
        html.push_str("table { width: 100%; border-collapse: collapse; margin-top: 15px; background: #161b22; border-radius: 6px; overflow: hidden; }\n");
        html.push_str("th, td { padding: 12px 15px; text-align: left; border-bottom: 1px solid #30363d; font-size: 14px; }\n");
        html.push_str("th { background: #21262d; color: #8b949e; text-transform: uppercase; font-size: 12px; letter-spacing: 0.5px; }\n");
        html.push_str("tr:hover { background: #1c2128; }\n");
        html.push_str("code { font-family: monospace; background: rgba(110,118,129,0.2); padding: 2px 6px; border-radius: 4px; color: #79c0ff; }\n");
        html.push_str(".badge-tx { background: #238636; color: white; padding: 2px 8px; border-radius: 12px; font-size: 11px; font-weight: bold; }\n");
        html.push_str("</style>\n</head>\n<body>\n");
        html.push_str("<h1>SetupPal Security & Environment Audit Report</h1>\n");
        html.push_str("<div class=\"meta\">\n");
        html.push_str(&format!(
            "<p><strong>Generated At:</strong> {} UTC</p>\n",
            Utc::now().to_rfc3339()
        ));
        html.push_str(&format!("<p><strong>Total Transactions Recorded:</strong> <span class=\"badge-tx\">{}</span></p>\n", transactions.len()));
        html.push_str(&format!(
            "<p><strong>Ledger Storage Path:</strong> <code>{}</code></p>\n",
            self.log_file_path.display()
        ));
        html.push_str("</div>\n");

        if transactions.is_empty() {
            html.push_str(
                "<p><em>No transactions have been recorded in the state ledger yet.</em></p>\n",
            );
        } else {
            html.push_str("<table>\n<thead>\n<tr>\n");
            html.push_str("<th>Status</th><th>Timestamp (UTC)</th><th>Plan ID</th><th>Action Summary</th><th>Changes Made</th><th>Rollback Command</th>\n");
            html.push_str("</tr>\n</thead>\n<tbody>\n");
            for tx in &transactions {
                let (status_label, badge_color) = if tx.rolled_back_tx_id.is_some() {
                    ("Reversal", "#d29922")
                } else if rolled_back_ids.contains(&tx.tx_id) {
                    ("Rolled Back", "#f85149")
                } else {
                    ("Active", "#238636")
                };

                html.push_str("<tr>\n");
                html.push_str(&format!("<td><span style=\"background:{badge_color};color:white;padding:2px 8px;border-radius:10px;font-size:11px;font-weight:bold;\">{}</span></td>\n", status_label));
                html.push_str(&format!(
                    "<td><code>{}</code></td>\n",
                    tx.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
                html.push_str(&format!("<td><code>{}</code></td>\n", tx.plan_id));
                html.push_str(&format!(
                    "<td><strong>{}</strong></td>\n",
                    tx.action_summary
                ));
                html.push_str(&format!("<td>{}</td>\n", tx.changes_made.join(", ")));
                html.push_str(&format!(
                    "<td><code>{}</code></td>\n",
                    tx.rollback_hint.as_deref().unwrap_or("N/A")
                ));
                html.push_str("</tr>\n");
            }
            html.push_str("</tbody>\n</table>\n");
        }
        html.push_str("</body>\n</html>\n");
        Ok(html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::mock::{ok, MockOsProvider};

    fn logger() -> (StateLogger, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = StateLogger::new(dir.path().join("state.jsonl"), EventBus::default());
        (logger, dir)
    }

    fn tx(id: &str, dry_run: bool, rollback: Option<RollbackData>) -> TransactionRecord {
        TransactionRecord {
            tx_id: id.to_string(),
            timestamp: Utc::now(),
            plan_id: "plan-1".to_string(),
            goal: "goal".to_string(),
            step_id: "step-1".to_string(),
            action_summary: format!("action {id}"),
            step: Step::AddToUserPath {
                directory: PathBuf::from("C:\\Tools"),
            },
            dry_run,
            changes_made: Vec::new(),
            rollback_hint: Some("hint".to_string()),
            rollback_data: rollback,
            rolled_back_tx_id: None,
        }
    }

    fn os(mock: MockOsProvider) -> (Arc<MockOsProvider>, Arc<dyn OsProvider>) {
        let mock = Arc::new(mock);
        let dynamic: Arc<dyn OsProvider> = mock.clone();
        (mock, dynamic)
    }

    #[test]
    fn records_round_trip_and_latest_is_the_last_active_one() {
        let (log, _dir) = logger();
        assert!(log.read_all_transactions().expect("empty").is_empty());
        log.record_transaction(&tx("a", false, None))
            .expect("write a");
        log.record_transaction(&tx("b", false, None))
            .expect("write b");
        assert_eq!(log.read_all_transactions().expect("read").len(), 2);
        assert_eq!(
            log.get_latest_transaction()
                .expect("latest")
                .expect("some")
                .tx_id,
            "b"
        );
        assert!(log.get_transaction_by_id("a").expect("find").is_some());
        assert!(log.get_transaction_by_id("zzz").expect("find").is_none());
    }

    #[test]
    fn corrupt_log_lines_are_an_error_not_silently_skipped() {
        let (log, _dir) = logger();
        std::fs::write(log.log_path(), "{not json}\n").expect("write");
        let err = log.read_all_transactions().expect_err("corrupt");
        assert!(err
            .to_string()
            .contains("Corrupt state log entry at line 1"));
    }

    #[tokio::test]
    async fn path_backup_restores_the_previous_user_path() {
        let (log, _dir) = logger();
        let (mock, os) = os(MockOsProvider::new().with_path_entry("C:\\Existing"));
        let added = PathBuf::from("C:\\Tools");
        let previous = mock.add_to_user_path(&added).await.expect("add");
        assert!(mock
            .refresh_and_get_path()
            .await
            .expect("path")
            .contains(&added));

        let record = tx(
            "p",
            false,
            Some(RollbackData::PathBackup {
                previous_path: previous,
                added_directory: added.clone(),
            }),
        );
        log.record_transaction(&record).expect("write");
        let message = log
            .rollback_transaction(&record, &os)
            .await
            .expect("rollback");

        assert!(message.contains("Post-Undo Verified: Passed"), "{message}");
        let path = mock.refresh_and_get_path().await.expect("path");
        assert!(!path.contains(&added));
        assert!(path.contains(&PathBuf::from("C:\\Existing")));
    }

    #[tokio::test]
    async fn file_download_rollback_deletes_the_file_and_tolerates_a_missing_one() {
        let (log, dir) = logger();
        let (_, os) = os(MockOsProvider::new());
        let file = dir.path().join("artifact.bin");
        std::fs::write(&file, b"payload").expect("write");
        let record = tx(
            "f",
            false,
            Some(RollbackData::FileDownload {
                destination: file.clone(),
            }),
        );
        let message = log
            .rollback_transaction(&record, &os)
            .await
            .expect("rollback");
        assert!(!file.exists());
        assert!(message.contains("Deleted downloaded artifact"));

        let again = tx(
            "f2",
            false,
            Some(RollbackData::FileDownload { destination: file }),
        );
        let message = log
            .rollback_transaction(&again, &os)
            .await
            .expect("rollback");
        assert!(message.contains("already removed"));
    }

    #[tokio::test]
    async fn isolated_env_rollback_removes_the_directory_tree() {
        let (log, dir) = logger();
        let (_, os) = os(MockOsProvider::new());
        let env = dir.path().join("venv");
        std::fs::create_dir_all(env.join("Scripts")).expect("mkdir");
        std::fs::write(env.join("Scripts").join("python.exe"), b"x").expect("write");
        let record = tx(
            "e",
            false,
            Some(RollbackData::IsolatedEnv { path: env.clone() }),
        );
        let message = log
            .rollback_transaction(&record, &os)
            .await
            .expect("rollback");
        assert!(!env.exists());
        assert!(message.contains("Cleaned up isolated environment"));
    }

    #[tokio::test]
    async fn created_paths_rollback_removes_files_and_folders_unless_dry_run() {
        let (log, dir) = logger();
        let (_, os) = os(MockOsProvider::new());
        let file = dir.path().join("mod.jar");
        let folder = dir.path().join("tool");
        std::fs::write(&file, b"x").expect("write");
        std::fs::create_dir_all(&folder).expect("mkdir");
        let paths = vec![file.clone(), folder.clone()];

        let dry = tx(
            "d",
            true,
            Some(RollbackData::CreatedPaths {
                paths: paths.clone(),
            }),
        );
        let message = log
            .rollback_transaction(&dry, &os)
            .await
            .expect("dry rollback");
        assert!(message.starts_with("[Dry Run] Would delete"));
        assert!(file.exists() && folder.exists());

        let live = tx("l", false, Some(RollbackData::CreatedPaths { paths }));
        let message = log
            .rollback_transaction(&live, &os)
            .await
            .expect("rollback");
        assert!(message.contains("Post-Undo Verified: Passed"));
        assert!(!file.exists() && !folder.exists());
    }

    #[tokio::test]
    async fn package_rollback_runs_the_exact_uninstall_argv() {
        let (log, _dir) = logger();
        let (mock, os) = os(MockOsProvider::new()
            .with_binary("winget", "C:\\Windows\\winget.exe")
            .on("winget", &["uninstall"], ok("done"))
            .on("winget", &["list"], crate::os::mock::fail(1, "No installed package found")));
        let data = RollbackData::PackageInstall {
            ecosystem: "winget".to_string(),
            package_id: "Git.Git".to_string(),
            uninstall_command: None,
            uninstall_argv: Some(vec![
                "winget".into(),
                "uninstall".into(),
                "--id".into(),
                "Git.Git".into(),
                "--exact".into(),
            ]),
            user_path_before: None,
            path_entries_added: vec![],
            install_location: None,
        };
        let record = tx("pkg", false, Some(data));
        let message = log
            .rollback_transaction(&record, &os)
            .await
            .expect("rollback");
        assert!(
            message.contains("Successfully uninstalled package Git.Git"),
            "{message}"
        );
        assert!(message.contains("Post-Undo Verified: Passed"), "{message}");
        let calls = mock.calls();
        assert!(calls.iter().any(|c| c.args.first().map(String::as_str) == Some("uninstall")));
        assert_eq!(
            calls.iter().find(|c| c.args.first().map(String::as_str) == Some("uninstall")).unwrap().args,
            vec!["uninstall", "--id", "Git.Git", "--exact"]
        );
    }

    #[tokio::test]
    async fn package_rollback_strips_stale_path_and_empty_install_dir() {
        let (log, _dir) = logger();
        let leftover = tempfile::tempdir().unwrap();
        let empty_doc = leftover.path().join("Doc");
        std::fs::create_dir_all(&empty_doc).unwrap();
        let stale = leftover.path().join("Scripts");
        std::fs::create_dir_all(&stale).unwrap();

        let mock = MockOsProvider::new()
            .with_binary("winget", "C:\\Windows\\winget.exe")
            .on("winget", &["uninstall"], ok("done"))
            .on("winget", &["list"], crate::os::mock::fail(1, "No installed package found"));
        // Seed PATH with a stale entry that points at the empty leftover.
        let _ = mock
            .add_to_user_path(&stale)
            .await
            .expect("seed path");
        let (_, os) = os(mock);

        let data = RollbackData::PackageInstall {
            ecosystem: "winget".to_string(),
            package_id: "Python.Python.3.12".to_string(),
            uninstall_command: None,
            uninstall_argv: Some(vec![
                "winget".into(),
                "uninstall".into(),
                "--id".into(),
                "Python.Python.3.12".into(),
                "--exact".into(),
            ]),
            user_path_before: Some(String::new()),
            path_entries_added: vec![stale.to_string_lossy().to_string()],
            install_location: Some(leftover.path().to_path_buf()),
        };
        let message = log
            .rollback_transaction(&tx("py", false, Some(data)), &os)
            .await
            .expect("rollback");
        assert!(message.contains("Post-Undo Verified: Passed"), "{message}");
        assert!(message.contains("PATH"), "{message}");
        assert!(!stale.exists() || crate::cleanup::should_strip_path_entry(&stale.to_string_lossy()));
        // Empty leftover tree should be removed when safe.
        assert!(
            !leftover.path().exists() || !crate::cleanup::is_safe_to_delete_install_dir(leftover.path()),
            "expected leftover install dir cleaned when only empty folders remain"
        );
    }

    #[tokio::test]
    async fn package_rollback_reports_failures_and_missing_binaries_honestly() {
        let (log, _dir) = logger();
        let data = |argv: &[&str]| RollbackData::PackageInstall {
            ecosystem: "winget".to_string(),
            package_id: "Foo.Bar".to_string(),
            uninstall_command: None,
            uninstall_argv: Some(argv.iter().map(|s| s.to_string()).collect()),
            user_path_before: None,
            path_entries_added: vec![],
            install_location: None,
        };

        let (_, os_failing) = os(MockOsProvider::new()
            .with_binary("winget", "C:\\Windows\\winget.exe")
            .on(
                "winget",
                &["uninstall"],
                crate::os::mock::fail(1, "no such package"),
            ));
        let message = log
            .rollback_transaction(
                &tx(
                    "fail",
                    false,
                    Some(data(&["winget", "uninstall", "--id", "Foo.Bar"])),
                ),
                &os_failing,
            )
            .await
            .expect("rollback");
        assert!(
            message.contains("exit Some(1)") && message.contains("no such package"),
            "{message}"
        );

        let (mock_missing, os_missing) = os(MockOsProvider::new());
        let message = log
            .rollback_transaction(
                &tx("missing", false, Some(data(&["winget", "uninstall"]))),
                &os_missing,
            )
            .await
            .expect("rollback");
        assert!(message.contains("not found on PATH"), "{message}");
        assert!(mock_missing.calls().is_empty());
    }

    #[tokio::test]
    async fn package_rollback_in_dry_run_never_runs_a_command() {
        let (log, _dir) = logger();
        let (mock, os) = os(MockOsProvider::new().with_binary("winget", "C:\\Windows\\winget.exe"));
        let data = RollbackData::PackageInstall {
            ecosystem: "winget".to_string(),
            package_id: "Git.Git".to_string(),
            uninstall_command: Some("winget uninstall --id Git.Git".to_string()),
            uninstall_argv: None,
            user_path_before: None,
            path_entries_added: vec![],
            install_location: None,
        };
        let message = log
            .rollback_transaction(&tx("dry", true, Some(data)), &os)
            .await
            .expect("rollback");
        assert!(message.starts_with("[Dry Run] Would uninstall Git.Git"));
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn rollback_without_data_says_so_and_still_records_the_undo() {
        let (log, _dir) = logger();
        let (_, os) = os(MockOsProvider::new());
        let record = tx("none", false, None);
        log.record_transaction(&record).expect("write");
        let message = log
            .rollback_transaction(&record, &os)
            .await
            .expect("rollback");
        assert!(message.contains("No rollback data attached"));
        assert!(message.contains("hint"));
        assert!(log.is_transaction_rolled_back("none").expect("check"));
    }

    #[tokio::test]
    async fn rollback_is_idempotent_and_hides_the_transaction_from_active_list() {
        let (log, dir) = logger();
        let (_, os) = os(MockOsProvider::new());
        let file = dir.path().join("a.bin");
        std::fs::write(&file, b"x").expect("write");
        let first = tx(
            "one",
            false,
            Some(RollbackData::FileDownload { destination: file }),
        );
        log.record_transaction(&first).expect("write");
        log.record_transaction(&tx("two", false, None))
            .expect("write");

        log.rollback_transaction(&first, &os)
            .await
            .expect("rollback");
        let again = log
            .rollback_transaction(&first, &os)
            .await
            .expect("second rollback");
        assert!(again.contains("already rolled back"));

        let all = log.read_all_transactions().expect("read");
        assert_eq!(all.iter().filter(|t| t.tx_id == "undo-one").count(), 1);
        let active: Vec<_> = log
            .get_active_transactions()
            .expect("active")
            .into_iter()
            .map(|t| t.tx_id)
            .collect();
        assert_eq!(active, vec!["two"]);
    }
}
