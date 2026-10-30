use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::config::CoreError;

pub const HKCU_ENV_KEY: &str = r"HKCU\Environment";
pub const HKLM_ENV_KEY: &str =
    r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

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

/// Reads a `REG_SZ` / `REG_EXPAND_SZ` value via `reg.exe query` (`value: None` reads `(Default)`).
pub async fn query_registry_string(reg_key: &str, value: Option<&str>) -> Option<String> {
    let mut cmd = Command::new("reg");
    cmd.arg("query").arg(reg_key);
    match value {
        Some(name) => cmd.args(["/v", name]),
        None => cmd.arg("/ve"),
    };
    let out = cmd.output().await.ok()?;
    if !out.status.success() {
        return None;
    }
    extract_reg_value(&String::from_utf8_lossy(&out.stdout), value.unwrap_or("(Default)"))
}

/// Extracts the data of `value_name` from `reg.exe query` output lines of the form
/// `    <name>    REG_SZ    <data>`.
pub fn extract_reg_value(reg_stdout: &str, value_name: &str) -> Option<String> {
    let name_lower = value_name.to_ascii_lowercase();
    for line in reg_stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with(&name_lower) {
            continue;
        }
        for kind in ["REG_EXPAND_SZ", "REG_SZ"] {
            if let Some(idx) = trimmed.find(kind) {
                let data = trimmed[idx + kind.len()..].trim();
                if !data.is_empty() {
                    return Some(expand_windows_env_vars(data));
                }
            }
        }
    }
    None
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

/// Adds a directory to the User PATH in `HKCU\Environment`, ensures no duplicate entries,
/// updates the Windows registry, and refreshes the current process PATH.
/// Returns the previous raw User PATH string for transaction rollback purposes.
pub async fn add_to_windows_user_path(directory: &Path) -> Result<String, CoreError> {
    let dir_str = directory.to_string_lossy().to_string();
    let current_raw = query_reg_path_value(HKCU_ENV_KEY).await.unwrap_or_default();

    let mut entries = parse_windows_path_string(&current_raw);
    let already_present = entries.iter().any(|p| {
        p.to_string_lossy().eq_ignore_ascii_case(&dir_str)
    });

    if !already_present {
        entries.push(directory.to_path_buf());
        let new_path_val = entries
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(";");

        let out = Command::new("reg")
            .args(["add", HKCU_ENV_KEY, "/v", "Path", "/t", "REG_EXPAND_SZ", "/d", &new_path_val, "/f"])
            .output()
            .await?;

        if !out.status.success() {
            return Err(CoreError::Os(format!(
                "Failed to update User PATH in registry: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
    }

    read_windows_registry_path().await?;
    Ok(current_raw)
}

/// Restores the User PATH in `HKCU\Environment` to a previous value (used by rollback).
pub async fn restore_windows_user_path(previous_raw: &str) -> Result<(), CoreError> {
    let cleaned = previous_raw
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(";");

    let out = Command::new("reg")
        .args(["add", HKCU_ENV_KEY, "/v", "Path", "/t", "REG_EXPAND_SZ", "/d", &cleaned, "/f"])
        .output()
        .await?;

    if !out.status.success() {
        return Err(CoreError::Os(format!(
            "Failed to restore User PATH in registry: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    read_windows_registry_path().await?;
    Ok(())
}
