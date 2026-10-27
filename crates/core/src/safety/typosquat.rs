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
        if corpus.iter().any(|&popular| canonicalize_pypi_name(popular) == canon_req) {
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
    fn catches_reqwests_and_colormaa_typosquats() {
        let m1 = detect_typosquat(Ecosystem::PyPI, "reqwests").expect("should catch reqwests");
        assert_eq!(m1.similar_popular_package, "requests");
        assert_eq!(m1.edit_distance, 1);

        let m2 = detect_typosquat(Ecosystem::PyPI, "colormaa").expect("should catch colormaa");
        assert_eq!(m2.similar_popular_package, "colorama");
        assert_eq!(m2.edit_distance, 2);
    }

    #[test]
    fn allows_exact_legitimate_package() {
        assert!(detect_typosquat(Ecosystem::PyPI, "requests").is_none());
        assert!(detect_typosquat(Ecosystem::Winget, "LLVM.LLVM").is_none());
    }

    #[test]
    fn catches_cyrillic_homoglyph_impersonation() {
        // Cyrillic 'е' (U+0435) lookalike mimicking ASCII 'requests'
        let fake_requests = "r\u{0435}quests";
        let m = detect_typosquat(Ecosystem::PyPI, fake_requests).expect("should catch homoglyph");
        assert_eq!(m.similar_popular_package, "requests");

        // Cyrillic 'о' (U+043E) lookalike mimicking ASCII 'colorama'
        let fake_colorama = "col\u{043E}rama";
        let m2 = detect_typosquat(Ecosystem::PyPI, fake_colorama).expect("should catch homoglyph");
        assert_eq!(m2.similar_popular_package, "colorama");
    }

    #[test]
    fn allows_pep503_canonical_equivalents() {
        // scikit_learn vs scikit-learn in PyPI is an official canonical alias, not a typosquat
        assert!(detect_typosquat(Ecosystem::PyPI, "scikit_learn").is_none());
        assert!(detect_typosquat(Ecosystem::PyPI, "scikit.learn").is_none());
    }
}
