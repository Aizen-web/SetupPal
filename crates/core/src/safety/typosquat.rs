use strsim::levenshtein;

use crate::safety::TyposquatMatch;
use crate::types::Ecosystem;

/// Curated top packages per ecosystem used by the Levenshtein typosquat radar.
pub fn top_packages_for_ecosystem(ecosystem: Ecosystem) -> &'static [&'static str] {
    match ecosystem {
        Ecosystem::PyPI => &[
            "requests",
            "urllib3",
            "colorama",
            "numpy",
            "pandas",
            "scipy",
            "torch",
            "torchvision",
            "tensorflow",
            "scikit-learn",
            "matplotlib",
            "fastapi",
            "flask",
            "django",
            "pydantic",
            "sqlalchemy",
            "pytest",
            "black",
            "ruff",
            "cryptography",
            "boto3",
            "pillow",
            "pyyaml",
            "jinja2",
            "httpx",
            "aiohttp",
            "beautifulsoup4",
            "selenium",
            "jupyterlab",
            "transformers",
        ],
        Ecosystem::Npm => &[
            "react",
            "react-dom",
            "next",
            "typescript",
            "express",
            "lodash",
            "axios",
            "chalk",
            "commander",
            "dotenv",
            "vite",
            "webpack",
            "eslint",
            "prettier",
            "tailwindcss",
            "zod",
            "prisma",
            "socket.io",
            "uuid",
            "cross-env",
        ],
        Ecosystem::Winget => &[
            "LLVM.LLVM",
            "Kitware.CMake",
            "Git.Git",
            "OpenJS.NodeJS.LTS",
            "OpenJS.NodeJS",
            "Python.Python.3.12",
            "EclipseAdoptium.Temurin.21.JDK",
            "Microsoft.VisualStudioCode",
            "Rustlang.Rustup",
            "MSYS2.MSYS2",
            "Ninja-build.Ninja",
            "7zip.7zip",
            "VideoLAN.VLC",
            "OBSProject.OBSStudio",
        ],
        Ecosystem::Cargo => &[
            "ripgrep",
            "bat",
            "fd-find",
            "tokei",
            "hyperfine",
            "just",
            "zoxide",
            "starship",
            "cargo-edit",
            "cargo-watch",
            "cargo-nextest",
            "wasm-pack",
            "mdbook",
            "tauri-cli",
        ],
        Ecosystem::Modrinth => &[
            "sodium",
            "lithium",
            "ferrite-core",
            "iris",
            "entityculling",
            "fabric-api",
            "modmenu",
            "immediatelyfast",
        ],
        Ecosystem::GitHub | Ecosystem::Vendor => &[],
    }
}

fn winget_publisher(id: &str) -> &str {
    id.split('.').next().unwrap_or(id)
}

/// Normalizes common Unicode confusable homoglyphs (e.g. Cyrillic/Greek lookalikes) to standard ASCII.
pub fn normalize_homoglyphs(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            // Cyrillic lookalikes
            '\u{0430}' | '\u{0410}' => 'a',
            '\u{0441}' | '\u{0421}' => 'c',
            '\u{0435}' | '\u{0415}' => 'e',
            '\u{0456}' | '\u{0406}' => 'i',
            '\u{0458}' | '\u{0408}' => 'j',
            '\u{043E}' | '\u{041E}' => 'o',
            '\u{0440}' | '\u{0420}' => 'p',
            '\u{0455}' | '\u{0405}' => 's',
            '\u{0445}' | '\u{0425}' => 'x',
            '\u{0443}' | '\u{0423}' => 'y',
            // Greek lookalikes
            '\u{03B1}' | '\u{0391}' => 'a',
            '\u{03B2}' | '\u{0392}' => 'b',
            '\u{03B5}' | '\u{0395}' => 'e',
            '\u{03BF}' | '\u{039F}' => 'o',
            '\u{03C1}' | '\u{03A1}' => 'p',
            _ => c,
        })
        .collect()
}

/// Canonicalizes a PyPI package name per PEP 503 by collapsing runs of `[-_.]+` into a single `-`.
pub fn canonicalize_pypi_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = false;
    for c in name.chars() {
        if c == '-' || c == '_' || c == '.' {
            if !prev_dash {
                out.push('-');
                prev_dash = true;
            }
        } else {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        }
    }
    out
}

/// Checks if `requested_package` is within Levenshtein edit distance `1..=2` of a popular
/// package in `ecosystem` without being an exact match, or if it uses Unicode homoglyph lookalikes.
///
/// Examples caught immediately:
/// - `reqwests` vs `requests` (distance = 1) -> `Red` + `suggested_alternative: Some("requests")`
/// - `colormaa` vs `colorama` (distance = 2) -> `Red` + `suggested_alternative: Some("colorama")`
/// - `rеquests` (Cyrillic 'е') vs `requests` -> `Red` + `suggested_alternative: Some("requests")`
pub fn detect_typosquat(ecosystem: Ecosystem, requested_package: &str) -> Option<TyposquatMatch> {
    let req_norm = requested_package.trim().to_ascii_lowercase();
    if req_norm.len() < 4 {
        return None;
    }

    let corpus = top_packages_for_ecosystem(ecosystem);

    // If the package is an exact ASCII match for a known popular package, it is legitimate.
    let is_exact_ascii = requested_package.is_ascii()
        && corpus
            .iter()
            .any(|&popular| popular.eq_ignore_ascii_case(&req_norm));
    if is_exact_ascii {
        return None;
    }

    // PEP 503 canonicalization for PyPI packages (e.g. scikit_learn vs scikit-learn are identical)
    if ecosystem == Ecosystem::PyPI && requested_package.is_ascii() {
        let canon_req = canonicalize_pypi_name(&req_norm);
        if corpus
            .iter()
            .any(|&popular| canonicalize_pypi_name(popular) == canon_req)
        {
            return None;
        }
    }

    // 1. Homoglyph confusable detection (e.g. Cyrillic lookalike characters mimicking ASCII names)
    let homo_norm = normalize_homoglyphs(&req_norm);
    if homo_norm != req_norm {
        if let Some(&popular) = corpus.iter().find(|&p| p.eq_ignore_ascii_case(&homo_norm)) {
            return Some(TyposquatMatch {
                requested_package: requested_package.to_string(),
                similar_popular_package: popular.to_string(),
                edit_distance: 1,
            });
        }
    }

    let mut best_match: Option<TyposquatMatch> = None;

    for &popular in corpus {
        let pop_norm = popular.to_ascii_lowercase();

        // winget publisher namespaces are moderated, so `Python.Python.3.13` is a sibling
        // of `Python.Python.3.12`, not a lookalike. Only other publishers can squat a name.
        if ecosystem == Ecosystem::Winget
            && winget_publisher(&req_norm) == winget_publisher(&pop_norm)
        {
            continue;
        }

        // Calculate distance on raw normalized and homoglyph-normalized strings
        let dist_raw = levenshtein(&req_norm, &pop_norm);
        let dist_homo = levenshtein(&homo_norm, &pop_norm);
        let dist = dist_raw.min(dist_homo);

        if (1..=2).contains(&dist) {
            let replace = match &best_match {
                None => true,
                Some(prev) => dist < prev.edit_distance,
            };
            if replace {
                best_match = Some(TyposquatMatch {
                    requested_package: requested_package.to_string(),
                    similar_popular_package: popular.to_string(),
                    edit_distance: dist,
                });
            }
        }
    }

    best_match
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_and_two_edit_typos_are_flagged_with_the_real_package() {
        let one = detect_typosquat(Ecosystem::PyPI, "reqwests").expect("flagged");
        assert_eq!(one.similar_popular_package, "requests");
        assert_eq!(one.edit_distance, 1);
        let two = detect_typosquat(Ecosystem::PyPI, "colormaa").expect("flagged");
        assert_eq!(two.similar_popular_package, "colorama");
        assert_eq!(two.edit_distance, 2);
        assert!(detect_typosquat(Ecosystem::Npm, "lodasch").is_some());
    }

    #[test]
    fn exact_popular_names_and_unrelated_names_are_clean() {
        assert!(detect_typosquat(Ecosystem::PyPI, "requests").is_none());
        assert!(detect_typosquat(Ecosystem::PyPI, "Requests").is_none());
        assert!(detect_typosquat(Ecosystem::Npm, "left-pad-but-different").is_none());
        assert!(detect_typosquat(Ecosystem::Cargo, "serde").is_none());
    }

    #[test]
    fn short_names_are_never_flagged() {
        assert!(detect_typosquat(Ecosystem::PyPI, "bla").is_none());
    }

    #[test]
    fn cyrillic_lookalikes_are_caught_by_homoglyph_folding() {
        let squat = detect_typosquat(Ecosystem::PyPI, "r\u{0435}quests").expect("flagged");
        assert_eq!(squat.similar_popular_package, "requests");
        assert_eq!(normalize_homoglyphs("r\u{0435}quests"), "requests");
    }

    #[test]
    fn pypi_names_are_compared_after_pep_503_canonicalization() {
        assert_eq!(canonicalize_pypi_name("Scikit__Learn"), "scikit-learn");
        assert!(detect_typosquat(Ecosystem::PyPI, "scikit_learn").is_none());
    }

    #[test]
    fn winget_siblings_from_the_same_publisher_are_not_lookalikes() {
        assert!(detect_typosquat(Ecosystem::Winget, "Python.Python.3.13").is_none());
        assert!(detect_typosquat(Ecosystem::Winget, "Gitt.Git").is_some());
    }

    #[test]
    fn ecosystems_without_a_corpus_never_match() {
        assert!(top_packages_for_ecosystem(Ecosystem::GitHub).is_empty());
        assert!(detect_typosquat(Ecosystem::Vendor, "anything-at-all").is_none());
    }
}
