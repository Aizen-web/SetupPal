use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strsim::levenshtein;
use url::Url;

use crate::config::{fixed_secret, CoreError, SecretProvider};

pub const TAVILY_SEARCH_URL: &str = "https://api.tavily.com/search";

/// Download portals, mirrors, and forums. They rank well for "<app> download" but never count
/// as the project's official home, and SEO-poisoned clones hide among them.
const NON_OFFICIAL_DOMAINS: &[&str] = &[
    "wikipedia.org",
    "reddit.com",
    "youtube.com",
    "quora.com",
    "medium.com",
    "stackoverflow.com",
    "softonic.com",
    "uptodown.com",
    "filehippo.com",
    "filehorse.com",
    "majorgeeks.com",
    "softpedia.com",
    "malavida.com",
    "cnet.com",
    "sourceforge.net",
    "alternativeto.net",
    "techspot.com",
];

/// Results Tavily scores below this are too loosely related to count toward consensus.
const MIN_RESULT_SCORE: f32 = 0.3;

/// Result of web-search verification of a download source.
/// Cross-references the candidate source against what search consensus calls official, to
/// catch SEO-poisoned clones and fake GitHub repositories (e.g. the DS4Windows scam).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TavilyVerificationResult {
    /// True if search consensus lists the candidate as an official source.
    pub is_verified: bool,
    /// True if the candidate imitates an official source (same repo name under another owner,
    /// or a lookalike domain).
    pub is_spoofed: bool,
    /// Confidence score between 0.0 and 1.0
    pub confidence_score: f32,
    /// Official sources found by search (`example.org` or `github.com/owner/repo`).
    pub official_domains: Vec<String>,
    /// Plain-English summary of the verdict, built by SetupPal (never model or web text).
    pub consensus_summary: String,
    /// Actionable warning when spoofing or impersonation is detected.
    pub warning: Option<String>,
}

#[async_trait]
pub trait TavilyVerifier: Send + Sync {
    async fn verify_source(
        &self,
        software_name: &str,
        candidate_url: &str,
    ) -> Result<TavilyVerificationResult, CoreError>;
}

/// Live HTTP client querying the Tavily search API. Verdicts are cached per source for the
/// life of the process, so re-planning the same thing costs no extra searches.
#[derive(Clone)]
pub struct HttpTavilyVerifier {
    api_key: SecretProvider,
    endpoint: String,
    http: reqwest::Client,
    cache: Arc<Mutex<HashMap<String, TavilyVerificationResult>>>,
}

impl std::fmt::Debug for HttpTavilyVerifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpTavilyVerifier")
            .field("endpoint", &self.endpoint)
            .field("has_api_key", &(self.api_key)().is_some())
            .finish()
    }
}

#[derive(Debug, Serialize)]
struct TavilySearchRequest<'a> {
    query: String,
    search_depth: &'static str,
    include_answer: bool,
    max_results: u8,
    exclude_domains: &'a [&'a str],
}

#[derive(Debug, Deserialize)]
struct TavilySearchResponse {
    #[serde(default)]
    results: Vec<TavilySearchResultItem>,
}

#[derive(Debug, Deserialize)]
pub struct TavilySearchResultItem {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub score: Option<f32>,
}

impl HttpTavilyVerifier {
    pub fn new(api_key: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .unwrap_or_default();
        Self {
            api_key: fixed_secret(Some(api_key.into())),
            endpoint: TAVILY_SEARCH_URL.to_string(),
            http,
            cache: Arc::default(),
        }
    }

    /// Reads the key on every call, so saving or removing it takes effect immediately.
    pub fn with_key_provider(provider: SecretProvider) -> Self {
        let mut client = Self::new(String::new());
        client.api_key = provider;
        client
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
        let api_key = (self.api_key)()
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .ok_or_else(|| CoreError::Config("TAVILY_API_KEY is not set".to_string()))?;
        let candidate = site_identity(candidate_url).ok_or_else(|| {
            CoreError::Validation(format!("Invalid candidate URL `{candidate_url}`"))
        })?;

        let cache_key = format!("{}|{candidate}", software_name.trim().to_ascii_lowercase());
        if let Some(hit) = self.cache.lock().ok().and_then(|c| c.get(&cache_key).cloned()) {
            return Ok(hit);
        }

        let name = software_name.trim();
        let query = if candidate.starts_with("github.com/") {
            format!("\"{name}\" official GitHub repository")
        } else {
            format!("\"{name}\" official website download")
        };
        let payload = TavilySearchRequest {
            query,
            search_depth: "basic",
            include_answer: false,
            max_results: 8,
            exclude_domains: NON_OFFICIAL_DOMAINS,
        };

        let res = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&api_key)
            .json(&payload)
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let reason = match status.as_u16() {
                401 | 403 => "Tavily rejected the API key".to_string(),
                429 => "Tavily rate limit reached; try again in a minute".to_string(),
                432 | 433 => "Tavily plan limit reached".to_string(),
                _ => format!("Tavily search API returned HTTP {status}"),
            };
            return Err(CoreError::Validation(reason));
        }

        let data: TavilySearchResponse = res.json().await?;
        let verdict = judge_sources(name, &candidate, &data.results);
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(cache_key, verdict.clone());
        }
        Ok(verdict)
    }
}

/// What a URL claims to be: `github.com/owner/repo` for GitHub (the host is shared by
/// everyone), otherwise the registrable domain (`get.videolan.org` -> `videolan.org`).
pub fn site_identity(raw_url: &str) -> Option<String> {
    let url = Url::parse(raw_url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host == "github.com" {
        let mut segments = url.path_segments()?.filter(|s| !s.is_empty());
        return Some(match (segments.next(), segments.next()) {
            (Some(owner), Some(repo)) => format!(
                "github.com/{}/{}",
                owner.to_ascii_lowercase(),
                repo.trim_end_matches(".git").to_ascii_lowercase()
            ),
            _ => "github.com".to_string(),
        });
    }
    Some(registrable_domain(host))
}

fn registrable_domain(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    let n = labels.len();
    if n <= 2 {
        return labels.join(".");
    }
    let two_part_suffix =
        labels[n - 1].len() == 2 && ["co", "com", "org", "net", "ac", "gov", "edu"].contains(&labels[n - 2]);
    labels[n - if two_part_suffix { 3 } else { 2 }..].join(".")
}

fn alnum(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

/// `loose` also treats one name containing the other (`obs-studio-pro`) as a copy.
fn similar_names_with(a: &str, b: &str, loose: bool) -> bool {
    let (a, b) = (alnum(a), alnum(b));
    if a.len() < 4 || b.len() < 4 {
        return a == b;
    }
    a == b
        || levenshtein(&a, &b) <= 2
        || (loose && a.len() >= 5 && b.len() >= 5 && (a.contains(&b) || b.contains(&a)))
}

/// True when `candidate` imitates `official` without being it.
fn imitates(candidate: &str, official: &str) -> bool {
    imitates_with(candidate, official, true)
}

fn imitates_with(candidate: &str, official: &str, loose: bool) -> bool {
    if candidate == official {
        return false;
    }
    let gh = |s: &str| {
        s.strip_prefix("github.com/")
            .and_then(|rest| rest.split_once('/'))
            .map(|(o, r)| (o.to_string(), r.to_string()))
    };
    match (gh(candidate), gh(official)) {
        // Same (or near-same) repository name under a different owner: the classic fake repo.
        (Some((c_owner, c_repo)), Some((o_owner, o_repo))) => {
            c_owner != o_owner && similar_names_with(&c_repo, &o_repo, loose)
        }
        (None, None) => {
            let label = |d: &str| d.split('.').next().unwrap_or(d).to_string();
            similar_names_with(&label(candidate), &label(official), loose)
        }
        _ => false,
    }
}

/// Turns search results into a verdict. Pure, so it is tested without the network.
pub fn judge_sources(
    software_name: &str,
    candidate: &str,
    results: &[TavilySearchResultItem],
) -> TavilyVerificationResult {
    let mut official: Vec<String> = Vec::new();
    for item in results {
        if item.score.is_some_and(|s| s < MIN_RESULT_SCORE) {
            continue;
        }
        let Some(identity) = site_identity(&item.url) else { continue };
        let bare_host = identity.split('/').next().unwrap_or(&identity);
        if identity == "github.com" || NON_OFFICIAL_DOMAINS.contains(&bare_host) {
            continue;
        }
        if !official.contains(&identity) {
            official.push(identity);
        }
    }

    // Ranking alone can be gamed (the fake DS4Windows site ranks first), so a plain website is
    // never verified by search. A GitHub repo is, unless a lookalike repo outranks it.
    let on_github = candidate.starts_with("github.com/");
    let position = official.iter().position(|o| o == candidate);
    let outranked_by = position.and_then(|p| {
        official[..p].iter().find(|o| o.starts_with("github.com/") && imitates_with(candidate, o, false)).cloned()
    });
    let is_verified = on_github && position.is_some() && outranked_by.is_none();
    let imitated = if position.is_some() && outranked_by.is_none() {
        None
    } else {
        outranked_by.clone().or_else(|| official.iter().find(|o| imitates(candidate, o)).cloned())
    };
    let is_spoofed = imitated.is_some() && position.is_none();
    let top = official.iter().take(3).cloned().collect::<Vec<_>>().join(", ");

    let consensus_summary = if is_verified {
        format!("Web search lists {candidate} as an official source for {software_name}.")
    } else if position.is_some() && !on_github {
        format!("Web search lists {candidate} for {software_name}, but search ranking alone can't prove a website is official.")
    } else if let Some(other) = &outranked_by {
        format!("Web search ranks {other} above {candidate} for {software_name}; one of them is a copy.")
    } else if official.is_empty() {
        format!("Web search found no clear official source for {software_name}.")
    } else {
        format!("Web search lists {top} for {software_name}, not {candidate}.")
    };
    let warning = imitated.as_ref().map(|o| {
        format!("`{candidate}` imitates the official source `{o}` for `{software_name}`")
    });
    let mut official_domains = official;
    if let Some(o) = &imitated {
        official_domains.retain(|d| d != o);
        official_domains.insert(0, o.clone());
    }

    TavilyVerificationResult {
        is_verified,
        is_spoofed,
        confidence_score: if is_verified || is_spoofed { 0.9 } else { 0.5 },
        official_domains,
        consensus_summary,
        warning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits(urls: &[&str]) -> Vec<TavilySearchResultItem> {
        urls.iter()
            .map(|u| TavilySearchResultItem { url: u.to_string(), score: Some(0.8) })
            .collect()
    }

    #[test]
    fn debug_output_redacts_api_key() {
        let verifier = HttpTavilyVerifier::new("test-tavily-secret");
        let debug = format!("{verifier:?}");
        assert!(!debug.contains("test-tavily-secret"));
        assert!(debug.contains("has_api_key"));
    }

    #[tokio::test]
    async fn missing_key_is_a_config_error_and_is_read_per_call() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let verifier = HttpTavilyVerifier::with_key_provider(Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            None
        }));
        for _ in 0..2 {
            let err = verifier.verify_source("x", "https://example.com").await.unwrap_err();
            assert!(matches!(err, CoreError::Config(_)));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn identities_use_repo_paths_on_github_and_registrable_domains_elsewhere() {
        assert_eq!(
            site_identity("https://github.com/OBSProject/obs-studio/releases").as_deref(),
            Some("github.com/obsproject/obs-studio")
        );
        assert_eq!(site_identity("https://get.videolan.org/vlc/x.exe").as_deref(), Some("videolan.org"));
        assert_eq!(site_identity("https://www.example.co.uk/a").as_deref(), Some("example.co.uk"));
    }

    #[test]
    fn the_official_repo_is_verified() {
        let v = judge_sources(
            "obs-studio",
            "github.com/obsproject/obs-studio",
            &hits(&["https://obsproject.com/", "https://github.com/obsproject/obs-studio"]),
        );
        assert!(v.is_verified);
        assert!(!v.is_spoofed);
    }

    #[test]
    fn same_repo_name_under_another_owner_is_a_fake() {
        let v = judge_sources(
            "obs-studio",
            "github.com/obs-free-tools/obs-studio",
            &hits(&["https://obsproject.com/", "https://github.com/obsproject/obs-studio"]),
        );
        assert!(v.is_spoofed);
        assert_eq!(v.official_domains.first().map(String::as_str), Some("github.com/obsproject/obs-studio"));
    }

    #[test]
    fn lookalike_domains_are_spoofs_but_unrelated_sources_are_only_unconfirmed() {
        let fake = judge_sources("DS4Windows", "ds4-windows.com", &hits(&["https://ds4windows.dev/"]));
        assert!(fake.is_spoofed);

        let unknown = judge_sources(
            "tinytool",
            "github.com/someone/tinytool",
            &hits(&["https://unrelated.dev/blog"]),
        );
        assert!(!unknown.is_verified);
        assert!(!unknown.is_spoofed);
    }

    #[test]
    fn a_top_ranked_website_is_not_verified_by_ranking_alone() {
        let v = judge_sources("DS4Windows", "ds4-windows.com", &hits(&["https://ds4-windows.com/"]));
        assert!(!v.is_verified);
        assert!(!v.is_spoofed);
    }

    #[test]
    fn a_lookalike_repo_ranked_higher_makes_the_candidate_unverified() {
        let v = judge_sources(
            "obs-studio",
            "github.com/obsproject/obs-studio",
            &hits(&["https://github.com/obs-free-tools/obs-studio", "https://github.com/obsproject/obs-studio"]),
        );
        assert!(!v.is_verified);
        assert!(!v.is_spoofed);
        assert!(v.consensus_summary.contains("one of them is a copy"));
    }

    #[test]
    fn portals_and_low_scores_never_count_as_official() {
        let mut results = hits(&["https://www.softonic.com/x", "https://en.wikipedia.org/wiki/X"]);
        results.push(TavilySearchResultItem { url: "https://github.com/a/x".into(), score: Some(0.1) });
        let v = judge_sources("x", "github.com/a/x", &results);
        assert!(v.official_domains.is_empty());
        assert!(!v.is_verified);
    }
}
