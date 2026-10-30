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
            format!("NVIDIA NIM (`{model}`)")
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
                    "     Action: DownloadArtifact | adapter={:?} | id={}@{} -> {} (sha256={})",
                    adapter,
                    id,
                    version,
                    destination.display(),
                    expected_sha256
                        .as_deref()
                        .map(|h| format!("{:.16}...", h))
                        .unwrap_or_else(|| "from upstream".to_string())
                );
            }
            Step::RunVendorInstaller {
                installer,
                mc_version,
                installer_path,
            } => {
                println!(
                    "     Action: RunVendorInstaller | installer={:?} | minecraft={} | jar={}",
                    installer,
                    mc_version,
                    installer_path.display()
                );
            }
            Step::AddDiscoveredToolToPath { binary } => {
                println!(
                    "     Action: AddDiscoveredToolToPath | binary={}",
                    binary.executable_name().bright_green()
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

        // Source transparency per SECURITY.md
        let (source_domain, source_url) = item.step.source_display();
        let source_formatted = match source_url {
            Some(u) => format!("{} ({})", source_domain.bright_white(), u.underline().bright_cyan()),
            None => source_domain.bright_white().to_string(),
        };
        println!("     Source: {}", source_formatted);

        if let Some(rep) = step_rep {
            let (verdict_title, verdict_desc) = rep.user_facing_translation();
            let verdict_badge = match rep.rating {
                RiskRating::Green => verdict_title.bright_green().bold(),
                RiskRating::Yellow => verdict_title.bright_yellow().bold(),
                RiskRating::Red => verdict_title.bright_red().bold(),
            };
            println!("     Verdict: {} — {}", verdict_badge, verdict_desc);

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

pub fn render_execution_summary(summary: &installer_core::PlanExecutionSummary) {
    let mode_str = if summary.dry_run {
        "[DRY RUN SIMULATION - NO SYSTEM CHANGES MADE]".bright_yellow().bold()
    } else {
        "[LIVE EXECUTION - SYSTEM CHANGES COMMITTED]".bright_green().bold()
    };

    println!("\n{}", "=== EXECUTION & VERIFICATION RESULTS ===".bold().bright_cyan());
    println!("  Mode: {}", mode_str);
    println!("  Goal: {}\n", summary.goal.bold());

    for step in &summary.step_results {
        if let Some(err) = &step.error {
            println!(
                "  {} {} — {}",
                "✖".bright_red().bold(),
                step.step_id.bold(),
                step.description
            );
            println!("    {} {}", "FAILED:".bright_red().bold(), err);
        } else if step.executed {
            println!(
                "  {} {} — {}",
                if step.succeeded() { "✔".bright_green().bold() } else { "✖".bright_red().bold() },
                step.step_id.bold(),
                step.description
            );
            if let Some(receipt) = &step.receipt {
                for change in &receipt.changes_made {
                    println!("    {} {}", "•".bright_cyan(), change.dimmed());
                }
            }
            if let Some(ver) = &step.verification {
                let badge = if ver.passed {
                    "PASS".bright_green().bold()
                } else {
                    "FAIL".bright_red().bold()
                };
                println!("    Verification [{}]: {}", badge, ver.command_run.dimmed());
                if !ver.stdout.is_empty() {
                    println!("      stdout: {}", ver.stdout.trim().dimmed());
                }
                if !ver.stderr.is_empty() {
                    println!("      stderr: {}", ver.stderr.trim().bright_red());
                }
            }
        } else {
            println!(
                "  {} {} — {}",
                "✖".bright_red().bold(),
                step.step_id.bold(),
                step.description
            );
            if let Some(reason) = &step.blocked_reason {
                println!("    {} {}", "BLOCKED:".bright_red().bold(), reason);
            }
        }
        println!();
    }

    if let Some(halted) = &summary.halted_at_step_id {
        println!(
            "  {} Stopped at step {} — it needs your approval. Re-run with --yes (and --allow-yellow for caution steps) to continue.\n",
            "⏸".bright_yellow().bold(),
            halted.bold()
        );
    }

    if let Some(cpp_ver) = &summary.toolchain_verification {
        println!("{}", "--- AUTOMATED END-TO-END C++20 VERIFICATION ---".bold().bright_magenta());
        if cpp_ver.passed {
            println!(
                "  {} Modern C++20 toolchain compiled and executed temporary hello.cpp successfully!",
                "✔ [VERIFIED]".bright_green().bold()
            );
            println!("  Command: {}", cpp_ver.command_run.dimmed());
            println!("  Output:  {}", cpp_ver.stdout.trim().bright_green());
        } else {
            println!(
                "  {} C++20 toolchain compilation or execution check failed.",
                "✖ [FAILED]".bright_red().bold()
            );
            println!("  Command: {}", cpp_ver.command_run.dimmed());
            if !cpp_ver.stderr.is_empty() {
                println!("  Error:   {}", cpp_ver.stderr.trim().bright_red());
            }
        }
        println!();
    }

    if summary.overall_passed {
        println!(
            "{}",
            "🎉 ALL APPROVED STEPS COMPLETED AND VERIFIED SUCCESSFULLY!"
                .bold()
                .bright_green()
        );
    } else {
        println!(
            "{}",
            "⚠ PLAN EXECUTION STOPPED OR ONE OR MORE CHECKS FAILED."
                .bold()
                .bright_yellow()
        );
    }
    println!();
}

pub fn render_transaction_log(records: &[installer_core::TransactionRecord]) {
    println!(
        "{}",
        "=== APPEND-ONLY AUDIT & STATE TRANSACTION LOG ===".bold().bright_cyan()
    );
    if records.is_empty() {
        println!("  No transactions recorded yet. Run `installer install <goal>` to record changes.\n");
        return;
    }

    println!("  Total recorded transactions: {}\n", records.len().to_string().bold());

    for (i, tx) in records.iter().enumerate() {
        let dry_badge = if tx.dry_run {
            "[DRY-RUN]".bright_yellow()
        } else {
            "[LIVE]".bright_green()
        };

        println!(
            "  {}. {} {} ({})",
            (i + 1).to_string().bold(),
            dry_badge,
            tx.action_summary.bold(),
            tx.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string().dimmed()
        );
        println!("     TX ID:   {}", tx.tx_id.dimmed());
        println!("     Plan ID: {} (Goal: {})", tx.plan_id.dimmed(), tx.goal);
        println!("     Step:    {}", tx.step_id);

        for change in &tx.changes_made {
            println!("     • {}", change.dimmed());
        }
        if let Some(hint) = &tx.rollback_hint {
            println!("     {} Rollback hint: {}", "↺".bright_yellow(), hint.dimmed());
        }
        println!();
    }
}

pub fn render_rollback_result(tx_id: &str, outcome: &str) {
    println!(
        "{}",
        "=== TRANSACTION ROLLBACK RESULT ===".bold().bright_cyan()
    );
    println!("  TX ID:   {}", tx_id.bold());
    println!("  Status:  {}", "SUCCESS".bright_green().bold());
    println!("  Outcome: {}\n", outcome);
}
