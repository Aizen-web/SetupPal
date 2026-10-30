//! Turns a plain app name ("LMMS", "Roblox", "install vlc please") into one exact winget
//! package ID, or nothing. Used for manual overrides and for install intents so the LLM never
//! invents IDs.

use crate::installability::winget_show;
use crate::os::OsProvider;
use crate::types::{AdapterKind, Ecosystem, Plan, PlanSource, PlannedStep, Step};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WingetSearchRow {
    pub name: String,
    pub id: String,
}

pub fn winget_search_args(query: &str) -> Vec<String> {
    [
        "search",
        query,
        "--source",
        "winget",
        "--accept-source-agreements",
        "--disable-interactivity",
    ]
    .map(String::from)
    .to_vec()
}

/// Parses the fixed-width table `winget search` prints. Column starts come from the header line,
/// counted in characters so names such as `HWiNFO®` don't shift the ID column.
pub fn parse_winget_search(stdout: &str) -> Vec<WingetSearchRow> {
    // winget redraws its progress spinner with `\r`; only the text after the last one is real.
    let lines: Vec<Vec<char>> = stdout
        .lines()
        .map(|l| l.rsplit('\r').next().unwrap_or_default().chars().collect())
        .collect();
    let Some(header_idx) = lines.iter().position(|l| {
        let s: String = l.iter().collect();
        s.trim_start().starts_with("Name") && s.contains(" Id ")
    }) else {
        return Vec::new();
    };
    let header: String = lines[header_idx].iter().collect();
    let Some(id_col) = header.find(" Id ").map(|b| header[..b].chars().count() + 1) else {
        return Vec::new();
    };
    let version_col = header
        .find(" Version")
        .map(|b| header[..b].chars().count() + 1)
        .filter(|&c| c > id_col);

    lines
        .iter()
        .skip(header_idx + 1)
        .filter(|l| !l.iter().all(|c| *c == '-' || c.is_whitespace()))
        .filter(|l| l.len() > id_col)
        .filter_map(|l| {
            let name: String = l[..id_col].iter().collect();
            let end = version_col.unwrap_or(l.len()).min(l.len());
            let id: String = l[id_col..end].iter().collect();
            let id = id.split_whitespace().next()?.to_string();
            Some(WingetSearchRow {
                name: name.trim().to_string(),
                id,
            })
        })
        .collect()
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Strips chat fluff so "install vlc please" / "get Audacity.Audacity from winget" become a query.
pub fn extract_package_query(goal: &str) -> String {
    let trimmed = goal.trim();
    // Prefer an explicit Publisher.Package id if the user typed one.
    if let Some(id) = extract_dotted_id(trimmed) {
        return id.to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    let mut start = 0;
    for prefix in [
        "please install ",
        "can you install ",
        "install ",
        "get ",
        "set up ",
        "setup ",
        "add ",
        "i need ",
        "i want ",
    ] {
        if lower[start..].starts_with(prefix) {
            start += prefix.len();
            break;
        }
    }
    let mut end = trimmed.len();
    let body_lower = lower[start..].to_string();
    for suffix in [" please", " for me", " from winget", " via winget", " with winget"] {
        if body_lower.ends_with(suffix) {
            end = start + body_lower.len() - suffix.len();
            break;
        }
    }
    trimmed[start..end]
        .trim()
        .trim_matches(|c: char| c == '.' || c == '!')
        .to_string()
}

/// Finds a `Publisher.Package` token in free text.
pub fn extract_dotted_id(text: &str) -> Option<&str> {
    text.split_whitespace().find(|tok| {
        let t = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-');
        let parts: Vec<_> = t.split('.').collect();
        parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    }).map(|tok| tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-'))
}

/// The one row that clearly is `query`: its name equals the query, or its ID is the query
/// (`Roblox` -> `Roblox.Roblox`). Truncated IDs (`…`) and ties return `None`.
pub fn pick_exact_match(query: &str, rows: &[WingetSearchRow]) -> Option<String> {
    let q = normalize(query);
    if q.is_empty() {
        return None;
    }
    // Exact ID typed by the user wins.
    let by_exact_id: Vec<_> = rows
        .iter()
        .filter(|r| !r.id.contains('…') && normalize(&r.id) == q)
        .collect();
    if by_exact_id.len() == 1 {
        return Some(by_exact_id[0].id.clone());
    }

    let usable = rows.iter().filter(|r| !r.id.contains('…'));
    let by_name: Vec<_> = usable.clone().filter(|r| normalize(&r.name) == q).collect();
    if by_name.len() == 1 {
        return Some(by_name[0].id.clone());
    }
    if by_name.len() > 1 {
        // Prefer Publisher.Name when the name part matches (Audacity.Audacity).
        let doubled = format!("{q}{q}");
        let canonical: Vec<_> = by_name
            .iter()
            .filter(|r| normalize(&r.id) == doubled || normalize(&r.id).ends_with(&q))
            .collect();
        if canonical.len() == 1 {
            return Some(canonical[0].id.clone());
        }
        return None;
    }
    let doubled = format!("{q}{q}");
    let by_id: Vec<_> = usable
        .filter(|r| {
            let id = normalize(&r.id);
            id == q || id == doubled || id.ends_with(&q)
        })
        .collect();
    if by_id.len() == 1 {
        return Some(by_id[0].id.clone());
    }
    // Moniker-style: Id ends with .Query (VideoLAN.VLC for "vlc").
    if by_id.len() > 1 {
        let tight: Vec<_> = by_id
            .iter()
            .filter(|r| {
                r.id
                    .rsplit('.')
                    .next()
                    .map(|last| normalize(last) == q)
                    .unwrap_or(false)
            })
            .collect();
        if tight.len() == 1 {
            return Some(tight[0].id.clone());
        }
    }
    None
}

/// Known one-to-one aliases when winget search is noisy but the community ID is stable.
fn known_alias(query: &str) -> Option<&'static str> {
    match normalize(query).as_str() {
        "vlc" => Some("VideoLAN.VLC"),
        "7zip" | "7z" => Some("7zip.7zip"),
        "audacity" => Some("Audacity.Audacity"),
        "ripgrep" | "rg" => Some("BurntSushi.ripgrep.MSVC"),
        "jq" => Some("jqlang.jq"),
        "notepad++" | "notepadpp" => Some("Notepad++.Notepad++"),
        "lmms" => Some("LMMS.LMMS"),
        "discord" => Some("Discord.Discord"),
        "obs" | "obsstudio" => Some("OBSProject.OBSStudio"),
        "steam" => Some("Valve.Steam"),
        "spotify" => Some("Spotify.Spotify"),
        "git" => Some("Git.Git"),
        "python312" | "python3.12" => Some("Python.Python.3.12"),
        _ => None,
    }
}

/// Searches winget for `query` and confirms the pick with `winget show`.
pub async fn resolve_winget_id(os: &dyn OsProvider, query: &str) -> Option<String> {
    let query = extract_package_query(query);
    if query.is_empty() {
        return None;
    }
    let paths = os.refresh_and_get_path().await.ok()?;
    let winget = os.which_in_paths("winget", &paths)?;

    // Exact ID the user typed: `winget show --id --exact` must succeed.
    if query.contains('.') {
        if winget_show(os, &query, None).await.is_some() {
            return Some(query);
        }
        return None;
    }

    let out = os
        .run_command(&winget, &winget_search_args(&query), &paths)
        .await
        .ok()?;
    if out.exit_code == Some(0) {
        if let Some(id) = pick_exact_match(&query, &parse_winget_search(&out.stdout)) {
            return winget_show(os, &id, None).await.map(|_| id);
        }
    }

    // Stable alias only when search did not produce a conflicting clear match.
    if let Some(alias) = known_alias(&query) {
        return winget_show(os, alias, None).await.map(|_| alias.to_string());
    }
    None
}

/// A plan with exactly one step: install `id` from winget. Verification is the adapter's own
/// `winget list --id <id> --exact`, so no extra command step is added.
pub fn single_winget_plan(query: &str, id: &str) -> Plan {
    let query = extract_package_query(query);
    let query = if query.is_empty() { id.to_string() } else { query };
    Plan {
        plan_id: format!("plan-winget-{}", id.to_ascii_lowercase()),
        goal: query.clone(),
        summary: format!("Installs {query} ({id}) from winget and checks that it is registered."),
        source: PlanSource::Resolved {
            query: query.clone(),
        },
        steps: vec![PlannedStep {
            id: "step-1".to_string(),
            description: format!("Install {query} ({id}) via winget"),
            step: Step::InstallPackage {
                ecosystem: Ecosystem::Winget,
                package_id: id.to_string(),
                version: None,
                source_adapter: AdapterKind::Winget,
            },
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::mock::{ok, MockOsProvider};

    const ROBLOX: &str = "\r   - \r   \\ \rName                      Id                                 Version       Match           Source
-------------------------------------------------------------------------------------------------
Roblox Player             Roblox.Roblox                      0.745         Moniker: roblox winget
Fishstrap                 Fishstrap.Fishstrap                3.1.2         Tag: roblox     winget
Roblox Studio             Roblox.RobloxStudio                1.6.3.172     Tag: roblox     winget
";

    const DISCORD: &str = "Name                        Id                                  Version   Match                Source
---------------------------------------------------------------------------------------------------------
Discord                     Discord.Discord                     1.0.9266  ProductCode: discord winget
Discord (arm64)             Discord.Discord.arm64               1.0.58    ProductCode: discord winget
";

    const HWINFO: &str = "Name                    Id                                     Version Match           Source
----------------------------------------------------------------------------------------------
HWiNFO®                 REALiX.HWiNFO                          8.54    Moniker: hwinfo winget
";

    const VLC: &str = "Name                            Id                           Version        Match        Source
-----------------------------------------------------------------------------------------------
VLC media player                VideoLAN.VLC                 3.0.21                      winget
";

    #[test]
    fn parses_columns_by_header_position() {
        let rows = parse_winget_search(ROBLOX);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], WingetSearchRow { name: "Roblox Player".into(), id: "Roblox.Roblox".into() });
        assert_eq!(parse_winget_search(HWINFO)[0].id, "REALiX.HWiNFO");
        assert!(parse_winget_search("No package found matching input criteria.").is_empty());
    }

    #[test]
    fn picks_only_a_clear_match() {
        assert_eq!(pick_exact_match("discord", &parse_winget_search(DISCORD)).as_deref(), Some("Discord.Discord"));
        assert_eq!(pick_exact_match("Roblox", &parse_winget_search(ROBLOX)).as_deref(), Some("Roblox.Roblox"));
        assert_eq!(pick_exact_match("hwinfo", &parse_winget_search(HWINFO)).as_deref(), Some("REALiX.HWiNFO"));
        assert_eq!(pick_exact_match("vlc", &parse_winget_search(VLC)).as_deref(), Some("VideoLAN.VLC"));
        assert_eq!(pick_exact_match("Minecraft Bedrock", &parse_winget_search(ROBLOX)), None);
        assert_eq!(pick_exact_match("", &parse_winget_search(ROBLOX)), None);
    }

    #[test]
    fn ties_and_truncated_ids_are_not_guessed() {
        let rows = vec![
            WingetSearchRow { name: "Example".into(), id: "A.Example".into() },
            WingetSearchRow { name: "Example".into(), id: "B.Example".into() },
        ];
        assert_eq!(pick_exact_match("example", &rows), None);
        let truncated = vec![WingetSearchRow { name: "Long".into(), id: "Publisher.VeryLong…".into() }];
        assert_eq!(pick_exact_match("long", &truncated), None);
    }

    #[test]
    fn strips_install_fluff_and_finds_dotted_ids() {
        assert_eq!(extract_package_query("install vlc please"), "vlc");
        assert_eq!(extract_package_query("Audacity.Audacity from winget"), "Audacity.Audacity");
        assert_eq!(extract_dotted_id("install Audacity.Audacity from winget"), Some("Audacity.Audacity"));
    }

    #[tokio::test]
    async fn resolves_through_search_then_show() {
        let os = MockOsProvider::new()
            .with_binary("winget", r"C:\winget.exe")
            .on("winget", &["search"], ok(ROBLOX))
            .on("winget", &["show"], ok("Found Roblox Player [Roblox.Roblox]\nInstaller:\n  Installer Type: portable\n"));
        assert_eq!(resolve_winget_id(&os, "roblox").await.as_deref(), Some("Roblox.Roblox"));
    }

    #[tokio::test]
    async fn exact_id_uses_show_not_search() {
        let os = MockOsProvider::new()
            .with_binary("winget", r"C:\winget.exe")
            .on("winget", &["show"], ok("Found Audacity [Audacity.Audacity]\nInstaller Type: nullsoft\n"));
        assert_eq!(
            resolve_winget_id(&os, "install Audacity.Audacity from winget")
                .await
                .as_deref(),
            Some("Audacity.Audacity")
        );
    }

    #[tokio::test]
    async fn nothing_found_is_none() {
        let os = MockOsProvider::new()
            .with_binary("winget", r"C:\winget.exe")
            .on("winget", &["search"], ok("No package found matching input criteria."));
        assert_eq!(resolve_winget_id(&os, "Minecraft Bedrock").await, None);
    }

    #[test]
    fn single_plan_has_one_install_step_and_nothing_else() {
        let plan = single_winget_plan("LMMS", "LMMS.LMMS");
        assert_eq!(plan.steps.len(), 1);
        assert!(matches!(
            &plan.steps[0].step,
            Step::InstallPackage { package_id, ecosystem: Ecosystem::Winget, .. } if package_id == "LMMS.LMMS"
        ));
    }
}
