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
//! they are different components with different remedies -- see [`z3_refusal`].
//!
//! Output markers (one per target, architect C7 — conformance never reports
//! a bare PASS, and the JSON distinguishes machine_check pass/fail/not_run):
//!   VERUS:TARGET:<name>:MACHINE-CHECKED-PASS
//!   VERUS:TARGET:<name>:MACHINE-CHECKED-FAIL
//!   VERUS:TARGET:<name>:CONFORMANCE-ONLY      (verus absent, conformance ok)
//!   VERUS:TARGET:<name>:CONFORMANCE-FAIL      (verus absent, conformance failed)
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
        z3), or re-run the proofs under a recorded proof-review and update the lock.\n\
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
        bundles its z3), or re-run the proofs under a recorded proof-review and update the \
        lock -- not the other way round.\n\
        verus-check:   Not affected: `cargo xtask build` and every host test tier never \
        need Verus or z3 (verification/verus/TOOLCHAIN.md).",
        source.describe()
    )
}

/// The solver check, end to end: `Ok(())` to proceed, `Err(text)` to refuse (print it,
/// exit non-zero). Pure over its inputs -- the lock text, the environment override and the
/// `PATH` -- so the refusal logic is testable without a prover.
fn check_z3_pin(
    lock_text: Option<&str>,
    env_override: Option<std::ffi::OsString>,
    path_var: &std::ffi::OsStr,
) -> Result<String, String> {
    let pinned = lock_text.and_then(read_lock_z3_version);
    let Some(pinned_v) = pinned.as_deref() else {
        return Err(z3_refusal(&Z3Problem::LockUnreadable, None));
    };
    let (path, source) =
        locate_z3(env_override, path_var).map_err(|p| z3_refusal(&p, Some(pinned_v)))?;
    let detected = detect_z3_version(&path).map_err(|p| z3_refusal(&p, Some(pinned_v)))?;
    let shown = path.display().to_string();
    if detected != pinned_v {
        return Err(z3_mismatch_refusal(&detected, pinned_v, &shown, &source));
    }
    Ok(format!(
        "verus-check: detected z3 version: {detected} ({shown}, {})\n\
        verus-check: pinned  z3 version: {pinned_v} (TOOLCHAIN.lock [z3] version)",
        source.describe()
    ))
}

pub fn cmd_verus_check(args: &[String]) -> ExitCode {
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
    //   * Verus release (`[verus] release`): a mismatch WARNS, and BLOCKS only under
    //     `--release-required`, as it always did.
    //   * z3 solver (`[z3] version`, RFC-0.43-001): a mismatch -- or a solver whose
    //     version cannot be established -- BLOCKS in EVERY mode. The z3 pin was, until
    //     then, recorded in the lock and compared by nothing (E-112).
    //
    // The z3 figure is the solver binary's own `--version`, located as the prover
    // locates it; the lock's old `[z3] path` field is gone (it was only ever right for
    // one install route).
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
        let release_required_run = args
            .first()
            .map(|a| a == "--release-required")
            .unwrap_or(false);
        // H-04 (architect review v0.19): release-required mode is fail-closed
        // on version identity — an unparseable detected version or an
        // unreadable lock both block, not just an explicit mismatch.
        if release_required_run && (detected.is_none() || pinned.is_none()) {
            println!(
                "verus-check: BLOCKING — could not establish Verus version \
                identity (detected: {}, pinned: {}). --release-required requires \
                a verifiable toolchain match against TOOLCHAIN.lock.",
                detected.as_deref().unwrap_or("unknown"),
                pinned.as_deref().unwrap_or("unreadable")
            );
            return ExitCode::FAILURE;
        }
        let mismatch = match (&detected, &pinned) {
            (Some(d), Some(p)) => {
                // TOOLCHAIN.lock stores "release/X.Y.Z"; verus binary reports "X.Y.Z".
                // Compare both stripped of the "release/" prefix.
                let d_bare = d.strip_prefix("release/").unwrap_or(d);
                let p_bare = p.strip_prefix("release/").unwrap_or(p);
                d_bare != p_bare
            }
            _ => false,
        };
        if mismatch {
            println!(
                "verus-check: WARNING: detected Verus version does not match \
                TOOLCHAIN.lock pin — proofs were certified under the pinned version. \
                Update TOOLCHAIN.lock if you intend to use this version."
            );
            if release_required_run {
                println!(
                    "verus-check: BLOCKING — --release-required requires the pinned \
                    Verus version (see TOOLCHAIN.lock). Certify proofs under the pinned \
                    version or update the lock with a recorded proof-review re-run."
                );
                return ExitCode::FAILURE;
            }
        }

        // RFC-0.43-001: the solver pin, in every mode. After the Verus comparison so
        // that, under `--release-required`, a Verus mismatch is reported as that and not
        // as a solver problem; the two refusals name different components.
        let lock_text = std::fs::read_to_string("verification/verus/TOOLCHAIN.lock").ok();
        let path_var = std::env::var_os("PATH").unwrap_or_default();
        match check_z3_pin(
            lock_text.as_deref(),
            std::env::var_os("VERUS_Z3_PATH"),
            &path_var,
        ) {
            Ok(report) => println!("{report}"),
            Err(refusal) => {
                println!("{refusal}");
                return ExitCode::FAILURE;
            }
        }
    }

    // Selection
    let selected: Vec<&Target> = match args.first().map(String::as_str) {
        Some("--all-pilot") | None => targets.iter().collect(),
        Some("--release-required") => targets.iter().filter(|t| t.release_required).collect(),
        Some(name) => {
            let hit: Vec<&Target> = targets.iter().filter(|t| t.name == name).collect();
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

    for t in selected {
        // Architect C7: status values distinguish a real machine-check from
        // the conformance fallback; conformance never reports a bare PASS.
        let (status, marker, machine_check) = if verus_present {
            if run_verus(t) {
                ("machine_checked_pass", "MACHINE-CHECKED-PASS", "pass")
            } else {
                ("machine_checked_fail", "MACHINE-CHECKED-FAIL", "fail")
            }
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
        // targets never block (Stage A philosophy).
        if t.release_required && machine_check != "pass" {
            any_blocking_fail = true;
        }
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
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
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
