use std::path::Path;
use url::Url;

use crate::types::Step;

/// Hardcoded allowlist of official package registries and vendor download domains.
/// Any URL outside this allowlist triggers an immediate `RiskRating::Red` hard block.
pub const ALLOWED_DOWNLOAD_DOMAINS: &[&str] = &[
    "pypi.org",
    "files.pythonhosted.org",
    "python.org",
    "registry.npmjs.org",
    "api.modrinth.com",
    "cdn.modrinth.com",
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "nodejs.org",
    "adoptium.net",
    "api.adoptium.net",
    "fabricmc.net",
    "maven.fabricmc.net",
    "meta.fabricmc.net",
    "crates.io",
    "static.crates.io",
    "cmake.org",
    "llvm.org",
    "git-scm.com",
    "apache.org",
    "learn.microsoft.com",
    "download.microsoft.com",
    "aka.ms",
    "sh.rustup.rs",
    "static.rust-lang.org",
];

/// Shell metacharacters forbidden inside `RunVerifyCommand` arguments or package identifiers.
pub const FORBIDDEN_SHELL_METACHARACTERS: &[char] = &[';', '|', '&', '>', '<', '$', '`', '\n', '\r'];

/// High-risk command/path patterns that are unconditionally blocked (`RiskRating::Red`).
pub const FORBIDDEN_DESTRUCTIVE_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf", "Recursive forced deletion (`rm -rf`) is prohibited"),
    ("remove-item -recurse", "PowerShell recursive deletion (`Remove-Item -Recurse`) is prohibited"),
    ("del /s", "Windows recursive file deletion (`del /s`) is prohibited"),
    ("rmdir /s", "Windows recursive directory deletion (`rmdir /s`) is prohibited"),
    ("format ", "Disk formatting command is prohibited"),
    ("curl|", "Piping remote content to a shell is prohibited"),
    ("curl ", "Raw curl execution inside step arguments is prohibited"),
    ("wget ", "Raw wget execution inside step arguments is prohibited"),
    ("iwr ", "PowerShell Invoke-WebRequest (`iwr`) pipe execution is prohibited"),
    ("invoke-webrequest", "PowerShell Invoke-WebRequest is prohibited"),
    ("invoke-expression", "PowerShell Invoke-Expression (`iex`) is prohibited"),
    ("iex ", "PowerShell `iex` execution is prohibited"),
    ("powershell -enc", "Encoded PowerShell payload execution is prohibited"),
    ("-encodedcommand", "Encoded PowerShell command is prohibited"),
    ("set-mppreference", "Tampering with Windows Defender (`Set-MpPreference`) is prohibited"),
    ("disablerealtimemonitoring", "Disabling Windows Defender real-time monitoring is prohibited"),
    ("netsh advfirewall", "Modifying Windows Firewall rules (`netsh advfirewall`) is prohibited"),
    (".ssh", "Accessing SSH keys or `.ssh` directory is prohibited"),
    ("id_rsa", "Accessing private SSH key `id_rsa` is prohibited"),
    ("login data", "Accessing browser credential store (`Login Data`) is prohibited"),
    ("AppData\\Local\\Google\\Chrome\\User Data", "Accessing Chrome user profile data is prohibited"),
    ("AppData\\Roaming\\Mozilla\\Firefox\\Profiles", "Accessing Firefox profile data is prohibited"),
    (".aws/credentials", "Accessing AWS credentials store is prohibited"),
];

/// Checks whether a URL uses `https` and belongs to the hardcoded `ALLOWED_DOWNLOAD_DOMAINS` list.
pub fn is_url_domain_allowlisted(raw_url: &str) -> Result<bool, String> {
    let parsed = Url::parse(raw_url).map_err(|e| format!("Invalid URL `{raw_url}`: {e}"))?;
    if parsed.scheme() != "https" {
        return Err(format!(
            "Insecure URL scheme `{}` in `{raw_url}` (HTTPS is mandatory)",
            parsed.scheme()
        ));
    }
    let Some(host) = parsed.host_str() else {
        return Err(format!("URL `{raw_url}` is missing a valid hostname"));
    };
    let host_lower = host.to_ascii_lowercase();
    let allowed = ALLOWED_DOWNLOAD_DOMAINS.iter().any(|&domain| {
        host_lower == domain || host_lower.ends_with(&format!(".{domain}"))
    });
    Ok(allowed)
}

/// Validates that a string does not contain any shell metacharacters (`; | & > < $ \` `).
pub fn contains_shell_metacharacters(input: &str) -> Option<char> {
    input
        .chars()
        .find(|c| FORBIDDEN_SHELL_METACHARACTERS.contains(c))
}

/// Checks any string or path against destructive, exfiltration, or security-tampering signatures.
pub fn scan_for_destructive_patterns(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let normalized_nospace = lower.replace(' ', "");
    let mut violations = Vec::new();

    for &(pattern, reason) in FORBIDDEN_DESTRUCTIVE_PATTERNS {
        let pat_lower = pattern.to_ascii_lowercase();
        if lower.contains(&pat_lower) || normalized_nospace.contains(&pat_lower.replace(' ', "")) {
            violations.push(reason.to_string());
        }
    }

    if (lower.contains("curl ") || lower.contains("wget ") || lower.contains("iwr "))
        && (lower.contains("| sh")
            || lower.contains("| bash")
            || lower.contains("| pwsh")
            || lower.contains("| powershell")
            || lower.contains("| iex"))
    {
        violations.push("Piping remote content directly into a shell is prohibited".to_string());
    }

    violations
}

/// Inspects a pre/post-install script snippet (`<untrusted_data>`) for suspicious behavioral signals
/// (network calls, base64 decoding, environment variable exfiltration, or shell spawning).
pub fn inspect_install_script(script_text: &str) -> Vec<String> {
    let lower = script_text.to_ascii_lowercase();
    let mut flags = Vec::new();

    for violation in scan_for_destructive_patterns(script_text) {
        flags.push(violation);
    }

    if lower.contains("http://") || lower.contains("https://") || lower.contains("urllib") || lower.contains("socket") {
        flags.push("Install script initiates outbound network communication".to_string());
    }
    if lower.contains("base64") || lower.contains("b64decode") || lower.contains("frombase64string") {
        flags.push("Install script decodes obfuscated base64 payload".to_string());
    }
    if lower.contains("os.environ") || lower.contains("process.env") || lower.contains("$env:") {
        flags.push("Install script reads process environment variables (potential secret exfiltration)".to_string());
    }
    if lower.contains("subprocess") || lower.contains("child_process") || lower.contains("os.system") || lower.contains("eval(") {
        flags.push("Install script spawns subprocesses or dynamic eval".to_string());
    }

    flags
}

/// Evaluates hard deterministic policy rules for a single `Step`.
/// Returns a list of policy violation descriptions. Any violation forces `RiskRating::Red`.
pub fn evaluate_step_policy(step: &Step, description: &str) -> Vec<String> {
    let mut violations = scan_for_destructive_patterns(description);

    match step {
        Step::InstallPackage {
            package_id,
            version,
            ..
        } => {
            if let Some(ch) = contains_shell_metacharacters(package_id) {
                violations.push(format!(
                    "Package ID `{package_id}` contains forbidden shell metacharacter `{ch}`"
                ));
            }
            if package_id.starts_with("http://") || package_id.starts_with("https://") {
                match is_url_domain_allowlisted(package_id) {
                    Ok(true) => {}
                    Ok(false) => violations.push(format!(
                        "Unapproved download domain in package identifier `{package_id}`"
                    )),
                    Err(e) => violations.push(e),
                }
            }
            if let Some(ver) = version {
                if let Some(ch) = contains_shell_metacharacters(ver) {
                    violations.push(format!(
                        "Version `{ver}` contains forbidden shell metacharacter `{ch}`"
                    ));
                }
            }
            violations.extend(scan_for_destructive_patterns(package_id));
        }
        Step::DownloadArtifact {
            id,
            version,
            destination,
            expected_sha256,
            ..
        } => {
            if id.starts_with("http://") || id.starts_with("https://") {
                match is_url_domain_allowlisted(id) {
                    Ok(true) => {}
                    Ok(false) => violations.push(format!(
                        "Artifact URL `{id}` uses an unapproved domain"
                    )),
                    Err(e) => violations.push(e),
                }
            }
            if let Some(ch) = contains_shell_metacharacters(id) {
                violations.push(format!(
                    "Artifact ID `{id}` contains forbidden shell metacharacter `{ch}`"
                ));
            }
            if let Some(ch) = contains_shell_metacharacters(version) {
                violations.push(format!(
                    "Artifact version `{version}` contains forbidden shell metacharacter `{ch}`"
                ));
            }
            validate_safe_path(destination, &mut violations);
            if let Some(hash) = expected_sha256 {
                if !is_valid_sha256_hex(hash) {
                    violations.push(format!(
                        "DownloadArtifact pins an invalid or placeholder SHA-256 hash (`{hash}`)"
                    ));
                }
            }
        }
        Step::RunVendorInstaller {
            mc_version,
            installer_path,
            ..
        } => {
            validate_safe_path(installer_path, &mut violations);
            let version_ok = !mc_version.is_empty()
                && mc_version.len() <= 32
                && mc_version
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
            if !version_ok {
                violations.push(format!(
                    "Vendor installer game version `{mc_version}` is not a plain version string"
                ));
            }
        }
        Step::AddDiscoveredToolToPath { .. } => {}
        Step::VerifyHash {
            file_path,
            expected_sha256,
        } => {
            validate_safe_path(file_path, &mut violations);
            if !is_valid_sha256_hex(expected_sha256) {
                violations.push(format!(
                    "VerifyHash requires a valid 64-character hex SHA-256 hash (got `{expected_sha256}`)"
                ));
            }
        }
        Step::AddToUserPath { directory } => {
            validate_safe_path(directory, &mut violations);
        }
        Step::CreateIsolatedEnv { path, .. } => {
            validate_safe_path(path, &mut violations);
        }
        Step::RunVerifyCommand { args, .. } => {
            for arg in args {
                if let Some(ch) = contains_shell_metacharacters(arg) {
                    violations.push(format!(
                        "Verify command argument `{arg}` contains forbidden shell metacharacter `{ch}`"
                    ));
                }
                violations.extend(scan_for_destructive_patterns(arg));
            }
        }
    }

    violations
}

fn validate_safe_path(path: &Path, violations: &mut Vec<String>) {
    let raw = path.to_string_lossy();
    if let Some(ch) = contains_shell_metacharacters(&raw) {
        violations.push(format!(
            "Path `{raw}` contains forbidden shell metacharacter `{ch}`"
        ));
    }
    violations.extend(scan_for_destructive_patterns(&raw));
}

/// Inputs whose digests show up as copy-paste placeholders (the empty file first).
const PLACEHOLDER_PREIMAGES: &[&[u8]] = &[b"", b"foo", b"bar", b"test", b"hello", b"hello world"];

/// True when `hash` is the digest of an empty file or of a well-known test string, or is a
/// single repeated character. Such values are placeholders, never real artifact digests.
pub fn is_placeholder_sha256(hash: &str) -> bool {
    use sha2::{Digest, Sha256};
    let norm = hash.trim().to_ascii_lowercase();
    if let Some(first) = norm.chars().next() {
        if norm.chars().all(|c| c == first) {
            return true;
        }
    }
    PLACEHOLDER_PREIMAGES
        .iter()
        .any(|pre| hex::encode(Sha256::digest(pre)) == norm)
}

/// A real-looking SHA-256: 64 hex characters and not a known placeholder digest.
pub fn is_valid_sha256_hex(hash: &str) -> bool {
    let trimmed = hash.trim();
    trimmed.len() == 64
        && trimmed.chars().all(|c| c.is_ascii_hexdigit())
        && !is_placeholder_sha256(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn placeholder_digests_are_rejected() {
        assert!(!is_valid_sha256_hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
        assert!(!is_valid_sha256_hex("2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"));
        assert!(!is_valid_sha256_hex(&"0".repeat(64)));
        assert!(!is_valid_sha256_hex("abc"));
        assert!(is_valid_sha256_hex("61e035bf7bf70153e127440ce34de47c9036f0a2d0c65d1529454bd35ceefe4f"));
    }

    #[test]
    fn shell_metacharacters_are_detected() {
        assert_eq!(contains_shell_metacharacters("numpy; rm"), Some(';'));
        assert_eq!(contains_shell_metacharacters("a|b"), Some('|'));
        assert_eq!(contains_shell_metacharacters("typescript@5.6.2"), None);
    }

    #[test]
    fn allowlist_requires_https_and_known_hosts() {
        assert_eq!(is_url_domain_allowlisted("https://maven.fabricmc.net/x.jar"), Ok(true));
        assert_eq!(is_url_domain_allowlisted("https://meta.fabricmc.net/v2"), Ok(true));
        assert_eq!(is_url_domain_allowlisted("https://static.crates.io/crates/x"), Ok(true));
        assert_eq!(is_url_domain_allowlisted("https://evil-github.com/x"), Ok(false));
        assert!(is_url_domain_allowlisted("http://github.com/x").is_err());
    }

    #[test]
    fn download_without_hash_is_not_a_policy_violation_but_bad_hash_is() {
        let mut step = Step::DownloadArtifact {
            adapter: crate::types::AdapterKind::Modrinth,
            id: "sodium".to_string(),
            version: "1".to_string(),
            destination: PathBuf::from("./mods/sodium.jar"),
            expected_sha256: None,
        };
        assert!(evaluate_step_policy(&step, "Download sodium").is_empty());
        if let Step::DownloadArtifact { expected_sha256, .. } = &mut step {
            *expected_sha256 = Some("0".repeat(64));
        }
        assert!(!evaluate_step_policy(&step, "Download sodium").is_empty());
    }
}
