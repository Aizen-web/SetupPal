use colored::{ColoredString, Colorize};
use installer_core::{Plan, PlanSafetyReport, PlanSource, RiskRating, SafetyReport, Step, SystemProfile};

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

pub fn render_demo_attack_banner(poisoned_readme: &str) {
    println!(
        "{}",
        "=== ZERO-TRUST ADVERSARIAL ATTACK SIMULATION (DEMO-ATTACK) ==="
            .bold()
            .bright_red()
    );
    println!(
        "  {}",
        "Untrusted Repository README Input (wrapped in <untrusted_external_data>):".bold()
    );
    for line in poisoned_readme.lines() {
        println!("    {}", line.bright_yellow());
    }
    println!();
}

fn format_badge(rating: RiskRating, score: u8) -> ColoredString {
    match rating {
        RiskRating::Green => format!("[GREEN  score={score:02}/100]").bold().bright_green(),
        RiskRating::Yellow => format!("[YELLOW score={score:02}/100]").bold().bright_yellow(),
        RiskRating::Red => format!("[RED    score={score:02}/100]").bold().bright_red(),
    }
}

pub fn render_plan_with_safety(
    plan: &Plan,
    safety: &PlanSafetyReport,
    dry_run: bool,
    nim_mock: bool,
) {
    println!(
        "{}",
        "=== ZERO-TRUST EXECUTION PLAN & SAFETY REPORT ==="
            .bold()
            .bright_green()
    );
    println!("  {} {}", "Plan ID:       ".bold(), plan.plan_id);
    println!("  {} {}", "Goal:          ".bold(), plan.goal.bright_white());
    println!("  {} {}", "Summary:       ".bold(), plan.summary);

    let source_label = match &plan.source {
        PlanSource::Recipe { recipe_id } => format!("Deterministic Recipe (`{recipe_id}`)")
            .bright_green()
            .to_string(),
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
    println!("  {} {}", "Source:        ".bold(), source_label);
    println!(
        "  {} {}",
        "Overall Safety:".bold(),
        format_badge(safety.overall_rating, safety.overall_score)
    );
    println!(
        "  {} {}",
        "Execution Gate:".bold(),
        if safety.can_execute {
            if dry_run {
                "ALLOWED (DRY-RUN mode)".bright_blue()
            } else {
                "ALLOWED (LIVE mode)".bright_green()
            }
        } else {
            "HARD BLOCKED BY DETERMINISTIC SAFETY ENGINE (Red steps can NEVER run)"
                .bold()
                .bright_red()
        }
    );
    println!();

    for (idx, item) in plan.steps.iter().enumerate() {
        let step_rep: Option<&SafetyReport> = safety
            .step_reports
            .iter()
            .find(|r| r.step_id == item.id);

        let badge = step_rep
            .map(|r| format_badge(r.rating, r.risk_score))
            .unwrap_or_else(|| "[UNCHECKED]".dimmed());

        println!(
            "  {} {} [{}] {}",
            format!("{}.", idx + 1).bold(),
            badge,
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
                    "     Action: DownloadArtifact | adapter={:?} | id={}@{} -> {} (sha256={:.16}...)",
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

        if let Some(rep) = step_rep {
            for reason in &rep.reasons {
                let bullet = match rep.rating {
                    RiskRating::Green => "✓".bright_green(),
                    RiskRating::Yellow => "⚠".bright_yellow(),
                    RiskRating::Red => "✗".bright_red(),
                };
                println!("     {} {}", bullet, reason);
            }
            if let Some(alt) = &rep.suggested_alternative {
                println!(
                    "     {} Safe alternative available: `{}`",
                    "→".bright_cyan().bold(),
                    alt.bright_green().bold()
                );
            }
        }
        println!();
    }
}
