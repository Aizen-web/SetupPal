use std::sync::Arc;
use anyhow::Result;

use installer_core::{
    CoreConfig, EventBus, NimClient, Planner, PlatformOsProvider, RecipeStore, SystemScanner,
};

use crate::{ui, Cli, Commands};

pub async fn dispatch(cli: Cli) -> Result<()> {
    let config = CoreConfig::from_env();
    let events = EventBus::default();
    let os_provider = Arc::new(PlatformOsProvider::new());
    let scanner = SystemScanner::new(os_provider, events.clone());

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

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                ui::render_system_profile(&profile);
                ui::render_plan(&plan, dry_run, config.nim_mock);
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
            let planner = Planner::new(recipes, llm, events);
            let plan = planner.create_plan(&goal, &profile, None).await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                ui::render_system_profile(&profile);
                ui::render_plan(&plan, effective_dry_run, config.nim_mock);
                println!(
                    "\\n[Milestone 1 Notice] Safety Engine & Executor wire-up arrives in Milestones 2 & 3 (flags: dry_run={effective_dry_run}, yes={yes}, allow_yellow={allow_yellow})."
                );
            }
            Ok(())
        }
        Commands::Undo { tx_id } => {
            println!(
                "Undo requested for transaction: {:?} (implemented in Milestone 3/8).",
                tx_id.as_deref().unwrap_or("latest")
            );
            Ok(())
        }
        Commands::Log => {
            println!(
                "State log path: {} (implemented in Milestone 3).",
                config.state_log_path.display()
            );
            Ok(())
        }
        Commands::DemoAttack => {
            println!("Demo attack showcase will be wired in Milestone 2 & Milestone 8.");
            Ok(())
        }
    }
}
