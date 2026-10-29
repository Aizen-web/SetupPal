use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::CoreError;
use crate::safety::VulnerabilitySummary;
use crate::types::Ecosystem;

pub const OSV_QUERY_URL: &str = "https://api.osv.dev/v1/query";

/// Trait abstracting OSV.dev vulnerability lookups so unit tests run offline with zero network dependency.
#[async_trait]
pub trait OsvClient: Send + Sync {
    async fn query_package(
        &self,
        ecosystem: Ecosystem,
        package_name: &str,
        version: Option<&str>,
    ) -> Result<Vec<VulnerabilitySummary>, CoreError>;
}

/// Live HTTP client querying `https://api.osv.dev/v1/query`.
#[derive(Debug, Clone)]
pub struct HttpOsvClient {
    http: reqwest::Client,
    endpoint: String,
}

#[derive(Debug, Serialize)]
struct OsvQueryRequest<'a> {
    package: OsvPackage<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct OsvPackage<'a> {
    name: &'a str,
    ecosystem: &'a str,
}

#[derive(Debug, Deserialize)]
struct OsvQueryResponse {
    #[serde(default)]
    vulns: Vec<OsvVulnEntry>,
}

#[derive(Debug, Deserialize)]
struct OsvVulnEntry {
    id: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    database_specific: Option<serde_json::Value>,
    #[serde(default)]
    affected: Vec<OsvAffected>,
}

#[derive(Debug, Deserialize)]
struct OsvAffected {
    #[serde(default)]
    ranges: Vec<OsvRange>,
}

#[derive(Debug, Deserialize)]
struct OsvRange {
    #[serde(default)]
    events: Vec<OsvRangeEvent>,
}

#[derive(Debug, Deserialize)]
struct OsvRangeEvent {
    #[serde(default)]
    fixed: Option<String>,
}

impl Default for HttpOsvClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpOsvClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            http,
            endpoint: OSV_QUERY_URL.to_string(),
        }
    }

    pub fn osv_ecosystem_name(ecosystem: Ecosystem) -> Option<&'static str> {
        match ecosystem {
            Ecosystem::PyPI => Some("PyPI"),
            Ecosystem::Npm => Some("npm"),
            _ => None,
        }
    }
}

#[async_trait]
impl OsvClient for HttpOsvClient {
    async fn query_package(
        &self,
        ecosystem: Ecosystem,
        package_name: &str,
        version: Option<&str>,
    ) -> Result<Vec<VulnerabilitySummary>, CoreError> {
        let Some(osv_eco) = Self::osv_ecosystem_name(ecosystem) else {
            return Ok( Vec::new() );
        };

        let req_body = OsvQueryRequest {
            package: OsvPackage {
                name: package_name,
                ecosystem: osv_eco,
            },
            version,
        };

        let resp = self.http.post(&self.endpoint).json(&req_body).send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let parsed: OsvQueryResponse = resp.json().await?;
        let summaries = parsed
            .vulns
            .into_iter()
            .map(|v| {
                let severity = v
                    .database_specific
                    .as_ref()
                    .and_then(|db| db.get("severity"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("HIGH")
                    .to_string();

                let fixed_version = v
                    .affected
                    .iter()
                    .flat_map(|a| &a.ranges)
                    .flat_map(|r| &r.events)
                    .find_map(|e| e.fixed.clone());

                let summary = v
                    .summary
                    .or_else(|| {
                        v.details
                            .map(|d| d.lines().next().unwrap_or_default().to_string())
                    })
                    .unwrap_or_else(|| "Known vulnerability reported by OSV.dev".to_string());

                VulnerabilitySummary {
                    id: v.id,
                    severity,
                    summary,
                    fixed_version,
                }
            })
            .collect();

        Ok(summaries)
    }
}
