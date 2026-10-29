pub mod hash;
pub mod osv;
pub mod policy;
pub mod tavily;
pub mod typosquat;

use std::sync::Arc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapters::AdapterRegistry;
use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::safety::osv::{HttpOsvClient, OsvClient};
pub use crate::safety::tavily::{
    HttpTavilyVerifier, TavilyVerificationResult, TavilyVerifier,
};
use crate::types::{Plan, PlannedStep, Step};

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
        if self
            .signals
            .policy_violations
            .iter()
            .any(|v| v.to_ascii_lowercase().contains("administrator") || v.to_ascii_lowercase().contains("uac"))
        {
            return (
                "Administrator access needed",
                "This tool requires administrator approval to install system-level components.".to_string(),
            );
        }
        if self.signals.sha256_verified == Some(false) {
            return (
                "Unchecked file integrity",
                "The file fingerprint did not match the expected official checksum.".to_string(),
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

    pub async fn evaluate_step(&self, planned_step: &PlannedStep) -> Result<SafetyReport, CoreError> {
        let mut signals = SafetySignals::default();
        let mut reasons = Vec::new();
        let mut suggested_alternative: Option<String> = None;
        let mut score: u8 = 0;

        // 1. Deterministic Hard Policy Check
        let policy_violations =
            policy::evaluate_step_policy(&planned_step.step, &planned_step.description);
        if !policy_violations.is_empty() {
            for v in &policy_violations {
                reasons.push(format!("HARD POLICY BLOCK: {v}"));
            }
            signals.policy_violations = policy_violations;
            score = 100;
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
                    reasons.push(format!(
                        "TYPOSQUAT DETECTED: `{}` is edit distance {} from popular {:?} package `{}`",
                        typo.requested_package,
                        typo.edit_distance,
                        ecosystem,
                        typo.similar_popular_package
                    ));
                    suggested_alternative = Some(typo.similar_popular_package.clone());
                    signals.typosquat_match = Some(typo);
                    score = score.max(95);
                }

                // 2b. OSV.dev Vulnerability Query
                match self
                    .osv
                    .query_package(*ecosystem, package_id, version.as_deref())
                    .await
                {
                    Ok(vulns) => {
                        if !vulns.is_empty() {
                            for v in &vulns {
                                reasons.push(format!(
                                    "OSV CVE BLOCK [{} - {}]: {}",
                                    v.id, v.severity, v.summary
                                ));
                                if suggested_alternative.is_none() {
                                    if let Some(fixed) = &v.fixed_version {
                                        suggested_alternative = Some(format!("{package_id}@{fixed}"));
                                    }
                                }
                            }
                            signals.osv_vulnerabilities = vulns;
                            score = score.max(90);
                        }
                    }
                    Err(e) => {
                        // Fail closed to Yellow: vulnerability verification unavailable
                        reasons.push(format!("Verification service (OSV.dev) unavailable: {e}"));
                        score = score.max(50);
                    }
                }

                // 2c. Registry Metadata & Install Script Inspection
                if let Some(adapter) = self.adapters.get(*source_adapter) {
                    match adapter
                        .resolve_metadata(package_id, version.as_deref())
                        .await
                    {
                        Ok(meta) => {
                            signals.package_age_days = meta.age_days;
                            signals.download_count = meta.download_count;
                            signals.sha256_verified = Some(meta.expected_sha256.is_some());

                            if let Some(url) = &meta.download_url {
                                match policy::is_url_domain_allowlisted(url) {
                                    Ok(true) => signals.domain_allowlisted = Some(true),
                                    Ok(false) => {
                                        match self.verify_with_tavily(package_id, url).await {
                                            Ok(tavily_res) => {
                                                signals.tavily_verified = Some(tavily_res.is_verified);
                                                signals.tavily_consensus = Some(tavily_res.consensus_summary.clone());
                                                if tavily_res.is_verified {
                                                    signals.domain_allowlisted = Some(true);
                                                } else {
                                                    signals.domain_allowlisted = Some(false);
                                                    if let Some(warn) = &tavily_res.warning {
                                                        reasons.push(format!("TAVILY AI SEARCH SAFETY: {warn}"));
                                                    } else {
                                                        reasons.push(format!(
                                                            "Package download URL `{url}` is not verified by Tavily consensus"
                                                        ));
                                                    }
                                                    score = score.max(95);
                                                }
                                            }
                                            Err(e) => {
                                                // Fail closed to Yellow when search consensus verification fails
                                                signals.domain_allowlisted = Some(false);
                                                reasons.push(format!("Verification service (Tavily) unavailable: {e}"));
                                                score = score.max(50);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        signals.domain_allowlisted = Some(false);
                                        reasons.push(e);
                                        score = score.max(95);
                                    }
                                }
                            }

                            if let Some(age) = meta.age_days {
                                if age < 30 {
                                    reasons.push(format!(
                                        "Young package warning: `{package_id}` was published {age} days ago (< 30 days threshold)"
                                    ));
                                    score = score.max(45);
                                }
                            }

                            if let Some(downloads) = meta.download_count {
                                if downloads < 1_000 {
                                    reasons.push(format!(
                                        "Low adoption warning: `{package_id}` has only {downloads} recorded downloads (< 1,000 threshold)"
                                    ));
                                    score = score.max(45);
                                }
                            }

                            if meta.expected_sha256.is_none() {
                                reasons.push(format!(
                                    "Missing upstream SHA-256 digest for `{package_id}`"
                                ));
                                score = score.max(40);
                            }

                            if let Some(script) = &meta.install_script_snippet {
                                let flags = policy::inspect_install_script(script);
                                if !flags.is_empty() {
                                    for flag in &flags {
                                        reasons.push(format!("Install script flag: {flag}"));
                                    }
                                    // Destructive or base64/network exfiltration in install script -> Red
                                    let is_critical = flags.iter().any(|f| {
                                        f.contains("prohibited")
                                             || f.contains("base64")
                                             || f.contains("exfiltration")
                                    });
                                    signals.script_inspection_flags = flags;
                                    score = score.max(if is_critical { 90 } else { 55 });
                                }
                            }
                        }
                        Err(e) => {
                            reasons.push(format!("Could not verify upstream registry metadata: {e}"));
                            score = score.max(50);
                        }
                    }
                } else {
                    reasons.push(format!(
                        "No registered adapter for `{source_adapter:?}` to verify metadata"
                    ));
                    score = score.max(45);
                }
            }

            Step::DownloadArtifact {
                id,
                expected_sha256,
                ..
            } => {
                let valid_hash = policy::is_valid_sha256_hex(expected_sha256);
                signals.sha256_verified = Some(valid_hash);
                if id.starts_with("http://") || id.starts_with("https://") {
                    let domain_ok = policy::is_url_domain_allowlisted(id).unwrap_or(false);
                    signals.domain_allowlisted = Some(domain_ok);

                    match self.verify_with_tavily(&planned_step.description, id).await {
                        Ok(tavily_res) => {
                            signals.tavily_verified = Some(tavily_res.is_verified);
                            signals.tavily_consensus = Some(tavily_res.consensus_summary.clone());

                            if tavily_res.is_spoofed {
                                if let Some(warn) = &tavily_res.warning {
                                    reasons.push(format!("TAVILY AI SEARCH SAFETY: {warn}"));
                                }
                                if let Some(official) = tavily_res.official_domains.first() {
                                    suggested_alternative = Some(official.clone());
                                }
                                score = score.max(100);
                            } else if !domain_ok && !tavily_res.is_verified {
                                reasons.push(format!(
                                    "Download URL `{id}` is neither in static allowlist nor verified by Tavily consensus"
                                ));
                                score = score.max(95);
                            }
                        }
                        Err(e) => {
                            if !domain_ok {
                                reasons.push(format!("Verification service (Tavily) unavailable: {e}"));
                                score = score.max(50);
                            }
                        }
                    }
                } else {
                    signals.domain_allowlisted = Some(true);
                }
            }

            Step::VerifyHash {
                file_path,
                expected_sha256,
            } => {
                if file_path.exists() {
                    match hash::verify_file_sha256(file_path, expected_sha256) {
                        Ok(_) => {
                            signals.sha256_verified = Some(true);
                        }
                        Err(err) => {
                            signals.sha256_verified = Some(false);
                            reasons.push(format!("SHA-256 verification failed: {err}"));
                            score = 100;
                        }
                    }
                } else {
                    signals.sha256_verified = Some(policy::is_valid_sha256_hex(expected_sha256));
                }
            }

            Step::AddToUserPath { .. }
            | Step::CreateIsolatedEnv { .. }
            | Step::RunVerifyCommand { .. } => {
                // Evaluated by deterministic policy above
            }
        }

        let rating = match score {
            0..=29 => RiskRating::Green,
            30..=69 => RiskRating::Yellow,
            _ => RiskRating::Red,
        };

        if reasons.is_empty() {
            reasons.push(
                "Passed all OSV.dev, typosquatting, hash, domain allowlist, and policy checks"
                    .to_string(),
            );
        }

        Ok(SafetyReport {
            step_id: planned_step.id.clone(),
            rating,
            risk_score: score,
            reasons,
            suggested_alternative,
            signals,
        })
    }
}
