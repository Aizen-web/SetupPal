use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::config::CoreError;

pub const HKCU_ENV_KEY: &str = r"HKCU\Environment";
pub const HKLM_ENV_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

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
        CoreError::Os(format!(
            "could not parse Path value from reg output for {reg_key}"
        ))
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
    extract_reg_value(
        &String::from_utf8_lossy(&out.stdout),
        value.unwrap_or("(Default)"),
    )
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
    for var_name in [
        "SystemRoot",
        "windir",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
    ] {
        let token = format!("%{var_name}%");
        if result
            .to_ascii_lowercase()
            .contains(&token.to_ascii_lowercase())
        {
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
            let gpu = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()?
                .trim()
                .to_string();
            if !gpu.is_empty() {
                return Some(gpu);
            }
        }
    }
    None
}

/// Letter of the drive that holds `target_dir` (falling back to the system drive).
fn drive_letter_for(target_dir: &Path) -> char {
    let absolute = target_dir.canonicalize().unwrap_or_else(|_| {
        if target_dir.is_absolute() {
            target_dir.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_else(|_| target_dir.to_path_buf())
        }
    });
    let text = absolute.to_string_lossy().replace(r"\\?\", "");
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic() => letter.to_ascii_uppercase(),
        _ => std::env::var("SystemDrive")
            .ok()
            .and_then(|d| d.chars().next())
            .filter(char::is_ascii_alphabetic)
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or('C'),
    }
}

/// Free bytes on the drive that holds `target_dir`. Uses PowerShell because `wmic` is removed
/// from current Windows 11 builds. Returns 0 only when the query itself fails.
pub async fn query_windows_free_disk(target_dir: &Path) -> u64 {
    let letter = drive_letter_for(target_dir);
    let script = format!("(Get-PSDrive -Name {letter} -PSProvider FileSystem).Free");
    match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .await
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<u64>()
            .unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_letter_comes_from_the_path_and_is_always_a_letter() {
        assert_eq!(drive_letter_for(Path::new(r"d:\projects")), 'D');
        assert!(drive_letter_for(Path::new("relative")).is_ascii_uppercase());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn free_disk_is_reported_for_the_current_drive() {
        let free = query_windows_free_disk(Path::new(".")).await;
        assert!(free > 0, "the PowerShell query returned no free space");
    }
}
/// Adds a directory to the User PATH in `HKCU\Environment`, ensures no duplicate entries,
/// updates the Windows registry, and refreshes the current process PATH.
/// Returns the previous raw User PATH string for transaction rollback purposes.
pub async fn add_to_windows_user_path(directory: &Path) -> Result<String, CoreError> {
    let dir_str = directory.to_string_lossy().to_string();
    let current_raw = query_reg_path_value(HKCU_ENV_KEY).await.unwrap_or_default();

    let mut entries = parse_windows_path_string(&current_raw);
    let already_present = entries
        .iter()
        .any(|p| p.to_string_lossy().eq_ignore_ascii_case(&dir_str));

    if !already_present {
        entries.push(directory.to_path_buf());
        let new_path_val = entries
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(";");

        let out = Command::new("reg")
            .args([
                "add",
                HKCU_ENV_KEY,
                "/v",
                "Path",
                "/t",
                "REG_EXPAND_SZ",
                "/d",
                &new_path_val,
                "/f",
            ])
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

/// Raw User PATH from `HKCU\Environment` (empty string when unset).
pub async fn read_user_path_raw() -> Result<String, CoreError> {
    Ok(query_reg_path_value(HKCU_ENV_KEY).await.unwrap_or_default())
}

/// True when another winget/msiexec installer appears to be running (R5 pre-flight).
pub async fn blocking_installer_message() -> Option<String> {
    for (image, label) in [("winget.exe", "winget"), ("msiexec.exe", "msiexec")] {
        if let Some(msg) = tasklist_running(image, label).await {
            return Some(msg);
        }
    }
    None
}

async fn tasklist_running(image: &str, label: &str) -> Option<String> {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/FO", "CSV", "/NH"])
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    // "INFO: No tasks are running..." or empty
    let line = stdout.lines().find(|l| l.to_ascii_lowercase().contains(&image.to_ascii_lowercase()))?;
    // CSV: "winget.exe","1234","Session","1","12,345 K"
    let started = line
        .split(',')
        .nth(1)
        .map(|s| s.trim_matches('"'))
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown");
    Some(format!(
        "Another installer is running ({label}, PID {started}). Close it or wait, then try again."
    ))
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
        .args([
            "add",
            HKCU_ENV_KEY,
            "/v",
            "Path",
            "/t",
            "REG_EXPAND_SZ",
            "/d",
            &cleaned,
            "/f",
        ])
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
