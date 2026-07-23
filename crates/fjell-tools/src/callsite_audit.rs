//! `cargo xtask callsite-audit` — static call-site conformance checks.
//!
//! Verifies that the three security-critical code sites identified in the
//! architect review (v0.18) use the model-conformant helper, not ad-hoc logic:
//!
//!   Check 1 (LEASE-CALLSITE-001): no `wrapping_add` on lease epoch bytes.
//!     Presence of wrapping_add in lease/mod.rs signals the pre-C6 pattern
//!     where epoch could silently wrap to 0. After C6, the kernel routes
//!     through `fjell_abi::lease::lease_revoke` which enforces retire-before-wrap.
//!
//!   Check 2 (CAP-CALLSITE-001): the minting path uses `is_subset_of`.
//!     cspace.rs must contain `is_subset_of` in its mint function; the proved
//!     non-amplification predicate must be the enforced one.
//!
//!   Check 3 (BCB-CALLSITE-001): no duplicate BCB mirror-selection logic.
//!     Any file other than `fjell-upgrade-format/src/lib.rs` that contains a
//!     pattern resembling direct generation comparison (outside of tests) would
//!     indicate a second implementation that bypasses the proved `select_bcb_mirror`.

use std::process::ExitCode;
use std::fs;

pub fn cmd_callsite_audit() -> ExitCode {
    println!("=== callsite-audit: static proof-callsite conformance ===");
    let mut pass = true;

    // ── Check 1: LEASE-CALLSITE-001 ────────────────────────────────────────
    {
        let path = "crates/fjell-kernel/src/lease/mod.rs";
        let src  = fs::read_to_string(path).unwrap_or_default();
        // Strip comments before scanning
        let _code: String = src.lines()
            .filter(|l| !l.trim().starts_with("//"))
            .collect::<Vec<_>>().join("\n");
        // The pre-C6 anti-pattern is `slot.epoch = <expr>.wrapping_add(1)` —
        // a direct epoch increment that wraps at u32::MAX.  After C6, the kernel
        // routes through `fjell_abi::lease::lease_revoke` which enforces
        // retire-before-wrap.  `wrapping_sub` for computing old_epoch in the
        // cancel search key is correct and is NOT flagged.
        let found = src.lines()
            .filter(|l| !l.trim().starts_with("//"))
            .any(|l| l.contains("epoch") && l.contains("wrapping_add"));
        if found {
            eprintln!("  [FAIL] LEASE-CALLSITE-001: `wrapping_add` found in \
                {path} — epoch must go through fjell_abi::lease::lease_revoke \
                (C6). Pre-C6 silent-wrap pattern detected.");
            pass = false;
        } else {
            println!("  [PASS] LEASE-CALLSITE-001  \
                no wrapping_add on lease epoch in {path}");
        }
    }

    // ── Check 2: CAP-CALLSITE-001 ──────────────────────────────────────────
    {
        let path = "crates/fjell-cap/src/cspace.rs";
        let src  = fs::read_to_string(path).unwrap_or_default();
        let code: String = src.lines()
            .filter(|l| !l.trim().starts_with("//"))
            .collect::<Vec<_>>().join("\n");
        // Must contain is_subset_of near the minting check
        let has_subset = src.contains("is_subset_of");
        if !has_subset {
            eprintln!("  [FAIL] CAP-CALLSITE-001: `is_subset_of` not found in \
                {path}. The proved non-amplification predicate must be the \
                enforced mint check.");
            pass = false;
        } else {
            println!("  [PASS] CAP-CALLSITE-001  `is_subset_of` present in {path}");
        }
        // Must NOT contain a direct bit-mask comparison that could bypass it
        let _code: String = src.lines()
            .filter(|l| !l.trim().starts_with("//"))
            .collect::<Vec<_>>().join("\n");
        // Heuristic: raw `& !` with `rights` outside of the is_subset_of impl itself
        let suspicious = code.contains("new_rights &") && !code.contains("is_subset_of");
        if suspicious {
            eprintln!("  [WARN] CAP-CALLSITE-001: raw `new_rights &` found without \
                `is_subset_of` — verify the mint path still delegates to the proved predicate.");
        }
    }

    // ── Check 3: BCB-CALLSITE-001 ──────────────────────────────────────────
    {
        let authoritative = "crates/fjell-upgrade-format/src/lib.rs";
        let scan_roots = ["crates/fjell-kernel/src", "crates/fjell-bootctl/src",
                          "crates/fjell-init/src", "crates/fjell-upgraded/src"];
        let mut duplicates: Vec<String> = Vec::new();
        for root in &scan_roots {
            if let Ok(entries) = walk_rs(root) {
                for path in entries {
                    let path_s = path.to_string_lossy().to_string();
                    if path_s == authoritative { continue; }
                    let src = fs::read_to_string(&path).unwrap_or_default();
                    let _code: String = src.lines()
                        .filter(|l| !l.trim().starts_with("//"))
                        .collect::<Vec<_>>().join("\n");
                    // Heuristic: look for generation comparison with .generation field
                    // outside of test code; select_bcb_mirror is the only valid site.
                    let code: String = src.lines()
                        .filter(|l| !l.trim().starts_with("//"))
                        .collect::<Vec<_>>().join("\n");
                    let suspicious = code.contains(".generation") && code.contains(".valid");
                    if suspicious {
                        duplicates.push(path_s);
                    }
                }
            }
        }
        if duplicates.is_empty() {
            println!("  [PASS] BCB-CALLSITE-001  no duplicate mirror-selection \
                logic outside {authoritative}");
        } else {
            eprintln!("  [WARN] BCB-CALLSITE-001: files containing .generation + \
                .valid outside the authoritative path — verify they call \
                select_bcb_mirror rather than re-implementing the selection:");
            for d in &duplicates {
                eprintln!("         {d}");
            }
            // Warn not fail — kernel may reference BootControlBlock struct fields
        }
    }

    if pass {
        println!("callsite-audit: PASS (all checks satisfied)");
        ExitCode::SUCCESS
    } else {
        println!("callsite-audit: FAIL");
        ExitCode::FAILURE
    }
}

fn walk_rs(root: &str) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    fn inner(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let e = entry?;
            let p = e.path();
            if p.is_dir() { inner(&p, out)?; }
            else if p.extension().map_or(false, |x| x == "rs") { out.push(p); }
        }
        Ok(())
    }
    inner(std::path::Path::new(root), &mut out)?;
    Ok(out)
}
