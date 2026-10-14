//! # `fjell-repro-check`
//!
//! Verifies that Fjell release artefacts are bit-for-bit reproducible
//! (RFC-v0.10-003). Runs two genuinely independent builds — a scoped
//! `cargo clean` immediately precedes each one, so the second build cannot
//! reuse the first build's incremental output — and asserts every targeted
//! output file has an identical SHA-256 digest.
//!
//! RFC-0.30-001: before this line, there was no clean and no separate target
//! directory between the two builds, so the second build was an incremental
//! no-op and the "comparison" was a file against itself (0.4s per "build",
//! measured on 2026-09-09). Re-measured after this fix, on the same machine:
//! a genuinely independent build costs on the order of a few seconds (~2s at
//! 8 parallel jobs, ~3.4s at 2 — see rfcs/answers/RFC-0.30-001-*-answer.md §5),
//! and two of them were found to be bit-for-bit identical — this project's
//! same-machine build is, as measured, actually reproducible.
//!
//! RFC-0.35-003 (closes E-071): an unrecognised argument used to be
//! silently ignored and fell through to the full two-build mode — so
//! `fjell-repro-check -- --help`, asking what the flags are, **started a
//! build** instead of answering. It has happened twice (a prebuilt rebuild
//! in 0.33, and `--help` itself at RFC-0.34-004, killed by a broken pipe
//! beside a live `test-all`). An unrecognised argument now exits **2** with
//! the usage below — not 1, which is this tool's "the artefacts are not
//! reproducible"; `--help`/`-h` prints it and exits zero. Neither
//! starts a build. `--artefacts <dir>` was documented here and never
//! implemented — removed from this usage rather than carried forward as a
//! second instance of the same defect this line exists to close.
//!
//! Usage:
//!   `fjell-repro-check [--build-cmd <cmd>]`
//!   `fjell-repro-check --skip-build [--baseline <path>] [--record-baseline]`
//!   `fjell-repro-check --help | -h`
//!
//! With no `--skip-build`, runs the two-build check above. `--skip-build`
//! assumes the artefacts already exist and just reports whether the
//! current digests match a committed baseline — quick enough for CI.
//! `--build-cmd <cmd>` passes one extra argument to `cargo xtask build`
//! (two-build mode only). `--baseline <path>` names the baseline file
//! (`--skip-build` only; default `tests/repro/baseline-digests.txt`).
//! `--record-baseline` records a new baseline instead of comparing
//! (`--skip-build` only; refuses if one already exists — delete it first).

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

/// Default artefact glob patterns relative to workspace root.
/// Extended as new services are added.
const DEFAULT_TARGETS: &[&str] = &[
    "target/riscv64gc-unknown-none-elf/release/fjell-kernel",
    "crates/fjell-kernel/prebuilt", // directory: all *.bin inside
];

/// The one target triple `DEFAULT_TARGETS` is built for. Used to scope the
/// clean between the two builds to exactly the artefacts under test, leaving
/// the host build cache (this tool's own binary included) warm.
const TARGET_TRIPLE: &str = "riscv64gc-unknown-none-elf";

const USAGE: &str = "\
Usage:
  fjell-repro-check [--build-cmd <cmd>]
  fjell-repro-check --skip-build [--baseline <path>] [--record-baseline]
  fjell-repro-check --help | -h

With no --skip-build, runs two genuinely independent builds (a scoped
`cargo clean` before each) and compares every targeted artefact's SHA-256
digest.

  --build-cmd <cmd>    one extra argument passed to `cargo xtask build`
                       (two-build mode only)

--skip-build compares the artefacts already on disk against a committed
baseline, without building -- quick enough for CI.

  --baseline <path>    baseline file (default: tests/repro/baseline-digests.txt)
  --record-baseline    record a new baseline instead of comparing
                       (refuses if one already exists -- delete it first)

  --help, -h           print this usage and exit";

/// Standalone flags this tool recognises, taking no value.
const KNOWN_FLAGS: &[&str] = &["--skip-build", "--record-baseline", "--help", "-h"];
/// Flags this tool recognises that consume the following argument as a value.
const KNOWN_VALUE_FLAGS: &[&str] = &["--build-cmd", "--baseline"];

/// RFC-0.35-003 (closes E-071): the first unrecognised argument, or `None`
/// if every argument is a known flag or a value a known flag consumed.
/// Walks forward rather than using `.any()` per flag, so a value-flag's own
/// value (e.g. the `<cmd>` after `--build-cmd`) is never itself checked
/// against `KNOWN_FLAGS` and misread as an unrecognised flag.
fn first_unrecognized(args: &[String]) -> Option<&str> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if KNOWN_VALUE_FLAGS.contains(&a) {
            i += 2; // skip the flag and its value, present or not
            continue;
        }
        if KNOWN_FLAGS.contains(&a) {
            i += 1;
            continue;
        }
        return Some(a);
    }
    None
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Checked before validation: asking for help is always answerable,
    // whatever else is on the command line.
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    if let Some(bad) = first_unrecognized(&args) {
        eprintln!("fjell-repro-check: unrecognised argument: {bad}");
        eprintln!("{USAGE}");
        // **2, not 1** (corrected at review, 2026-10-14). `1` is already
        // this tool's "the artefacts are not reproducible" — the loudest
        // thing it can say. Returning it for a mistyped flag would make a
        // caller report a reproducibility failure that did not happen,
        // which is the same confusion between "could not ask" and "the
        // answer is no" that this line exists to remove.
        // `fjell-unsafe-audit` separates them this way already.
        return ExitCode::from(2);
    }

    let skip_build = args.iter().any(|a| a == "--skip-build");

    if skip_build {
        eprintln!("fjell-repro-check: --skip-build — checking existing digests");
        return check_existing_digests(&args);
    }

    eprintln!("fjell-repro-check: performing two-build reproducibility check");
    two_build_check(&args)
}

/// Name of the flag that opts in to recording a new baseline. Recording must
/// never be a side effect of an ordinary check run — see RFC-v0.21.3-001 §M4.
const RECORD_BASELINE_FLAG: &str = "--record-baseline";

// ── Two-build check ───────────────────────────────────────────────────────────

fn two_build_check(args: &[String]) -> ExitCode {
    // Collect extra xtask build args (e.g. feature flags)
    let extra: Vec<&str> = args
        .windows(2)
        .find(|w| w[0] == "--build-cmd")
        .map(|w| vec![w[1].as_str()])
        .unwrap_or_default();

    // Build #1 — a scoped clean immediately before, so this build cannot
    // reuse anything left over from a previous invocation either.
    if !clean_target() {
        eprintln!("fjell-repro-check: clean before build 1 failed");
        return ExitCode::FAILURE;
    }
    eprintln!("fjell-repro-check: build 1 / 2 …");
    let t0 = Instant::now();
    if !run_build(&extra) {
        eprintln!("fjell-repro-check: build 1 failed");
        return ExitCode::FAILURE;
    }
    let build_1_elapsed = t0.elapsed();
    let digests_1 = collect_digests();

    // Build #2 — cleaned again first. Without this, the second build is an
    // incremental no-op against build 1's own output: the defect this line
    // exists to fix (RFC-0.30-001).
    if !clean_target() {
        eprintln!("fjell-repro-check: clean before build 2 failed");
        return ExitCode::FAILURE;
    }
    eprintln!("fjell-repro-check: build 2 / 2 …");
    let t1 = Instant::now();
    if !run_build(&extra) {
        eprintln!("fjell-repro-check: build 2 failed");
        return ExitCode::FAILURE;
    }
    let build_2_elapsed = t1.elapsed();
    let digests_2 = collect_digests();

    eprintln!(
        "fjell-repro-check: build 1 took {:.2}s, build 2 took {:.2}s \
         (each preceded by `cargo clean --release --target {}` — \
         genuinely independent, not an incremental no-op)",
        build_1_elapsed.as_secs_f64(),
        build_2_elapsed.as_secs_f64(),
        TARGET_TRIPLE,
    );

    compare_and_report(digests_1, digests_2)
}

/// Removes this target triple's release artefacts only — not the whole
/// `target/` tree, which would also evict this tool's own host build cache
/// and every other crate's. Scoped so the two builds are independent of each
/// other without being independent of everything else on the machine.
fn clean_target() -> bool {
    Command::new("cargo")
        .args(["clean", "--release", "--target", TARGET_TRIPLE])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
    let baseline_path = args
        .windows(2)
        .find(|w| w[0] == "--baseline")
        .and_then(|w| w.get(1))
        .map(String::as_str)
        .unwrap_or("tests/repro/baseline-digests.txt");
    let record_baseline = args.iter().any(|a| a == RECORD_BASELINE_FLAG);

    let mut current = collect_digests();
    // The committed baseline only tracks committed artefacts (prebuilt/*.bin).
    // target/ outputs are volatile — absent in fresh checkouts and removed by
    // `cargo clean` — and are covered by the full two-build mode instead.
    current.retain(|k, _| !k.starts_with("target/"));

    // A missing baseline must fail closed, not silently record and pass —
    // otherwise this tier detects nothing (RFC-v0.21.3-001 §M4). Recording a
    // new baseline is only ever an explicit, opt-in action.
    if !Path::new(baseline_path).exists() {
        if record_baseline {
            return match save_digests(&current, baseline_path) {
                Ok(_) => {
                    eprintln!("fjell-repro-check: baseline recorded to {}", baseline_path);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("fjell-repro-check: {}", e);
                    ExitCode::FAILURE
                }
            };
        }
        eprintln!(
            "fjell-repro-check: FAIL — no baseline at {} (this is not a passing state).",
            baseline_path
        );
        eprintln!(
            "  To record one deliberately: cargo run -p fjell-repro-check -- \
             --skip-build {} [--baseline <path>]",
            RECORD_BASELINE_FLAG
        );
        return ExitCode::FAILURE;
    }

    if record_baseline {
        eprintln!(
            "fjell-repro-check: {} has no effect when a baseline already exists at {} \
             — delete it first if you intend to re-record.",
            RECORD_BASELINE_FLAG, baseline_path
        );
        return ExitCode::FAILURE;
    }

    // RFC-0.30-003 D1: read before `load_digests` strips comment lines —
    // this is the attribution Finding 5 says never existed ("the detector
    // fired and could say nothing about why").
    let baseline_toolchain = read_recorded_toolchain(baseline_path);

    let baseline = match load_digests(baseline_path) {
        Ok(mut d) => {
            // Tolerate legacy baselines that recorded target/ entries.
            d.retain(|k, _| !k.starts_with("target/"));
            d
        }
        Err(e) => {
            eprintln!("fjell-repro-check: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let result = compare_and_report(baseline, current);
    if result != ExitCode::SUCCESS {
        report_toolchain_context(baseline_toolchain.as_deref());
    }
    result
}

/// On a mismatch, say whether it might be toolchain drift rather than a
/// real reproducibility failure. Digests differing for the same committed
/// files were, before this line, unattributable — this does not resolve
/// that ambiguity (this project makes no cross-machine reproducibility
/// claim; see RFC-0.30-001's residual), it only makes it visible.
fn report_toolchain_context(baseline_toolchain: Option<&str>) {
    let observed = fjell_consistency_check::toolchain::observe();
    match (baseline_toolchain, &observed) {
        (Some(recorded), Some(current)) if recorded != current.format_line() => {
            eprintln!(
                "\nfjell-repro-check: NOTE — the baseline was recorded under a \
                 different toolchain than this check just ran under:\n  \
                 baseline : {recorded}\n  \
                 this run : {}\n  \
                 This mismatch may be toolchain drift, not a real reproducibility \
                 failure — confirm the toolchains agree before treating it as a bug.",
                current.format_line()
            );
        }
        (None, _) => {
            eprintln!(
                "\nfjell-repro-check: NOTE — this baseline has no recorded toolchain \
                 (predates RFC-0.30-003), so toolchain drift cannot be ruled out or in \
                 as an explanation for the mismatch above."
            );
        }
        _ => {}
    }
}

/// Reads the `# toolchain: ...` comment line `save_digests` writes,
/// without going through `load_digests` (which discards comments).
fn read_recorded_toolchain(path: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    content
        .lines()
        .find_map(|l| l.strip_prefix("# toolchain: ").map(str::to_string))
}

// ── Digest collection ─────────────────────────────────────────────────────────

fn collect_digests() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for target in DEFAULT_TARGETS {
        let path = Path::new(target);
        if path.is_dir() {
            // Collect all *.bin files in the directory
            if let Ok(entries) = fs::read_dir(path) {
                let mut paths: Vec<PathBuf> = entries
                    .flatten()
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
    Some(sha256_hex(&buf))
}

/// SHA-256 of the file contents, lowercase hex.
///
/// Switched from FNV-1a in RFC-v0.16-005 (architect review H-04): the gate's
/// purpose is change detection, but a cryptographic digest avoids the
/// external confusion of a non-cryptographic hash appearing in a trust-
/// oriented project's tooling, at negligible cost.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ── Comparison and report ─────────────────────────────────────────────────────

fn compare_and_report(a: BTreeMap<String, String>, b: BTreeMap<String, String>) -> ExitCode {
    let mut mismatches: Vec<String> = Vec::new();
    let mut present = 0usize;

    for (path, digest_a) in &a {
        present += 1;
        match b.get(path) {
            None => mismatches.push(format!("MISSING in build 2: {}", path)),
            Some(digest_b) if digest_a != digest_b => {
                let da = &digest_a[..digest_a.len().min(8)];
                let db = &digest_b[..digest_b.len().min(8)];
                mismatches.push(format!(
                    "DIGEST DIFFERS: {}\n  build1={}\n  build2={}",
                    path, da, db
                ));
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
        eprintln!(
            "fjell-repro-check: FAIL — {} mismatch(es):",
            mismatches.len()
        );
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
    // RFC-0.30-003 D1/D2: record what actually produced these digests, not
    // what `rust-toolchain.toml` says should have. Observation failure is a
    // hard error here — a baseline recorded with no toolchain line at all
    // is an honest gap this project has already lived with; one recorded
    // with a guessed value would not be.
    let Some(observed) = fjell_consistency_check::toolchain::observe() else {
        return Err(std::io::Error::other(
            "cannot observe the toolchain (`rustc -vV` failed) — refusing to record a baseline \
             with no toolchain line rather than one with a guessed value",
        ));
    };
    let mut out = String::from("# algo: sha256 (64 hex chars per entry)\n");
    out.push_str(&format!("# toolchain: {}\n", observed.format_line()));
    for (k, v) in map {
        out.push_str(&format!("{} {}\n", v, k));
    }
    fs::write(path, out)
}

fn load_digests(path: &str) -> std::io::Result<BTreeMap<String, String>> {
    let content = fs::read_to_string(path)?;
    let mut map = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue; // header / comment
        }
        let mut parts = line.splitn(2, ' ');
        if let (Some(digest), Some(file)) = (parts.next(), parts.next()) {
            // Guard against algorithm drift: the tool computes SHA-256
            // (64 hex chars). A 16-char entry is a legacy FNV-1a baseline
            // from before RFC-v0.16-005 (H-04) — comparing across
            // algorithms produces meaningless per-file "mismatches", so
            // fail loudly with the actual cause instead.
            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "baseline {} contains a non-SHA-256 digest ({} chars) for {} — \
                         this is a legacy (pre-RFC-v0.16-005 FNV-1a) baseline. \
                         Delete the file and re-run with --skip-build to re-record \
                         it with SHA-256.",
                        path,
                        digest.len(),
                        file
                    ),
                ));
            }
            map.insert(file.to_string(), digest.to_string());
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_is_recognised() {
        assert_eq!(first_unrecognized(&strs(&[])), None);
    }

    #[test]
    fn every_known_standalone_flag_is_recognised() {
        for f in KNOWN_FLAGS {
            assert_eq!(first_unrecognized(&strs(&[f])), None, "{f}");
        }
    }

    #[test]
    fn a_value_flag_s_own_value_is_not_checked_against_known_flags() {
        // The value happens to look like a flag (`--skip-build`) -- it must
        // still be consumed as --build-cmd's value, not flagged itself.
        assert_eq!(
            first_unrecognized(&strs(&["--build-cmd", "--skip-build"])),
            None
        );
    }

    #[test]
    fn combining_recognised_flags_is_fine() {
        assert_eq!(
            first_unrecognized(&strs(&[
                "--skip-build",
                "--baseline",
                "x.txt",
                "--record-baseline"
            ])),
            None
        );
    }

    #[test]
    fn an_unrecognised_flag_is_named() {
        assert_eq!(first_unrecognized(&strs(&["--bogus"])), Some("--bogus"));
    }

    #[test]
    fn an_unrecognised_flag_after_recognised_ones_is_still_found() {
        assert_eq!(
            first_unrecognized(&strs(&["--skip-build", "--bogus"])),
            Some("--bogus")
        );
    }

    #[test]
    fn removed_artefacts_flag_is_now_unrecognised() {
        // RFC-0.35-003: --artefacts was documented, never implemented, and
        // is not carried forward as a second instance of the same defect.
        assert_eq!(
            first_unrecognized(&strs(&["--artefacts", "/tmp"])),
            Some("--artefacts")
        );
    }

    #[test]
    fn sha256_deterministic() {
        assert_eq!(sha256_hex(b"hello"), sha256_hex(b"hello"));
        assert_ne!(sha256_hex(b"hello"), sha256_hex(b"world"));
        // Known SHA-256 of "hello"
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
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
        map.insert("a.bin".into(), sha256_hex(b"a"));
        save_digests(&map, path.to_str().unwrap()).unwrap();
        let loaded = load_digests(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded, map);
    }

    #[test]
    fn legacy_fnv_baseline_rejected_loudly() {
        let dir = std::env::temp_dir().join("fjell-repro-test");
        fs::create_dir_all(&dir).ok();
        let path = dir.join("legacy.txt");
        // 16-hex-char FNV-1a style entry (pre-RFC-v0.16-005 baseline format).
        fs::write(&path, "ef78d19e980473e6 crates/x.bin\n").unwrap();
        let err = load_digests(path.to_str().unwrap()).unwrap_err();
        assert!(
            err.to_string().contains("legacy"),
            "must name the real cause"
        );
    }
}
