use colored::Colorize;
use installer_core::{Plan, PlanSource, Step, SystemProfile};

pub fn render_system_profile(profile: &SystemProfile) {
    println!(
        "{}",
        "=== HOST SYSTEM SCAN SUMMARY ===".bold().bright_cyan()
    );
    println!(
        "  {} {} ({}) [{}]",
        "OS:".bold(),
        profile.os_name,
        profile.os_version,
        profile.arch
    );
    println!(
        "  {} {}",
        "GPU:".bold(),
        profile
            .gpu_detected
            .as_deref()
            .unwrap_or("None detected / CPU-only")
    );
    let free_gb = profile.free_disk_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    println!("  {} {:.2} GB free", "Disk:".bold(), free_gb);
    println!(
        "  {} {} entries active",
        "PATH:".bold(),
        profile.active_path_entries.len()
    );

    if profile.installed_tools.is_empty() {
        println!("  {} none detected", "Tools:".bold());
    } else {
        println!("  {}", "Detected Toolchain:".bold());
        for tool in &profile.installed_tools {
            println!(
                "    {} {:<10} {} ({})",
                "•".bright_green(),
                tool.binary.executable_name().bold(),
                tool.version_output.dimmed(),
                tool.path.display()
            );
        }
    }
    println!();
}

pub fn render_plan(plan: &Plan, dry_run: bool, nim_mock: bool) {
    println!(
        "{}",
        "=== ZERO-TRUST EXECUTION PLAN ===".bold().bright_green()
    );
    println!("  {} {}", "Plan ID:".bold(), plan.plan_id);
    println!("  {} {}", "Goal:   ".bold(), plan.goal.bright_white());
    println!("  {} {}", "Summary:".bold(), plan.summary);

    let source_label = match &plan.source {
        PlanSource::Recipe { recipe_id } => {
            format!("Deterministic Recipe (`{recipe_id}`)").bright_green().to_string()
        }
        PlanSource::Llm { model } => {
            let mode = if nim_mock { "offline mock" } else { "live NIM" };
            format!("NVIDIA NIM (`{model}` - {mode})")
                .bright_yellow()
                .to_string()
        }
        PlanSource::Healer {
            failed_step_id,
            attempt,
        } => format!("Self-Healer (step `{failed_step_id}`, attempt #{attempt})")
            .bright_magenta()
            .to_string(),
    };
    println!("  {} {}", "Source: ".bold(), source_label);
    println!(
        "  {} {}",
        "Mode:   ".bold(),
        if dry_run {
            "DRY-RUN (no system changes)".bright_blue()
        } else {
            "LIVE EXECUTION".bright_red()
        }
    );
    println!();

    for (idx, item) in plan.steps.iter().enumerate() {
        println!(
            "  {} [{}] {}",
            format!("{}.", idx + 1).bold(),
            item.id.bright_cyan(),
            item.description.bold()
        );
        match &item.step {
            Step::InstallPackage {
                ecosystem,
                package_id,
                version,
                source_adapter,
            } => {
                println!(
                    "     Action: InstallPackage | ecosystem={:?} | pkg={} | version={} | adapter={:?}",
                    ecosystem,
                    package_id.bright_white(),
                    version.as_deref().unwrap_or("latest"),
                    source_adapter
                );
            }
            Step::DownloadArtifact {
                adapter,
                id,
                version,
                destination,
                expected_sha256,
            } => {
                println!(
                    "     Action: DownloadArtifact | adapter={:?} | id={}@{} -> {} (sha256={:.12}...)",
                    adapter,
                    id,
                    version,
                    destination.display(),
                    expected_sha256
                );
            }
            Step::VerifyHash {
                file_path,
                expected_sha256,
            } => {
                println!(
                    "     Action: VerifyHash | file={} | sha256={:.16}...",
                    file_path.display(),
                    expected_sha256
                );
            }
            Step::AddToUserPath { directory } => {
                println!(
                    "     Action: AddToUserPath | directory={}",
                    directory.display()
                );
            }
            Step::CreateIsolatedEnv { kind, path } => {
                println!(
                    "     Action: CreateIsolatedEnv | kind={:?} | path={}",
                    kind,
                    path.display()
                );
            }
            Step::RunVerifyCommand { binary, args } => {
                println!(
                    "     Action: RunVerifyCommand | binary={} | args={:?}",
                    binary.executable_name().bright_green(),
                    args
                );
            }
        }
    }
}
