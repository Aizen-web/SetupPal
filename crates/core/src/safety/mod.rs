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
use crate::types::{Plan, PlannedStep, Step};

/// Reason text for steps without an upstream digest (keeps them at least Yellow).
pub const NO_UPSTREAM_CHECKSUM_REASON: &str = "No upstream checksum available";
/// Reason text when a plan-pinned digest disagrees with the upstream registry.
pub const CHECKSUM_MISMATCH_REASON: &str = "Pinned checksum does not match the upstream registry";
/// Reason prefix used when OSV.dev cannot be reached or returns an error.
pub const OSV_UNAVAILABLE_REASON: &str = "Verification service (OSV.dev) unavailable";

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

        for planned_step in &plan.steps {
            let report = self.evaluate_step(planned_step).await?;
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
        let plan_report = PlanSafetyReport {
            plan_id: plan.plan_id.clone(),
            overall_rating,
            overall_score,
            step_reports,
            can_execute,
        };

        self.events.emit(AgentEvent::SafetyCheckCompleted {
            report: Box::new(plan_report.clone()),
        });

        Ok(plan_report)
    }

    /// Checks a download URL against the static allowlist, consulting Tavily consensus only
    /// for hosts outside it.
    async fn check_download_url(&self, software_name: &str, url: &str, eval: &mut StepEvaluation) {
        match policy::is_url_domain_allowlisted(url) {
            Ok(true) => eval.signals.domain_allowlisted = Some(true),
            Ok(false) => match self.verify_with_tavily(software_name, url).await {
                Ok(tavily_res) => {
                    eval.signals.tavily_verified = Some(tavily_res.is_verified);
                    eval.signals.tavily_consensus = Some(tavily_res.consensus_summary.clone());
                    eval.signals.domain_allowlisted = Some(tavily_res.is_verified);
                    if tavily_res.is_spoofed {
                        if let Some(warn) = &tavily_res.warning {
                            eval.reasons.push(format!("TAVILY AI SEARCH SAFETY: {warn}"));
                        }
                        if let Some(official) = tavily_res.official_domains.first() {
                            eval.suggested_alternative = Some(official.clone());
                        }
                        eval.raise(100);
                    } else if !tavily_res.is_verified {
                        eval.reasons.push(format!(
                            "Download URL `{url}` is neither in the static allowlist nor verified by Tavily consensus"
                        ));
                        eval.raise(95);
                    }
                }
                Err(e) => {
                    eval.signals.domain_allowlisted = Some(false);
                    eval.reasons.push(format!("Verification service (Tavily) unavailable: {e}"));
                    eval.raise(50);
                }
            },
            Err(e) => {
                eval.signals.domain_allowlisted = Some(false);
                eval.reasons.push(e);
                eval.raise(95);
            }
        }
    }

    fn check_registry_heuristics(package_id: &str, meta: &PackageMetadata, eval: &mut StepEvaluation) {
        eval.signals.package_age_days = meta.age_days;
        eval.signals.download_count = meta.download_count;

        if let Some(age) = meta.age_days {
            if age < 30 {
                eval.reasons.push(format!(
                    "Young package warning: `{package_id}` was published {age} days ago (< 30 days threshold)"
                ));
                eval.raise(45);
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

    pub async fn evaluate_step(&self, planned_step: &PlannedStep) -> Result<SafetyReport, CoreError> {
        let mut eval = StepEvaluation::default();

        // 1. Deterministic Hard Policy Check
        let policy_violations =
            policy::evaluate_step_policy(&planned_step.step, &planned_step.description);
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
                if let Some(typo) = typosquat::detect_typosquat(*ecosystem, package_id) {
                    eval.reasons.push(format!(
                        "TYPOSQUAT DETECTED: `{}` is edit distance {} from popular {:?} package `{}`",
                        typo.requested_package,
                        typo.edit_distance,
                        ecosystem,
                        typo.similar_popular_package
                    ));
                    eval.suggested_alternative = Some(typo.similar_popular_package.clone());
                    eval.signals.typosquat_match = Some(typo);
                    eval.raise(95);
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
                } else {
                    eval.reasons.push(format!(
                        "OSV.dev does not cover {ecosystem:?} packages; relying on registry metadata, checksums, and allowlists"
                    ));
                }

                // 2c. Registry Metadata & Install Script Inspection
                if let Some(adapter) = self.adapters.get(*source_adapter) {
                    match adapter.resolve_metadata(package_id, version.as_deref()).await {
                        Ok(meta) => {
                            let has_digest = meta.has_upstream_digest();
                            eval.signals.sha256_verified = Some(has_digest);
                            if let Some(url) = &meta.download_url {
                                self.check_download_url(package_id, url, &mut eval).await;
                            }
                            Self::check_registry_heuristics(package_id, &meta, &mut eval);
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
                    self.check_download_url(&planned_step.description, url, &mut eval).await;
                }

                if let Some(m) = &meta {
                    Self::check_registry_heuristics(id, m, &mut eval);
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
                    (Some(_), _) => {
                        eval.reasons.push(format!(
                            "Checksum for `{id}` is pinned by the plan; the upstream source did not publish one"
                        ));
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

    #[test]
    fn escalate_step_updates_plan_totals() {
        let mut plan_report = PlanSafetyReport {
            plan_id: "p".to_string(),
            overall_rating: RiskRating::Green,
            overall_score: 0,
            step_reports: vec![StepEvaluation::default().into_report("step-1")],
            can_execute: true,
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
}

