mod commands;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "orchestrator",
    version,
    about = "Zero-Trust AI Environment & Package Orchestrator"
)]
pub struct Cli {
    /// Output machine-readable JSON
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Scan host system and generate a structured, zero-trust setup Plan (dry-run by default)
    Plan {
        /// Plain-English setup goal (e.g., "set me up for C++20 dev")
        goal: String,

        /// Keep in dry-run mode (default true for `plan`)
        #[arg(long, default_value_t = true)]
        dry_run: bool,

        /// Print the generated `schemars` JSON Schema for `Plan`
        #[arg(long)]
        print_schema: bool,
    },
    /// Generate, safety-check, and execute a setup Plan
    Install {
        /// Plain-English setup goal
        goal: String,

        /// Simulate execution without modifying the system
        #[arg(long)]
        dry_run: bool,

        /// Execute approved steps on the host machine
        #[arg(long)]
        execute: bool,

        /// Auto-approve Green steps (Yellow still prompts unless --allow-yellow is passed; Red is NEVER executed)
        #[arg(long, short = 'y')]
        yes: bool,

        /// Allow Yellow (caution) steps without interactive prompt
        #[arg(long)]
        allow_yellow: bool,
    },
    /// Run a full hardware, PATH, and installed developer tool diagnostic scan
    Doctor,
    /// Roll back a previous installation transaction from the append-only state log
    Undo {
        /// Optional transaction ID (defaults to most recent transaction)
        tx_id: Option<String>,
    },
    /// View the append-only JSONL transaction history
    Log,
    /// Run the scripted prompt-injection + typosquatting interception showcase
    DemoAttack,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    commands::dispatch(cli).await
}
