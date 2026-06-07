//! Release rehearsal — RFC-v0.16-008.
//!
//! Runs the mechanical v1.0 tag gates 1–8 and prints a PASS/FAIL matrix.
//! Gate 9 (release-notes limitations section) is a human checklist item
//! and is reported as a manual reminder, not auto-checked.
//!
//! This does not apply any tag. The v1.0.0 tag remains owner/architect
//! gated; this command only produces the evidence that the gates pass.

use std::process::{Command, ExitCode};

struct Gate {
    id: &'static str,
    name: &'static str,
    passed: bool,
    detail: String,
}

pub fn cmd_release_rehearsal(_args: &[String]) -> ExitCode {
    println!("=== Fjell OS release rehearsal (RFC-v0.16-008) ===");
    println!("Running v1.0 tag gates. No tag will be applied.\n");

    let mut gates: Vec<Gate> = Vec::new();

    // Gate 1 — host tests
    gates.push(run_gate("1", "Host test suite (0 failures)", || {
        let out = sh(&["cargo", "test", "--workspace", "--lib",
                        "--exclude", "fjell-proptest"]);
        let failed = out.contains("FAILED") || out.contains("test result: FAILED");
        (!failed, "host lib tests".into())
    }));

    // Gate 2 — unsafe audit
    gates.push(run_gate("2", "Unsafe audit (0 missing)", || {
        let out = sh(&["cargo", "run", "-q", "-p", "fjell-unsafe-audit", "--",
                       "--workspace", ".", "--check"]);
        (out.contains("missing comment    : 0"), "unsafe-audit".into())
    }));

    // Gate 3 — MMIO audit
    gates.push(run_gate("3", "MMIO audit (0 missing)", || {
        let out = sh(&["cargo", "run", "-q", "-p", "fjell-mmio-audit", "--",
                       "--workspace", ".", "--check"]);
        (out.contains("missing annotation : 0"), "mmio-audit".into())
    }));

    // Gate 4 — ABI snapshot verify
    gates.push(run_gate("4", "ABI snapshot verify", || {
        let out = sh(&["cargo", "run", "-q", "-p", "fjell-tools", "--",
                       "abi-snapshot", "--verify"]);
        (out.contains("Result         : PASS") || out.contains("Result: PASS"),
         "abi-snapshot".into())
    }));

    // Gate 5 — readiness matrix: 0 OPEN
    gates.push(run_gate("5", "Readiness matrix (0 OPEN)", || {
        let out = sh(&["cargo", "run", "-q", "-p", "fjell-tools", "--", "readiness-check"]);
        (out.contains("PASS") && out.contains("OPEN     : 0"),
         "via readiness-check".into())
    }));

    // Gate 6 — trust report populates
    gates.push(run_gate("6", "Trust report (6 sections)", || {
        let _ = sh(&["cargo", "run", "-q", "-p", "fjell-tools", "--", "trust-report"]);
        let report = sh(&["cat", "docs/release/trust-report.txt"]);
        let sections = ["§1", "§2", "§3", "§4", "§5", "§6"]
            .iter().filter(|s| report.contains(**s)).count();
        (sections >= 6, format!("{} sections", sections))
    }));

    // Gate 7 — ERRATA: 0 OPEN
    gates.push(run_gate("7", "ERRATA register (0 OPEN)", || {
        let out = sh(&["grep", "-c", "| OPEN |", "docs/rfcs/ERRATA.md"]);
        let n: usize = out.trim().parse().unwrap_or(99);
        (n == 0, format!("{} OPEN errata", n))
    }));

    // Gate 8 — validation drills
    gates.push(run_gate("8", "Validation drills (markers)", || {
        let mut ok = true;
        let mut missing = Vec::new();

        let ed1 = sh(&["cargo", "test", "-p", "fjell-sig-ed25519",
                       "--features", "sign", "from_seed_matches_tv1_public"]);
        let ed2 = sh(&["cargo", "test", "-p", "fjell-sig-ed25519",
                       "--features", "sign", "sign_tv1_produces_tv1_sig"]);
        if !ed1.contains("1 passed") { ok = false; missing.push("ed25519-derive-tv1"); }
        if !ed2.contains("1 passed") { ok = false; missing.push("ed25519-sign-tv1"); }
        let part = sh(&["cargo", "test", "-p", "fjell-fleet-sync",
                        "--test", "partition_drill", "--", "--nocapture"]);
        for m in ["DRILL:FLEET-PARTITION-RECONCILE:PASS",
                  "DRILL:FLEET-PARTITION-ROLLBACK-REJECTED:PASS"] {
            if !part.contains(m) { ok = false; missing.push(m); }
        }
        let sdk = sh(&["cargo", "test", "-p", "fjell-config-sync",
                       "--test", "runtime_trial", "--", "--nocapture"]);
        for m in ["DRILL:SDK-CONFIG-SYNC-RUNTIME:PASS",
                  "DRILL:SDK-CONFIG-SYNC-CONVERGENCE:PASS"] {
            if !sdk.contains(m) { ok = false; missing.push(m); }
        }
        let detail = if ok { "all 5 markers present".into() }
                     else { format!("missing: {}", missing.join(", ")) };
        (ok, detail)
    }));

    // ── Matrix ────────────────────────────────────────────────────────────────
    println!("\n=== Gate matrix ===");
    let mut all_pass = true;
    for g in &gates {
        let mark = if g.passed { "PASS" } else { "FAIL" };
        if !g.passed { all_pass = false; }
        println!("  [{}] Gate {:<2} {:<32} {}", mark, g.id, g.name, g.detail);
    }

    println!("\n  [ -- ] Gate 9  Release-notes limitations    MANUAL: confirm v1.0 \
              limitations section lists hardware, multi-hart, POSIX, kernel-IPC, \
              ZeroizeOnDrop, trust-anchor provisioning");

    // Verus proof targets. Experimental targets (Stage A) are reported but
    // never block. Release-required targets (RFC-v0.18-001) are a hard gate:
    // `verus-check --release-required` must exit 0 (every release-required
    // target PROVED, prover actually run).
    let verus = sh(&["cargo", "run", "-q", "-p", "fjell-tools", "--",
                     "verus-check", "--all-pilot"]);
    let pass  = verus.matches(":MACHINE-CHECKED-PASS").count();
    let conf  = verus.matches(":CONFORMANCE-ONLY").count();
    let vfail = verus.matches(":MACHINE-CHECKED-FAIL").count()
              + verus.matches(":CONFORMANCE-FAIL").count();
    println!("  [ ~~ ] Verus  Proof targets (all)           {} machine-checked, {} conformance-only, {} fail (experimental targets non-blocking)",
             pass, conf, vfail);

    let rr_ok = run_cmd_status(&["cargo", "run", "-q", "-p", "fjell-tools", "--",
                                 "verus-check", "--release-required"]);
    let rr_mark = if rr_ok { "PASS" } else { "FAIL" };
    if !rr_ok { all_pass = false; }
    println!("  [{}] Gate 10 Verus release-required proofs  every release-required target MACHINE-CHECKED-PASS",
             rr_mark);

    if all_pass {
        println!("\nRELEASE-REHEARSAL: ALL MECHANICAL GATES PASS");
        println!("v1.0.0 tag remains owner/architect-gated (gate 9 + explicit approval).");
        ExitCode::SUCCESS
    } else {
        println!("\nRELEASE-REHEARSAL: ONE OR MORE GATES FAILED");
        ExitCode::FAILURE
    }
}

fn run_gate(id: &'static str, name: &'static str,
            f: impl FnOnce() -> (bool, String)) -> Gate {
    eprint!("  running gate {} ({}) ... ", id, name);
    let (passed, detail) = f();
    eprintln!("{}", if passed { "ok" } else { "FAIL" });
    Gate { id, name, passed, detail }
}

fn run_cmd_status(parts: &[&str]) -> bool {
    Command::new(parts[0]).args(&parts[1..]).status()
        .map(|s| s.success()).unwrap_or(false)
}

fn sh(parts: &[&str]) -> String {
    let out = Command::new(parts[0]).args(&parts[1..]).output();
    match out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            s
        }
        Err(e) => format!("command failed: {}", e),
    }
}
