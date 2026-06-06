//! `fjell-ci-coverage` — CI coverage validator (RFC-v0.7.1-002).
//!
//! Usage:
//!   fjell-ci-coverage [--check] [--workspace <path>] [--ci <path>]
//!
//! Enumerates workspace members from `Cargo.toml`, parses the CI YAML
//! for `-p <name>` references, and reports which packages are missing.
//!
//! Exits 0 if every package is either tested in CI or listed under
//! `[workspace.metadata.fjell.ci_excluded]`.
//! Exits 1 on any uncovered package (with `--check`).

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let check_mode    = args.iter().any(|a| a == "--check");
    let workspace_dir = args.windows(2)
        .find(|w| w[0] == "--workspace")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| PathBuf::from("."));
    let ci_path = args.windows(2)
        .find(|w| w[0] == "--ci")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| workspace_dir.join(".github/workflows/ci.yml"));

    let cargo_toml_path = workspace_dir.join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| { eprintln!("Cannot read {}: {e}", cargo_toml_path.display()); process::exit(2); });

    // ── Parse workspace members ───────────────────────────────────────────────
    let members = parse_workspace_members(&cargo_toml);
    // ── Parse CI exclusions ───────────────────────────────────────────────────
    let excluded = parse_ci_excluded(&cargo_toml);
    // ── Parse CI coverage ─────────────────────────────────────────────────────
    let ci_text = fs::read_to_string(&ci_path).unwrap_or_default();
    let ci_covered = parse_ci_covered(&ci_text);

    println!("fjell-ci-coverage  workspace={}", workspace_dir.display());
    println!("  workspace members : {}", members.len());
    println!("  CI-excluded       : {}", excluded.len());
    println!("  covered in CI     : {}", ci_covered.len());

    // Generate coverage table
    let mut missing = Vec::new();
    let mut covered_count = 0usize;
    for name in &members {
        if excluded.contains_key(name.as_str()) {
            continue;
        }
        if ci_covered.contains(name.as_str()) {
            covered_count += 1;
        } else {
            missing.push(name.clone());
        }
    }

    println!("  tested/checked    : {covered_count}");
    println!("  not covered       : {}", missing.len());

    if !missing.is_empty() {
        println!("\nPACKAGES NOT COVERED BY CI (add to CI or ci_excluded):");
        for name in &missing {
            println!("  - {name}");
        }
        if check_mode {
            process::exit(1);
        }
    } else {
        println!("\nAll workspace members are covered or explicitly excluded.");
    }
}

/// Extract members = ["crates/..."] from workspace Cargo.toml.
fn parse_workspace_members(toml: &str) -> Vec<String> {
    let mut in_members = false;
    let mut members = Vec::new();
    for line in toml.lines() {
        let t = line.trim();
        if t == "members = [" || t.starts_with("members = [") { in_members = true; }
        if in_members {
            if let Some(s) = t.strip_prefix('"').and_then(|s| s.strip_suffix("\",").or_else(|| s.strip_suffix('"'))) {
                // Extract the crate name from the path
                let name = Path::new(s)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(s)
                    .to_string();
                members.push(name);
            }
            if t == "]" { in_members = false; }
        }
    }
    members
}

/// Extract [workspace.metadata.fjell.ci_excluded] entries.
fn parse_ci_excluded(toml: &str) -> BTreeMap<String, String> {
    let mut in_section = false;
    let mut excluded = BTreeMap::new();
    for line in toml.lines() {
        let t = line.trim();
        if t == "[workspace.metadata.fjell.ci_excluded]" { in_section = true; continue; }
        if in_section {
            if t.starts_with('[') { break; }
            // Parse: "crate-name" = { reason = "..." }
            if let Some(pos) = t.find(" = ") {
                let name = t[..pos].trim().trim_matches('"').to_string();
                let reason = t[pos+3..].trim().to_string();
                excluded.insert(name, reason);
            }
        }
    }
    excluded
}

/// Extract all `-p <name>` package references from CI YAML.
fn parse_ci_covered(yaml: &str) -> std::collections::BTreeSet<String> {
    let mut covered = std::collections::BTreeSet::new();
    for line in yaml.lines() {
        let t = line.trim();
        // Match: -p fjell-foo or --package fjell-foo
        for prefix in ["-p ", "--package "] {
            if let Some(rest) = t.strip_prefix(prefix) {
                let name = rest.split_whitespace().next()
                    .unwrap_or("")
                    .trim_end_matches('\\')
                    .to_string();
                if !name.is_empty() && !name.starts_with('-') {
                    covered.insert(name);
                }
            }
            // Also match inline: "-p foo -p bar ..."
            let mut s = t;
            while let Some(idx) = s.find(prefix) {
                s = &s[idx + prefix.len()..];
                let name = s.split_whitespace().next()
                    .unwrap_or("")
                    .trim_end_matches('\\')
                    .to_string();
                if !name.is_empty() && !name.starts_with('-') {
                    covered.insert(name);
                }
            }
        }
    }
    covered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_members_extracts_crate_names() {
        let toml = r#"
members = [
    "crates/fjell-kernel",
    "crates/fjell-cap",
    "tools/fjell-unsafe-audit",
]
"#;
        let m = parse_workspace_members(toml);
        assert!(m.contains(&"fjell-kernel".to_string()));
        assert!(m.contains(&"fjell-cap".to_string()));
        assert!(m.contains(&"fjell-unsafe-audit".to_string()));
    }

    #[test]
    fn parse_ci_covered_finds_p_flags() {
        let yaml = r#"
      - run: cargo test -p fjell-cap -p fjell-ipc --lib
      - run: cargo check --package fjell-kernel
"#;
        let covered = parse_ci_covered(yaml);
        assert!(covered.contains("fjell-cap"));
        assert!(covered.contains("fjell-ipc"));
        assert!(covered.contains("fjell-kernel"));
    }

    #[test]
    fn parse_ci_excluded_extracts_entries() {
        let toml = r#"
[workspace.metadata.fjell.ci_excluded]
"fjell-kernel"    = { reason = "cross-compile only" }
"fjell-neg-test"  = { reason = "covered by QEMU smoke" }
"#;
        let ex = parse_ci_excluded(toml);
        assert!(ex.contains_key("fjell-kernel"));
        assert!(ex.contains_key("fjell-neg-test"));
    }

    #[test]
    fn no_missing_when_all_covered() {
        let members = vec!["fjell-cap".to_string(), "fjell-kernel".to_string()];
        let mut excluded = BTreeMap::new();
        excluded.insert("fjell-kernel".to_string(), "cross-compile only".to_string());
        let mut ci_covered = std::collections::BTreeSet::new();
        ci_covered.insert("fjell-cap".to_string());

        let missing: Vec<_> = members.iter()
            .filter(|n| !excluded.contains_key(n.as_str()) && !ci_covered.contains(n.as_str()))
            .collect();
        assert!(missing.is_empty());
    }
}
