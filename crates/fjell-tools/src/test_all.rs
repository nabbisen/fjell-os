//! `cargo xtask test-all [--no-qemu]` — full test suite runner.
//!
//! Runs every test tier in order, captures all output, and writes a
//! dated log bundle to `tests/runs/<timestamp>/`. The bundle is safe
//! to archive and attach to release notes.
//!
//! ## Tier order
//!
//! 1. **host-lib**   — `cargo test --workspace --lib --exclude fjell-proptest`
//! 2. **proptest**   — `cargo test -p fjell-proptest --release`
//! 3. **unsafe-audit**— `cargo run -p fjell-unsafe-audit -- --workspace . --check`
//! 4. **qemu-smoke** — `cargo xtask qemu-test <profile>` × 4 profiles
//! 5. **qemu-neg**   — `cargo xtask qemu-negative <category>` × 9 categories
//!
//! Tiers 4 and 5 are skipped when `--no-qemu` is passed or when
//! `qemu-system-riscv64` is not on `PATH`.
//!
//! ## Output layout
//!
//! ```text
//! tests/runs/
//!   2026-05-24T12-34-56/
//!     summary.txt        ← human-readable pass/fail table
//!     summary.json       ← machine-readable for CI artefacts
//!     01-host-lib.log
//!     02-proptest.log
//!     03-unsafe-audit.log
//!     04-qemu-smoke-m8.log
//!     04-qemu-smoke-v0.4-net.log
//!     04-qemu-smoke-v0.5-platform.log
//!     04-qemu-smoke-v0.7-sync.log
//!     05-qemu-neg-capability.log
//!     05-qemu-neg-mmio.log
//!     … (one file per negative category)
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Smoke test profiles run in tier 4.
const SMOKE_PROFILES: &[&str] = &[
    "m8",
    "v0.4-net",
    "v0.5-platform",
    "v0.7-sync",
];

/// Negative test categories run in tier 5.
const NEG_CATEGORIES: &[&str] = &[
    "capability",
    "mmio",
    "dma",
    "user-copy",
    "audit",
    "policy",
    "ipc",
    "svc",
    "harness",
];

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn cmd_test_all(args: &[String]) -> ExitCode {
    let no_qemu = args.iter().any(|a| a == "--no-qemu");
    let qemu_available = !no_qemu && qemu_on_path();

    // Create the dated run directory.
    let run_dir = make_run_dir();
    println!("[test-all] run dir: {}", run_dir.display());
    println!("[test-all] QEMU: {}", if qemu_available { "enabled" } else { "skipped" });
    println!();

    let mut results: Vec<TierResult> = Vec::new();

    // ── Tier 1: host lib tests ────────────────────────────────────────────────
    results.push(run_tier(
        &run_dir,
        "01-host-lib",
        "Host library tests",
        &["cargo", "test", "--workspace", "--lib", "--exclude", "fjell-proptest"],
    ));

    // ── Tier 2: proptest ─────────────────────────────────────────────────────
    results.push(run_tier(
        &run_dir,
        "02-proptest",
        "Property tests (proptest)",
        &["cargo", "test", "-p", "fjell-proptest", "--release"],
    ));

    // ── Tier 3: unsafe audit ─────────────────────────────────────────────────
    results.push(run_tier(
        &run_dir,
        "03-unsafe-audit",
        "Unsafe site audit",
        &["cargo", "run", "-p", "fjell-unsafe-audit", "--",
          "--workspace", ".", "--check"],
    ));

    // ── Tier 3c: MMIO ordering audit ───────────────────────────────────────
    results.push(run_tier(
        &run_dir,
        "03c-mmio-audit",
        "MMIO ordering audit",
        &["cargo", "run", "-p", "fjell-mmio-audit", "--", "--workspace", ".", "--check"],
    ));

    // ── Tier 3b: repro-check (skip-build mode — fast) ───────────────────────
    results.push(run_tier(
        &run_dir,
        "03b-repro-check",
        "Reproducible build (skip-build)",
        &["cargo", "run", "-p", "fjell-repro-check", "--", "--skip-build"],
    ));

    // ── Tier 4: QEMU smoke ───────────────────────────────────────────────────
    if qemu_available {
        // Build once before running any QEMU test.
        println!("[test-all] building kernel + services …");
        let build_ok = run_silent(&["cargo", "xtask", "build"]);
        if !build_ok {
            results.push(TierResult {
                id: "04-qemu-build".into(),
                label: "QEMU build (kernel + services)".into(),
                passed: false,
                duration: Duration::ZERO,
                log_path: run_dir.join("04-qemu-build.log"),
                note: Some("build failed; QEMU tiers skipped".into()),
            });
        } else {
            for profile in SMOKE_PROFILES {
                let id = format!("04-qemu-smoke-{}", profile.replace('.', "_"));
                let label = format!("QEMU smoke: {}", profile);
                results.push(run_tier(
                    &run_dir,
                    &id,
                    &label,
                    &["cargo", "xtask", "qemu-test", profile],
                ));
            }

            // ── Tier 5: QEMU negative ─────────────────────────────────────
            for cat in NEG_CATEGORIES {
                let id = format!("05-qemu-neg-{}", cat);
                let label = format!("QEMU negative: {}", cat);
                results.push(run_tier(
                    &run_dir,
                    &id,
                    &label,
                    &["cargo", "xtask", "qemu-negative", cat],
                ));
            }
        }
    } else {
        let note = if no_qemu {
            "skipped via --no-qemu"
        } else {
            "qemu-system-riscv64 not found on PATH"
        };
        println!("[test-all] QEMU tiers skipped: {}", note);
        for label in SMOKE_PROFILES.iter().map(|p| format!("QEMU smoke: {}", p))
            .chain(NEG_CATEGORIES.iter().map(|c| format!("QEMU negative: {}", c)))
        {
            results.push(TierResult::skipped(label, note));
        }
    }

    // ── Write summary ────────────────────────────────────────────────────────
    // A tier is SKIP iff it carries one of the skip notes; everything not
    // passed and not skipped is a real FAIL. (Previously the FAIL filter only
    // excluded notes starting with "skipped", so the "qemu-system-riscv64 not
    // found on PATH" skips were double-counted as failures and a skip-only
    // run exited FAILURE.)
    fn is_skip(r: &TierResult) -> bool {
        matches!(r.note.as_deref(),
            Some("skipped via --no-qemu")
            | Some("qemu-system-riscv64 not found on PATH"))
    }
    let passed  = results.iter().filter(|r| r.passed).count();
    let skipped = results.iter().filter(|r| is_skip(r)).count();
    let failed  = results.iter().filter(|r| !r.passed && !is_skip(r)).count();

    let txt = format_summary(&results, passed, skipped, failed);
    let json = format_summary_json(&results);

    let txt_path  = run_dir.join("summary.txt");
    let json_path = run_dir.join("summary.json");
    fs::write(&txt_path,  &txt).ok();
    fs::write(&json_path, &json).ok();

    println!();
    print!("{}", txt);
    println!("Logs: {}", run_dir.display());

    if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

// ── Tier execution ────────────────────────────────────────────────────────────

struct TierResult {
    id:       String,
    label:    String,
    passed:   bool,
    duration: Duration,
    log_path: PathBuf,
    note:     Option<String>,
}

impl TierResult {
    fn skipped(label: impl Into<String>, note: &str) -> Self {
        Self {
            id: "skip".into(),
            label: label.into(),
            passed: false,
            duration: Duration::ZERO,
            log_path: PathBuf::new(),
            note: Some(note.into()),
        }
    }
}

fn run_tier(run_dir: &Path, id: &str, label: &str, argv: &[&str]) -> TierResult {
    let log_path = run_dir.join(format!("{}.log", id));
    print!("[test-all] {:.<55} ", label);
    std::io::stdout().flush().ok();

    let t0 = Instant::now();
    let (passed, output) = capture_command(argv);
    let duration = t0.elapsed();

    // Write the log regardless of pass/fail.
    let header = format!(
        "# fjell test-all — tier: {}\n# command: {}\n# {}\n\n",
        label,
        argv.join(" "),
        format_duration(duration),
    );
    fs::write(&log_path, header + &output).ok();

    let status = if passed { "PASS" } else { "FAIL" };
    println!("{} ({:.1}s)", status, duration.as_secs_f32());

    TierResult { id: id.into(), label: label.into(), passed, duration, log_path, note: None }
}

fn capture_command(argv: &[&str]) -> (bool, String) {
    let (prog, rest) = match argv.split_first() {
        Some(pair) => pair,
        None => return (false, "empty command".into()),
    };

    let out = Command::new(prog)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match out {
        Ok(o) => {
            let combined = format!("{}{}", 
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr));
            (o.status.success(), combined)
        }
        Err(e) => (false, format!("spawn error: {}", e)),
    }
}

fn run_silent(argv: &[&str]) -> bool {
    let (prog, rest) = match argv.split_first() {
        Some(pair) => pair,
        None => return false,
    };
    Command::new(prog).args(rest)
        .stdout(Stdio::null()).stderr(Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_run_dir() -> PathBuf {
    let ts = timestamp_str();
    let dir = PathBuf::from("tests/runs").join(&ts);
    fs::create_dir_all(&dir).expect("cannot create run dir");
    dir
}

fn timestamp_str() -> String {
    // Use UNIX seconds so we never need chrono.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    // Format as YYYYMMDD-HHMMSS (approximate UTC from raw epoch).
    let s = secs % 86400;
    let d = secs / 86400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    // Days since epoch → approximate date (good enough for a log filename).
    // All arithmetic uses u64; no signed adjustment needed.
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
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", year, month, day, hh, mm, ss)
}

fn qemu_on_path() -> bool {
    Command::new("qemu-system-riscv64")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn format_duration(d: Duration) -> String {
    if d.as_secs() >= 60 {
        format!("{}m {:.0}s", d.as_secs() / 60, d.as_secs() % 60)
    } else {
        format!("{:.1}s", d.as_secs_f32())
    }
}

// ── Summary formatters ────────────────────────────────────────────────────────

fn format_summary(results: &[TierResult], passed: usize, skipped: usize, failed: usize) -> String {
    let total = results.len();
    let mut s = String::new();

    s.push_str("═══════════════════════════════════════════════════════════\n");
    s.push_str("                    FJELL TEST-ALL SUMMARY\n");
    s.push_str("═══════════════════════════════════════════════════════════\n");
    s.push('\n');

    // Column headers
    s.push_str(&format!("{:<5} {:<50} {:>6}  {}\n", "Tier", "Label", "Time", "Result"));
    s.push_str(&format!("{}\n", "─".repeat(75)));

    for (i, r) in results.iter().enumerate() {
        let result = if let Some(note) = &r.note {
            format!("SKIP ({})", note)
        } else if r.passed {
            "PASS".into()
        } else {
            "FAIL ←".into()
        };
        let time = if r.duration == Duration::ZERO {
            String::new()
        } else {
            format!("{:>5.1}s", r.duration.as_secs_f32())
        };
        s.push_str(&format!("{:<5} {:<50} {:>6}  {}\n",
            i + 1, &r.label[..r.label.len().min(49)], time, result));
    }

    s.push_str(&format!("{}\n", "─".repeat(75)));
    s.push_str(&format!(
        "Total: {}  |  PASS: {}  |  FAIL: {}  |  SKIP: {}\n",
        total, passed, failed, skipped
    ));
    s.push('\n');

    if failed == 0 {
        s.push_str("✓ ALL REQUIRED TIERS PASSED\n");
    } else {
        s.push_str("✗ FAILURES DETECTED — check per-tier logs above\n");
    }
    s
}

fn format_summary_json(results: &[TierResult]) -> String {
    let mut entries = Vec::new();
    for r in results {
        entries.push(format!(
            "  {{\"id\":{:?},\"label\":{:?},\"passed\":{},\"duration_ms\":{},\"log\":{:?}{}}}",
            r.id,
            r.label,
            r.passed,
            r.duration.as_millis(),
            r.log_path.display().to_string(),
            r.note.as_ref().map(|n| format!(",\"note\":{:?}", n)).unwrap_or_default(),
        ));
    }
    format!("[\n{}\n]\n", entries.join(",\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_str_has_correct_length() {
        let ts = timestamp_str();
        // YYYYMMDD-HHMMSS = 15 chars
        assert_eq!(ts.len(), 15, "timestamp: {}", ts);
        assert!(ts.chars().nth(8) == Some('-'));
    }

    #[test]
    fn format_duration_short() {
        assert!(format_duration(Duration::from_secs(5)).contains('s'));
    }

    #[test]
    fn format_duration_long() {
        let s = format_duration(Duration::from_secs(125));
        assert!(s.contains('m') && s.contains('s'));
    }

    #[test]
    fn format_summary_contains_headers() {
        let r = vec![
            TierResult {
                id: "01".into(), label: "host tests".into(),
                passed: true,
                duration: Duration::from_secs(3),
                log_path: PathBuf::from("01.log"),
                note: None,
            },
        ];
        let txt = format_summary(&r, 1, 0, 0);
        assert!(txt.contains("PASS"));
        assert!(txt.contains("host tests"));
        assert!(txt.contains("ALL REQUIRED TIERS PASSED"));
    }

    #[test]
    fn format_summary_shows_failures() {
        let r = vec![
            TierResult {
                id: "01".into(), label: "broken".into(),
                passed: false,
                duration: Duration::from_secs(1),
                log_path: PathBuf::from("01.log"),
                note: None,
            },
        ];
        let txt = format_summary(&r, 0, 0, 1);
        assert!(txt.contains("FAIL"));
        assert!(txt.contains("FAILURES DETECTED"));
    }

    #[test]
    fn format_summary_json_is_valid_array() {
        let r = vec![
            TierResult {
                id: "x".into(), label: "y".into(),
                passed: true, duration: Duration::ZERO,
                log_path: PathBuf::new(), note: None,
            },
        ];
        let j = format_summary_json(&r);
        assert!(j.starts_with('['));
        assert!(j.trim_end().ends_with(']'));
        assert!(j.contains("\"passed\":true"));
    }

    #[test]
    fn smoke_and_neg_constants_non_empty() {
        assert!(!SMOKE_PROFILES.is_empty());
        assert!(!NEG_CATEGORIES.is_empty());
    }
}
