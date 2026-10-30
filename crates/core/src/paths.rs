//! Path resolution shared by the executor, adapters, and installability checks.

use std::path::{Component, Path, PathBuf};

use crate::config::CoreError;

/// Folder tokens a step path may start with. Each expands to the matching environment variable.
const PATH_TOKENS: &[(&str, &str)] = &[
    ("%APPDATA%", "APPDATA"),
    ("%LOCALAPPDATA%", "LOCALAPPDATA"),
    ("%USERPROFILE%", "USERPROFILE"),
];

/// Resolves a path taken from a plan step into an absolute path.
///
/// - A leading `%APPDATA%`, `%LOCALAPPDATA%`, or `%USERPROFILE%` token expands to that folder.
/// - Other relative paths resolve against `working_dir`.
/// - `..` components are rejected so a step cannot escape its base folder.
pub fn resolve_step_path(path: &Path, working_dir: &Path) -> Result<PathBuf, CoreError> {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(CoreError::Validation(format!(
            "path `{}` must not contain `..` components",
            path.display()
        )));
    }

    let raw = path.to_string_lossy();
    for (token, var) in PATH_TOKENS {
        if raw.len() >= token.len() && raw[..token.len()].eq_ignore_ascii_case(token) {
            let base = std::env::var_os(var).map(PathBuf::from).ok_or_else(|| {
                CoreError::Config(format!("{var} is not set; cannot resolve `{raw}`"))
            })?;
            let rest = raw[token.len()..].trim_start_matches(['/', '\\']);
            return Ok(if rest.is_empty() { base } else { base.join(rest) });
        }
    }

    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let cleaned: PathBuf = path
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect();
        Ok(working_dir.join(cleaned))
    }
}

/// The current user's profile folder (`%USERPROFILE%` on Windows, `$HOME` elsewhere).
pub fn user_profile_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Per-user SetupPal data folder (`%LOCALAPPDATA%\SetupPal` on Windows).
pub fn setuppal_data_dir() -> Result<PathBuf, CoreError> {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(local).join("SetupPal"));
    }
    user_profile_dir()
        .map(|home| home.join(".local").join("share").join("setuppal"))
        .ok_or_else(|| CoreError::Config("cannot locate a per-user data folder".to_string()))
}

/// Default Minecraft Java Edition launcher data folder for this platform.
pub fn minecraft_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        user_profile_dir().map(|h| h.join("Library").join("Application Support").join("minecraft"))
    } else {
        user_profile_dir().map(|h| h.join(".minecraft"))
    }
}

/// Validates a user-chosen project folder for npm installs: it must exist, be a directory,
/// and live inside `user_profile`. Returns the canonical path.
pub fn validate_project_dir(dir: &Path, user_profile: &Path) -> Result<PathBuf, CoreError> {
    if !dir.is_dir() {
        return Err(CoreError::Validation(format!(
            "project folder `{}` does not exist or is not a folder",
            dir.display()
        )));
    }
    let canonical = dir.canonicalize()?;
    let profile = user_profile.canonicalize()?;
    if !canonical.starts_with(&profile) {
        return Err(CoreError::Validation(format!(
            "project folder `{}` must be inside your user profile",
            dir.display()
        )));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_resolve_against_working_dir() {
        let wd = Path::new("/work/proj");
        let resolved = resolve_step_path(Path::new("./mods/a.jar"), wd).unwrap();
        assert_eq!(resolved, wd.join("mods").join("a.jar"));
    }

    #[test]
    fn parent_components_are_rejected() {
        let err = resolve_step_path(Path::new("../../etc/passwd"), Path::new("/w"));
        assert!(err.is_err());
    }

    #[test]
    fn folder_tokens_expand_from_environment() {
        let Some(local) = std::env::var_os("LOCALAPPDATA") else {
            return;
        };
        let resolved =
            resolve_step_path(Path::new("%LOCALAPPDATA%/SetupPal/x.jar"), Path::new("/w")).unwrap();
        assert_eq!(resolved, PathBuf::from(local).join("SetupPal/x.jar"));
    }

    #[test]
    fn project_dir_must_be_inside_profile() {
        let profile = tempfile::tempdir().unwrap();
        let inside = profile.path().join("app");
        std::fs::create_dir_all(&inside).unwrap();
        assert!(validate_project_dir(&inside, profile.path()).is_ok());

        let outside = tempfile::tempdir().unwrap();
        assert!(validate_project_dir(outside.path(), profile.path()).is_err());
        assert!(validate_project_dir(&profile.path().join("missing"), profile.path()).is_err());
    }
}
