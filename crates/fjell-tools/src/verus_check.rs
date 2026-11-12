//! `cargo xtask verus-check` — RFC-v0.17-005.
//!
//! Stable project-local interface for Verus proof targets. Reads
//! `verification/verus/verus-targets.toml`. If the `verus` binary is on PATH
//! it runs the proof; otherwise it falls back to the target's Rust
//! conformance test and reports CONFORMANCE-ONLY (Stage A: proofs are additive,
//! never a blocker, while Experimental). Once a target is promoted to
//! `release_required = true` (RFC-v0.18-001), anything other than a real Verus
//! PASS blocks `--release-required` — including CONFORMANCE-ONLY, since a
//! release-required proof cannot be certified without running the prover.
//!
//! Two toolchain pins are compared against `verification/verus/TOOLCHAIN.lock` before any
//! proof runs (RFC-0.43-001): the **Verus release** (`[verus] release`) and the **z3
//! solver** (`[z3] version`). They are reported separately and refuse separately, because
//! they are different components with different remedies -- see [`z3_refusal`] and
//! [`verus_mismatch_refusal`]. **Both block in every mode** (Amendment 1, R8).
//!
//! **`--candidate` is the one way to run the proofs with a pin mismatched** (R7): a new
//! prover bundles a new z3, so at an upgrade no ordinary run can happen until the lock
//! moves, and the lock moves only behind a recorded proof-review re-run, which needs the
//! proofs run. A candidate run does that and **certifies nothing**: its markers are
//! `CANDIDATE-ONLY` / `CANDIDATE-FAIL`, never `MACHINE-CHECKED-PASS` (which Gate 10 counts
//! and CI retains), it states its own status and both pairs of versions, and
//! `--release-required` refuses it.
//!
//! Output markers (one per target, architect C7 — conformance never reports
//! a bare PASS, and the JSON distinguishes machine_check pass/fail/not_run):
//!   VERUS:TARGET:<name>:MACHINE-CHECKED-PASS
//!   VERUS:TARGET:<name>:MACHINE-CHECKED-FAIL
//!   VERUS:TARGET:<name>:CONFORMANCE-ONLY      (verus absent, conformance ok)
//!   VERUS:TARGET:<name>:CONFORMANCE-FAIL      (verus absent, conformance failed)
//!   VERUS:TARGET:<name>:CANDIDATE-ONLY        (--candidate: proofs ran and passed; certifies nothing)
//!   VERUS:TARGET:<name>:CANDIDATE-FAIL        (--candidate: a proof failed)
//!
//! Plus a JSON summary line per target for CI.

use std::process::{Command, ExitCode};

struct Target {
    name: String,
    proof: String,
    tier: u8,
    release_required: bool,
    conformance_cmd: String,
}

/// Run `verus --version` and return the version string, e.g.
/// "0.2026.05.24.ecee80a".
fn detect_verus_version() -> Option<String> {
    let out = std::process::Command::new("verus")
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    // Expected output:
    //   Verus
    //     Version: 0.2026.05.24.ecee80a
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Version:") {
            return Some(rest.trim().to_string());
        }
        // Also accept bare version tokens that look like a date-based version
        if trimmed.starts_with("release/")
            || trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
        {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Read the `release` field from TOOLCHAIN.lock [verus] section.
fn read_lock_release(path: &str) -> Option<String> {
    let src = std::fs::read_to_string(path).ok()?;
    for line in src.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("release") {
            if let Some(val) = rest
                .trim_start_matches(|c: char| c == ' ' || c == '=')
                .strip_prefix('"')
            {
                return Some(val.trim_end_matches('"').to_string());
            }
        }
    }
    None
}

// ── The solver pin (RFC-0.43-001, E-112) ─────────────────────────────────────

/// Why the solver's identity could not be established. Every variant **refuses**: a
/// check that cannot tell which solver will run has not checked the pin, and "assume it
/// matches" is the defect this exists to remove.
#[derive(Debug, PartialEq, Eq)]
enum Z3Problem {
    /// No `verus` executable on `PATH` to find a solver beside.
    NoProver,
    /// The located solver does not exist or could not be started.
    CannotRun { path: String, why: String },
    /// It ran, but its `--version` output is not the shape this check recognises.
    /// `output` is the exact text found, so the refusal can show it.
    UnrecognisedFormat { path: String, output: String },
    /// The lock has no readable `[z3] version`.
    LockUnreadable,
}

/// Where the solver path came from -- printed, so a reader can see which rule applied.
#[derive(Debug, PartialEq, Eq, Clone)]
enum Z3Source {
    /// `VERUS_Z3_PATH`: the prover's own override, which it consults first.
    EnvOverride,
    /// `z3` in the directory of the (symlink-resolved) `verus` that is on `PATH`.
    BesideProver,
}

impl Z3Source {
    fn describe(&self) -> &'static str {
        match self {
            Z3Source::EnvOverride => "from VERUS_Z3_PATH",
            Z3Source::BesideProver => "beside the prover",
        }
    }
}

/// Strictly parse z3's `--version` output: exactly `Z3 version <N.N.N[.N]> - <32|64> bit`.
///
/// Anything else -- a different banner, a missing version, extra lines, a version with a
/// non-numeric part -- is `None`, and the caller refuses. Not a prefix match: a loose
/// parser here would let a string it did not understand compare as "fine".
fn parse_z3_version(output: &str) -> Option<String> {
    let line = output.trim();
    if line.contains('\n') {
        return None;
    }
    let rest = line.strip_prefix("Z3 version ")?;
    let (ver, tail) = rest.split_once(' ')?;
    if tail != "- 64 bit" && tail != "- 32 bit" {
        return None;
    }
    let parts: Vec<&str> = ver.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if !(3..=4).contains(&parts.len()) || !parts.iter().all(numeric) {
        return None;
    }
    Some(ver.to_string())
}

/// Read `version` from the lock's `[z3]` section, and only from there.
///
/// Strict: the value must be a double-quoted, non-empty string; a trailing `# comment`
/// after the closing quote is allowed. A missing section, key or quote is `None`.
fn read_lock_z3_version(lock_text: &str) -> Option<String> {
    let mut in_z3 = false;
    for raw in lock_text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_z3 = line.starts_with("[z3]");
            continue;
        }
        if !in_z3 {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "version" {
            continue;
        }
        let val = val.trim().strip_prefix('"')?;
        let (inner, after) = val.split_once('"')?;
        let after = after.trim();
        if inner.is_empty() || !(after.is_empty() || after.starts_with('#')) {
            return None;
        }
        return Some(inner.to_string());
    }
    None
}

/// The first executable called `name` in `path_var`'s directories, as `PATH` lookup does.
fn find_on_path(name: &str, path_var: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(path_var)
        .map(|d| d.join(name))
        .find(|p| {
            std::fs::metadata(p)
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        })
}

/// Which `z3` the prover is going to run, by the prover's own rule: `VERUS_Z3_PATH` if it
/// is set, otherwise `z3` beside the real (symlink-resolved) `verus` executable. Taking
/// the path from the lock instead is what `[z3] path` did, and why it was fiction.
fn locate_z3(
    env_override: Option<std::ffi::OsString>,
    path_var: &std::ffi::OsStr,
) -> Result<(std::path::PathBuf, Z3Source), Z3Problem> {
    if let Some(p) = env_override {
        return Ok((std::path::PathBuf::from(p), Z3Source::EnvOverride));
    }
    let verus = find_on_path("verus", path_var).ok_or(Z3Problem::NoProver)?;
    let real = std::fs::canonicalize(&verus).map_err(|e| Z3Problem::CannotRun {
        path: verus.display().to_string(),
        why: format!("cannot resolve the prover's real location: {e}"),
    })?;
    let dir = real.parent().ok_or(Z3Problem::NoProver)?;
    Ok((dir.join("z3"), Z3Source::BesideProver))
}

/// Run the located solver's `--version` and parse it strictly.
fn detect_z3_version(path: &std::path::Path) -> Result<String, Z3Problem> {
    let shown = path.display().to_string();
    let out = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|e| Z3Problem::CannotRun {
            path: shown.clone(),
            why: e.to_string(),
        })?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        return Err(Z3Problem::CannotRun {
            path: shown,
            why: format!("`--version` exited with {}", out.status),
        });
    }
    parse_z3_version(&text).ok_or(Z3Problem::UnrecognisedFormat {
        path: shown,
        output: text.trim().to_string(),
    })
}

/// The refusal for a solver that is not the pinned one, or whose identity could not be
/// established. It says **which component** (the solver, not the prover release), **both
/// versions or what was found instead**, **what to do**, and **that an ordinary build is
/// unaffected** -- a contributor who needs Verus for nothing must be able to read it and
/// stop worrying (RFC-0.43-001 R6).
fn z3_refusal(problem: &Z3Problem, pinned: Option<&str>) -> String {
    let what = match problem {
        Z3Problem::NoProver => {
            "no `verus` executable found on PATH, so there is no solver to identify".to_string()
        }
        Z3Problem::CannotRun { path, why } => format!("could not run `{path} --version`: {why}"),
        Z3Problem::UnrecognisedFormat { path, output } => format!(
            "`{path} --version` printed {output:?}, which is not the expected form \
            `Z3 version <N.N.N> - 64 bit`, so its version cannot be established"
        ),
        Z3Problem::LockUnreadable => "TOOLCHAIN.lock has no readable `[z3] version`".to_string(),
    };
    format!(
        "verus-check: BLOCKING — z3 SOLVER pin not established (this is the solver, \
        not the Verus release). Pinned z3: {}. {what}.\n\
        verus-check:   z3 decides whether the proofs verify, so an unidentified solver is \
        an uncertified result. Use the Verus release TOOLCHAIN.lock pins (it bundles its \
        z3); `--candidate` also needs the solver's version, to be able to name it.\n\
        verus-check:   Not affected: `cargo xtask build` and every host test tier never \
        need Verus or z3 (verification/verus/TOOLCHAIN.md).",
        pinned.unwrap_or("(unreadable)")
    )
}

/// The refusal for a solver that ran and reported a version different from the pin.
fn z3_mismatch_refusal(detected: &str, pinned: &str, path: &str, source: &Z3Source) -> String {
    format!(
        "verus-check: BLOCKING — z3 SOLVER version mismatch (this is the solver, not the \
        Verus release). TOOLCHAIN.lock [z3] pins {pinned}; the prover's solver ({path}, {}) \
        reports {detected}.\n\
        verus-check:   z3 decides whether the proofs verify, so the proofs were certified \
        under {pinned}, not {detected}. Use the Verus release TOOLCHAIN.lock pins (it \
        bundles its z3), or -- to run the proofs for a recorded proof-review re-run before \
        moving the pin -- `cargo xtask verus-check --candidate`, which certifies nothing. \
        The lock moves after that re-run, not before.\n\
        verus-check:   Not affected: `cargo xtask build` and every host test tier never \
        need Verus or z3 (verification/verus/TOOLCHAIN.md).",
        source.describe()
    )
}

/// The z3 solver as detected and as pinned, with where it was found.
#[derive(Debug, PartialEq, Eq)]
struct Z3Reading {
    detected: String,
    pinned: String,
    path: String,
    source: Z3Source,
}

/// Establish the solver's identity: the lock's pin, the located solver, its parsed version.
/// `Err` is the refusal text for anything that stops that being established -- a mismatch
/// is **not** an error here: it is a reading, and the caller decides (an ordinary run
/// refuses it, a candidate run reports it). Pure over its inputs -- the lock text, the
/// environment override and the `PATH` -- so it is testable without a prover.
fn read_z3_pin(
    lock_text: Option<&str>,
    env_override: Option<std::ffi::OsString>,
    path_var: &std::ffi::OsStr,
) -> Result<Z3Reading, String> {
    let pinned = lock_text.and_then(read_lock_z3_version);
    let Some(pinned_v) = pinned.as_deref() else {
        return Err(z3_refusal(&Z3Problem::LockUnreadable, None));
    };
    let (path, source) =
        locate_z3(env_override, path_var).map_err(|p| z3_refusal(&p, Some(pinned_v)))?;
    let detected = detect_z3_version(&path).map_err(|p| z3_refusal(&p, Some(pinned_v)))?;
    Ok(Z3Reading {
        detected,
        pinned: pinned_v.to_string(),
        path: path.display().to_string(),
        source,
    })
}

/// The solver check for an ordinary or release-required run: `Ok(report)` to proceed,
/// `Err(text)` to refuse (print it, exit non-zero).
fn check_z3_pin(
    lock_text: Option<&str>,
    env_override: Option<std::ffi::OsString>,
    path_var: &std::ffi::OsStr,
) -> Result<String, String> {
    let r = read_z3_pin(lock_text, env_override, path_var)?;
    if r.detected != r.pinned {
        return Err(z3_mismatch_refusal(
            &r.detected,
            &r.pinned,
            &r.path,
            &r.source,
        ));
    }
    Ok(format!(
        "verus-check: detected z3 version: {} ({}, {})\n\
        verus-check: pinned  z3 version: {} (TOOLCHAIN.lock [z3] version)",
        r.detected,
        r.path,
        r.source.describe(),
        r.pinned
    ))
}

// ── Invocation, the two pin readings, and the candidate mode (Amendment 1) ───

/// Which targets to run.
#[derive(Debug, PartialEq, Eq)]
enum Selection {
    AllPilot,
    ReleaseRequired,
    Target(String),
}

/// A parsed command line: the selection, and whether this is a candidate run.
#[derive(Debug, PartialEq, Eq)]
struct Invocation {
    candidate: bool,
    selection: Selection,
}

/// Parse `args`. `--candidate` may appear anywhere, once; at most one selector remains
/// (`--all-pilot`, `--release-required` or a target name; none means `--all-pilot`).
/// `--candidate` with `--release-required` is refused: a candidate run certifies nothing
/// and a release-required run exists to certify. Anything else is refused rather than
/// ignored -- an extra argument that is silently dropped is how a flag stops meaning
/// what its author thought.
fn parse_invocation(args: &[String]) -> Result<Invocation, String> {
    let mut candidate = false;
    let mut selectors: Vec<&str> = Vec::new();
    for a in args {
        if a == "--candidate" {
            if candidate {
                return Err("`--candidate` given twice".into());
            }
            candidate = true;
        } else {
            selectors.push(a);
        }
    }
    if selectors.len() > 1 {
        return Err(format!(
            "more than one target selector ({}): give at most one of --all-pilot, \
            --release-required or a target name",
            selectors.join(", ")
        ));
    }
    let selection = match selectors.first().copied() {
        None | Some("--all-pilot") => Selection::AllPilot,
        Some("--release-required") => Selection::ReleaseRequired,
        Some(name) => Selection::Target(name.to_string()),
    };
    if candidate && selection == Selection::ReleaseRequired {
        return Err(
            "`--candidate` cannot be combined with `--release-required`: a candidate \
            run certifies nothing, and a release-required run exists to certify. Run the \
            candidate, record the re-run, move the pin in TOOLCHAIN.lock, then run \
            `--release-required` on its own."
                .into(),
        );
    }
    Ok(Invocation {
        candidate,
        selection,
    })
}

/// The Verus release as detected and as pinned. Compared with the `release/` prefix
/// stripped from both (`TOOLCHAIN.lock` stores `release/X.Y.Z`; the binary reports `X.Y.Z`).
#[derive(Debug, PartialEq, Eq)]
struct VerusReading {
    detected: String,
    pinned: String,
}

impl VerusReading {
    fn matches(&self) -> bool {
        let bare = |v: &str| v.strip_prefix("release/").unwrap_or(v).to_string();
        bare(&self.detected) == bare(&self.pinned)
    }
}

/// Establish the Verus release identity, or say why it cannot be established. Unlike the
/// z3 reading this never "proceeds with unknown": an unparseable detected version or an
/// unreadable lock is a refusal in every mode (it used to be one only under
/// `--release-required`).
fn read_verus_pin(detected: Option<&str>, pinned: Option<&str>) -> Result<VerusReading, String> {
    match (detected, pinned) {
        (Some(d), Some(p)) => Ok(VerusReading {
            detected: d.to_string(),
            pinned: p.to_string(),
        }),
        _ => Err(format!(
            "verus-check: BLOCKING — Verus RELEASE identity not established (this is the \
            prover release, not the z3 solver). Detected: {}. Pinned: {}.\n\
            verus-check:   A run that cannot tell which prover it used certifies nothing. \
            Fix the installation or TOOLCHAIN.lock; `--candidate` needs both versions too, \
            to be able to name them.\n\
            verus-check:   Not affected: `cargo xtask build` and every host test tier never \
            need Verus or z3 (verification/verus/TOOLCHAIN.md).",
            detected.unwrap_or("unknown"),
            pinned.unwrap_or("(unreadable)"),
        )),
    }
}

/// The refusal for a prover release that is not the pinned one. Names the component (the
/// prover release, not the solver), both versions, the way to run the proofs anyway for a
/// recorded re-run, and that an ordinary build is unaffected.
fn verus_mismatch_refusal(r: &VerusReading) -> String {
    format!(
        "verus-check: BLOCKING — Verus RELEASE mismatch (this is the prover release, not the \
        z3 solver). TOOLCHAIN.lock [verus] pins {}; the installed prover reports {}.\n\
        verus-check:   the proofs were certified under {}, not {}. Use the pinned release, \
        or -- to run the proofs for a recorded proof-review re-run before moving the pin -- \
        `cargo xtask verus-check --candidate`, which certifies nothing.\n\
        verus-check:   Not affected: `cargo xtask build` and every host test tier never \
        need Verus or z3 (verification/verus/TOOLCHAIN.md).",
        r.pinned, r.detected, r.pinned, r.detected
    )
}

/// The statement a candidate run makes about itself, before any proof runs: that it
/// certifies nothing, both detected versions, both pins, and which pin (if any) differs.
fn candidate_notice(v: &VerusReading, z: &Z3Reading) -> String {
    let word = |m: bool| if m { "matches" } else { "DIFFERS" };
    let zm = z.detected == z.pinned;
    let mut out = format!(
        "verus-check: CANDIDATE RUN -- this run certifies nothing.\n\
        verus-check:   Verus release: detected {}, pinned {} ({})\n\
        verus-check:   z3 solver:     detected {}, pinned {} ({})\n\
        verus-check:   The proofs are run for a recorded proof-review re-run \
        (TOOLCHAIN.lock: \"Update only with a recorded proof-review re-run\"). Markers below \
        are CANDIDATE-ONLY / CANDIDATE-FAIL, never the machine-checked pass marker, and \
        `--release-required` refuses this mode. To certify: record the re-run, move the \
        pin(s) in TOOLCHAIN.lock, then run `verus-check --release-required` on its own.",
        v.detected,
        v.pinned,
        word(v.matches()),
        z.detected,
        z.pinned,
        word(zm),
    );
    if v.matches() && zm {
        out.push_str(
            "\nverus-check:   (both pins match: an ordinary run would certify this; \
            --candidate is for when they do not.)",
        );
    }
    out
}

/// `(status, marker, machine_check)` for one target that ran under the prover. A candidate
/// run **never** produces `MACHINE-CHECKED-PASS`, whatever the proof did: Gate 10 counts
/// that exact string and CI retains the marker log, so a candidate run that emitted it
/// would leave a certified-looking line in the record with nothing to tell it apart.
fn prover_outcome(candidate: bool, passed: bool) -> (&'static str, &'static str, &'static str) {
    match (candidate, passed) {
        (false, true) => ("machine_checked_pass", "MACHINE-CHECKED-PASS", "pass"),
        (false, false) => ("machine_checked_fail", "MACHINE-CHECKED-FAIL", "fail"),
        (true, true) => ("candidate_only", "CANDIDATE-ONLY", "candidate_pass"),
        (true, false) => ("candidate_fail", "CANDIDATE-FAIL", "candidate_fail"),
    }
}

pub fn cmd_verus_check(args: &[String]) -> ExitCode {
    let inv = match parse_invocation(args) {
        Ok(i) => i,
        Err(e) => {
            println!("verus-check: BLOCKING — {e}");
            return ExitCode::FAILURE;
        }
    };

    let targets = match load_targets("verification/verus/verus-targets.toml") {
        Ok(t) => t,
        Err(e) => {
            eprintln!("verus-check: {e}");
            return ExitCode::FAILURE;
        }
    };

    let verus_present = which("verus");

    // Architect follow-up (v0.18 review §8.3.3): print the detected Verus and z3
    // versions and compare each against its TOOLCHAIN.lock pin.
    //
    //   * Verus release (`[verus] release`) and z3 solver (`[z3] version`,
    //     RFC-0.43-001): a mismatch -- or a version that cannot be established --
    //     BLOCKS, in EVERY mode (Amendment 1, R8: the Verus comparison used to warn
    //     outside `--release-required`, which let an ordinary run print
    //     `MACHINE-CHECKED-PASS` under a prover the proofs were not certified with).
    //   * `--candidate` (R7) is the only way past a mismatch: it runs the proofs for a
    //     recorded re-run, states that it certifies nothing, names both pairs of versions,
    //     and never emits `MACHINE-CHECKED-PASS`.
    //
    // The z3 figure is the solver binary's own `--version`, located as the prover
    // locates it; the lock's old `[z3] path` field is gone (it was only ever right for
    // one install route).
    if inv.candidate && !verus_present {
        println!(
            "verus-check: BLOCKING — `--candidate` needs the prover: there is nothing to run \
            a candidate proof with, and the conformance fallback certifies no re-run."
        );
        return ExitCode::FAILURE;
    }
    let mut candidate_notice_text: Option<String> = None;
    if verus_present {
        let detected = detect_verus_version();
        let pinned = read_lock_release("verification/verus/TOOLCHAIN.lock");
        println!(
            "verus-check: detected version: {}",
            detected.as_deref().unwrap_or("unknown")
        );
        println!(
            "verus-check: pinned  version: {}",
            pinned.as_deref().unwrap_or("(unreadable)")
        );
        let verus_reading = match read_verus_pin(detected.as_deref(), pinned.as_deref()) {
            Ok(r) => r,
            Err(refusal) => {
                println!("{refusal}");
                return ExitCode::FAILURE;
            }
        };
        if !inv.candidate && !verus_reading.matches() {
            println!("{}", verus_mismatch_refusal(&verus_reading));
            return ExitCode::FAILURE;
        }

        // The solver pin, after the Verus comparison so that a Verus mismatch is reported
        // as that and not as a solver problem; the two refusals name different components.
        let lock_text = std::fs::read_to_string("verification/verus/TOOLCHAIN.lock").ok();
        let path_var = std::env::var_os("PATH").unwrap_or_default();
        let z3_env = std::env::var_os("VERUS_Z3_PATH");
        if inv.candidate {
            match read_z3_pin(lock_text.as_deref(), z3_env, &path_var) {
                Ok(z) => {
                    candidate_notice_text = Some(candidate_notice(&verus_reading, &z));
                }
                Err(refusal) => {
                    println!("{refusal}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            match check_z3_pin(lock_text.as_deref(), z3_env, &path_var) {
                Ok(report) => println!("{report}"),
                Err(refusal) => {
                    println!("{refusal}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    if let Some(n) = &candidate_notice_text {
        println!("{n}");
    }

    // Selection
    let selected: Vec<&Target> = match &inv.selection {
        Selection::AllPilot => targets.iter().collect(),
        Selection::ReleaseRequired => targets.iter().filter(|t| t.release_required).collect(),
        Selection::Target(name) => {
            let hit: Vec<&Target> = targets.iter().filter(|t| &t.name == name).collect();
            if hit.is_empty() {
                eprintln!(
                    "verus-check: unknown target `{name}` \
                    (known: {})",
                    targets
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                return ExitCode::FAILURE; // fail fast on unknown target
            }
            hit
        }
    };

    if !verus_present {
        eprintln!(
            "verus-check: `verus` not on PATH — conformance-only mode \
            (see verification/verus/TOOLCHAIN.md)"
        );
    }

    let mut any_blocking_fail = false;
    let mut any_candidate_fail = false;
    let mut ran = 0usize;

    for t in selected {
        // Architect C7: status values distinguish a real machine-check from
        // the conformance fallback; conformance never reports a bare PASS.
        let (status, marker, machine_check) = if verus_present {
            ran += 1;
            let passed = run_verus(t);
            if inv.candidate && !passed {
                any_candidate_fail = true;
            }
            prover_outcome(inv.candidate, passed)
        } else {
            let ok = run_cmd(&t.conformance_cmd);
            if ok {
                ("not_run_conformance_pass", "CONFORMANCE-ONLY", "not_run")
            } else {
                ("not_run_conformance_fail", "CONFORMANCE-FAIL", "not_run")
            }
        };

        println!("VERUS:TARGET:{}:{}", t.name, marker);
        println!(
            "{{\"target\":\"{}\",\"machine_check\":\"{}\",\"status\":\"{}\",\"tier\":{},\"experimental\":{},\"release_required\":{}}}",
            t.name, machine_check, status, t.tier, !t.release_required, t.release_required
        );

        // Release-required targets must be PROVED for a release: only a real
        // machine-checked pass clears the gate. A FAIL *and* the not_run
        // fallback (prover absent) both block — you cannot certify a
        // release-required proof without actually running Verus. Experimental
        // targets never block (Stage A philosophy). A candidate run is not a
        // release-required run and certifies nothing, so this does not apply to it.
        if !inv.candidate && t.release_required && machine_check != "pass" {
            any_blocking_fail = true;
        }
    }

    if inv.candidate {
        if any_candidate_fail {
            println!(
                "VERUS-CHECK: CANDIDATE RUN FAILED -- a proof did not verify under the candidate \
                toolchain (certifies nothing either way)"
            );
            return ExitCode::FAILURE;
        }
        println!(
            "VERUS-CHECK: candidate run complete -- {ran} target(s) verified under the candidate \
            toolchain; this certifies nothing. Record the re-run, then move the pin(s)."
        );
        return ExitCode::SUCCESS;
    }
    if any_blocking_fail {
        println!("VERUS-CHECK: BLOCKING FAILURE (release-required target not proved)");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_verus(t: &Target) -> bool {
    // Proof modules are library files (no `main`); Verus needs --crate-type=lib.
    Command::new("verus")
        .arg("--crate-type=lib")
        .arg(&t.proof)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_cmd(cmd: &str) -> bool {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }
    Command::new(parts[0])
        .args(&parts[1..])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn which(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Minimal TOML reader for the `[[target]]` array — avoids a new dependency.
fn load_targets(path: &str) -> Result<Vec<Target>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut targets = Vec::new();
    let mut cur: Option<Target> = None;

    let field = |line: &str| -> Option<String> {
        line.split_once('=').map(|(_, v)| {
            // Strip an inline `# ...` comment. Values here are bare scalars or
            // double-quoted strings with no '#', so cutting at the first '#'
            // outside quotes is safe.
            let v = match v.split_once(" #") {
                Some((head, _)) if !head.contains('"') || head.matches('"').count() % 2 == 0 => {
                    head
                }
                _ => v,
            };
            v.trim().trim_matches('"').to_string()
        })
    };

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("[[target]]") {
            if let Some(t) = cur.take() {
                targets.push(t);
            }
            cur = Some(Target {
                name: String::new(),
                proof: String::new(),
                tier: 0,
                release_required: false,
                conformance_cmd: String::new(),
            });
        } else if let Some(t) = cur.as_mut() {
            if line.starts_with("name") {
                if let Some(v) = field(line) {
                    t.name = v;
                }
            } else if line.starts_with("proof") {
                if let Some(v) = field(line) {
                    t.proof = v;
                }
            } else if line.starts_with("tier") {
                if let Some(v) = field(line) {
                    t.tier = v.parse().unwrap_or(0);
                }
            } else if line.starts_with("release_required") {
                if let Some(v) = field(line) {
                    t.release_required = v == "true";
                }
            } else if line.starts_with("conformance_cmd") {
                if let Some(v) = field(line) {
                    t.conformance_cmd = v;
                }
            }
        }
    }
    if let Some(t) = cur.take() {
        targets.push(t);
    }
    if targets.is_empty() {
        return Err("no [[target]] entries found".into());
    }
    Ok(targets)
}

#[cfg(test)]
mod rfc_0_43_001_tests {
    use super::*;
    use std::ffi::{OsStr, OsString};

    #[test]
    fn parses_the_real_banner() {
        assert_eq!(
            parse_z3_version("Z3 version 4.16.0 - 64 bit\n"),
            Some("4.16.0".to_string())
        );
        assert_eq!(
            parse_z3_version("Z3 version 4.12.5 - 32 bit"),
            Some("4.12.5".to_string())
        );
        assert_eq!(
            parse_z3_version("Z3 version 4.8.12.0 - 64 bit"),
            Some("4.8.12.0".to_string())
        );
    }

    /// Fail closed: every shape that is not exactly the banner is `None`.
    #[test]
    fn refuses_every_unrecognised_shape() {
        for bad in [
            "",
            "Z3 version",
            "Z3 version  - 64 bit",
            "Z3 version 4.16 - 64 bit",
            "Z3 version 4.16.x - 64 bit",
            "Z3 version 4.16.0",
            "Z3 version 4.16.0 - 128 bit",
            "z3 version 4.16.0 - 64 bit",
            "cvc5 version 1.0.0",
            "Z3 version 4.16.0 - 64 bit\nsecond line",
            "Z3 version 4.16.0 - 64 bit trailing",
            "4.16.0",
        ] {
            assert_eq!(parse_z3_version(bad), None, "accepted {bad:?}");
        }
    }

    const LOCK: &str = "[verus]\nrelease = \"release/0.1\"\nversion = \"not-z3\"\n\n\
        [z3]\nversion        = \"4.16.0\"   # a comment\nsource = \"x\"\n\n[run]\nversion = \"nope\"\n";

    #[test]
    fn reads_only_the_z3_sections_version() {
        assert_eq!(read_lock_z3_version(LOCK), Some("4.16.0".to_string()));
    }

    #[test]
    fn lock_reader_is_strict() {
        assert_eq!(read_lock_z3_version("[verus]\nversion = \"1\"\n"), None); // no [z3]
        assert_eq!(read_lock_z3_version("[z3]\nsource = \"x\"\n"), None); // no key
        assert_eq!(read_lock_z3_version("[z3]\nversion = 4.16.0\n"), None); // unquoted
        assert_eq!(read_lock_z3_version("[z3]\nversion = \"\"\n"), None); // empty
        assert_eq!(
            read_lock_z3_version("[z3]\nversion = \"4.16.0\" junk\n"),
            None
        );
    }

    /// R3: nothing reads a `path` from the lock any more.
    #[test]
    fn a_lock_path_field_is_ignored() {
        let lock = "[z3]\nversion = \"4.16.0\"\npath = \"/opt/verus/z3\"\n";
        assert_eq!(read_lock_z3_version(lock), Some("4.16.0".to_string()));
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("fjell-0431-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn script(path: &std::path::Path, body: &str) {
        crate::verus_check::test_support::write_executable(path, body);
    }

    /// A prover directory with a `verus` and a `z3` that reports `banner`, plus a symlink
    /// to `verus` elsewhere on PATH (the hand-install layout).
    fn fake_install(tag: &str, banner: &str) -> (std::path::PathBuf, OsString) {
        let root = scratch(tag);
        let real = root.join("share");
        let bin = root.join("bin");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        script(&real.join("verus"), "exit 0");
        script(&real.join("z3"), &format!("echo '{banner}'"));
        std::os::unix::fs::symlink(real.join("verus"), bin.join("verus")).unwrap();
        (root.clone(), bin.into_os_string())
    }

    const PIN: &str = "[z3]\nversion = \"4.16.0\"\n";

    /// The pass, through a symlinked `verus` (z3 sits beside the REAL binary).
    #[test]
    fn matching_solver_passes_through_a_symlink() {
        let (_r, path) = fake_install("pass", "Z3 version 4.16.0 - 64 bit");
        let out = check_z3_pin(Some(PIN), None, &path).expect("should pass");
        assert!(out.contains("detected z3 version: 4.16.0"));
        assert!(out.contains("beside the prover"));
        assert!(out.contains("pinned  z3 version: 4.16.0"));
    }

    /// The refusal: names the component, both versions, and the blast radius (R6).
    #[test]
    fn a_different_solver_is_refused_with_the_whole_message() {
        let (_r, path) = fake_install("mismatch", "Z3 version 4.12.5 - 64 bit");
        let e = check_z3_pin(Some(PIN), None, &path).expect_err("must refuse");
        assert!(e.contains("BLOCKING"));
        assert!(e.contains("z3 SOLVER version mismatch"));
        assert!(e.contains("not the Verus release"));
        assert!(e.contains("pins 4.16.0"));
        assert!(e.contains("reports 4.12.5"));
        assert!(e.contains("cargo xtask build"));
        assert!(e.contains("host test tier"));
    }

    #[test]
    fn an_unrecognised_banner_refuses_and_shows_it() {
        let (_r, path) = fake_install("odd", "Z3 4.16.0");
        let e = check_z3_pin(Some(PIN), None, &path).expect_err("must refuse");
        assert!(e.contains("not established"));
        assert!(
            e.contains("\"Z3 4.16.0\""),
            "must quote the exact string: {e}"
        );
    }

    #[test]
    fn a_missing_solver_or_prover_or_lock_refuses() {
        let (_r, path) = fake_install("gone", "Z3 version 4.16.0 - 64 bit");
        let bin_only = scratch("empty-path");
        // no verus on PATH at all
        let e = check_z3_pin(Some(PIN), None, bin_only.as_os_str()).expect_err("no prover");
        assert!(e.contains("no `verus` executable"));
        // the override points at nothing
        let e = check_z3_pin(Some(PIN), Some(OsString::from("/nonexistent/z3")), &path)
            .expect_err("no solver");
        assert!(e.contains("could not run"));
        // the lock has no [z3] version
        let e = check_z3_pin(Some("[verus]\nrelease=\"x\"\n"), None, &path).expect_err("no pin");
        assert!(e.contains("no readable `[z3] version`"));
        assert!(check_z3_pin(None, None, &path).is_err());
    }

    /// The prover's own override is honoured, and wins over the layout.
    #[test]
    fn verus_z3_path_overrides_the_layout() {
        let (r, path) = fake_install("env", "Z3 version 4.16.0 - 64 bit");
        let other = r.join("other-z3");
        script(&other, "echo 'Z3 version 4.99.0 - 64 bit'");
        let e = check_z3_pin(Some(PIN), Some(other.clone().into_os_string()), &path)
            .expect_err("override reports 4.99.0");
        assert!(e.contains("reports 4.99.0"));
        assert!(e.contains("from VERUS_Z3_PATH"));
        let _ = OsStr::new("");
    }

    /// The two refusals are distinguishable: the solver's says SOLVER and says it is not
    /// the Verus release; the Verus release's text (in `cmd_verus_check`) says Verus.
    #[test]
    fn solver_refusal_never_reads_as_a_verus_release_refusal() {
        let e = z3_mismatch_refusal("4.12.5", "4.16.0", "/x/z3", &Z3Source::BesideProver);
        assert!(e.contains("z3 SOLVER"));
        assert!(!e.contains("requires the pinned Verus version"));
    }
}

#[cfg(test)]
mod rfc_0_43_001_amendment_tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn invocation_parsing() {
        assert_eq!(
            parse_invocation(&a(&[])).unwrap(),
            Invocation {
                candidate: false,
                selection: Selection::AllPilot
            }
        );
        assert_eq!(
            parse_invocation(&a(&["--release-required"]))
                .unwrap()
                .selection,
            Selection::ReleaseRequired
        );
        let c = parse_invocation(&a(&["--candidate"])).unwrap();
        assert!(c.candidate && c.selection == Selection::AllPilot);
        // the flag may come before or after a selector
        for args in [&["--candidate", "lease"][..], &["lease", "--candidate"][..]] {
            let i = parse_invocation(&a(args)).unwrap();
            assert!(i.candidate);
            assert_eq!(i.selection, Selection::Target("lease".into()));
        }
    }

    /// R7: a candidate run can never stand in for certification.
    #[test]
    fn candidate_is_refused_under_release_required_in_either_order() {
        for args in [
            &["--candidate", "--release-required"][..],
            &["--release-required", "--candidate"][..],
        ] {
            let e = parse_invocation(&a(args)).unwrap_err();
            assert!(
                e.contains("cannot be combined with `--release-required`"),
                "{e}"
            );
            assert!(e.contains("certifies nothing"));
        }
    }

    #[test]
    fn extra_or_repeated_arguments_are_refused_not_ignored() {
        assert!(parse_invocation(&a(&["--candidate", "--candidate"])).is_err());
        assert!(parse_invocation(&a(&["lease", "capability"])).is_err());
        assert!(parse_invocation(&a(&["--all-pilot", "--release-required"])).is_err());
    }

    /// R7: whatever the proof does, a candidate run never yields the certified marker --
    /// and an ordinary run still does, so the distinction is the mode and not the outcome.
    #[test]
    fn a_candidate_run_never_emits_machine_checked_pass() {
        for passed in [true, false] {
            let (status, marker, mc) = prover_outcome(true, passed);
            assert!(!marker.contains("MACHINE-CHECKED"), "{marker}");
            assert!(!marker.contains("PASS"), "{marker}");
            assert!(!status.contains("machine_checked"));
            assert!(mc.starts_with("candidate_"));
        }
        assert_eq!(prover_outcome(true, true).1, "CANDIDATE-ONLY");
        assert_eq!(prover_outcome(true, false).1, "CANDIDATE-FAIL");
        assert_eq!(prover_outcome(false, true).1, "MACHINE-CHECKED-PASS");
        assert_eq!(prover_outcome(false, false).1, "MACHINE-CHECKED-FAIL");
    }

    #[test]
    fn the_verus_pin_compares_without_the_release_prefix() {
        let m = read_verus_pin(
            Some("0.2026.11.02.8bf68ee"),
            Some("release/0.2026.11.02.8bf68ee"),
        )
        .unwrap();
        assert!(m.matches());
        let d = read_verus_pin(
            Some("0.2026.11.02.8bf68ee"),
            Some("release/0.2026.07.27.31579f0"),
        )
        .unwrap();
        assert!(!d.matches());
    }

    /// R8: the refusal names the component, both versions, the way through, and the blast
    /// radius -- and does not read as the solver's.
    #[test]
    fn the_verus_refusal_says_what_it_is() {
        let r = read_verus_pin(
            Some("0.2026.11.02.8bf68ee"),
            Some("release/0.2026.07.27.31579f0"),
        )
        .unwrap();
        let e = verus_mismatch_refusal(&r);
        assert!(e.contains("BLOCKING"));
        assert!(e.contains("Verus RELEASE mismatch"));
        assert!(e.contains("not the z3 solver"));
        assert!(e.contains("release/0.2026.07.27.31579f0") && e.contains("0.2026.11.02.8bf68ee"));
        assert!(e.contains("--candidate"));
        assert!(e.contains("cargo xtask build"));
        let z = z3_mismatch_refusal("4.12.5", "4.16.0", "/x/z3", &Z3Source::BesideProver);
        assert!(z.contains("--candidate"));
        assert!(!e.contains("z3 SOLVER") && !z.contains("Verus RELEASE"));
    }

    /// An unparseable detected version, or an unreadable lock, refuses in EVERY mode now
    /// (it used to only under `--release-required`).
    #[test]
    fn an_unestablished_verus_identity_refuses() {
        let e = read_verus_pin(None, Some("release/x")).unwrap_err();
        assert!(e.contains("not established") && e.contains("Detected: unknown"));
        let e = read_verus_pin(Some("1"), None).unwrap_err();
        assert!(e.contains("(unreadable)"));
    }

    fn z3(detected: &str, pinned: &str) -> Z3Reading {
        Z3Reading {
            detected: detected.into(),
            pinned: pinned.into(),
            path: "/x/z3".into(),
            source: Z3Source::BesideProver,
        }
    }

    /// R7: the run states that it certifies nothing and names BOTH pairs of versions.
    #[test]
    fn the_candidate_notice_names_both_pairs_and_says_it_certifies_nothing() {
        let v = read_verus_pin(
            Some("0.2026.11.02.8bf68ee"),
            Some("release/0.2026.07.27.31579f0"),
        )
        .unwrap();
        let n = candidate_notice(&v, &z3("4.16.0", "4.12.5"));
        assert!(n.contains("certifies nothing"));
        assert!(n.contains(
            "detected 0.2026.11.02.8bf68ee, pinned release/0.2026.07.27.31579f0 (DIFFERS)"
        ));
        assert!(n.contains("detected 4.16.0, pinned 4.12.5 (DIFFERS)"));
        assert!(
            n.contains("CANDIDATE-ONLY") && n.contains("never the machine-checked pass marker")
        );
        // The literal must be ABSENT from a candidate run's output, prose included: a plain
        // `grep MACHINE-CHECKED-PASS` over a retained log must find nothing to mistake.
        assert!(!n.contains("MACHINE-CHECKED"), "{n}");
        assert!(!n.contains("both pins match"));
    }

    #[test]
    fn a_candidate_run_with_nothing_mismatched_says_so() {
        let v = read_verus_pin(Some("0.1"), Some("release/0.1")).unwrap();
        let n = candidate_notice(&v, &z3("4.16.0", "4.16.0"));
        assert!(n.contains("(matches)") && n.contains("both pins match"));
        assert!(n.contains("certifies nothing"));
        assert!(!n.contains("MACHINE-CHECKED"), "{n}");
    }

    /// `read_z3_pin` reports a mismatch as a reading (the candidate path); `check_z3_pin`
    /// refuses it (the ordinary path). One source of truth for the figure.
    #[test]
    fn a_z3_mismatch_is_a_reading_for_candidate_and_a_refusal_for_ordinary() {
        let root = std::env::temp_dir().join(format!("fjell-0431a-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let z = root.join("z3");
        crate::verus_check::test_support::write_executable(&z, "echo 'Z3 version 4.99.0 - 64 bit'");
        let pin = "[z3]\nversion = \"4.16.0\"\n";
        let env = Some(z.clone().into_os_string());
        let r = read_z3_pin(Some(pin), env.clone(), std::ffi::OsStr::new("")).unwrap();
        assert_eq!(
            (r.detected.as_str(), r.pinned.as_str()),
            ("4.99.0", "4.16.0")
        );
        assert!(check_z3_pin(Some(pin), env, std::ffi::OsStr::new("")).is_err());
        // but an unestablishable solver is an error on BOTH paths
        let bad = Some(std::ffi::OsString::from("/nonexistent/z3"));
        assert!(read_z3_pin(Some(pin), bad, std::ffi::OsStr::new("")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// Shared by the tests that execute a script they have just written.
#[cfg(test)]
mod test_support {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    /// Write `body` as an executable `#!/bin/sh` script at `path`, and do not return until it
    /// can actually be executed.
    ///
    /// Why the wait: tests run on parallel threads, and a thread that forks while this one
    /// still holds the new file open for writing hands the child a copy of that descriptor;
    /// `exec` of the file then fails with `ETXTBSY` ("Text file busy", errno 26) until that
    /// child execs or exits. It showed up as a test failing about one run in eight. The
    /// production code under test reported it correctly (it refuses, and says "Text file
    /// busy") -- the fault was the fixture's, so the fixture waits it out rather than the
    /// product being made to retry.
    pub fn write_executable(path: &Path, body: &str) {
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        for _ in 0..500 {
            match Command::new(path).arg("--version").output() {
                Err(e) if e.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                _ => return,
            }
        }
        panic!("{} still `Text file busy` after 5s", path.display());
    }
}
