use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::CoreError;

pub const TAVILY_SEARCH_URL: &str = "https://api.tavily.com/search";

/// Result of AI search-based source and domain verification.
/// Cross-references candidate download links against trusted search consensus
/// to defeat SEO-poisoned clones and malware mirrors (e.g. the DS4Windows scam).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TavilyVerificationResult {
    /// True if Tavily consensus confirms this domain/URL belongs to the authentic project
    pub is_verified: bool,
    /// True if Tavily consensus flags this domain as a known clone, typosquat, or lookalike
    pub is_spoofed: bool,
    /// Confidence score between 0.0 and 1.0
    pub confidence_score: f32,
    /// Authoritative domains/repositories identified by web consensus
    pub official_domains: Vec<String>,
    /// Natural-language summary explaining the consensus verdict
    pub consensus_summary: String,
    /// Actionable warning message when spoofing or impersonation is detected
    pub warning: Option<String>,
}

/// Trait abstracting Tavily web search verification .
#[async_trait]
pub trait TavilyVerifier: Send + Sync {
    async fn verify_source(
        &self,
        software_name: &str,
        candidate_url: &str,
    ) -> Result<TavilyVerificationResult, CoreError>;
}

/// Live HTTP client querying Tavily API (`https://api.tavily.com/search`).
#[derive(Debug, Clone)]
pub struct HttpTavilyVerifier {
    api_key: String,
    endpoint: String,
    http: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct TavilySearchRequest<'a> {
    api_key: &'a str,
    query: String,
    search_depth: &'a str,
    include_answer: bool,
    max_results: u8,
}

#[derive(Debug, Deserialize)]
struct TavilySearchResponse {
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    results: Vec<TavilySearchResultItem>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TavilySearchResultItem {
    #[serde(default)]
    url: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    score: Option<f32>,
}

impl HttpTavilyVerifier {
    pub fn new(api_key: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            api_key: api_key.into(),
            endpoint: TAVILY_SEARCH_URL.to_string(),
            http,
        }
    }

    pub fn with_custom_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let mut client = Self::new(api_key);
        client.endpoint = endpoint.into();
        client
    }
}

#[async_trait]
impl TavilyVerifier for HttpTavilyVerifier {
    async fn verify_source(
        &self,
        software_name: &str,
        candidate_url: &str,
    ) -> Result<TavilyVerificationResult, CoreError> {
        let candidate_parsed = Url::parse(candidate_url)
            .map_err(|e| CoreError::Validation(format!("Invalid candidate URL `{candidate_url}`: {e}")))?;
        let candidate_host = candidate_parsed
            .host_str()
            .unwrap_or("")
            .to_ascii_lowercase();

        let query = format!("official website and github repository for {software_name}");
        let payload = TavilySearchRequest {
            api_key: &self.api_key,
            query,
            search_depth: "basic",
            include_answer: true,
            max_results: 5,
        };

        let res = self
            .http
            .post(&self.endpoint)
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(CoreError::Validation(format!(
                "Tavily search API returned HTTP {status}: {body}"
            )));
        }

        let data: TavilySearchResponse = res.json().await?;
        let mut official_domains = Vec::new();
        let mut matches_candidate = false;

        for item in &data.results {
            if let Ok(parsed) = Url::parse(&item.url) {
                if let Some(host) = parsed.host_str() {
                    let host_lower = host.to_ascii_lowercase();
                    if !official_domains.contains(&host_lower) {
                        official_domains.push(host_lower.clone());
                    }
                    if host_lower == candidate_host
                        || candidate_host.ends_with(&format!(".{host_lower}"))
                        || host_lower.ends_with(&format!(".{candidate_host}"))
                    {
                        matches_candidate = true;
                    }
                }
            }
        }

        let summary = data
            .answer
            .unwrap_or_else(|| format!("Tavily identified official sources: {}", official_domains.join(", ")));

        // Lookalike / SEO poisoning detection:
        // If Tavily found authoritative domains (e.g. github.com, official domain)
        // and candidate domain is completely unrepresented in top consensus results:
        let is_spoofed = !matches_candidate && !official_domains.is_empty();
        let warning = if is_spoofed {
            Some(format!(
                "SEO Poisoning / Impersonation Alert: `{candidate_url}` does not match verified sources ({}) for `{software_name}`",
                official_domains.join(", ")
            ))
        } else {
            None
        };

        Ok(TavilyVerificationResult {
            is_verified: matches_candidate,
            is_spoofed,
            confidence_score: if matches_candidate { 0.95 } else { 0.85 },
            official_domains,
            consensus_summary: summary,
            warning,
        })
    }
}
