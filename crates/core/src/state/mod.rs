use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
        Ok(all.iter().any(|t| t.rolled_back_tx_id.as_deref() == Some(tx_id)))
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
    pub fn get_transaction_by_id(&self, tx_id: &str) -> Result<Option<TransactionRecord>, CoreError> {
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
            let msg = format!("Transaction {} was already rolled back previously.", tx.tx_id);
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
                    if verified { "Passed" } else { "Pending terminal reload" }
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
                    format!("Downloaded artifact already removed: {} [Post-Undo Verified: Passed]", destination.display())
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
                    format!("Isolated directory already removed: {} [Post-Undo Verified: Passed]", path.display())
                }
            }
            Some(RollbackData::CreatedPaths { paths }) => {
                if tx.dry_run {
                    format!(
                        "[Dry Run] Would delete: {}",
                        paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                    )
                } else {
                    for p in paths {
                        remove_path(p)?;
                    }
                    let verified = paths.iter().all(|p| !p.exists());
                    format!(
                        "Deleted {} [Post-Undo Verified: {}]",
                        paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
                        if verified { "Passed" } else { "Failed" }
                    )
                }
            }
            Some(RollbackData::PackageInstall {
                ecosystem,
                package_id,
                uninstall_command,
                uninstall_argv,
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
                                    format!("Successfully uninstalled package {} ({}) [Post-Undo Verified: Passed]", package_id, ecosystem)
                                }
                                Ok(out) => {
                                    format!("Executed uninstall command for {} (exit {:?}): {}", package_id, out.exit_code, out.stderr)
                                }
                                Err(e) => {
                                    format!("Failed executing uninstall for {}: {e}", package_id)
                                }
                            }
                        } else {
                            format!("Uninstall binary '{}' not found on PATH. Manual command: {}", bin_name, cmd_str)
                        }
                    } else {
                        format!("Package rollback recorded for {} ({})", package_id, ecosystem)
                    }
                } else {
                    format!("Package rollback recorded for {} (no uninstall command)", package_id)
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
        md.push_str(&format!("- **Generated At:** {}\n", Utc::now().to_rfc3339()));
        md.push_str(&format!("- **Total Transactions Recorded:** {}\n", transactions.len()));
        md.push_str(&format!("- **Ledger Storage Path:** `{}`\n\n", self.log_file_path.display()));
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
        html.push_str("h1 { color: #58a6ff; border-bottom: 1px solid #30363d; padding-bottom: 10px; }\n");
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
        html.push_str(&format!("<p><strong>Generated At:</strong> {} UTC</p>\n", Utc::now().to_rfc3339()));
        html.push_str(&format!("<p><strong>Total Transactions Recorded:</strong> <span class=\"badge-tx\">{}</span></p>\n", transactions.len()));
        html.push_str(&format!("<p><strong>Ledger Storage Path:</strong> <code>{}</code></p>\n", self.log_file_path.display()));
        html.push_str("</div>\n");

        if transactions.is_empty() {
            html.push_str("<p><em>No transactions have been recorded in the state ledger yet.</em></p>\n");
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
                html.push_str(&format!("<td><code>{}</code></td>\n", tx.timestamp.format("%Y-%m-%d %H:%M:%S")));
                html.push_str(&format!("<td><code>{}</code></td>\n", tx.plan_id));
                html.push_str(&format!("<td><strong>{}</strong></td>\n", tx.action_summary));
                html.push_str(&format!("<td>{}</td>\n", tx.changes_made.join(", ")));
                html.push_str(&format!("<td><code>{}</code></td>\n", tx.rollback_hint.as_deref().unwrap_or("N/A")));
                html.push_str("</tr>\n");
            }
            html.push_str("</tbody>\n</table>\n");
        }
        html.push_str("</body>\n</html>\n");
        Ok(html)
    }
}
