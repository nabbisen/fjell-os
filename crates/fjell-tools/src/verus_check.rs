//! `cargo xtask verus-check` — RFC-v0.17-005.
//!
//! Stable project-local interface for Verus proof targets. Reads
//! `verification/verus/verus-targets.toml`. If the `verus` binary is on PATH
//! it runs the proof; otherwise it falls back to the target's Rust
//! conformance test and reports CONFORMANCE-ONLY (Stage A policy: proofs are
//! additive, never a blocker, until promoted).
//!
//! Output markers (one per target):
//!   VERUS:TARGET:<name>:PASS
//!   VERUS:TARGET:<name>:FAIL
//!   VERUS:TARGET:<name>:CONFORMANCE-ONLY
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

pub fn cmd_verus_check(args: &[String]) -> ExitCode {
    let targets = match load_targets("verification/verus/verus-targets.toml") {
        Ok(t) => t,
        Err(e) => { eprintln!("verus-check: {e}"); return ExitCode::FAILURE; }
    };

    let verus_present = which("verus");

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
        let (status, marker) = if verus_present {
            run_verus(t)
        } else {
            // Conformance-only fallback.
            let ok = run_cmd(&t.conformance_cmd);
            (if ok { "conformance-only" } else { "fail" },
             if ok { "CONFORMANCE-ONLY" } else { "FAIL" })
        };

        println!("VERUS:TARGET:{}:{}", t.name, marker);
        println!(
            "{{\"target\":\"{}\",\"status\":\"{}\",\"tier\":{},\"release_required\":{},\"verus\":{}}}",
            t.name, status, t.tier, t.release_required, verus_present
        );

        // Only a real proof FAIL on a release-required target is blocking.
        if marker == "FAIL" && t.release_required {
            any_blocking_fail = true;
        }
    }

    if any_blocking_fail {
        println!("VERUS-CHECK: BLOCKING FAILURE (release-required target failed)");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_verus(t: &Target) -> (&'static str, &'static str) {
    // Proof modules are library files (no `main`); Verus needs --crate-type=lib.
    let ok = Command::new("verus")
        .arg("--crate-type=lib")
        .arg(&t.proof)
        .status()
        .map(|s| s.success()).unwrap_or(false);
    if ok { ("pass", "PASS") } else { ("fail", "FAIL") }
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
        line.split_once('=').map(|(_, v)| v.trim().trim_matches('"').to_string())
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
