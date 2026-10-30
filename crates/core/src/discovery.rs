//! Locates installed tools that installers did not put on PATH.

use std::path::{Path, PathBuf};

use crate::os::OsProvider;
use crate::types::AllowedBinary;

/// Registry keys whose default (or named) value points at a tool's install root.
fn registry_roots(binary: AllowedBinary) -> &'static [(&'static str, Option<&'static str>, &'static str)] {
    match binary {
        AllowedBinary::Clang => &[
            (r"HKLM\SOFTWARE\LLVM\LLVM", None, "bin"),
            (r"HKCU\SOFTWARE\LLVM\LLVM", None, "bin"),
            (r"HKLM\SOFTWARE\WOW6432Node\LLVM\LLVM", None, "bin"),
        ],
        AllowedBinary::Cmake => &[(r"HKLM\SOFTWARE\Kitware\CMake", Some("InstallDir"), "bin")],
        AllowedBinary::Git => &[(r"HKLM\SOFTWARE\GitForWindows", Some("InstallPath"), "cmd")],
        _ => &[],
    }
}

/// Well-known install folders, relative to an environment variable.
fn default_dirs(binary: AllowedBinary) -> &'static [(&'static str, &'static str)] {
    match binary {
        AllowedBinary::Clang => &[("ProgramFiles", r"LLVM\bin"), ("LOCALAPPDATA", r"Programs\LLVM\bin")],
        AllowedBinary::Cmake => &[("ProgramFiles", r"CMake\bin"), ("LOCALAPPDATA", r"Programs\CMake\bin")],
        AllowedBinary::Git => &[("ProgramFiles", r"Git\cmd"), ("LOCALAPPDATA", r"Programs\Git\cmd")],
        AllowedBinary::Node | AllowedBinary::Npm => &[("ProgramFiles", "nodejs"), ("LOCALAPPDATA", r"Programs\nodejs")],
        AllowedBinary::Cargo | AllowedBinary::Rustc => &[("USERPROFILE", r".cargo\bin")],
        _ => &[],
    }
}

fn exe_in(dir: &Path, binary: AllowedBinary) -> bool {
    let name = binary.executable_name();
    ["exe", "cmd", "bat"]
        .iter()
        .any(|ext| dir.join(format!("{name}.{ext}")).is_file())
}

/// Searches winget's per-user package folders (`%LOCALAPPDATA%\Microsoft\WinGet\Packages`)
/// up to two levels deep for the executable.
fn search_winget_packages(binary: AllowedBinary) -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("Microsoft")
        .join("WinGet")
        .join("Packages");
    let mut queue: Vec<(PathBuf, u8)> = vec![(root, 0)];
    while let Some((dir, depth)) = queue.pop() {
        if exe_in(&dir, binary) {
            return Some(dir);
        }
        if depth >= 3 {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut children: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
            children.sort();
            queue.extend(children.into_iter().rev().map(|c| (c, depth + 1)));
        }
    }
    None
}

/// Finds the folder that holds `binary`'s executable, or `None` when it is not installed.
pub async fn discover_tool_dir(os: &dyn OsProvider, binary: AllowedBinary) -> Option<PathBuf> {
    for (key, value, sub) in registry_roots(binary) {
        if let Some(root) = os.read_registry_string(key, *value).await {
            let dir = PathBuf::from(root.trim()).join(sub);
            if exe_in(&dir, binary) {
                return Some(dir);
            }
        }
    }
    for (var, sub) in default_dirs(binary) {
        if let Some(base) = std::env::var_os(var) {
            let dir = PathBuf::from(base).join(sub);
            if exe_in(&dir, binary) {
                return Some(dir);
            }
        }
    }
    search_winget_packages(binary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::mock::MockOsProvider;

    #[tokio::test]
    async fn registry_root_is_used_for_llvm() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("clang++.exe"), b"").unwrap();

        let os = MockOsProvider::new().with_registry(
            r"HKLM\SOFTWARE\LLVM\LLVM",
            None,
            &root.path().display().to_string(),
        );
        assert_eq!(discover_tool_dir(&os, AllowedBinary::Clang).await, Some(bin));
    }

    #[tokio::test]
    async fn registry_root_without_binary_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let os = MockOsProvider::new().with_registry(
            r"HKLM\SOFTWARE\LLVM\LLVM",
            None,
            &root.path().display().to_string(),
        );
        let found = discover_tool_dir(&os, AllowedBinary::Clang).await;
        assert_ne!(found, Some(root.path().join("bin")));
    }
}
