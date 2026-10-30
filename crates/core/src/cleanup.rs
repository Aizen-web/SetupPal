//! PATH diffs and leftover-folder cleanup used after winget uninstall (R6).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Entries present in `after` but not in `before` (case-insensitive on Windows).
pub fn path_entries_added(before_raw: &str, after_raw: &str) -> Vec<String> {
    let before: HashSet<String> = split_path(before_raw)
        .into_iter()
        .map(|e| normalize_key(&e))
        .collect();
    split_path(after_raw)
        .into_iter()
        .filter(|e| !before.contains(&normalize_key(e)))
        .collect()
}

/// Removes `to_remove` entries from a raw PATH string. Keeps original separators/order otherwise.
pub fn remove_path_entries(raw: &str, to_remove: &[String]) -> String {
    let drop: HashSet<String> = to_remove.iter().map(|e| normalize_key(e.as_str())).collect();
    raw.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| !drop.contains(&normalize_key(s)))
        .collect::<Vec<_>>()
        .join(";")
}

fn split_path(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_key(s: &str) -> String {
    let t = s.trim().trim_end_matches(['\\', '/']);
    if cfg!(windows) {
        t.to_ascii_lowercase()
    } else {
        t.to_string()
    }
}

/// True when the path is missing, empty, or only contains empty dirs / installer leftovers.
pub fn is_safe_to_delete_install_dir(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    if path.is_file() {
        return is_installer_leftover_name(path);
    }
    dir_only_leftovers(path).unwrap_or(false)
}

fn is_installer_leftover_name(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    name.starts_with("unins")
        || name.ends_with(".log")
        || name.ends_with(".tmp")
        || name == "install.log"
        || name == "uninstall.exe"
}

fn dir_only_leftovers(path: &Path) -> std::io::Result<bool> {
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if !is_installer_leftover_name(&p) {
                // Any real user/content file → leave the tree alone.
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Deletes `path` when safe; returns a short status for the undo message.
pub fn try_remove_install_dir(path: &Path) -> String {
    if !path.exists() {
        return format!("{} already gone", path.display());
    }
    if !is_safe_to_delete_install_dir(path) {
        return format!(
            "left {} (still has user files — remove manually if unwanted)",
            path.display()
        );
    }
    match remove_tree(path) {
        Ok(()) => format!("removed leftover folder {}", path.display()),
        Err(e) => format!("could not remove {}: {e}", path.display()),
    }
}

fn remove_tree(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

/// Looks under `%LOCALAPPDATA%\Microsoft\WinGet\Packages` for a folder whose name starts with `package_id`.
pub fn find_winget_package_folder(package_id: &str) -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("Microsoft")
        .join("WinGet")
        .join("Packages");
    if !root.is_dir() {
        return None;
    }
    let needle = package_id.to_ascii_lowercase();
    let mut matches: Vec<PathBuf> = std::fs::read_dir(&root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_ascii_lowercase().starts_with(&needle))
                .unwrap_or(false)
        })
        .collect();
    matches.sort();
    matches.into_iter().next()
}

/// Prefer the parent of the first added PATH entry; else a WinGet Packages folder.
pub fn infer_install_location(package_id: &str, path_entries_added: &[String]) -> Option<PathBuf> {
    if let Some(entry) = path_entries_added.first() {
        let p = PathBuf::from(entry);
        if p.is_dir() {
            return Some(p.clone());
        }
        if let Some(parent) = p.parent() {
            if parent.as_os_str().len() > 3 {
                return Some(parent.to_path_buf());
            }
        }
    }
    find_winget_package_folder(package_id)
}

/// Whether a PATH entry should be stripped after uninstall: it was added by us and now
/// points at a missing or empty/leftover folder (never strip a live directory with files).
pub fn should_strip_path_entry(entry: &str) -> bool {
    let p = PathBuf::from(entry);
    if !p.exists() {
        return true;
    }
    is_safe_to_delete_install_dir(&p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_diff_finds_new_entries() {
        let before = r"C:\Windows;C:\Windows\System32";
        let after = r"C:\Windows;C:\Windows\System32;C:\Users\x\AppData\Local\Programs\Python\Python312;C:\Users\x\AppData\Local\Programs\Python\Python312\Scripts";
        let added = path_entries_added(before, after);
        assert_eq!(added.len(), 2);
        assert!(added[0].contains("Python312"));
    }

    #[test]
    fn remove_entries_keeps_others() {
        let raw = r"C:\A;C:\B;C:\C";
        let next = remove_path_entries(raw, &["C:\\B".into()]);
        assert_eq!(next, r"C:\A;C:\C");
    }

    #[test]
    fn empty_dir_is_safe_to_delete() {
        let dir = tempfile::tempdir().unwrap();
        let leftover = dir.path().join("Doc");
        std::fs::create_dir_all(&leftover).unwrap();
        assert!(is_safe_to_delete_install_dir(&leftover));
        let with_file = dir.path().join("real");
        std::fs::create_dir_all(&with_file).unwrap();
        std::fs::write(with_file.join("notes.txt"), b"keep me").unwrap();
        assert!(!is_safe_to_delete_install_dir(&with_file));
    }

    #[test]
    fn strip_when_folder_gone() {
        assert!(should_strip_path_entry(r"C:\does\not\exist\python"));
    }
}
