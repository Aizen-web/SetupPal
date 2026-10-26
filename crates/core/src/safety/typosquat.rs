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

/// Checks if `requested_package` is within Levenshtein edit distance `1..=2` of a popular
/// package in `ecosystem` without being an exact match.
///
/// Examples caught immediately:
/// - `reqwests` vs `requests` (distance = 1) -> `Red` + `suggested_alternative: Some("requests")`
/// - `colormaa` vs `colorama` (distance = 2) -> `Red` + `suggested_alternative: Some("colorama")`
pub fn detect_typosquat(ecosystem: Ecosystem, requested_package: &str) -> Option<TyposquatMatch> {
    let req_norm = requested_package.trim().to_ascii_lowercase();
    if req_norm.len() < 4 {
        return None;
    }

    let corpus = top_packages_for_ecosystem(ecosystem);

    // If the package is an exact match for a known popular package, it is legitimate.
    if corpus
        .iter()
        .any(|&popular| popular.eq_ignore_ascii_case(&req_norm))
    {
        return None;
    }

    let mut best_match: Option<TyposquatMatch> = None;

    for &popular in corpus {
        let pop_norm = popular.to_ascii_lowercase();
        let dist = levenshtein(&req_norm, &pop_norm);
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
}
