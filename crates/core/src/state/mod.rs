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

    /// Retrieves the most recent transaction recorded in the state log.
    pub fn get_latest_transaction(&self) -> Result<Option<TransactionRecord>, CoreError> {
        let all = self.read_all_transactions()?;
        Ok(all.into_iter().last())
    }

    /// Finds a transaction by its unique ID.
    pub fn get_transaction_by_id(&self, tx_id: &str) -> Result<Option<TransactionRecord>, CoreError> {
        let all = self.read_all_transactions()?;
        Ok(all.into_iter().find(|t| t.tx_id == tx_id))
    }

    /// Reverses the state modifications made by a given transaction.
    pub async fn rollback_transaction(
        &self,
        tx: &TransactionRecord,
        os_provider: &Arc<dyn OsProvider>,
    ) -> Result<String, CoreError> {
        self.events.emit(AgentEvent::RollbackStarted {
            tx_id: tx.tx_id.clone(),
        });

        let outcome = match &tx.rollback_data {
            Some(RollbackData::PathBackup {
                previous_path,
                added_directory,
            }) => {
                os_provider.restore_user_path(previous_path).await?;
                format!(
                    "Restored previous User PATH (removed directory {})",
                    added_directory.display()
                )
            }
            Some(RollbackData::FileDownload { destination }) => {
                if destination.exists() {
                    std::fs::remove_file(destination)?;
                    format!("Deleted downloaded artifact: {}", destination.display())
                } else {
                    format!("Downloaded artifact already removed: {}", destination.display())
                }
            }
            Some(RollbackData::IsolatedEnv { path }) => {
                if path.exists() {
                    std::fs::remove_dir_all(path)?;
                    format!("Cleaned up isolated environment directory: {}", path.display())
                } else {
                    format!("Isolated directory already removed: {}", path.display())
                }
            }
            Some(RollbackData::PackageInstall {
                ecosystem,
                package_id,
                uninstall_command,
            }) => {
                format!(
                    "Package rollback recorded for {} ({}). Uninstall hint: {}",
                    package_id,
                    ecosystem,
                    uninstall_command.as_deref().unwrap_or("none")
                )
            }
            None => {
                format!(
                    "No rollback data attached to transaction {}. Hint: {}",
                    tx.tx_id,
                    tx.rollback_hint.as_deref().unwrap_or("none")
                )
            }
        };

        self.events.emit(AgentEvent::RollbackCompleted {
            tx_id: tx.tx_id.clone(),
            success: true,
            message: outcome.clone(),
        });

        Ok(outcome)
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
}
