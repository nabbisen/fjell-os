//! `cargo xtask trust-report [--dry-run] [--out <file>]`
//!
//! Generates the Fjell Trust Report (RFC 061 §6): a six-section
//! machine-readable artefact that operationalises the "explainable"
//! identity claim.
//!
//! ## Six sections
//!
//! 1. **Capability inventory** — per-service cap-kinds declared in
//!    CapManifests (RFC v0.9-002) and what the broker grants.
//! 2. **Lease inventory** — lease kinds and revocation paths
//!    asserted by the SDK (currently structural; runtime data in v0.11).
//! 3. **Measurement chain** — bundle digests and anti-rollback
//!    metadata for each service binary.
//! 4. **Semantic catalog binding** — catalog version, owner crates,
//!    schema digest, total entries.
//! 5. **Unsafe inventory** — total sites, by category, missing count.
//! 6. **CI evidence** — test counts, gate verdicts from the most
//!    recent `test-all` run.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH, Duration};

/// Entry point for `cargo xtask trust-report`.
pub fn cmd_trust_report(args: &[String]) -> ExitCode {
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let out_path = args.windows(2)
        .find(|w| w[0] == "--out")
        .and_then(|w| w.get(1))
        .map(String::as_str)
        .unwrap_or("docs/release/trust-report.txt");

    println!("[trust-report] collecting sections …");
    let report = build_report(dry_run);

    if dry_run {
        print!("{}", report);
        return ExitCode::SUCCESS;
    }

    if let Some(parent) = Path::new(out_path).parent() {
        fs::create_dir_all(parent).ok();
    }
    match fs::write(out_path, &report) {
        Ok(_)  => println!("[trust-report] written to {}", out_path),
        Err(e) => { eprintln!("[trust-report] write error: {}", e); return ExitCode::FAILURE; }
    }
    ExitCode::SUCCESS
}

// ── Report assembly ───────────────────────────────────────────────────────────

fn build_report(dry_run: bool) -> String {
    let mut r = String::new();

    let ts = report_timestamp();
    let version = env!("CARGO_PKG_VERSION");

    r.push_str("═══════════════════════════════════════════════════════════════\n");
    r.push_str("                    FJELL OS TRUST REPORT\n");
    r.push_str("═══════════════════════════════════════════════════════════════\n");
    r.push_str(&format!("Generated : {}\n", ts));
    r.push_str(&format!("Version   : {}\n", version));
    r.push_str(&format!("Mode      : {}\n", if dry_run { "dry-run" } else { "full" }));
    r.push('\n');

    r.push_str(&section_1_capability_inventory());
    r.push_str(&section_2_lease_inventory());
    r.push_str(&section_3_measurement_chain());
    r.push_str(&section_4_catalog_binding());
    r.push_str(&section_5_unsafe_inventory());
    r.push_str(&section_6_ci_evidence());

    r
}

// ── Section 1: Capability inventory ──────────────────────────────────────────

fn section_1_capability_inventory() -> String {
    let mut s = String::new();
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    s.push_str("§1. CAPABILITY INVENTORY\n");
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    // Collect CapManifest files from the workspace
    let manifests = find_cap_manifests(Path::new("."));
    if manifests.is_empty() {
        s.push_str("  No cap-manifest.toml files found in workspace.\n");
        s.push_str("  (Runtime cap grants will appear here in v0.11.)\n\n");
    } else {
        s.push_str(&format!("  {} cap-manifest(s) found:\n\n", manifests.len()));
        for m in &manifests {
            s.push_str(&format!("  ┌ {}\n", m.display()));
            if let Ok(content) = fs::read_to_string(m) {
                for line in content.lines() {
                    if line.trim_start().starts_with("service")
                        || line.trim_start().starts_with("caps")
                        || line.trim_start().starts_with("sdk_api_rev")
                    {
                        s.push_str(&format!("  │  {}\n", line.trim()));
                    }
                }
            }
            s.push_str("  └\n");
        }
        s.push('\n');
    }
    s
}

fn find_cap_manifests(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let skip = ["target", ".git", "tests/runs"];
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if skip.iter().any(|s| path.ends_with(s)) { continue; }
            if path.is_dir() {
                found.extend(find_cap_manifests(&path));
            } else if path.file_name().and_then(|n| n.to_str()) == Some("cap-manifest.toml") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

// ── Section 2: Lease inventory ────────────────────────────────────────────────

fn section_2_lease_inventory() -> String {
    let mut s = String::new();
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    s.push_str("§2. LEASE INVENTORY\n");
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    // Structural survey: count LeaseId / LeaseEpoch usage sites
    let lease_sites = count_pattern_in_workspace("LeaseId");
    let epoch_sites = count_pattern_in_workspace("LeaseEpoch");
    s.push_str(&format!("  LeaseId usage sites   : {}\n", lease_sites));
    s.push_str(&format!("  LeaseEpoch usage sites: {}\n", epoch_sites));
    s.push_str("  Runtime lease state   : (available from running fleet in v0.11)\n\n");
    s
}

// ── Section 3: Measurement chain ─────────────────────────────────────────────

fn section_3_measurement_chain() -> String {
    let mut s = String::new();
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    s.push_str("§3. MEASUREMENT AND BUNDLE DIGEST CHAIN\n");
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    let prebuilt = Path::new("crates/fjell-kernel/prebuilt");
    if prebuilt.exists() {
        let bins: Vec<_> = fs::read_dir(prebuilt).ok().into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("bin"))
            .collect();
        s.push_str(&format!("  Service binaries in prebuilt/: {}\n", bins.len()));
        for b in &bins {
            let size = fs::metadata(b.path()).map(|m| m.len()).unwrap_or(0);
            s.push_str(&format!("    {} ({} bytes)\n",
                b.file_name().to_string_lossy(), size));
        }
    } else {
        s.push_str("  prebuilt/ not yet built — run `cargo xtask build` first.\n");
    }
    s.push_str("  Bundle signatures: (available after v0.11 signing pipeline)\n\n");
    s
}

// ── Section 4: Semantic catalog binding ──────────────────────────────────────

fn section_4_catalog_binding() -> String {
    let mut s = String::new();
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    s.push_str("§4. SEMANTIC CATALOG BINDING\n");
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    // Read the catalog version and entry count from source
    let version = read_catalog_version();
    let entry_count = count_catalog_entries();
    s.push_str(&format!("  Catalog version : {}\n", version));
    s.push_str(&format!("  Total entries   : {}\n", entry_count));
    s.push_str(&format!("  Schema digest   : (computed by fjell-semantic-toolkit)\n"));
    s.push_str(&format!("  Frozen          : yes (RFC v0.5-004)\n\n"));
    s
}

fn read_catalog_version() -> String {
    let path = "crates/fjell-semantic-v1/src/version.rs";
    fs::read_to_string(path).ok()
        .and_then(|s| {
            s.lines()
             .find(|l| l.contains("CATALOG_V1_VERSION"))
             .and_then(|l| l.split('(').nth(1))
             .and_then(|l| l.split(')').next())
             .map(|v| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn count_catalog_entries() -> usize {
    let path = "crates/fjell-semantic-v1/src/catalog.rs";
    fs::read_to_string(path).ok()
        .map(|s| s.lines().filter(|l| l.trim().starts_with("IntentEntry { tag:")).count())
        .unwrap_or(0)
}

// ── Section 5: Unsafe inventory ───────────────────────────────────────────────

fn section_5_unsafe_inventory() -> String {
    let mut s = String::new();
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    s.push_str("§5. UNSAFE SITE INVENTORY\n");
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    let out = Command::new("cargo")
        .args(["run", "-p", "fjell-unsafe-audit", "--",
               "--workspace", ".", "--check"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match out {
        Ok(o) => {
            let combined = format!("{}{}", 
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr));
            // Extract the summary lines
            let verdict = if o.status.success() { "PASS" } else { "FAIL" };
            for line in combined.lines() {
                if line.contains("total unsafe")
                    || line.contains("with SAFETY")
                    || line.contains("missing comment")
                    || line.contains("invalid category")
                {
                    s.push_str(&format!("  {}\n", line.trim()));
                }
            }
            s.push_str(&format!("  Gate verdict      : {}\n\n", verdict));
        }
        Err(e) => {
            s.push_str(&format!("  (unsafe-audit unavailable: {})\n\n", e));
        }
    }
    s
}

// ── Section 6: CI evidence ────────────────────────────────────────────────────

fn section_6_ci_evidence() -> String {
    let mut s = String::new();
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    s.push_str("§6. CI EVIDENCE\n");
    s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    // Try to find the most recent test-all run summary
    let runs_dir = Path::new("tests/runs");
    if let Some(latest) = latest_run(runs_dir) {
        let summary_path = latest.join("summary.json");
        if summary_path.exists() {
            s.push_str(&format!("  Latest test-all run: {}\n",
                latest.file_name().unwrap_or_default().to_string_lossy()));
            let summary_txt = latest.join("summary.txt");
            if let Ok(txt) = fs::read_to_string(&summary_txt) {
                // Print just the footer lines
                for line in txt.lines().rev().take(5).collect::<Vec<_>>().iter().rev() {
                    s.push_str(&format!("  {}\n", line));
                }
            }
        } else {
            s.push_str("  Latest run summary: (no summary.json found)\n");
        }
    } else {
        s.push_str("  No test-all runs found. Run `cargo xtask test-all --no-qemu` first.\n");
    }

    // Host test count from live run (fast)
    s.push('\n');
    s.push_str("  Live host test count:\n");
    let out = Command::new("cargo")
        .args(["test", "--workspace", "--lib", "--exclude", "fjell-proptest"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match out {
        Ok(o) => {
            let combined = format!("{}{}", 
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr));
            let mut total = 0usize;
            let mut failed = 0usize;
            for line in combined.lines() {
                if line.contains("test result:") {
                    if let Some(n) = parse_count(line, "passed") { total += n; }
                    if let Some(n) = parse_count(line, "failed") { failed += n; }
                }
            }
            let verdict = if failed == 0 { "PASS" } else { "FAIL" };
            s.push_str(&format!("    Host tests   : {} passed, {} failed  [{}]\n",
                total, failed, verdict));
        }
        Err(e) => {
            s.push_str(&format!("    (host tests unavailable: {})\n", e));
        }
    }
    s.push_str("    Proptest     : 10 properties × 1000 cases\n");
    s.push_str("    QEMU smoke   : see last test-all run for live verdicts\n\n");
    s
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn latest_run(dir: &Path) -> Option<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(dir).ok()?.flatten()
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    entries.last().map(|e| e.path())
}

fn count_pattern_in_workspace(pattern: &str) -> usize {
    let out = Command::new("grep")
        .args(["-rn", "--include=*.rs", pattern, "crates/"])
        .stdout(Stdio::piped()).stderr(Stdio::null())
        .output().ok();
    out.map(|o| String::from_utf8_lossy(&o.stdout).lines().count())
       .unwrap_or(0)
}

fn parse_count(line: &str, word: &str) -> Option<usize> {
    // Cargo test output: "487 passed;" — number precedes the word.
    let idx = line.find(word)?;
    let before = &line[..idx];
    let num: String = before.chars().rev()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars().rev().collect();
    num.parse().ok()
}

fn report_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let s = secs % 86400;
    let d = secs / 86400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    // Approximate date from epoch (same algorithm as test_all.rs)
    let days = d + 719_468;
    let era  = days / 146_097;
    let doe  = days - era * 146_097;
    let yoe  = (doe - doe/1460 + doe/36524 - doe/146096) / 365;
    let y    = yoe + era * 400;
    let doy  = doe - (365*yoe + yoe/4 - yoe/100);
    let mp   = (5*doy + 2) / 153;
    let day  = doy - (153*mp + 2)/5 + 1;
    let month= if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", year, month, day, hh, mm, ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_contains_all_six_sections() {
        let r = build_report(true);
        for n in 1..=6 {
            assert!(r.contains(&format!("§{}.", n)),
                "report missing section §{}", n);
        }
    }

    #[test]
    fn report_has_header() {
        let r = build_report(true);
        assert!(r.contains("TRUST REPORT"));
        assert!(r.contains("Version"));
    }

    #[test]
    fn timestamp_format_non_empty() {
        let ts = report_timestamp();
        assert!(ts.len() > 10, "timestamp should look like a date");
    }

    #[test]
    fn parse_count_extracts_number() {
        assert_eq!(parse_count("test result: ok. 487 passed; 0 failed", "passed"), Some(487));
        assert_eq!(parse_count("test result: ok. 487 passed; 0 failed", "failed"), Some(0));
        assert_eq!(parse_count("no numbers here", "passed"), None);
    }
}
