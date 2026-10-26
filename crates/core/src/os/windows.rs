use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::config::CoreError;

pub const HKCU_ENV_KEY: &str = r"HKCU\\Environment";
pub const HKLM_ENV_KEY: &str =
    r"HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";

/// Queries `reg.exe query` for both `HKLM` (System) and `HKCU` (User) `Path` values,
/// merges them with the current process `PATH`, expands `%USERPROFILE%` / `%SystemRoot%`
/// variables, and updates the active process `PATH` environment variable in place.
pub async fn read_windows_registry_path() -> Result<Vec<PathBuf>, CoreError> {
    let mut combined = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(sys_raw) = query_reg_path_value(HKLM_ENV_KEY).await {
        for p in parse_windows_path_string(&sys_raw) {
            let key = p.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                combined.push(p);
            }
        }
    }

    if let Ok(user_raw) = query_reg_path_value(HKCU_ENV_KEY).await {
        for p in parse_windows_path_string(&user_raw) {
            let key = p.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                combined.push(p);
            }
        }
    }

    if let Some(proc_path) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&proc_path) {
            if !p.as_os_str().is_empty() {
                let key = p.to_string_lossy().to_ascii_lowercase();
                if seen.insert(key) {
                    combined.push(p);
                }
            }
        }
    }

    if let Ok(joined) = std::env::join_paths(&combined) {
        std::env::set_var("PATH", joined);
    }

    Ok(combined)
}

async fn query_reg_path_value(reg_key: &str) -> Result<String, CoreError> {
    let out = Command::new("reg")
        .args(["query", reg_key, "/v", "Path"])
        .output()
        .await?;

    if !out.status.success() {
        return Err(CoreError::Os(format!(
            "reg query failed for {reg_key}: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    extract_reg_query_value(&stdout).ok_or_else(|| {
        CoreError::Os(format!("could not parse Path value from reg output for {reg_key}"))
    })
}

/// Parses the `reg.exe query <KEY> /v Path` output format:
/// `    Path    REG_EXPAND_SZ    C:\\Windows\\system32;C:\\Windows;...`
pub fn extract_reg_query_value(reg_stdout: &str) -> Option<String> {
    for line in reg_stdout.lines() {
        let trimmed = line.trim();
        if trimmed.to_ascii_lowercase().starts_with("path ") {
            if let Some(idx) = trimmed.find("REG_EXPAND_SZ") {
                let after = trimmed[idx + "REG_EXPAND_SZ".len()..].trim();
                return Some(after.to_string());
            }
            if let Some(idx) = trimmed.find("REG_SZ") {
                let after = trimmed[idx + "REG_SZ".len()..].trim();
                return Some(after.to_string());
            }
        }
    }
    None
}

/// Splits a semicolon-delimited Windows PATH string and expands common `%VAR%` placeholders.
pub fn parse_windows_path_string(raw: &str) -> Vec<PathBuf> {
    raw.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(expand_windows_env_vars)
        .map(PathBuf::from)
        .collect()
}

fn expand_windows_env_vars(input: &str) -> String {
    let mut result = input.to_string();
    for var_name in ["SystemRoot", "windir", "USERPROFILE", "LOCALAPPDATA", "APPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        let token = format!("%{var_name}%");
        if result.to_ascii_lowercase().contains(&token.to_ascii_lowercase()) {
            if let Ok(val) = std::env::var(var_name) {
                result = replace_case_insensitive(&result, &token, &val);
            }
        }
    }
    result
}

fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> String {
    let lower_h = haystack.to_ascii_lowercase();
    let lower_n = needle.to_ascii_lowercase();
    if let Some(pos) = lower_h.find(&lower_n) {
        let mut out = String::with_capacity(haystack.len() + replacement.len());
        out.push_str(&haystack[..pos]);
        out.push_str(replacement);
        out.push_str(&haystack[pos + needle.len()..]);
        out
    } else {
        haystack.to_string()
    }
}

pub async fn query_windows_version() -> String {
    if let Ok(out) = Command::new("cmd").args(["/C", "ver"]).output().await {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }
    "Windows".to_string()
}

pub async fn detect_windows_gpu() -> Option<String> {
    if let Ok(out) = Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .await
    {
        if out.status.success() {
            let gpu = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_string();
            if !gpu.is_empty() {
                return Some(gpu);
            }
        }
    }
    None
}

pub async fn query_windows_free_disk(_target_dir: &Path) -> u64 {
    if let Ok(out) = Command::new("wmic")
        .args(["logicaldisk", "get", "FreeSpace"])
        .output()
        .await
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if let Ok(bytes) = line.trim().parse::<u64>() {
                    return bytes;
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reg_query_output_correctly() {
        let sample = r#"
HKEY_CURRENT_USER\Environment
    Path    REG_EXPAND_SZ    C:\Users\dev\.cargo\bin;C:\Program Files\LLVM\bin
"#;
        let extracted = extract_reg_query_value(sample);
        assert_eq!(
            extracted.as_deref(),
            Some(r"C:\Users\dev\.cargo\bin;C:\Program Files\LLVM\bin")
        );

        let paths = parse_windows_path_string(extracted.as_deref().unwrap_or_default());
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[1], PathBuf::from(r"C:\Program Files\LLVM\bin"));
    }
}
