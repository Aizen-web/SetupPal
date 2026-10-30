//! Curated same-category winget alternatives for manual cards and broad-request replies (R18/R22).

use serde::{Deserialize, Serialize};

/// High-level install category used to pick fitting alternatives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppCategory {
    Gaming,
    Music,
    Video,
    Dev,
    Utilities,
}

/// One installable alternative SetupPal can offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alternative {
    pub name: String,
    pub winget_id: String,
}

const GAMING: &[(&str, &str)] = &[
    ("Steam", "Valve.Steam"),
    ("Epic Games Launcher", "EpicGames.EpicGamesLauncher"),
    ("Discord", "Discord.Discord"),
    ("OBS Studio", "OBSProject.OBSStudio"),
    ("MSI Afterburner", "Guru3D.Afterburner"),
    ("Roblox", "Roblox.Roblox"),
];

const MUSIC: &[(&str, &str)] = &[
    ("LMMS", "LMMS.LMMS"),
    ("Audacity", "Audacity.Audacity"),
    ("Spotify", "Spotify.Spotify"),
    ("VLC", "VideoLAN.VLC"),
];

const VIDEO: &[(&str, &str)] = &[
    ("OBS Studio", "OBSProject.OBSStudio"),
    ("VLC", "VideoLAN.VLC"),
    ("Audacity", "Audacity.Audacity"),
    ("HandBrake", "HandBrake.HandBrake"),
];

const DEV: &[(&str, &str)] = &[
    ("Visual Studio Code", "Microsoft.VisualStudioCode"),
    ("Git", "Git.Git"),
    ("Node.js LTS", "OpenJS.NodeJS.LTS"),
    ("Python 3.12", "Python.Python.3.12"),
    ("Rustup", "Rustlang.Rustup"),
];

const UTILITIES: &[(&str, &str)] = &[
    ("7-Zip", "7zip.7zip"),
    ("Notepad++", "Notepad++.Notepad++"),
    ("HWiNFO", "REALiX.HWiNFO"),
    ("PowerToys", "Microsoft.PowerToys"),
];

fn list_for(category: AppCategory) -> &'static [(&'static str, &'static str)] {
    match category {
        AppCategory::Gaming => GAMING,
        AppCategory::Music => MUSIC,
        AppCategory::Video => VIDEO,
        AppCategory::Dev => DEV,
        AppCategory::Utilities => UTILITIES,
    }
}

/// Infers a category from free text (goal, manual item, or user message).
pub fn detect_category(text: &str) -> Option<AppCategory> {
    let t = text.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
    if has(&[
        "game", "gaming", "steam", "epic", "xbox", "play", "fps", "minecraft", "roblox", "discord",
        "afterburner",
    ]) {
        return Some(AppCategory::Gaming);
    }
    if has(&[
        "music", "audio", "daw", "beat", "lmms", "audacity", "spotify", "song", "producer",
    ]) {
        return Some(AppCategory::Music);
    }
    if has(&["video", "stream", "obs", "record", "youtube", "handbrake", "vlc", "edit video"]) {
        return Some(AppCategory::Video);
    }
    if has(&[
        "code", "coding", "dev", "developer", "programming", "student", "learn", "python", "node",
        "typescript", "rust", "git", "vscode", "ide",
    ]) {
        return Some(AppCategory::Dev);
    }
    if has(&["util", "zip", "notepad", "powertoy", "hwinfo", "tool"]) {
        return Some(AppCategory::Utilities);
    }
    None
}

fn name_already_installed(name: &str, winget_id: &str, installed: &[String]) -> bool {
    let needles: Vec<String> = {
        let mut v = vec![name.to_ascii_lowercase()];
        for part in winget_id.split('.') {
            let p = part.to_ascii_lowercase();
            if p.len() >= 3 {
                v.push(p);
            }
        }
        for word in name.split(|c: char| !c.is_ascii_alphanumeric()) {
            let w = word.to_ascii_lowercase();
            if w.len() >= 3 {
                v.push(w);
            }
        }
        v
    };
    installed.iter().any(|tool| {
        let t = tool.to_ascii_lowercase();
        needles.iter().any(|n| t == *n || t.contains(n) || n.contains(&t))
    })
}

/// Returns 2–4 same-category winget alternatives, skipping anything already installed.
pub fn alternatives_for(
    category: AppCategory,
    installed: &[String],
    limit: usize,
) -> Vec<Alternative> {
    let limit = limit.clamp(2, 4);
    list_for(category)
        .iter()
        .filter(|(name, id)| !name_already_installed(name, id, installed))
        .take(limit)
        .map(|(name, id)| Alternative {
            name: (*name).to_string(),
            winget_id: (*id).to_string(),
        })
        .collect()
}

/// Keeps only alternative display names that belong to `category` (or are on its curated list).
pub fn filter_same_category(names: &[String], category: AppCategory) -> Vec<String> {
    let allowed: Vec<String> = list_for(category)
        .iter()
        .map(|(n, _)| n.to_ascii_lowercase())
        .collect();
    names
        .iter()
        .filter(|n| {
            let lower = n.to_ascii_lowercase();
            allowed.iter().any(|a| lower.contains(a) || a.contains(&lower))
                || detect_category(n) == Some(category)
        })
        .cloned()
        .collect()
}

/// True when the text names a category/theme rather than a specific program.
pub fn is_broad_request(text: &str) -> bool {
    const BROAD: &[&str] = &[
        "i want to game",
        "i wanna game",
        "wanna game",
        "i make music",
        "i'm a student",
        "im a student",
        "i'm learning to code",
        "im learning to code",
        "learning to code",
        "new pc",
        "what do i need",
        "set me up for gaming",
        "gaming setup",
        "music setup",
        "coding tools",
        "dev tools",
        "some apps",
        "the game launcher",
    ];
    let t = text.trim().to_ascii_lowercase();
    if BROAD.iter().any(|b| t.contains(b)) {
        return true;
    }
    detect_category(&t).is_some()
        && t.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .all(|w| {
                matches!(
                    w,
                    "i" | "im" | "i'm" | "a" | "an" | "the" | "to" | "for" | "my" | "on" | "pc"
                        | "computer" | "new" | "just" | "got" | "and" | "wanna" | "want" | "need"
                        | "what" | "do" | "me" | "up" | "setup" | "some" | "any" | "tools" | "tool"
                        | "apps" | "app" | "software" | "game" | "gaming" | "games" | "music"
                        | "code" | "coding" | "learn" | "learning" | "student" | "dev"
                        | "developer" | "development" | "programming" | "make" | "play" | "video"
                )
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaming_alternatives_skip_installed_and_stay_in_category() {
        let alts = alternatives_for(
            AppCategory::Gaming,
            &["steam".into(), "discord".into()],
            4,
        );
        assert!(alts.len() >= 2 && alts.len() <= 4);
        assert!(alts.iter().all(|a| detect_category(&a.name) == Some(AppCategory::Gaming)
            || GAMING.iter().any(|(n, _)| *n == a.name)));
        assert!(!alts.iter().any(|a| a.name == "Steam" || a.name == "Discord"));
        assert!(!alts.iter().any(|a| a.name == "Git" || a.name == "Visual Studio Code"));
    }

    #[test]
    fn music_category_does_not_offer_dev_tools() {
        let alts = alternatives_for(AppCategory::Music, &[], 4);
        assert!(alts.iter().any(|a| a.name == "LMMS"));
        assert!(!alts.iter().any(|a| a.winget_id.starts_with("Git.")
            || a.winget_id.contains("VisualStudioCode")));
    }

    #[test]
    fn filter_drops_wrong_category_names() {
        let kept = filter_same_category(
            &["Steam".into(), "Git".into(), "OBS Studio".into()],
            AppCategory::Gaming,
        );
        assert!(kept.contains(&"Steam".into()));
        assert!(kept.contains(&"OBS Studio".into()));
        assert!(!kept.iter().any(|k| k == "Git"));
    }

    #[test]
    fn broad_requests_are_detected() {
        assert!(is_broad_request("I just got a new pc and I wanna game on it"));
        assert!(is_broad_request("I make music"));
        assert!(is_broad_request("I'm learning to code"));
        assert!(!is_broad_request("install LMMS"));
        assert!(!is_broad_request("Minecraft Bedrock"));
    }
}
