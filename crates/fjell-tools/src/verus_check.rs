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
        .arg("--version").output().ok()?;
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
        if trimmed.starts_with("release/") || trimmed.chars().next()
            .map_or(false, |c| c.is_ascii_digit()) {
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
            if let Some(val) = rest.trim_start_matches(|c: char| c == ' ' || c == '=').strip_prefix('"') {
                return Some(val.trim_end_matches('"').to_string());
            }
        }
    }
    None
}

pub fn cmd_verus_check(args: &[String]) -> ExitCode {
    let targets = match load_targets("verification/verus/verus-targets.toml") {
        Ok(t) => t,
        Err(e) => { eprintln!("verus-check: {e}"); return ExitCode::FAILURE; }
    };

    let verus_present = which("verus");

    // Architect follow-up (v0.18 review §8.3.3): print detected Verus/z3
    // versions and compare against the pinned TOOLCHAIN.lock release.
    if verus_present {
        let detected = detect_verus_version();
        let pinned   = read_lock_release("verification/verus/TOOLCHAIN.lock");
        println!("verus-check: detected version: {}", detected.as_deref().unwrap_or("unknown"));
        println!("verus-check: pinned  version: {}", pinned.as_deref().unwrap_or("(unreadable)"));
        let release_required_run = args.first().map(|a| a == "--release-required").unwrap_or(false);
        // H-04 (architect review v0.19): release-required mode is fail-closed
        // on version identity — an unparseable detected version or an
        // unreadable lock both block, not just an explicit mismatch.
        if release_required_run && (detected.is_none() || pinned.is_none()) {
            println!("verus-check: BLOCKING — could not establish Verus version \
                identity (detected: {}, pinned: {}). --release-required requires \
                a verifiable toolchain match against TOOLCHAIN.lock.",
                detected.as_deref().unwrap_or("unknown"),
                pinned.as_deref().unwrap_or("unreadable"));
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
            println!("verus-check: WARNING: detected Verus version does not match \
                TOOLCHAIN.lock pin — proofs were certified under the pinned version. \
                Update TOOLCHAIN.lock if you intend to use this version.");
            if release_required_run {
                println!("verus-check: BLOCKING — --release-required requires the pinned \
                    Verus version (see TOOLCHAIN.lock). Certify proofs under the pinned \
                    version or update the lock with a recorded proof-review re-run.");
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
                eprintln!("verus-check: unknown target `{name}` \
                    (known: {})", targets.iter().map(|t| t.name.as_str())
                        .collect::<Vec<_>>().join(", "));
                return ExitCode::FAILURE; // fail fast on unknown target
            }
            hit
        }
    };

    if !verus_present {
        eprintln!("verus-check: `verus` not on PATH — conformance-only mode \
            (see verification/verus/TOOLCHAIN.md)");
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
            if ok { ("not_run_conformance_pass", "CONFORMANCE-ONLY", "not_run") }
            else  { ("not_run_conformance_fail", "CONFORMANCE-FAIL", "not_run") }
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
        .map(|s| s.success()).unwrap_or(false)
}

fn run_cmd(cmd: &str) -> bool {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return false; }
    Command::new(parts[0]).args(&parts[1..]).status()
        .map(|s| s.success()).unwrap_or(false)
}

fn which(bin: &str) -> bool {
    Command::new(bin).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Minimal TOML reader for the `[[target]]` array — avoids a new dependency.
fn load_targets(path: &str) -> Result<Vec<Target>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut targets = Vec::new();
    let mut cur: Option<Target> = None;

    let field = |line: &str| -> Option<String> {
        line.split_once('=').map(|(_, v)| {
            // Strip an inline `# ...` comment. Values here are bare scalars or
            // double-quoted strings with no '#', so cutting at the first '#'
            // outside quotes is safe.
            let v = match v.split_once(" #") {
                Some((head, _)) if !head.contains('"') || head.matches('"').count() % 2 == 0 => head,
                _ => v,
            };
            v.trim().trim_matches('"').to_string()
        })
    };

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("[[target]]") {
            if let Some(t) = cur.take() { targets.push(t); }
            cur = Some(Target {
                name: String::new(), proof: String::new(), tier: 0,
                release_required: false, conformance_cmd: String::new(),
            });
        } else if let Some(t) = cur.as_mut() {
            if line.starts_with("name") { if let Some(v) = field(line) { t.name = v; } }
            else if line.starts_with("proof") { if let Some(v) = field(line) { t.proof = v; } }
            else if line.starts_with("tier") {
                if let Some(v) = field(line) { t.tier = v.parse().unwrap_or(0); }
            }
            else if line.starts_with("release_required") {
                if let Some(v) = field(line) { t.release_required = v == "true"; }
            }
            else if line.starts_with("conformance_cmd") {
                if let Some(v) = field(line) { t.conformance_cmd = v; }
            }
        }
    }
    if let Some(t) = cur.take() { targets.push(t); }
    if targets.is_empty() {
        return Err("no [[target]] entries found".into());
    }
    Ok(targets)
}
