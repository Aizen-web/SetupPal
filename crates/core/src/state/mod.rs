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
    },
    FileDownload {
        destination: PathBuf,
    },
    IsolatedEnv {
        path: PathBuf,
    },
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
            Some(RollbackData::PackageInstall {
                ecosystem,
                package_id,
                uninstall_command,
            }) => {
                if tx.dry_run {
                    format!(
                        "[Dry Run] Would uninstall {} ({}) via: {}",
                        package_id,
                        ecosystem,
                        uninstall_command.as_deref().unwrap_or("none")
                    )
                } else if let Some(cmd_str) = uninstall_command {
                    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
                    if let Some((bin_name, args)) = parts.split_first() {
                        let paths = os_provider.refresh_and_get_path().await.unwrap_or_default();
                        if let Some(bin_path) = os_provider.which_in_paths(bin_name, &paths) {
                            let arg_strings: Vec<String> = args.iter().map(|s| s.to_string()).collect();
                            match os_provider.run_command(&bin_path, &arg_strings, &paths).await {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::MockOsProvider;
    use crate::types::{AdapterKind, Ecosystem};
    use uuid::Uuid;

    #[tokio::test]
    async fn state_logger_records_and_reads_transactions() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let log_file = temp_dir.path().join("test_state.jsonl");
        let bus = EventBus::new(16);
        let logger = StateLogger::new(log_file, bus);

        let tx = TransactionRecord {
            tx_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            plan_id: "plan-123".to_string(),
            goal: "set me up for C++20 dev".to_string(),
            step_id: "step-1".to_string(),
            action_summary: "Install LLVM.LLVM".to_string(),
            step: Step::InstallPackage {
                ecosystem: Ecosystem::Winget,
                package_id: "LLVM.LLVM".to_string(),
                version: Some("18.1.8".to_string()),
                source_adapter: AdapterKind::Winget,
            },
            dry_run: true,
            changes_made: vec!["Simulated LLVM install".to_string()],
            rollback_hint: Some("winget uninstall --id LLVM.LLVM".to_string()),
            rollback_data: Some(RollbackData::PackageInstall {
                ecosystem: "winget".to_string(),
                package_id: "LLVM.LLVM".to_string(),
                uninstall_command: Some("winget uninstall --id LLVM.LLVM".to_string()),
            }),
            rolled_back_tx_id: None,
        };

        logger.record_transaction(&tx).expect("record transaction");

        let all = logger.read_all_transactions().expect("read transactions");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].tx_id, tx.tx_id);
        assert_eq!(all[0].goal, "set me up for C++20 dev");

        let latest = logger.get_latest_transaction().expect("latest").expect("some");
        assert_eq!(latest.tx_id, tx.tx_id);

        let os: Arc<dyn OsProvider> = Arc::new(MockOsProvider::new());
        let res = logger.rollback_transaction(&tx, &os).await.expect("rollback");
        assert!(res.contains("LLVM.LLVM"));
    }

    #[tokio::test]
    async fn state_logger_rollback_cleans_up_files_and_directories() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let log_file = temp_dir.path().join("test_state.jsonl");
        let logger = StateLogger::new(log_file, EventBus::new(16));
        let os: Arc<dyn OsProvider> = Arc::new(MockOsProvider::new());

        // 1. File download rollback test
        let test_file = temp_dir.path().join("downloaded.jar");
        std::fs::write(&test_file, b"test content").unwrap();
        assert!(test_file.exists());

        let tx_file = TransactionRecord {
            tx_id: "tx-file".to_string(),
            timestamp: Utc::now(),
            plan_id: "p1".to_string(),
            goal: "download".to_string(),
            step_id: "s1".to_string(),
            action_summary: "download artifact".to_string(),
            step: Step::VerifyHash {
                file_path: test_file.clone(),
                expected_sha256: "abc".to_string(),
            },
            dry_run: false,
            changes_made: vec![],
            rollback_hint: None,
            rollback_data: Some(RollbackData::FileDownload {
                destination: test_file.clone(),
            }),
            rolled_back_tx_id: None,
        };
        let res1 = logger.rollback_transaction(&tx_file, &os).await.unwrap();
        assert!(res1.contains("Deleted downloaded artifact"));
        assert!(!test_file.exists());

        // 2. Isolated env directory rollback test
        let test_env = temp_dir.path().join(".test-env");
        std::fs::create_dir_all(&test_env).unwrap();
        assert!(test_env.exists());

        let tx_env = TransactionRecord {
            tx_id: "tx-env".to_string(),
            timestamp: Utc::now(),
            plan_id: "p2".to_string(),
            goal: "create env".to_string(),
            step_id: "s2".to_string(),
            action_summary: "create env".to_string(),
            step: Step::CreateIsolatedEnv {
                kind: crate::types::IsolationKind::PythonVenv,
                path: test_env.clone(),
            },
            dry_run: false,
            changes_made: vec![],
            rollback_hint: None,
            rollback_data: Some(RollbackData::IsolatedEnv {
                path: test_env.clone(),
            }),
            rolled_back_tx_id: None,
        };
        let res2 = logger.rollback_transaction(&tx_env, &os).await.unwrap();
        assert!(res2.contains("Cleaned up isolated environment"));
        assert!(!test_env.exists());

        // 3. User PATH rollback test
        let tx_path = TransactionRecord {
            tx_id: "tx-path".to_string(),
            timestamp: Utc::now(),
            plan_id: "p3".to_string(),
            goal: "path".to_string(),
            step_id: "s3".to_string(),
            action_summary: "add to path".to_string(),
            step: Step::AddToUserPath {
                directory: PathBuf::from("C:\\fake\\bin"),
            },
            dry_run: false,
            changes_made: vec![],
            rollback_hint: None,
            rollback_data: Some(RollbackData::PathBackup {
                previous_path: "C:\\Windows\\system32".to_string(),
                added_directory: PathBuf::from("C:\\fake\\bin"),
            }),
            rolled_back_tx_id: None,
        };
        let res3 = logger.rollback_transaction(&tx_path, &os).await.unwrap();
        assert!(res3.contains("Restored previous User PATH"));
    }

    #[tokio::test]
    async fn audit_report_exports_valid_markdown_and_html() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let log_file = temp_dir.path().join("audit_test.jsonl");
        let logger = StateLogger::new(log_file, EventBus::new(16));

        // Empty state export
        let empty_md = logger.export_audit_report_markdown().expect("export md");
        assert!(empty_md.contains("No transactions have been recorded"));

        let tx = TransactionRecord {
            tx_id: "tx-audit-1".to_string(),
            timestamp: Utc::now(),
            plan_id: "plan-python".to_string(),
            goal: "set up python".to_string(),
            step_id: "step-1".to_string(),
            action_summary: "Install Python 3.12 via winget".to_string(),
            step: Step::InstallPackage {
                ecosystem: Ecosystem::Winget,
                package_id: "Python.Python.3.12".to_string(),
                version: Some("3.12.2".to_string()),
                source_adapter: AdapterKind::Winget,
            },
            dry_run: false,
            changes_made: vec!["Installed Python".to_string()],
            rollback_hint: Some("winget uninstall Python.Python.3.12".to_string()),
            rollback_data: None,
            rolled_back_tx_id: None,
        };
        logger.record_transaction(&tx).expect("record");

        // Non-empty export
        let md = logger.export_audit_report_markdown().expect("export md");
        assert!(md.contains("Total Transactions Recorded:** 1"));
        assert!(md.contains("Python.Python.3.12"));
        assert!(md.contains("winget uninstall"));
        assert!(md.contains("Active"));

        let html = logger.export_audit_report_html().expect("export html");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("SetupPal Security & Environment Audit Report") || html.contains("SetupPal Security &amp; Environment Audit Report"));
        assert!(html.contains("plan-python"));
        assert!(html.contains("Python.Python.3.12"));
        assert!(html.contains("Active"));
    }

    #[tokio::test]
    async fn state_logger_idempotent_rollback_and_compensating_tx() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let log_file = temp_dir.path().join("idempotent_test.jsonl");
        let logger = StateLogger::new(log_file, EventBus::new(16));
        let os: Arc<dyn OsProvider> = Arc::new(MockOsProvider::new());

        let tx = TransactionRecord {
            tx_id: "tx-original".to_string(),
            timestamp: Utc::now(),
            plan_id: "plan-undo".to_string(),
            goal: "test undo".to_string(),
            step_id: "step-1".to_string(),
            action_summary: "Mock package install".to_string(),
            step: Step::InstallPackage {
                ecosystem: Ecosystem::Winget,
                package_id: "Test.Pkg".to_string(),
                version: Some("1.0".to_string()),
                source_adapter: AdapterKind::Winget,
            },
            dry_run: true,
            changes_made: vec!["Installed Test.Pkg".to_string()],
            rollback_hint: Some("uninstall Test.Pkg".to_string()),
            rollback_data: Some(RollbackData::PackageInstall {
                ecosystem: "winget".to_string(),
                package_id: "Test.Pkg".to_string(),
                uninstall_command: Some("uninstall Test.Pkg".to_string()),
            }),
            rolled_back_tx_id: None,
        };
        logger.record_transaction(&tx).expect("record");

        // Before rollback, active transactions contains 1
        let active_before = logger.get_active_transactions().expect("active");
        assert_eq!(active_before.len(), 1);

        // Perform first rollback
        let res1 = logger.rollback_transaction(&tx, &os).await.expect("rollback 1");
        assert!(res1.contains("Would uninstall Test.Pkg"));

        // Verify compensating transaction was appended
        let all_after = logger.read_all_transactions().expect("all");
        assert_eq!(all_after.len(), 2);
        assert_eq!(all_after[1].rolled_back_tx_id.as_deref(), Some("tx-original"));

        // After rollback, active transactions is 0
        let active_after = logger.get_active_transactions().expect("active");
        assert_eq!(active_after.len(), 0);
        assert!(logger.get_latest_transaction().expect("latest").is_none());

        // Perform second rollback (idempotency test)
        let res2 = logger.rollback_transaction(&tx, &os).await.expect("rollback 2");
        assert!(res2.contains("was already rolled back previously"));

        // Ledger total count should still be 2 (no duplicate undo records added)
        assert_eq!(logger.read_all_transactions().expect("all").len(), 2);
    }
}
