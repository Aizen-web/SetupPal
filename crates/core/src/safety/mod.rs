pub mod hash;
pub mod osv;
pub mod policy;
pub mod tavily;
pub mod typosquat;

use std::sync::Arc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapters::{AdapterRegistry, PackageMetadata};
use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::safety::osv::{HttpOsvClient, OsvClient};
pub use crate::safety::tavily::{
    HttpTavilyVerifier, TavilyVerificationResult, TavilyVerifier,
};
use crate::types::{AdapterKind, Ecosystem, Plan, PlanSource, PlannedStep, Step};

/// Reason text for steps without an upstream digest (keeps them at least Yellow).
pub const NO_UPSTREAM_CHECKSUM_REASON: &str = "No upstream checksum available";
/// Reason text when a plan-pinned digest disagrees with the upstream registry.
pub const CHECKSUM_MISMATCH_REASON: &str = "Pinned checksum does not match the upstream registry";
/// Reason prefix used when OSV.dev cannot be reached or returns an error.
pub const OSV_UNAVAILABLE_REASON: &str = "Verification service (OSV.dev) unavailable";
/// Plan-level note when the plan includes winget steps OSV.dev cannot scan.
pub const OSV_WINGET_COVERAGE_NOTE: &str =
    "OSV.dev does not cover winget packages; those steps rely on winget metadata and policy checks.";
/// Reason prefix when a GitHub repository imitates the official one.
pub const LOOKALIKE_PROJECT_REASON: &str = "Lookalike project";
/// Reason prefix when web search could not confirm a GitHub repository is official.
pub const UNCONFIRMED_PROJECT_REASON: &str = "Could not confirm this is the official project";
/// Reason prefix when a GitHub repository could not be checked because no Tavily key is saved.
pub const TAVILY_KEY_MISSING_REASON: &str = "Add a Tavily key in Settings";

/// Traffic-light risk classification.
/// `Ord` is derived in ascending order of severity (`Green < Yellow < Red`)
/// so overall plan risk can be computed via `.max()`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskRating {
    /// Safe to execute (score 0..=29). Auto-approved with `--yes`.
    Green,
    /// Caution / unverified metadata / young package (score 30..=69).
    /// Requires explicit user confirmation or isolated execution.
    Yellow,
    /// Hard policy block, CVE, or typosquat (score 70..=100).
    /// NEVER executed under any flag.
    Red,
}

/// Comprehensive safety evaluation for a single `PlannedStep` or an entire `Plan`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SafetyReport {
    pub step_id: String,
    pub rating: RiskRating,
    /// Normalized risk score from 0 (lowest risk) to 100 (critical threat)
    pub risk_score: u8,
    /// Plain-English explanations for the assigned rating
    pub reasons: Vec<String>,
    /// Populated when a typosquat or vulnerable package has a known safe replacement
    pub suggested_alternative: Option<String>,
    /// Detailed signal breakdown for the CLI & GUI expandable drawer
    pub signals: SafetySignals,
}

impl SafetyReport {
    /// Translates technical signals into user-facing plain English according to SECURITY.md.
    /// Returns a tuple of `(Title / Label, Detailed explanation)`.
    pub fn user_facing_translation(&self) -> (&'static str, String) {
        if let Some(typo) = &self.signals.typosquat_match {
            return (
                "Lookalike warning",
                format!(
                    "This looks like an unofficial copy of the popular package `{}`.",
                    typo.similar_popular_package
                ),
            );
        }
        if !self.signals.osv_vulnerabilities.is_empty() {
            let alt = self
                .suggested_alternative
                .as_deref()
                .unwrap_or("a patched version");
            return (
                "Known security flaw",
                format!("This version has an unresolved security issue. SetupPal recommends {alt} instead."),
            );
        }
        let has_reason = |prefix: &str| self.reasons.iter().any(|r| r.starts_with(prefix));
        if has_reason(LOOKALIKE_PROJECT_REASON) {
            let official = self.suggested_alternative.as_deref().unwrap_or("the official project");
            return (
                "Lookalike warning",
                format!("This GitHub project copies the name of {official}. It is likely a fake."),
            );
        }
        if has_reason(UNCONFIRMED_PROJECT_REASON) {
            return (
                "Unconfirmed project",
                "SetupPal couldn't confirm this GitHub project is the official one. Check it before approving.".to_string(),
            );
        }
        if has_reason(TAVILY_KEY_MISSING_REASON) {
            return (
                "Unconfirmed project",
                "Add a Tavily key in Settings so SetupPal can check unfamiliar GitHub projects.".to_string(),
            );
        }
        let mentions_admin = |v: &String| {
            let lower = v.to_ascii_lowercase();
            lower.contains("administrator") || lower.contains("uac")
        };
        if self.signals.policy_violations.iter().any(mentions_admin) || self.reasons.iter().any(mentions_admin) {
            return (
                "Administrator access needed",
                "This tool requires administrator approval to install system-level components.".to_string(),
            );
        }
        if self.reasons.iter().any(|r| r.contains(CHECKSUM_MISMATCH_REASON)) {
            return (
                "Unchecked file integrity",
                "The file fingerprint did not match the expected official checksum.".to_string(),
            );
        }
        if self.signals.sha256_verified == Some(false) {
            return (
                "No official checksum",
                "SetupPal could not get an official checksum for this download, so it cannot prove the file is untampered.".to_string(),
            );
        }
        if self
            .reasons
            .iter()
            .any(|r| r.contains("Verification service") || r.contains("unavailable"))
        {
            return (
                "Verification unavailable",
                "Couldn't reach our safety checks right now. Proceed with caution?".to_string(),
            );
        }
        if self.signals.domain_allowlisted == Some(false) {
            return (
                "Unverified website",
                "This download comes from a website SetupPal could not independently verify.".to_string(),
            );
        }
        if let Some(age) = self.signals.package_age_days {
            if age < 30 {
                return (
                    "Newly published software",
                    format!("This package was released recently ({age} days ago, threshold: 30 days)."),
                );
            }
        }
        if let Some(dl) = self.signals.download_count {
            if dl < 1000 {
                return (
                    "Low community adoption",
                    format!("Very few people have downloaded this package so far ({dl} downloads)."),
                );
            }
        }
        if self.signals.sha256_verified == Some(true) && self.signals.domain_allowlisted == Some(true) {
            return (
                "Verified source",
                "File fingerprint matches the official release from a verified domain.".to_string(),
            );
        }

        match self.rating {
            RiskRating::Green => (
                "Verified source",
                "Passed all security, provenance, and policy checks.".to_string(),
            ),
            RiskRating::Yellow => (
                "Caution advised",
                self.reasons
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Unverified package metadata.".to_string()),
            ),
            RiskRating::Red => (
                "Safety block",
                self.reasons
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Prohibited or dangerous action detected.".to_string()),
            ),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SafetySignals {
    pub osv_vulnerabilities: Vec<VulnerabilitySummary>,
    pub typosquat_match: Option<TyposquatMatch>,
    pub package_age_days: Option<u64>,
    pub download_count: Option<u64>,
    pub sha256_verified: Option<bool>,
    pub domain_allowlisted: Option<bool>,
    pub tavily_verified: Option<bool>,
    pub tavily_consensus: Option<String>,
    pub policy_violations: Vec<String>,
    pub script_inspection_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct VulnerabilitySummary {
    pub id: String,
    pub severity: String,
    pub summary: String,
    pub fixed_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TyposquatMatch {
    pub requested_package: String,
    pub similar_popular_package: String,
    pub edit_distance: usize,
}

/// Aggregate safety evaluation for a full `Plan`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PlanSafetyReport {
    pub plan_id: String,
    pub overall_rating: RiskRating,
    pub overall_score: u8,
    pub step_reports: Vec<SafetyReport>,
    pub can_execute: bool,
    /// Plan-level notes shown once on the card summary (not repeated per step).
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Deterministic Zero-Trust Safety Engine.
/// Evaluates every `PlannedStep` in a `Plan` against:
/// 1. Hard policy rules (shell metacharacters, destructive commands, credential paths, domain allowlist)
/// 2. Levenshtein typosquatting radar (`strsim::levenshtein` distance `1..=2`)
/// 3. OSV.dev CVE database lookup
/// 4. Tavily AI search consensus (defeats SEO poisoning & lookalike malware traps like fake DS4Windows sites)
/// 5. Upstream registry metadata heuristics (package age < 30 days, low downloads, missing SHA-256)
/// 6. Pre/post-install script inspection (`<untrusted_data>`)
pub struct SafetyEngine {
    adapters: AdapterRegistry,
    osv: Arc<dyn OsvClient>,
    tavily: Option<Arc<dyn TavilyVerifier>>,
    events: EventBus,
}

impl SafetyEngine {
    pub fn new(
        adapters: AdapterRegistry,
        osv: Arc<dyn OsvClient>,
        tavily: Option<Arc<dyn TavilyVerifier>>,
        events: EventBus,
    ) -> Self {
        Self {
            adapters,
            osv,
            tavily,
            events,
        }
    }

    /// Creates a `SafetyEngine` backed by the real system adapters, live OSV.dev lookups,
    /// and Tavily consensus checks when `tavily_api_key` is configured.
    pub fn with_system_defaults(
        os_provider: Arc<dyn crate::os::OsProvider>,
        tavily_api_key: Option<String>,
        events: EventBus,
    ) -> Self {
        Self {
            adapters: AdapterRegistry::with_system_defaults(os_provider),
            osv: Arc::new(HttpOsvClient::new()),
            tavily: tavily_api_key
                .map(|key| Arc::new(HttpTavilyVerifier::new(key)) as Arc<dyn TavilyVerifier>),
            events,
        }
    }

    /// Like [Self::with_system_defaults], but the Tavily and GitHub keys are read at the moment
    /// they are used, so keys saved while the app runs apply to the next evaluation.
    pub fn with_secret_providers(
        os_provider: Arc<dyn crate::os::OsProvider>,
        tavily_key: crate::config::SecretProvider,
        github_token: crate::config::SecretProvider,
        events: EventBus,
    ) -> Self {
        Self {
            adapters: AdapterRegistry::with_github_token(os_provider, github_token),
            osv: Arc::new(HttpOsvClient::new()),
            tavily: Some(Arc::new(HttpTavilyVerifier::with_key_provider(tavily_key))),
            events,
        }
    }

    async fn verify_with_tavily(
        &self,
        software_name: &str,
        candidate_url: &str,
    ) -> Result<TavilyVerificationResult, CoreError> {
        match &self.tavily {
            Some(verifier) => verifier.verify_source(software_name, candidate_url).await,
            None => Err(CoreError::Config("TAVILY_API_KEY is not set".to_string())),
        }
    }

    pub async fn evaluate_plan(&self, plan: &Plan) -> Result<PlanSafetyReport, CoreError> {
        self.events.emit(AgentEvent::SafetyCheckStarted {
            plan_id: plan.plan_id.clone(),
        });

        let mut step_reports = Vec::with_capacity(plan.steps.len());
        let mut overall_rating = RiskRating::Green;
        let mut overall_score: u8 = 0;
        let pins_trusted = matches!(plan.source, PlanSource::Recipe { .. });

        for (index, planned_step) in plan.steps.iter().enumerate() {
            let mut plan_violations = Vec::new();
            if !policy::vendor_installer_source_ok(plan, index) {
                plan_violations.push(
                    "The installer step must run the file an earlier step downloaded from the vendor's release feed"
                        .to_string(),
                );
            }
            let ctx = StepContext { pins_trusted, plan_violations };
            let report = self.evaluate_step_in(planned_step, &ctx).await?;
            if report.rating > overall_rating {
                overall_rating = report.rating;
            }
            if report.risk_score > overall_score {
                overall_score = report.risk_score;
            }
            self.events.emit(AgentEvent::StepSafetyEvaluated {
                report: Box::new(report.clone()),
            });
            step_reports.push(report);
        }

        let can_execute = overall_rating != RiskRating::Red;
        let mut notes = Vec::new();
        let has_winget = plan.steps.iter().any(|s| {
            matches!(
                &s.step,
                Step::InstallPackage {
                    ecosystem: Ecosystem::Winget,
                    ..
                }
            )
        });
        if has_winget {
            notes.push(OSV_WINGET_COVERAGE_NOTE.to_string());
        }
        let plan_report = PlanSafetyReport {
            plan_id: plan.plan_id.clone(),
            overall_rating,
            overall_score,
            step_reports,
            can_execute,
            notes,
        };

        self.events.emit(AgentEvent::SafetyCheckCompleted {
            report: Box::new(plan_report.clone()),
        });

        Ok(plan_report)
    }

    /// Checks a download URL against the static allowlist. The downloader refuses every other
    /// host, so anything outside it can never run and is blocked here too.
    fn check_download_url(url: &str, eval: &mut StepEvaluation) {
        match policy::is_url_domain_allowlisted(url) {
            Ok(true) => eval.signals.domain_allowlisted = Some(true),
            Ok(false) => {
                eval.signals.domain_allowlisted = Some(false);
                eval.reasons.push(format!(
                    "Download URL `{url}` is not on SetupPal's list of approved download sites"
                ));
                eval.raise(95);
            }
            Err(e) => {
                eval.signals.domain_allowlisted = Some(false);
                eval.reasons.push(e);
                eval.raise(95);
            }
        }
    }

    /// github.com hosts anyone's code, so a repository is only trusted when it is curated or
    /// web search (Tavily) confirms it is the project's official home.
    async fn check_github_project(&self, repo: &str, eval: &mut StepEvaluation) {
        let repo = repo
            .trim()
            .trim_start_matches("https://github.com/")
            .trim_matches('/');
        if crate::adapters::github::winget_id_for_repo(repo).is_some() {
            return;
        }
        let name = repo.rsplit('/').next().unwrap_or(repo);
        match self.verify_with_tavily(name, &format!("https://github.com/{repo}")).await {
            Ok(res) => {
                eval.signals.tavily_verified = Some(res.is_verified);
                eval.signals.tavily_consensus = Some(res.consensus_summary.clone());
                if res.is_spoofed {
                    eval.reasons.push(format!(
                        "{LOOKALIKE_PROJECT_REASON}: {}",
                        res.warning.as_deref().unwrap_or(&res.consensus_summary)
                    ));
                    eval.suggested_alternative = res.official_domains.first().cloned();
                    eval.raise(100);
                } else if !res.is_verified {
                    eval.reasons.push(format!(
                        "{UNCONFIRMED_PROJECT_REASON}: {}",
                        res.consensus_summary
                    ));
                    eval.raise(45);
                }
            }
            Err(CoreError::Config(_)) => {
                eval.reasons.push(format!(
                    "{TAVILY_KEY_MISSING_REASON} to confirm `github.com/{repo}` is the official project"
                ));
                eval.raise(45);
            }
            Err(e) => {
                eval.reasons.push(format!("Verification service (Tavily) unavailable: {e}"));
                eval.raise(45);
            }
        }
    }

    fn check_typosquat(ecosystem: Ecosystem, package_id: &str, eval: &mut StepEvaluation) {
        if let Some(typo) = typosquat::detect_typosquat(ecosystem, package_id) {
            eval.reasons.push(format!(
                "TYPOSQUAT DETECTED: `{}` is edit distance {} from popular {:?} package `{}`",
                typo.requested_package, typo.edit_distance, ecosystem, typo.similar_popular_package
            ));
            eval.suggested_alternative = Some(typo.similar_popular_package.clone());
            eval.signals.typosquat_match = Some(typo);
            eval.raise(95);
        }
    }

    fn check_registry_heuristics(
        package_id: &str,
        meta: &PackageMetadata,
        eval: &mut StepEvaluation,
        // When true, `meta.age_days` is treated as first-publish / identifier age.
        apply_age: bool,
    ) {
        eval.signals.package_age_days = meta.age_days;
        eval.signals.download_count = meta.download_count;

        if apply_age {
            if let Some(age) = meta.age_days {
                if age < 30 {
                    eval.reasons.push(format!(
                        "Young package warning: `{package_id}` was first published {age} days ago (< 30 days threshold)"
                    ));
                    eval.raise(45);
                }
            }
        }
        if let Some(downloads) = meta.download_count {
            if downloads < 1_000 {
                eval.reasons.push(format!(
                    "Low adoption warning: `{package_id}` has only {downloads} recorded downloads (< 1,000 threshold)"
                ));
                eval.raise(45);
            }
        }
        if let Some(script) = &meta.install_script_snippet {
            let flags = policy::inspect_install_script(script);
            if !flags.is_empty() {
                for flag in &flags {
                    eval.reasons.push(format!("Install script flag: {flag}"));
                }
                let is_critical = flags.iter().any(|f| {
                    f.contains("prohibited") || f.contains("base64") || f.contains("exfiltration")
                });
                eval.signals.script_inspection_flags = flags;
                eval.raise(if is_critical { 90 } else { 55 });
            }
        }
    }

    /// Evaluates one step on its own: checksums pinned in the step are treated as untrusted.
    pub async fn evaluate_step(&self, planned_step: &PlannedStep) -> Result<SafetyReport, CoreError> {
        self.evaluate_step_in(planned_step, &StepContext::default()).await
    }

    async fn evaluate_step_in(
        &self,
        planned_step: &PlannedStep,
        ctx: &StepContext,
    ) -> Result<SafetyReport, CoreError> {
        let mut eval = StepEvaluation::default();

        // 1. Deterministic Hard Policy Check
        let mut policy_violations =
            policy::evaluate_step_policy(&planned_step.step, &planned_step.description);
        policy_violations.extend(ctx.plan_violations.iter().cloned());
        if !policy_violations.is_empty() {
            for v in &policy_violations {
                eval.reasons.push(format!("HARD POLICY BLOCK: {v}"));
            }
            eval.signals.policy_violations = policy_violations;
            eval.raise(100);
        }

        // 2. Step-specific Multi-Signal Checks
        match &planned_step.step {
            Step::InstallPackage {
                ecosystem,
                package_id,
                version,
                source_adapter,
            } => {
                // 2a. Levenshtein Typosquatting Radar
                Self::check_typosquat(*ecosystem, package_id, &mut eval);
                if *source_adapter == AdapterKind::GitHubReleases {
                    self.check_github_project(package_id, &mut eval).await;
                }

                // 2b. OSV.dev Vulnerability Query (fails closed to Yellow)
                if osv::osv_ecosystem_name(*ecosystem).is_some() {
                    match self
                        .osv
                        .query_package(*ecosystem, package_id, version.as_deref())
                        .await
                    {
                        Ok(vulns) => {
                            if !vulns.is_empty() {
                                for v in &vulns {
                                    eval.reasons.push(format!(
                                        "OSV CVE BLOCK [{} - {}]: {}",
                                        v.id, v.severity, v.summary
                                    ));
                                    if eval.suggested_alternative.is_none() {
                                        if let Some(fixed) = &v.fixed_version {
                                            eval.suggested_alternative = Some(format!("{package_id}@{fixed}"));
                                        }
                                    }
                                }
                                eval.signals.osv_vulnerabilities = vulns;
                                eval.raise(90);
                            }
                        }
                        Err(e) => {
                            eval.reasons.push(format!("{OSV_UNAVAILABLE_REASON}: {e}"));
                            eval.raise(50);
                        }
                    }
                }
                // OSV coverage gaps are noted once on the plan summary, not on every winget step.

                // 2c. Registry Metadata & Install Script Inspection
                if let Some(adapter) = self.adapters.get(*source_adapter) {
                    match adapter.resolve_metadata(package_id, version.as_deref()).await {
                        Ok(meta) => {
                            let has_digest = meta.has_upstream_digest();
                            eval.signals.sha256_verified = Some(has_digest);
                            if let Some(url) = &meta.download_url {
                                Self::check_download_url(url, &mut eval);
                            }
                            // Winget adapters only set age_days for young identifiers; established
                            // packages leave it None so they stay Green. npm/PyPI use first-publish age.
                            Self::check_registry_heuristics(package_id, &meta, &mut eval, true);
                            if !has_digest {
                                eval.reasons.push(format!("{NO_UPSTREAM_CHECKSUM_REASON} for `{package_id}`"));
                                eval.raise(40);
                            }
                        }
                        Err(e) => {
                            eval.reasons.push(format!("Could not verify upstream registry metadata: {e}"));
                            eval.raise(50);
                        }
                    }
                } else {
                    eval.reasons.push(format!(
                        "No registered adapter for `{source_adapter:?}` to verify metadata"
                    ));
                    eval.raise(45);
                }
            }

            Step::DownloadArtifact {
                adapter,
                id,
                version,
                expected_sha256,
                ..
            } => {
                let is_url = id.starts_with("http://") || id.starts_with("https://");
                if !is_url {
                    if let Some(ecosystem) = registry_ecosystem(*adapter) {
                        Self::check_typosquat(ecosystem, id, &mut eval);
                    }
                    if *adapter == AdapterKind::GitHubReleases {
                        self.check_github_project(id, &mut eval).await;
                    }
                }
                let meta = match self.adapters.get(*adapter) {
                    Some(a) => match a.resolve_metadata(id, Some(version)).await {
                        Ok(m) => Some(m),
                        Err(e) => {
                            eval.reasons.push(format!("Could not verify upstream registry metadata: {e}"));
                            eval.raise(50);
                            None
                        }
                    },
                    None => {
                        eval.reasons.push(format!("No registered adapter for `{adapter:?}` to verify metadata"));
                        eval.raise(45);
                        None
                    }
                };

                let url = if is_url {
                    Some(id.clone())
                } else {
                    meta.as_ref().and_then(|m| m.download_url.clone())
                };
                if let Some(url) = &url {
                    Self::check_download_url(url, &mut eval);
                }

                if let Some(m) = &meta {
                    // Downloads are version-aged like npm/PyPI (not winget package-id age).
                    Self::check_registry_heuristics(id, m, &mut eval, true);
                }

                let upstream_256 = meta.as_ref().and_then(|m| m.expected_sha256.clone());
                let has_upstream = meta.as_ref().is_some_and(|m| m.has_upstream_digest());
                match (expected_sha256, &upstream_256) {
                    (Some(pinned), Some(upstream)) if !pinned.trim().eq_ignore_ascii_case(upstream) => {
                        eval.signals.sha256_verified = Some(false);
                        eval.reasons.push(format!(
                            "{CHECKSUM_MISMATCH_REASON} for `{id}` (plan {pinned}, upstream {upstream})"
                        ));
                        eval.raise(100);
                    }
                    _ if has_upstream => eval.signals.sha256_verified = Some(true),
                    (Some(_), _) if ctx.pins_trusted => {
                        eval.reasons.push(format!(
                            "Checksum for `{id}` is pinned by the plan; the upstream source did not publish one"
                        ));
                    }
                    (Some(_), _) => {
                        eval.signals.sha256_verified = Some(false);
                        eval.reasons.push(format!(
                            "{NO_UPSTREAM_CHECKSUM_REASON} for `{id}` (a checksum written by the AI planner proves nothing)"
                        ));
                        eval.raise(40);
                    }
                    (None, _) => {
                        eval.signals.sha256_verified = Some(false);
                        eval.reasons.push(format!("{NO_UPSTREAM_CHECKSUM_REASON} for `{id}`"));
                        eval.raise(40);
                    }
                }
            }

            Step::VerifyHash {
                file_path,
                expected_sha256,
            } => {
                if file_path.exists() {
                    match hash::verify_file_sha256(file_path, expected_sha256) {
                        Ok(_) => eval.signals.sha256_verified = Some(true),
                        Err(err) => {
                            eval.signals.sha256_verified = Some(false);
                            eval.reasons.push(format!("{CHECKSUM_MISMATCH_REASON}: {err}"));
                            eval.raise(100);
                        }
                    }
                }
            }

            Step::RunVendorInstaller { .. } => {
                eval.reasons.push(
                    "Runs the official vendor installer headlessly with fixed, typed arguments".to_string(),
                );
            }

            Step::AddToUserPath { .. }
            | Step::CreateIsolatedEnv { .. }
            | Step::RunVerifyCommand { .. }
            | Step::AddDiscoveredToolToPath { .. } => {
                // Evaluated by deterministic policy above
            }
        }

        Ok(eval.into_report(&planned_step.id))
    }
}

/// What a step's evaluation needs to know about the plan around it.
#[derive(Default)]
struct StepContext {
    /// Checksums written into the plan are only trusted when the plan is a curated recipe.
    pins_trusted: bool,
    /// Plan-level policy findings that belong to this step.
    plan_violations: Vec<String>,
}

/// Registry the typosquat radar should compare a download id against.
fn registry_ecosystem(adapter: AdapterKind) -> Option<Ecosystem> {
    match adapter {
        AdapterKind::Modrinth => Some(Ecosystem::Modrinth),
        AdapterKind::PyPI => Some(Ecosystem::PyPI),
        AdapterKind::Npm => Some(Ecosystem::Npm),
        AdapterKind::Cargo => Some(Ecosystem::Cargo),
        AdapterKind::Winget | AdapterKind::GitHubReleases | AdapterKind::OfficialVendorUrl => None,
    }
}

/// Mutable accumulator for a single step's safety evaluation.
#[derive(Default)]
struct StepEvaluation {
    signals: SafetySignals,
    reasons: Vec<String>,
    suggested_alternative: Option<String>,
    score: u8,
}

impl StepEvaluation {
    fn raise(&mut self, score: u8) {
        self.score = self.score.max(score);
    }

    fn into_report(mut self, step_id: &str) -> SafetyReport {
        if self.reasons.is_empty() {
            self.reasons.push(
                "Passed all OSV.dev, typosquatting, hash, domain allowlist, and policy checks".to_string(),
            );
        }
        SafetyReport {
            step_id: step_id.to_string(),
            rating: rating_for_score(self.score),
            risk_score: self.score,
            reasons: self.reasons,
            suggested_alternative: self.suggested_alternative,
            signals: self.signals,
        }
    }
}

/// Maps a 0-100 risk score to its traffic-light rating.
pub fn rating_for_score(score: u8) -> RiskRating {
    match score {
        0..=29 => RiskRating::Green,
        30..=69 => RiskRating::Yellow,
        _ => RiskRating::Red,
    }
}

impl PlanSafetyReport {
    /// Raises one step to at least `min_rating` with an extra reason, keeping the plan-level
    /// rating, score, and `can_execute` consistent.
    pub fn escalate_step(&mut self, step_id: &str, min_rating: RiskRating, reason: String) {
        let min_score = match min_rating {
            RiskRating::Green => 0,
            RiskRating::Yellow => 50,
            RiskRating::Red => 100,
        };
        if let Some(report) = self.step_reports.iter_mut().find(|r| r.step_id == step_id) {
            if report.reasons.len() == 1 && report.reasons[0].starts_with("Passed all") {
                report.reasons.clear();
            }
            report.reasons.push(reason);
            report.risk_score = report.risk_score.max(min_score);
            report.rating = report.rating.max(min_rating).max(rating_for_score(report.risk_score));
        }
        self.overall_rating = self
            .step_reports
            .iter()
            .map(|r| r.rating)
            .max()
            .unwrap_or(RiskRating::Green);
        self.overall_score = self.step_reports.iter().map(|r| r.risk_score).max().unwrap_or(0);
        self.can_execute = self.overall_rating != RiskRating::Red;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{
        AdapterError, ExecutionContext, ExecutionReceipt, FetchedArtifact, SourceAdapter,
        VerificationResult,
    };
    use crate::types::{AdapterKind, Ecosystem};
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};

    struct FailingOsv;

    #[async_trait]
    impl OsvClient for FailingOsv {
        async fn query_package(
            &self,
            _ecosystem: Ecosystem,
            _package_name: &str,
            _version: Option<&str>,
        ) -> Result<Vec<VulnerabilitySummary>, CoreError> {
            Err(CoreError::Os("HTTP 503 from OSV.dev".to_string()))
        }
    }

    struct CleanOsv;

    #[async_trait]
    impl OsvClient for CleanOsv {
        async fn query_package(
            &self,
            _ecosystem: Ecosystem,
            _package_name: &str,
            _version: Option<&str>,
        ) -> Result<Vec<VulnerabilitySummary>, CoreError> {
            Ok(Vec::new())
        }
    }

    /// Adapter returning fixed metadata so evaluations run offline.
    struct FixedAdapter {
        kind: AdapterKind,
        meta: PackageMetadata,
    }

    #[async_trait]
    impl SourceAdapter for FixedAdapter {
        fn kind(&self) -> AdapterKind {
            self.kind
        }
        async fn resolve_metadata(&self, _id: &str, _v: Option<&str>) -> Result<PackageMetadata, AdapterError> {
            Ok(self.meta.clone())
        }
        async fn fetch_artifact(
            &self,
            _m: &PackageMetadata,
            _d: &Path,
            _c: &ExecutionContext,
        ) -> Result<FetchedArtifact, AdapterError> {
            Err(AdapterError::Other("not used".to_string()))
        }
        async fn install(
            &self,
            _m: &PackageMetadata,
            _a: Option<&FetchedArtifact>,
            _c: &ExecutionContext,
        ) -> Result<ExecutionReceipt, AdapterError> {
            Err(AdapterError::Other("not used".to_string()))
        }
        async fn verify(&self, _m: &PackageMetadata, _c: &ExecutionContext) -> Result<VerificationResult, AdapterError> {
            Err(AdapterError::Other("not used".to_string()))
        }
    }

    fn engine(osv: Arc<dyn OsvClient>, adapter: FixedAdapter) -> SafetyEngine {
        let mut reg = AdapterRegistry::new();
        reg.register(Arc::new(adapter));
        SafetyEngine::new(reg, osv, None, EventBus::default())
    }

    fn pypi_meta(sha: Option<&str>) -> PackageMetadata {
        let mut m = PackageMetadata::bare(AdapterKind::PyPI, "numpy", "2.1.1");
        m.age_days = Some(400);
        m.download_url = Some("https://files.pythonhosted.org/packages/numpy.whl".to_string());
        m.expected_sha256 = sha.map(str::to_string);
        m
    }

    fn install_step() -> PlannedStep {
        PlannedStep {
            id: "step-1".to_string(),
            description: "Install numpy".to_string(),
            step: Step::InstallPackage {
                ecosystem: Ecosystem::PyPI,
                package_id: "numpy".to_string(),
                version: Some("2.1.1".to_string()),
                source_adapter: AdapterKind::PyPI,
            },
        }
    }

    const REAL_DIGEST: &str = "61e035bf7bf70153e127440ce34de47c9036f0a2d0c65d1529454bd35ceefe4f";

    #[tokio::test]
    async fn osv_outage_fails_closed_to_yellow() {
        let eng = engine(
            Arc::new(FailingOsv),
            FixedAdapter {
                kind: AdapterKind::PyPI,
                meta: pypi_meta(Some(REAL_DIGEST)),
            },
        );
        let report = eng.evaluate_step(&install_step()).await.unwrap();
        assert_eq!(report.rating, RiskRating::Yellow);
        assert!(report.reasons.iter().any(|r| r.starts_with(OSV_UNAVAILABLE_REASON)));
    }

    #[tokio::test]
    async fn upstream_digest_is_green_and_missing_digest_is_yellow() {
        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter {
                kind: AdapterKind::PyPI,
                meta: pypi_meta(Some(REAL_DIGEST)),
            },
        );
        let report = eng.evaluate_step(&install_step()).await.unwrap();
        assert_eq!(report.rating, RiskRating::Green);
        assert_eq!(report.signals.sha256_verified, Some(true));

        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter {
                kind: AdapterKind::PyPI,
                meta: pypi_meta(None),
            },
        );
        let report = eng.evaluate_step(&install_step()).await.unwrap();
        assert_eq!(report.rating, RiskRating::Yellow);
        assert_eq!(report.signals.sha256_verified, Some(false));
        assert!(report.reasons.iter().any(|r| r.starts_with(NO_UPSTREAM_CHECKSUM_REASON)));
        assert_eq!(report.user_facing_translation().0, "No official checksum");
    }

    fn download_step(pinned: Option<&str>) -> PlannedStep {
        PlannedStep {
            id: "step-2".to_string(),
            description: "Download installer".to_string(),
            step: Step::DownloadArtifact {
                adapter: AdapterKind::OfficialVendorUrl,
                id: "fabric-installer".to_string(),
                version: "1.1.2".to_string(),
                destination: PathBuf::from("%LOCALAPPDATA%/SetupPal/downloads/fabric-installer.jar"),
                expected_sha256: pinned.map(str::to_string),
            },
        }
    }

    fn vendor_meta(sha: Option<&str>) -> PackageMetadata {
        let mut m = PackageMetadata::bare(AdapterKind::OfficialVendorUrl, "fabric-installer", "1.1.2");
        m.download_url = Some("https://maven.fabricmc.net/net/fabricmc/fabric-installer/1.1.2/fabric-installer-1.1.2.jar".to_string());
        m.expected_sha256 = sha.map(str::to_string);
        m
    }

    #[tokio::test]
    async fn download_without_any_checksum_is_yellow() {
        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter {
                kind: AdapterKind::OfficialVendorUrl,
                meta: vendor_meta(None),
            },
        );
        let report = eng.evaluate_step(&download_step(None)).await.unwrap();
        assert_eq!(report.rating, RiskRating::Yellow);
        assert!(report.reasons.iter().any(|r| r.starts_with(NO_UPSTREAM_CHECKSUM_REASON)));
    }

    #[tokio::test]
    async fn download_with_matching_upstream_digest_is_green_and_mismatch_is_red() {
        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter {
                kind: AdapterKind::OfficialVendorUrl,
                meta: vendor_meta(Some(REAL_DIGEST)),
            },
        );
        let report = eng.evaluate_step(&download_step(Some(REAL_DIGEST))).await.unwrap();
        assert_eq!(report.rating, RiskRating::Green);
        assert_eq!(report.signals.sha256_verified, Some(true));

        let other = "537bc45224e81dd1cbae25c1a972f14dac2456f0f8725ced3ca88ae41d0156d3";
        let report = eng.evaluate_step(&download_step(Some(other))).await.unwrap();
        assert_eq!(report.rating, RiskRating::Red);
        assert_eq!(report.user_facing_translation().0, "Unchecked file integrity");
    }

    #[tokio::test]
    async fn placeholder_pinned_hash_is_blocked() {
        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter {
                kind: AdapterKind::OfficialVendorUrl,
                meta: vendor_meta(None),
            },
        );
        let report = eng.evaluate_step(&download_step(Some(&"f".repeat(64)))).await.unwrap();
        assert_eq!(report.rating, RiskRating::Red);
    }

    fn plan_of(source: PlanSource, steps: Vec<PlannedStep>) -> Plan {
        Plan {
            plan_id: "p".to_string(),
            goal: "fabric".to_string(),
            summary: "s".to_string(),
            source,
            steps,
        }
    }

    fn llm() -> PlanSource {
        PlanSource::Llm { model: "m".to_string() }
    }

    #[tokio::test]
    async fn a_checksum_pinned_by_the_ai_does_not_verify_a_download() {
        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter { kind: AdapterKind::OfficialVendorUrl, meta: vendor_meta(None) },
        );
        let ai = eng.evaluate_plan(&plan_of(llm(), vec![download_step(Some(REAL_DIGEST))])).await.unwrap();
        assert_eq!(ai.step_reports[0].rating, RiskRating::Yellow);
        assert_eq!(ai.step_reports[0].signals.sha256_verified, Some(false));

        let recipe = PlanSource::Recipe { recipe_id: "r".to_string() };
        let curated = eng.evaluate_plan(&plan_of(recipe, vec![download_step(Some(REAL_DIGEST))])).await.unwrap();
        assert_eq!(curated.step_reports[0].rating, RiskRating::Green);
    }

    fn fabric_installer_step(path: &str) -> PlannedStep {
        PlannedStep {
            id: "step-3".to_string(),
            description: "Install the Fabric loader".to_string(),
            step: Step::RunVendorInstaller {
                installer: crate::types::VendorInstaller::FabricClient,
                mc_version: "1.21.1".to_string(),
                installer_path: PathBuf::from(path),
            },
        }
    }

    #[tokio::test]
    async fn vendor_installer_must_run_the_jar_this_plan_downloaded() {
        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter { kind: AdapterKind::OfficialVendorUrl, meta: vendor_meta(Some(REAL_DIGEST)) },
        );
        let good = eng
            .evaluate_plan(&plan_of(llm(), vec![
                download_step(None),
                fabric_installer_step("%LOCALAPPDATA%/SetupPal/downloads/fabric-installer.jar"),
            ]))
            .await
            .unwrap();
        assert_eq!(good.step_reports[1].rating, RiskRating::Green);

        let other_jar = eng
            .evaluate_plan(&plan_of(llm(), vec![
                download_step(None),
                fabric_installer_step("%LOCALAPPDATA%/SetupPal/downloads/other.jar"),
            ]))
            .await
            .unwrap();
        assert_eq!(other_jar.step_reports[1].rating, RiskRating::Red);
        assert!(!other_jar.can_execute);
    }

    #[tokio::test]
    async fn lookalike_mod_slugs_are_caught_on_download_steps() {
        let mut meta = PackageMetadata::bare(AdapterKind::Modrinth, "sodiun", "1");
        meta.download_url = Some("https://cdn.modrinth.com/data/x/sodiun.jar".to_string());
        meta.expected_sha512 = Some("a".repeat(128));
        let eng = engine(Arc::new(CleanOsv), FixedAdapter { kind: AdapterKind::Modrinth, meta });
        let step = PlannedStep {
            id: "step-1".to_string(),
            description: "Download sodiun".to_string(),
            step: Step::DownloadArtifact {
                adapter: AdapterKind::Modrinth,
                id: "sodiun".to_string(),
                version: "1".to_string(),
                destination: PathBuf::from("%APPDATA%/.minecraft/mods/sodiun.jar"),
                expected_sha256: None,
            },
        };
        let report = eng.evaluate_step(&step).await.unwrap();
        assert_eq!(report.rating, RiskRating::Red);
        assert_eq!(report.suggested_alternative.as_deref(), Some("sodium"));
    }

    struct FixedTavily(TavilyVerificationResult);

    #[async_trait]
    impl TavilyVerifier for FixedTavily {
        async fn verify_source(&self, _n: &str, _u: &str) -> Result<TavilyVerificationResult, CoreError> {
            Ok(self.0.clone())
        }
    }

    fn github_step(repo: &str) -> PlannedStep {
        PlannedStep {
            id: "step-1".to_string(),
            description: format!("Install {repo}"),
            step: Step::InstallPackage {
                ecosystem: Ecosystem::GitHub,
                package_id: repo.to_string(),
                version: None,
                source_adapter: AdapterKind::GitHubReleases,
            },
        }
    }

    fn github_engine(tavily: Option<TavilyVerificationResult>) -> SafetyEngine {
        let mut meta = PackageMetadata::bare(AdapterKind::GitHubReleases, "x/y", "v1");
        meta.download_url = Some("https://github.com/x/y/releases/download/v1/y.zip".to_string());
        meta.expected_sha256 = Some(REAL_DIGEST.to_string());
        meta.age_days = Some(400);
        let mut reg = AdapterRegistry::new();
        reg.register(Arc::new(FixedAdapter { kind: AdapterKind::GitHubReleases, meta }));
        let tavily = tavily.map(|r| Arc::new(FixedTavily(r)) as Arc<dyn TavilyVerifier>);
        SafetyEngine::new(reg, Arc::new(CleanOsv), tavily, EventBus::default())
    }

    #[tokio::test]
    async fn github_repos_are_checked_with_tavily_unless_curated() {
        let fake = tavily::judge_sources(
            "obs-studio",
            "github.com/obs-free/obs-studio",
            &[tavily::TavilySearchResultItem {
                url: "https://github.com/obsproject/obs-studio".to_string(),
                score: Some(0.9),
            }],
        );
        let report = github_engine(Some(fake)).evaluate_step(&github_step("obs-free/obs-studio")).await.unwrap();
        assert_eq!(report.rating, RiskRating::Red);
        assert_eq!(report.user_facing_translation().0, "Lookalike warning");

        let no_key = github_engine(None).evaluate_step(&github_step("someone/tinytool")).await.unwrap();
        assert_eq!(no_key.rating, RiskRating::Yellow);
        assert!(no_key.user_facing_translation().1.contains("Tavily key"));

        let curated = github_engine(None).evaluate_step(&github_step("BurntSushi/ripgrep")).await.unwrap();
        assert_eq!(curated.rating, RiskRating::Green);
    }

    #[test]
    fn escalate_step_updates_plan_totals() {
        let mut plan_report = PlanSafetyReport {
            plan_id: "p".to_string(),
            overall_rating: RiskRating::Green,
            overall_score: 0,
            step_reports: vec![StepEvaluation::default().into_report("step-1")],
            can_execute: true,
            notes: Vec::new(),
        };
        plan_report.escalate_step("step-1", RiskRating::Yellow, "Administrator access needed".to_string());
        assert_eq!(plan_report.overall_rating, RiskRating::Yellow);
        assert_eq!(plan_report.step_reports[0].reasons, vec!["Administrator access needed".to_string()]);
        assert_eq!(
            plan_report.step_reports[0].user_facing_translation().0,
            "Administrator access needed"
        );
        assert!(plan_report.can_execute);
    }

    #[test]
    fn young_first_publish_is_yellow_established_is_green() {
        let mut young = StepEvaluation::default();
        let mut meta = PackageMetadata::bare(AdapterKind::PyPI, "brandnew", "0.1.0");
        meta.age_days = Some(7);
        SafetyEngine::check_registry_heuristics("brandnew", &meta, &mut young, true);
        assert!(young.score >= 45);
        assert!(young.reasons.iter().any(|r| r.contains("first published")));

        let mut old = StepEvaluation::default();
        meta.age_days = Some(400);
        SafetyEngine::check_registry_heuristics("numpy", &meta, &mut old, true);
        assert_eq!(old.score, 0);

        let mut winget_established = StepEvaluation::default();
        let mut wmeta = PackageMetadata::bare(AdapterKind::Winget, "OBSProject.OBSStudio", "31.0.0");
        wmeta.age_days = None; // multi-version identifier
        SafetyEngine::check_registry_heuristics("OBSProject.OBSStudio", &wmeta, &mut winget_established, true);
        assert_eq!(winget_established.score, 0);
    }

    #[tokio::test]
    async fn winget_plans_get_one_osv_coverage_note() {
        let mut meta = PackageMetadata::bare(AdapterKind::Winget, "VideoLAN.VLC", "3.0.21");
        meta.age_days = None;
        meta.expected_sha256 = Some(REAL_DIGEST.to_string());
        let eng = engine(
            Arc::new(CleanOsv),
            FixedAdapter {
                kind: AdapterKind::Winget,
                meta,
            },
        );
        let plan = plan_of(
            PlanSource::Resolved {
                query: "vlc".into(),
            },
            vec![PlannedStep {
                id: "step-1".to_string(),
                description: "Install VLC".to_string(),
                step: Step::InstallPackage {
                    ecosystem: Ecosystem::Winget,
                    package_id: "VideoLAN.VLC".to_string(),
                    version: None,
                    source_adapter: AdapterKind::Winget,
                },
            }],
        );
        let report = eng.evaluate_plan(&plan).await.unwrap();
        assert_eq!(report.notes, vec![OSV_WINGET_COVERAGE_NOTE.to_string()]);
        assert!(!report.step_reports[0]
            .reasons
            .iter()
            .any(|r| r.contains("OSV.dev does not cover")));
    }
}

