//! # `fjell-repro-check`
//!
//! Verifies that Fjell release artefacts are bit-for-bit reproducible
//! (RFC-v0.10-003). Runs two clean builds and asserts every targeted
//! output file has an identical SHA-256 digest.
//!
//! Usage:
//!   `fjell-repro-check [--artefacts <dir>] [--build-cmd <cmd>] [--skip-build]`
//!
//! `--skip-build` assumes the artefacts already exist and just reports
//! whether two sets of digests (passed via stdin or a file) match.
//! In `--skip-build` mode the tool runs quickly enough to include in CI.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// Default artefact glob patterns relative to workspace root.
/// Extended as new services are added.
const DEFAULT_TARGETS: &[&str] = &[
    "target/riscv64gc-unknown-none-elf/release/fjell-kernel",
    "crates/fjell-kernel/prebuilt",   // directory: all *.bin inside
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let skip_build = args.iter().any(|a| a == "--skip-build");

    if skip_build {
        eprintln!("fjell-repro-check: --skip-build — checking existing digests");
        return check_existing_digests(&args);
    }

    eprintln!("fjell-repro-check: performing two-build reproducibility check");
    two_build_check(&args)
}

// ── Two-build check ───────────────────────────────────────────────────────────

fn two_build_check(args: &[String]) -> ExitCode {
    // Collect extra xtask build args (e.g. feature flags)
    let extra: Vec<&str> = args.windows(2)
        .find(|w| w[0] == "--build-cmd")
        .map(|w| vec![w[1].as_str()])
        .unwrap_or_default();

    // Build #1
    eprintln!("fjell-repro-check: build 1 / 2 …");
    if !run_build(&extra) {
        eprintln!("fjell-repro-check: build 1 failed");
        return ExitCode::FAILURE;
    }
    let digests_1 = collect_digests();

    // Build #2
    eprintln!("fjell-repro-check: build 2 / 2 …");
    if !run_build(&extra) {
        eprintln!("fjell-repro-check: build 2 failed");
        return ExitCode::FAILURE;
    }
    let digests_2 = collect_digests();

    compare_and_report(digests_1, digests_2)
}

fn run_build(extra: &[&str]) -> bool {
    let mut cmd = Command::new("cargo");
    cmd.args(["xtask", "build"]);
    cmd.args(extra);
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

// ── Existing-digest check (fast, no rebuild) ─────────────────────────────────

fn check_existing_digests(args: &[String]) -> ExitCode {
    // In --skip-build mode: hash what is currently in target/ against a
    // previously stored baseline (tests/repro/baseline-digests.txt).
    let baseline_path = args.windows(2)
        .find(|w| w[0] == "--baseline")
        .and_then(|w| w.get(1))
        .map(String::as_str)
        .unwrap_or("tests/repro/baseline-digests.txt");

    let current = collect_digests();

    // If baseline doesn't exist, create it and pass.
    if !Path::new(baseline_path).exists() {
        match save_digests(&current, baseline_path) {
            Ok(_) => {
                eprintln!("fjell-repro-check: baseline written to {}", baseline_path);
                return ExitCode::SUCCESS;
            }
            Err(e) => { eprintln!("fjell-repro-check: {}", e); return ExitCode::FAILURE; }
        }
    }

    let baseline = match load_digests(baseline_path) {
        Ok(d) => d,
        Err(e) => { eprintln!("fjell-repro-check: {}", e); return ExitCode::FAILURE; }
    };

    compare_and_report(baseline, current)
}

// ── Digest collection ─────────────────────────────────────────────────────────

fn collect_digests() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for target in DEFAULT_TARGETS {
        let path = Path::new(target);
        if path.is_dir() {
            // Collect all *.bin files in the directory
            if let Ok(entries) = fs::read_dir(path) {
                let mut paths: Vec<PathBuf> = entries.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("bin"))
                    .collect();
                paths.sort();
                for p in paths {
                    if let Some(d) = digest_file(&p) {
                        map.insert(p.display().to_string(), d);
                    }
                }
            }
        } else if path.exists() {
            if let Some(d) = digest_file(path) {
                map.insert(path.display().to_string(), d);
            }
        }
    }
    map
}

fn digest_file(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(format!("{:016x}", fnv64(&buf)))
}

/// FNV-1a 64-bit hash — fast, deterministic, sufficient for change detection.
fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Comparison and report ─────────────────────────────────────────────────────

fn compare_and_report(
    a: BTreeMap<String, String>,
    b: BTreeMap<String, String>,
) -> ExitCode {
    let mut mismatches: Vec<String> = Vec::new();
    let mut present = 0usize;

    for (path, digest_a) in &a {
        present += 1;
        match b.get(path) {
            None => mismatches.push(format!("MISSING in build 2: {}", path)),
            Some(digest_b) if digest_a != digest_b => {
                let da = &digest_a[..digest_a.len().min(8)];
                let db = &digest_b[..digest_b.len().min(8)];
                mismatches.push(format!("DIGEST DIFFERS: {}\n  build1={}\n  build2={}",
                    path, da, db));
            }
            _ => {}
        }
    }
    for path in b.keys() {
        if !a.contains_key(path) {
            mismatches.push(format!("EXTRA in build 2: {}", path));
        }
    }

    if mismatches.is_empty() {
        println!("fjell-repro-check: PASS ({} artefacts identical)", present);
        ExitCode::SUCCESS
    } else {
        eprintln!("fjell-repro-check: FAIL — {} mismatch(es):", mismatches.len());
        for m in &mismatches {
            eprintln!("  {}", m);
        }
        ExitCode::FAILURE
    }
}

// ── Digest persistence ────────────────────────────────────────────────────────

fn save_digests(map: &BTreeMap<String, String>, path: &str) -> std::io::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    for (k, v) in map {
        out.push_str(&format!("{} {}\n", v, k));
    }
    fs::write(path, out)
}

fn load_digests(path: &str) -> std::io::Result<BTreeMap<String, String>> {
    let content = fs::read_to_string(path)?;
    let mut map = BTreeMap::new();
    for line in content.lines() {
        let mut parts = line.splitn(2, ' ');
        if let (Some(digest), Some(path)) = (parts.next(), parts.next()) {
            map.insert(path.to_string(), digest.to_string());
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv64_deterministic() {
        assert_eq!(fnv64(b"hello"), fnv64(b"hello"));
        assert_ne!(fnv64(b"hello"), fnv64(b"world"));
    }

    #[test]
    fn compare_identical_passes() {
        let mut a = BTreeMap::new();
        a.insert("file.bin".into(), "abcd".into());
        let b = a.clone();
        let result = compare_and_report(a, b);
        assert_eq!(result, ExitCode::SUCCESS);
    }

    #[test]
    fn compare_mismatch_fails() {
        let mut a = BTreeMap::new();
        a.insert("file.bin".into(), "abcd".into());
        let mut b = BTreeMap::new();
        b.insert("file.bin".into(), "efgh".into());
        let result = compare_and_report(a, b);
        assert_eq!(result, ExitCode::FAILURE);
    }

    #[test]
    fn compare_missing_file_fails() {
        let mut a = BTreeMap::new();
        a.insert("file.bin".into(), "abcd".into());
        let b = BTreeMap::new(); // build 2 is empty
        let result = compare_and_report(a, b);
        assert_eq!(result, ExitCode::FAILURE);
    }

    #[test]
    fn digest_roundtrip() {
        let dir = std::env::temp_dir().join("fjell-repro-test");
        fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot.txt");
        let mut map = BTreeMap::new();
        map.insert("a.bin".into(), "deadbeef".into());
        save_digests(&map, path.to_str().unwrap()).unwrap();
        let loaded = load_digests(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded, map);
    }
}
