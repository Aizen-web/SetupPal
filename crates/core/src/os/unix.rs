use std::path::{Path, PathBuf};
use tokio::process::Command;

pub fn read_unix_env_path() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|raw| std::env::split_paths(&raw).filter(|p| !p.as_os_str().is_empty()).collect())
        .unwrap_or_default()
}

pub async fn query_unix_version() -> String {
    if let Ok(out) = Command::new("uname").arg("-sr").output().await {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    std::env::consts::OS.to_string()
}

pub async fn detect_unix_gpu() -> Option<String> {
    if let Ok(out) = Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .await
    {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let gpu = first.trim();
                if !gpu.is_empty() {
                    return Some(gpu.to_string());
                }
            }
        }
    }
    None
}

pub async fn query_unix_free_disk(target_dir: &Path) -> u64 {
    let dir_str = target_dir.to_str().unwrap_or(".");
    if let Ok(out) = Command::new("df").args(["-Pk", dir_str]).output().await {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(data_line) = stdout.lines().nth(1) {
                let cols: Vec<&str> = data_line.split_whitespace().collect();
                if cols.len() >= 4 {
                    if let Ok(avail_kb) = cols[3].parse::<u64>() {
                        return avail_kb.saturating_mul(1024);
                    }
                }
            }
        }
    }
    0
}
