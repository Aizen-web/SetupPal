use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SafetySignals {
    pub osv_vulnerabilities: Vec<VulnerabilitySummary>,
    pub typosquat_match: Option<TyposquatMatch>,
    pub package_age_days: Option<u64>,
    pub download_count: Option<u64>,
    pub sha256_verified: Option<bool>,
    pub domain_allowlisted: Option<bool>,
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
