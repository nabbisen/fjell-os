//! RFC-0.27-001: the `version-currency` subcheck (S3).
//!
//! E-023's specified-but-never-built check: the release tool was supposed
//! to "grep for stale version mentions outside `CHANGELOG.md`" and exit
//! non-zero on any inconsistency. Nothing was ever built, and `README.md`
//! sat at `0.21.3` — five releases stale, with five wrong counts — until
//! the owner happened to read it.
//!
//! ## Scope: `README.md` only, not every tracked document
//!
//! A tree-wide sweep for any `X.Y.Z`-shaped token that is not the current
//! version was tried and rejected: measured against the current tree, it
//! surfaces **over 800 occurrences across 30+ distinct historical version
//! strings** (`v0.1.0`, `v0.7.4`, `v0.21.2`, …), almost all of them
//! legitimate — `ROADMAP.md`'s shipped-release table, and every RFC's own
//! prose discussing the releases before or around it. Those documents are
//! historical narrative by design, exactly like `CHANGELOG.md` and
//! `docs/release/records/`, which the RFC already excludes; a whole-tree
//! version sweep would need the same exclusion extended to essentially
//! every RFC file and to `ROADMAP.md`, at which point it is not "outside
//! `CHANGELOG.md`" but "outside almost everything," and would still need
//! to distinguish a version number from the `X.Y.Z` substring embedded in
//! an RFC identifier like `RFC-v0.21.3-001` (which is not a version claim
//! at all). That is not a buildable check; it is `verify every number in
//! every document` wearing a different name — the exact shape D3/R4 warns
//! against, here rather than in S5.
//!
//! `README.md` is different in kind: it is the one document whose entire
//! purpose is "what is Fjell OS *right now*" — a live badge, not a dated
//! record — which is exactly what the E-023 incident was about. Scoping to
//! it is the same principle the RFC already applies to `CHANGELOG.md` and
//! the release records, pointed the other way: those are excluded because
//! they are historical by design, and `README.md` is included because it
//! is current-state by design.

use crate::read_file;
use std::process::ExitCode;

const README_PATH: &str = "README.md";
const CARGO_TOML_PATH: &str = "Cargo.toml";

pub fn check() -> ExitCode {
    let Some(readme_src) = read_file(README_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(cargo_src) = read_file(CARGO_TOML_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(current) = parse_workspace_version(&cargo_src) else {
        eprintln!("consistency-check: cannot find workspace version in {CARGO_TOML_PATH}");
        return ExitCode::FAILURE;
    };
    run_check(&readme_src, &current)
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
pub fn run_check(readme_src: &str, current: &str) -> ExitCode {
    let stale = find_stale_versions(readme_src, current);
    if stale.is_empty() {
        println!("version-currency: PASS ({README_PATH} matches workspace version {current})");
        ExitCode::SUCCESS
    } else {
        eprintln!("version-currency: FAIL");
        for v in &stale {
            eprintln!("  {README_PATH} mentions {v}, but the workspace version is {current}");
        }
        ExitCode::FAILURE
    }
}

/// Every `X.Y.Z` (optionally `v`-prefixed) token in `src` that is not
/// `current` and is not part of a larger identifier (an RFC id embeds a
/// version-shaped substring, e.g. `RFC-v0.21.3-001`, and is not a version
/// claim).
fn find_stale_versions(src: &str, current: &str) -> Vec<String> {
    let bytes = src.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'v' || bytes[i].is_ascii_digit() {
            let start = i;
            // Reject if glued to a preceding word character (mid-identifier,
            // e.g. `x0.21.3`) or immediately after this project's `RFC-`
            // identifier prefix (`RFC-v0.21.3-001` starts mid-identifier at
            // the `v`). A bare preceding hyphen is *not* disqualifying on
            // its own — `badge/version-0.21.3-blue.svg` is a real version
            // claim sitting between slug hyphens, not part of a larger
            // identifier.
            let preceded_by_rfc_prefix = start >= 4 && src.get(start - 4..start) == Some("RFC-");
            let preceded_by_ident =
                (i > 0 && bytes[i - 1].is_ascii_alphanumeric()) || preceded_by_rfc_prefix;
            let mut j = i;
            if bytes[j] == b'v' {
                j += 1;
            }
            let num_start = j;
            // Consume digit groups separated by '.', but never a trailing
            // '.' not followed by another digit (a sentence-ending period
            // must not be absorbed into the token).
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
                if j < bytes.len()
                    && bytes[j] == b'.'
                    && j + 1 < bytes.len()
                    && bytes[j + 1].is_ascii_digit()
                {
                    j += 1;
                }
            }
            let token = &src[num_start..j];
            let parts: Vec<&str> = token.split('.').collect();
            let is_semver = parts.len() == 3
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
            // Reject if immediately followed by another alphanumeric —
            // that means the digit run continues into a token this loop's
            // char-class check did not treat as part of the number (there
            // is none today, but this guards against a future digit-suffix
            // shape being mis-split).
            let followed_by_ident = j < bytes.len() && bytes[j].is_ascii_alphanumeric();
            if is_semver && !preceded_by_ident && !followed_by_ident {
                let full = &src[start..j];
                let normalised = full.trim_start_matches('v');
                if normalised != current {
                    found.push(full.to_string());
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    found
}

/// Read `[workspace.package] version = "X.Y.Z"` from the root `Cargo.toml`.
fn parse_workspace_version(src: &str) -> Option<String> {
    let mut in_section = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.package]" {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') && trimmed != "[workspace.package]" {
            in_section = false;
            continue;
        }
        if in_section {
            if let Some(rest) = trimmed.strip_prefix("version") {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('=') {
                    let v = rest.trim().trim_matches('"');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_version() {
        let src = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.26.0\"\nedition = \"2024\"\n";
        assert_eq!(parse_workspace_version(src), Some("0.26.0".to_string()));
    }

    #[test]
    fn current_version_only_passes() {
        let readme =
            "[![Version](https://img.shields.io/badge/version-0.26.0-blue.svg)](CHANGELOG.md)\n";
        assert_eq!(run_check(readme, "0.26.0"), ExitCode::SUCCESS);
    }

    /// Required failure demonstration: the recorded incident — README stuck
    /// at a version five releases behind.
    #[test]
    fn stale_version_badge_fails() {
        let readme =
            "[![Version](https://img.shields.io/badge/version-0.21.3-blue.svg)](CHANGELOG.md)\n";
        assert_eq!(run_check(readme, "0.26.0"), ExitCode::FAILURE);
    }

    #[test]
    fn rfc_identifier_substrings_are_not_flagged() {
        // "0.21.3" is a substring of "RFC-v0.21.3-001" but is not itself a
        // version claim — must not be flagged.
        let readme = "See RFC-v0.21.3-001 for the build restoration record.\n";
        assert_eq!(run_check(readme, "0.26.0"), ExitCode::SUCCESS);
    }

    #[test]
    fn plain_v_prefixed_stale_version_is_caught() {
        let readme = "Current release: v0.21.3.\n";
        assert_eq!(run_check(readme, "0.26.0"), ExitCode::FAILURE);
    }

    #[test]
    fn two_component_version_is_not_a_semver_claim() {
        // "0.27" (a milestone, not a full version) must not be flagged.
        let readme = "Targeting 0.27 next.\n";
        assert_eq!(run_check(readme, "0.26.0"), ExitCode::SUCCESS);
    }
}
