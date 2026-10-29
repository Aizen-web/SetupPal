use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;

use installer_core::{
    AdapterRegistry, CoreConfig, EventBus, ExecutionOptions, Executor,
    NimClient, Planner, PlatformOsProvider, RecipeStore,
    SafetyEngine, StateLogger, SystemScanner, Verifier, OsProvider,
};

use crate::{ui, Cli, Commands};

pub async fn dispatch(cli: Cli) -> Result<()> {
    let config = CoreConfig::from_env();
    let events = EventBus::default();
    let os_provider: Arc<dyn OsProvider> = Arc::new(PlatformOsProvider::new());
    let scanner = SystemScanner::new(os_provider.clone(), events.clone());
    let safety_engine = SafetyEngine::with_system_defaults(os_provider.clone(), config.tavily_api_key.clone(), events.clone());

    match cli.command {
        Commands::Doctor => {
            let profile = scanner.scan().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&profile)?);
            } else {
                ui::render_system_profile(&profile);
            }
            Ok(())
        }
        Commands::Plan {
            goal,
            dry_run,
            print_schema,
        } => {
            if print_schema {
                let schema = NimClient::plan_json_schema()?;
                println!("{}", serde_json::to_string_pretty(&schema)?);
                return Ok(());
            }

            let profile = scanner.scan().await?;
            let recipes = RecipeStore::load_from_dir_or_embedded(&config.recipes_dir)?;
            let llm = Arc::new(NimClient::new(config.clone())?);
            let planner = Planner::new(recipes, llm, events);

            let plan = planner.create_plan(&goal, &profile, None).await?;
            let safety_report = safety_engine.evaluate_plan(&plan).await?;

            if cli.json {
                let combined = serde_json::json!({
                    "plan": plan,
                    "safety": safety_report,
                });
                println!("{}", serde_json::to_string_pretty(&combined)?);
            } else {
                ui::render_system_profile(&profile);
                ui::render_plan_with_safety(&plan, &safety_report, dry_run);
            }
            Ok(())
        }
        Commands::Install {
            goal,
            dry_run,
            execute,
            yes,
            allow_yellow,
        } => {
            let effective_dry_run = if execute { dry_run } else { true };
            let profile = scanner.scan().await?;
            let recipes = RecipeStore::load_from_dir_or_embedded(&config.recipes_dir)?;
            let llm = Arc::new(NimClient::new(config.clone())?);
            let planner = Planner::new(recipes, llm, events.clone());

            let plan = planner.create_plan(&goal, &profile, None).await?;
            let safety_report = safety_engine.evaluate_plan(&plan).await?;

            if !cli.json {
                ui::render_system_profile(&profile);
                ui::render_plan_with_safety(
                    &plan,
                    &safety_report,
                    effective_dry_run,
                );
            }

            let state_logger = StateLogger::new(config.state_log_path.clone(), events.clone());
            let verifier = Verifier::new(os_provider.clone(), events.clone());
            let adapters = AdapterRegistry::with_system_defaults(os_provider.clone());
            let executor = Executor::new(
                adapters,
                os_provider.clone(),
                state_logger,
                verifier,
                events.clone(),
            );

            let options = ExecutionOptions {
                dry_run: effective_dry_run,
                auto_approve_green: yes,
                allow_yellow,
                working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                trusted_categories: std::collections::HashSet::new(),
            };

            let execution_summary = executor
                .execute_plan(&plan, &safety_report, &options)
                .await?;

            if cli.json {
                let combined = serde_json::json!({
                    "plan": plan,
                    "safety": safety_report,
                    "execution": execution_summary,
                });
                println!("{}", serde_json::to_string_pretty(&combined)?);
            } else {
                ui::render_execution_summary(&execution_summary);
            }
            Ok(())
        }
        Commands::Undo { tx_id } => {
            let state_logger = StateLogger::new(config.state_log_path.clone(), events.clone());
            let tx = match tx_id.as_deref() {
                Some(id) => state_logger.get_transaction_by_id(id)?,
                None => state_logger.get_latest_transaction()?,
            };

            match tx {
                Some(t) => {
                    let outcome = state_logger.rollback_transaction(&t, &os_provider).await?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "tx_id": t.tx_id,
                                "status": "rolled_back",
                                "outcome": outcome,
                            }))?
                        );
                    } else {
                        ui::render_rollback_result(&t.tx_id, &outcome);
                    }
                }
                None => {
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "error": "No matching transaction found in state log."
                            }))?
                        );
                    } else {
                        println!("No matching transaction found in state log to rollback.");
                    }
                }
            }
            Ok(())
        }
        Commands::Log { export_md, export_html } => {
            let state_logger = StateLogger::new(config.state_log_path.clone(), events.clone());

            if let Some(md_path) = export_md {
                let report = state_logger.export_audit_report_markdown()?;
                std::fs::write(&md_path, &report)?;
                println!("Successfully exported audit report to: {}", md_path.display());
                return Ok(());
            }

            if let Some(html_path) = export_html {
                let report = state_logger.export_audit_report_html()?;
                std::fs::write(&html_path, &report)?;
                println!("Successfully exported styled HTML audit report to: {}", html_path.display());
                return Ok(());
            }

            let transactions = state_logger.read_all_transactions()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&transactions)?);
            } else {
                ui::render_transaction_log(&transactions);
            }
            Ok(())
        }
    }
}
