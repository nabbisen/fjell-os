//! `cargo xtask callsite-audit` — static call-site conformance checks.
//!
//! Verifies that the three security-critical code sites identified in the
//! architect review (v0.18) use the model-conformant helper, not ad-hoc logic:
//!
//!   Check 1 (LEASE-CALLSITE-001): no `wrapping_add` on lease epoch bytes.
//!     Presence of wrapping_add in lease::revoke signals the pre-C6 pattern
//!     where epoch could silently wrap to 0. After C6, the kernel routes
//!     through `fjell_abi::lease::lease_revoke` which enforces retire-before-wrap.
//!
//!   Check 2 (CAP-CALLSITE-001): the minting path uses `is_subset_of`.
//!     cspace.rs's `mint` function must contain `is_subset_of`; the proved
//!     non-amplification predicate must be the enforced one.
//!
//!   Check 3 (BCB-CALLSITE-001): no duplicate BCB mirror-selection logic.
//!     Any file other than `fjell-upgrade-format/src/lib.rs` that contains a
//!     pattern resembling direct generation comparison (outside of tests) would
//!     indicate a second implementation that bypasses the proved `select_bcb_mirror`.
//!
//! ## RFC-v0.22-001 (Gate Integrity) rigor upgrade
//!
//! The previous implementation decided a check by `str::contains` over
//! (mostly) whole-file text: a token counted whether it appeared in a
//! comment, a string literal, a doc-string, or an unrelated function —
//! anywhere in the file. This module now:
//!
//!   1. Strips `//` and `/* */` comments and string-literal contents before
//!      any token search (`strip_comments_and_strings`), so a token
//!      mentioned only in prose or a log message can no longer satisfy a
//!      check.
//!   2. For checks 1 and 2, which are about *one specific function's*
//!      behaviour, locates that function by name and brace-matches its
//!      body (`find_function_body`), and searches only within it — not the
//!      whole file. A token present elsewhere in the file no longer counts.
//!
//! Check 3 is inherently cross-file ("does any file *other than* the
//! authoritative one duplicate this logic"), so it has no single relevant
//! function to scope to; it keeps the file-level scan but benefits from the
//! same comment/string stripping.
//!
//! No parser dependency is used, by design (RFC-v0.22-001 explicitly
//! rules this out) — brace-matching over pre-stripped text is sufficient
//! for these three fixed, narrow checks.

use std::fs;
use std::process::ExitCode;

pub fn cmd_callsite_audit() -> ExitCode {
    println!("=== callsite-audit: static proof-callsite conformance ===");
    let mut pass = true;

    // ── Check 1: LEASE-CALLSITE-001 ────────────────────────────────────────
    {
        let path = "crates/fjell-kernel/src/lease/mod.rs";
        let src = fs::read_to_string(path).unwrap_or_default();
        match check_lease_callsite(&src) {
            CheckResult::Pass => {
                println!("  [PASS] LEASE-CALLSITE-001  no wrapping_add on lease epoch in {path}");
            }
            CheckResult::Fail(reason) => {
                eprintln!("  [FAIL] LEASE-CALLSITE-001: {reason} ({path})");
                pass = false;
            }
        }
    }

    // ── Check 2: CAP-CALLSITE-001 ──────────────────────────────────────────
    {
        let path = "crates/fjell-cap/src/cspace.rs";
        let src = fs::read_to_string(path).unwrap_or_default();
        match check_cap_callsite(&src) {
            CheckResult::Pass => {
                println!("  [PASS] CAP-CALLSITE-001  `is_subset_of` present in {path}");
            }
            CheckResult::Fail(reason) => {
                eprintln!("  [FAIL] CAP-CALLSITE-001: {reason} ({path})");
                pass = false;
            }
        }
    }

    // ── Check 4 (RFC-0.28-002, E-032): SYSCALL-CALLSITE-001 ────────────────
    {
        let violations = check_syscall_asm_callsite(std::path::Path::new("."));
        if violations.is_empty() {
            println!(
                "  [PASS] SYSCALL-CALLSITE-001  every raw syscall asm! block outside \
                fjell-syscall is on the allowlist, with correct clobbers"
            );
        } else {
            for v in &violations {
                eprintln!("  [FAIL] SYSCALL-CALLSITE-001: {v}");
            }
            pass = false;
        }
    }

    // ── Check 5 (RFC-0.28-004, E-033 widened): SYSCALL-CALLSITE-002 ────────
    {
        let violations = check_fjell_syscall_internal_registers(std::path::Path::new("."));
        if violations.is_empty() {
            println!(
                "  [PASS] SYSCALL-CALLSITE-002  every IpcRecv/IpcCall/CapInspect \
                block inside fjell-syscall declares the registers that syscall writes"
            );
        } else {
            for v in &violations {
                eprintln!("  [FAIL] SYSCALL-CALLSITE-002: {v}");
            }
            pass = false;
        }
    }

    // ── Check 3: BCB-CALLSITE-001 ──────────────────────────────────────────
    {
        let authoritative = "crates/fjell-upgrade-format/src/lib.rs";
        let scan_roots = [
            "crates/fjell-kernel/src",
            "crates/fjell-bootctl/src",
            "crates/fjell-init/src",
            "crates/fjell-upgraded/src",
        ];
        let mut duplicates: Vec<String> = Vec::new();
        for root in &scan_roots {
            if let Ok(entries) = walk_rs(root) {
                for path in entries {
                    let path_s = path.to_string_lossy().to_string();
                    if path_s == authoritative {
                        continue;
                    }
                    let src = fs::read_to_string(&path).unwrap_or_default();
                    if bcb_pattern_present(&src) {
                        duplicates.push(path_s);
                    }
                }
            }
        }
        if duplicates.is_empty() {
            println!(
                "  [PASS] BCB-CALLSITE-001  no duplicate mirror-selection \
                logic outside {authoritative}"
            );
        } else {
            eprintln!(
                "  [WARN] BCB-CALLSITE-001: files containing .generation + \
                .valid outside the authoritative path — verify they call \
                select_bcb_mirror rather than re-implementing the selection:"
            );
            for d in &duplicates {
                eprintln!("         {d}");
            }
            // Warn not fail — kernel may reference BootControlBlock struct fields.
            // Unchanged from the pre-RFC-v0.22-001 behaviour (RFC §4 item 3:
            // "keep all three existing checks semantically the same").
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

enum CheckResult {
    Pass,
    Fail(String),
}

/// LEASE-CALLSITE-001, scoped to the `revoke` function's body.
fn check_lease_callsite(src: &str) -> CheckResult {
    let stripped = strip_comments_and_strings(src);
    let Some(body) = find_function_body(&stripped, "revoke") else {
        return CheckResult::Fail(
            "could not locate `fn revoke` — cannot verify; update this audit if it was renamed"
                .to_string(),
        );
    };
    // The pre-C6 anti-pattern is `slot.epoch = <expr>.wrapping_add(1)` — a
    // direct epoch increment that wraps at u32::MAX. After C6, the kernel
    // routes through `fjell_abi::lease::lease_revoke`, which enforces
    // retire-before-wrap.
    if body.contains("epoch") && body.contains("wrapping_add") {
        CheckResult::Fail(
            "`wrapping_add` found in `revoke`'s body — epoch must go through \
             fjell_abi::lease::lease_revoke (C6); pre-C6 silent-wrap pattern detected"
                .to_string(),
        )
    } else {
        CheckResult::Pass
    }
}

/// CAP-CALLSITE-001, scoped to the `mint` function's body.
fn check_cap_callsite(src: &str) -> CheckResult {
    let stripped = strip_comments_and_strings(src);
    let Some(body) = find_function_body(&stripped, "mint") else {
        return CheckResult::Fail(
            "could not locate `fn mint` — cannot verify; update this audit if it was renamed"
                .to_string(),
        );
    };
    if !body.contains("is_subset_of") {
        return CheckResult::Fail(
            "`is_subset_of` not found in `mint`'s body — the proved \
             non-amplification predicate must be the enforced mint check"
                .to_string(),
        );
    }
    // Heuristic: a raw bitwise rights check inside the same function,
    // alongside `is_subset_of`, is not itself a failure (it may be
    // legitimate supporting logic) but is worth a human look.
    if body.contains("new_rights &") {
        eprintln!(
            "  [WARN] CAP-CALLSITE-001: raw `new_rights &` found alongside \
             `is_subset_of` in `mint` — verify the mint path still delegates \
             to the proved predicate."
        );
    }
    CheckResult::Pass
}

/// BCB-CALLSITE-001's duplicate-detection pattern, applied after stripping
/// comments and strings so a file that only *mentions* `.generation` and
/// `.valid` in prose cannot trigger a false positive.
fn bcb_pattern_present(src: &str) -> bool {
    let stripped = strip_comments_and_strings(src);
    stripped.contains(".generation") && stripped.contains(".valid")
}

// ── Check 4: SYSCALL-CALLSITE-001 (RFC-0.28-002, E-032) ─────────────────────
//
// Shape 3 with a guard-owned allowlist (see the governing RFC's answer
// document): any raw syscall-issuing `asm!` block outside `fjell-syscall`
// is refused unless its `(file, enclosing function)` is named here, and an
// allowlisted site must still declare every register the kernel writes for
// that syscall as a correct clobber. Adding a 36th hand-rolled block, or
// weakening one of the seven kept here, means editing this list — a
// reviewed, two-line diff, not a comment convention anyone could add next
// to their own new block.
//
// A syscall number's presence here does NOT mean "no wrapper exists" — it
// means the *word count this exact site needs* isn't what the wrapper
// covers (`IpcCall`/22 has a 3-word wrapper; all three kept 22-sites need
// 4). Per-syscall-number refusal was the wrong axis; see the answer
// document for why.
const ALLOWED_RAW_SYSCALL_SITES: &[(&str, &str)] = &[
    // 4-word IpcCall (22): sys_ipc_call_words only covers 3 words.
    ("crates/fjell-service-api/src/lib.rs", "ipc_call4"),
    ("crates/services/fjell-init/src/main.rs", "ipc_call"),
    (
        "crates/services/fjell-proxy-text/src/main.rs",
        "ipc_call_action",
    ),
    // Worded IpcReply (23): sys_ipc_reply carries the tag only, no payload.
    ("crates/services/fjell-measuredd/src/main.rs", "reply"),
    ("crates/services/fjell-proxy-text/src/main.rs", "reply"),
    ("crates/services/fjell-recoveryd/src/main.rs", "reply"),
    ("crates/services/fjell-semantic-stream/src/main.rs", "reply"),
];

/// Directories this check does not scan: `fjell-syscall` is where raw
/// syscall asm belongs; `fjell-kernel` and `fjell-abi` are the RFC's own
/// non-goals (the kernel is the receiving side of a syscall, never the
/// issuing side, and `crates/fjell-kernel/src/task/user_image.rs` contains
/// literal encoded instruction bytes for synthetic test images, not a real
/// `asm!` block — excluding the directory makes that exclusion structural
/// rather than relying on the byte-encoding happening not to match).
/// `fjell-tools` is excluded too: this check's own test fixtures below are
/// string literals containing `"li a7, N"` as *sample source text*, not
/// real syscall sites, and the module is host-side dev tooling that never
/// runs as a service in the first place — confirmed live: before this
/// exclusion, this check found itself as ten violations.
const SYSCALL_CALLSITE_EXCLUDED_DIRS: &[&str] = &[
    "crates/fjell-syscall/",
    "crates/fjell-kernel/",
    "crates/fjell-abi/",
    "crates/fjell-tools/",
];

/// Registers the kernel writes for a given syscall number, beyond `a0`/`a1`
/// (both already conventionally declared `inlateout` everywhere in this
/// tree; a future syscall violating that convention is a new finding, not
/// one this check's narrow scope claims to catch).
fn registers_requiring_inlateout(syscall_nr: u32) -> &'static [&'static str] {
    match syscall_nr {
        23 => &["a0"], // IpcReply: kernel writes the replier's own status into a0.
        22 => &["a2", "a3", "a4", "a5"], // IpcCall: sys_ipc_reply overwrites these on completion.
        _ => &[],
    }
}

/// `workspace_root` is `.` for the real invocation (`cargo run`/`cargo
/// xtask` always run from the workspace root); tests pass an absolute path
/// computed from `CARGO_MANIFEST_DIR` instead, since `cargo test` runs test
/// binaries with the *crate's* directory as the working directory, not the
/// workspace's.
fn check_syscall_asm_callsite(workspace_root: &std::path::Path) -> Vec<String> {
    let mut violations = Vec::new();
    let Ok(files) = walk_rs(&workspace_root.join("crates")) else {
        return vec!["could not walk crates/ — cannot verify".to_string()];
    };

    // Pre-compute each allowlisted site's brace-matched body span, keyed by
    // (file, fn_name), scoped to *that file's* stripped source.
    let mut allowed_spans: Vec<(&str, &str, usize, usize)> = Vec::new();
    for (path, fn_name) in ALLOWED_RAW_SYSCALL_SITES {
        let Ok(src) = fs::read_to_string(workspace_root.join(path)) else {
            violations.push(format!(
                "allowlist names {path}::{fn_name}, but the file could not be read"
            ));
            continue;
        };
        let stripped = strip_comments_only(&src);
        match find_function_body_span(&stripped, fn_name) {
            Some((start, end)) => allowed_spans.push((path, fn_name, start, end)),
            None => violations.push(format!(
                "allowlist names {path}::{fn_name}, but that function could not be found \
                 — update this list if it was renamed or removed"
            )),
        }
    }

    for path in &files {
        let path_s = path
            .strip_prefix(workspace_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if SYSCALL_CALLSITE_EXCLUDED_DIRS
            .iter()
            .any(|d| path_s.starts_with(d))
        {
            continue;
        }
        let Ok(src) = fs::read_to_string(path) else {
            continue;
        };
        // Comments only — NOT strings: the pattern being searched for
        // (`"li a7, N"`) lives inside an asm! template string literal by
        // construction, so stripping string contents (as the other three
        // checks correctly do for *their* token searches) would erase the
        // very thing this check exists to find. Comments still must be
        // stripped: fjell-init's and fjell-proxy-text's own explanatory
        // comments quote `"li a7, 22"` verbatim, which would otherwise be a
        // false positive — confirmed live during this RFC's audit.
        let stripped = strip_comments_only(&src);
        for (offset, syscall_nr) in find_raw_syscall_sites(&stripped) {
            let line = 1 + stripped[..offset].matches('\n').count();
            let in_allowed_span = allowed_spans
                .iter()
                .find(|(f, _, start, end)| *f == path_s && offset >= *start && offset < *end);
            let Some((_, fn_name, span_start, span_end)) = in_allowed_span else {
                violations.push(format!(
                    "{path_s}:{line}: raw syscall {syscall_nr} asm! block not on the \
                     allowlist — call the fjell-syscall wrapper, or add an entry here \
                     if none covers this shape"
                ));
                continue;
            };
            let body = &stripped[*span_start..*span_end];
            for reg in registers_requiring_inlateout(syscall_nr) {
                let plain_in = format!("in(\"{reg}\")");
                if body.contains(&plain_in) {
                    violations.push(format!(
                        "{path_s}:{line}: allowlisted site {fn_name} declares `{reg}` as a \
                         plain `in` — syscall {syscall_nr} requires `inlateout(\"{reg}\")` \
                         (the kernel overwrites it on completion)"
                    ));
                }
            }
        }
    }

    violations
}

/// Find every raw syscall-issuing site in already comment-stripped source:
/// the literal `"li a7, N"` asm template fragment this codebase's
/// hand-rolled blocks use exclusively (confirmed during this RFC's audit —
/// `fjell-syscall`'s own wrapper crate is the only place a syscall number
/// is set via a register instead). Returns `(byte_offset, syscall_number)`
/// pairs, offset pointing at the opening `"`.
fn find_raw_syscall_sites(stripped_src: &str) -> Vec<(usize, u32)> {
    const NEEDLE: &str = "\"li a7, ";
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = stripped_src[search_from..].find(NEEDLE) {
        let abs = search_from + rel;
        let digits_start = abs + NEEDLE.len();
        let digits_end = stripped_src[digits_start..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|i| digits_start + i)
            .unwrap_or(stripped_src.len());
        if let Ok(nr) = stripped_src[digits_start..digits_end].parse::<u32>() {
            out.push((abs, nr));
        }
        search_from = digits_end.max(abs + NEEDLE.len());
    }
    out
}

// ── Check 5: SYSCALL-CALLSITE-002 (RFC-0.28-004, D3) ────────────────────────
//
// SYSCALL-CALLSITE-001 (above) exempts `fjell-syscall` entirely — the crate
// raw syscall asm legitimately belongs in. That total exemption is why
// RFC-0.28-002's own guard, built specifically to catch this defect class,
// never looked at the two broken helpers RFC-0.28-004 found living inside
// the crate it routes everyone else to. This check does not lift the
// exemption (`fjell-syscall`'s generic `ecall0`-`ecall3` helpers correctly
// stay unaudited per-syscall — see the governing RFC's answer document
// §5(b) for why "every block declares a0-a6" is shape 1's cost paid a
// second time). It adds one narrow, additional rule: a **bespoke**,
// syscall-specific raw block inside `fjell-syscall` — one that names its
// syscall either by the literal `"li a7, N"` template fragment or by
// `SyscallNumber::<Variant>` as its `a7` operand — must declare every
// register that specific syscall's kernel handler writes, for exactly the
// three syscalls known to write past `a1`: `IpcRecv`(21), `IpcCall`(22, via
// the reply path — RFC-0.28-002's Finding 1), `CapInspect`(14).

/// Registers a bespoke, syscall-specific block issuing this syscall must
/// declare as `inlateout`/`lateout` (not a plain `in`, and not omitted)
/// because the kernel writes each of them. Only the three syscalls
/// RFC-0.28-004 verified write past `a1` are listed; every other syscall
/// number returns an empty slice and is not checked by this rule (the
/// crate's generic helpers issue many others, correctly, with no per-
/// syscall register knowledge — that is `ecall0`-`ecall3`'s whole point).
fn fjell_syscall_internal_required_registers(syscall_nr: u32) -> &'static [&'static str] {
    match syscall_nr {
        21 => &["a2", "a3", "a4", "a5", "a6"], // IpcRecv: words + RFC-055 sender identity.
        22 => &["a2", "a3", "a4", "a5"], // IpcCall: sys_ipc_reply overwrites these on completion.
        14 => &["a1", "a2", "a3"],       // CapInspect: kind, rights, badge.
        _ => &[],
    }
}

/// `SyscallNumber` variants this check recognises when a block names its
/// syscall symbolically rather than with a literal `"li a7, N"`.
const SYSCALL_VARIANT_NUMBERS: &[(&str, u32)] =
    &[("IpcRecv", 21), ("IpcCall", 22), ("CapInspect", 14)];

/// `workspace_root` is `.` for the real invocation; tests pass an absolute
/// path computed from `CARGO_MANIFEST_DIR` (see `check_syscall_asm_callsite`
/// for why `cargo test`'s working directory differs from `cargo run`'s).
fn check_fjell_syscall_internal_registers(workspace_root: &std::path::Path) -> Vec<String> {
    let path = "crates/fjell-syscall/src/lib.rs";
    let Ok(src) = fs::read_to_string(workspace_root.join(path)) else {
        return vec![format!("{path}: could not read — cannot verify")];
    };
    let stripped = strip_comments_only(&src);
    let mut violations = Vec::new();
    for (start, end) in find_asm_block_spans(&stripped) {
        let block = &stripped[start..end];
        let Some(nr) = syscall_number_in_block(block) else {
            continue;
        };
        let required = fjell_syscall_internal_required_registers(nr);
        if required.is_empty() {
            continue;
        }
        let line = 1 + stripped[..start].matches('\n').count();
        for reg in required {
            let declared_output = block.contains(&format!("inlateout(\"{reg}\")"))
                || block.contains(&format!("lateout(\"{reg}\")"));
            if declared_output {
                continue;
            }
            if block.contains(&format!("in(\"{reg}\")")) {
                violations.push(format!(
                    "{path}:{line}: block issuing syscall {nr} declares `{reg}` as a \
                     plain `in` — the kernel overwrites it on completion"
                ));
            } else {
                violations.push(format!(
                    "{path}:{line}: block issuing syscall {nr} does not declare `{reg}` \
                     at all — the kernel overwrites it on completion"
                ));
            }
        }
    }
    violations
}

/// Determine which syscall a raw asm block's text issues, if any: the
/// literal `"li a7, N"` template fragment, or `SyscallNumber::<Variant>` as
/// text (this crate's own register-based `in("a7") ...` convention — see
/// `ecall2`/`ecall3`/`sys_dma_alloc`/`sys_ipc_call_words`). Returns `None`
/// for a block whose syscall number comes from a runtime parameter (the
/// generic `ecall0`-`ecall3` helpers themselves) — deliberately: those stay
/// unaudited per-syscall by this check, per the governing RFC's settled
/// scope.
fn syscall_number_in_block(block: &str) -> Option<u32> {
    const NEEDLE: &str = "\"li a7, ";
    if let Some(rel) = block.find(NEEDLE) {
        let digits_start = rel + NEEDLE.len();
        let digits_end = block[digits_start..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|i| digits_start + i)
            .unwrap_or(block.len());
        if let Ok(nr) = block[digits_start..digits_end].parse::<u32>() {
            return Some(nr);
        }
    }
    for (name, nr) in SYSCALL_VARIANT_NUMBERS {
        if block.contains(&format!("SyscallNumber::{name}")) {
            return Some(*nr);
        }
    }
    None
}

/// Find every `core::arch::asm!( ... )` call's `[start, end)` byte span
/// (start at `core`, end just past the matching close-paren). Safe over
/// already comment-stripped source with parens counted naively (no string-
/// literal awareness needed): none of this crate's asm template strings
/// contain a paren character, confirmed by inspection.
fn find_asm_block_spans(stripped_src: &str) -> Vec<(usize, usize)> {
    const NEEDLE: &str = "core::arch::asm!(";
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = stripped_src[search_from..].find(NEEDLE) {
        let abs = search_from + rel;
        let open_paren = abs + NEEDLE.len() - 1;
        match find_matching_paren(stripped_src, open_paren) {
            Some(close) => {
                out.push((abs, close + 1));
                search_from = close + 1;
            }
            None => search_from = abs + NEEDLE.len(),
        }
    }
    out
}

/// Given the byte index of an opening `(`, return the index of its
/// depth-matched closing `)`.
fn find_matching_paren(src: &str, open_idx: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut i = open_idx;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Like `find_function_body`, but returns the body's `[start, end)` byte
/// span in the (already stripped) source rather than a copied substring —
/// needed here to test whether a *different* found offset falls inside it.
fn find_function_body_span(stripped_src: &str, fn_name: &str) -> Option<(usize, usize)> {
    let needle = format!("fn {fn_name}");
    let bytes = stripped_src.as_bytes();
    let mut search_from = 0usize;
    loop {
        let rel = stripped_src.get(search_from..)?.find(&needle)?;
        let abs = search_from + rel;
        let before_ok = abs == 0 || !is_ident_byte(bytes[abs - 1]);
        let after_idx = abs + needle.len();
        let after_ok = bytes.get(after_idx).is_none_or(|b| !is_ident_byte(*b));
        if before_ok && after_ok {
            let open_rel = stripped_src.get(after_idx..)?.find('{')?;
            let open_abs = after_idx + open_rel;
            let body = extract_braced_body(stripped_src, open_abs)?;
            let body_start = open_abs + 1;
            return Some((body_start, body_start + body.len()));
        }
        search_from = abs + needle.len();
        if search_from >= stripped_src.len() {
            return None;
        }
    }
}

/// Strip `//` line comments and `/* */` block comments only — string
/// literal contents are left intact. See `check_syscall_asm_callsite`'s
/// comment for why this check needs that (unlike the other three, which
/// use `strip_comments_and_strings` below).
///
/// `pub(crate)` for RFC-0.31-001 D2: `smoke.rs`'s kernel-marker check needs
/// the same "comments out, string contents in" reading of the same kernel
/// source, and a second copy of a comment parser is the duplicate-logic
/// defect this project keeps removing — not something to introduce while
/// closing an erratum about a list that was copied.
pub(crate) fn strip_comments_only(src: &str) -> String {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    let mut in_string = false;
    while i < n {
        let c = bytes[i];
        if in_string {
            out.push(c as char);
            if c == b'\\' && i + 1 < n {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_string = true;
            out.push(c as char);
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
            while i < n && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                out.push(if bytes[i] == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            if i + 1 < n {
                out.push(' ');
                out.push(' ');
                i += 2;
            } else {
                i = n;
            }
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

/// Strip `//` line comments, `/* */` block comments, and the contents of
/// string literals, replacing stripped characters with spaces (newlines are
/// preserved) so no comment or string-literal text can satisfy a token
/// search or perturb brace-matching.
///
/// Character literals (`'a'`) are deliberately left unstripped: this
/// project's Rust source also uses `'a` for lifetimes, and reliably telling
/// the two apart without a real parser is exactly the complexity a textual
/// scanner is meant to avoid. Leaving them alone is safe for this module's
/// purposes because every token these checks search for (`is_subset_of`,
/// `wrapping_add`, `.generation`, `.valid`) is longer than a single
/// character and so cannot be hidden inside a char literal.
fn strip_comments_and_strings(src: &str) -> String {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        let c = bytes[i];
        // Line comment: blank out to end of line.
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
            while i < n && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        // Block comment: blank out to the matching `*/` (no nesting —
        // rustc itself supports nested block comments, but none of the
        // audited files use them; a real parser would be needed for full
        // correctness, which this module deliberately avoids).
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                out.push(if bytes[i] == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            if i + 1 < n {
                out.push(' ');
                out.push(' ');
                i += 2;
            } else {
                i = n;
            }
            continue;
        }
        // String literal: blank out contents, respecting `\"` escapes.
        if c == b'"' {
            out.push(' ');
            i += 1;
            while i < n {
                let cc = bytes[i];
                if cc == b'\\' && i + 1 < n {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                }
                if cc == b'"' {
                    out.push(' ');
                    i += 1;
                    break;
                }
                out.push(if cc == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

/// Find `fn <name>` as a whole word in already-stripped source, then
/// brace-match from the next `{` to return its body text. Returns `None`
/// if the function cannot be found or has no `{ ... }` body.
fn find_function_body(stripped_src: &str, fn_name: &str) -> Option<String> {
    let needle = format!("fn {fn_name}");
    let bytes = stripped_src.as_bytes();
    let mut search_from = 0usize;
    loop {
        let rel = stripped_src.get(search_from..)?.find(&needle)?;
        let abs = search_from + rel;
        let before_ok = abs == 0 || !is_ident_byte(bytes[abs - 1]);
        let after_idx = abs + needle.len();
        let after_ok = bytes.get(after_idx).is_none_or(|b| !is_ident_byte(*b));
        if before_ok && after_ok {
            let open_rel = stripped_src.get(after_idx..)?.find('{')?;
            let open_abs = after_idx + open_rel;
            return extract_braced_body(stripped_src, open_abs);
        }
        search_from = abs + needle.len();
        if search_from >= stripped_src.len() {
            return None;
        }
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Given the byte index of an opening `{`, return the text strictly
/// between it and its depth-matched closing `}`. Operates on already
/// comment/string-stripped text, so no stray brace from a comment or
/// string can perturb the depth count.
fn extract_braced_body(src: &str, open_idx: usize) -> Option<String> {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut body_start = None;
    let mut i = open_idx;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                if body_start.is_none() {
                    body_start = Some(i + 1);
                }
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(src[body_start?..i].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn walk_rs(root: impl AsRef<std::path::Path>) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    fn inner(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let e = entry?;
            let p = e.path();
            if p.is_dir() {
                inner(&p, out)?;
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
        Ok(())
    }
    inner(root.as_ref(), &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_pass(r: CheckResult) -> bool {
        matches!(r, CheckResult::Pass)
    }

    // ── strip_comments_and_strings ──────────────────────────────────────

    #[test]
    fn strips_line_comments() {
        let src = "let x = 1; // is_subset_of mentioned only here\nlet y = 2;";
        let stripped = strip_comments_and_strings(src);
        assert!(!stripped.contains("is_subset_of"));
        assert!(stripped.contains("let x = 1;"));
        assert!(stripped.contains("let y = 2;"));
    }

    #[test]
    fn strips_block_comments() {
        let src = "fn a() {}\n/* is_subset_of in a block comment\n   spanning lines */\nfn b() {}";
        let stripped = strip_comments_and_strings(src);
        assert!(!stripped.contains("is_subset_of"));
        assert!(stripped.contains("fn a"));
        assert!(stripped.contains("fn b"));
    }

    #[test]
    fn strips_string_literal_contents() {
        let src = r#"let msg = "is_subset_of failed"; do_real_check();"#;
        let stripped = strip_comments_and_strings(src);
        assert!(!stripped.contains("is_subset_of"));
        assert!(stripped.contains("do_real_check();"));
    }

    #[test]
    fn preserves_lifetimes_and_char_literals() {
        // Not stripped, by design — see the function's doc comment. Neither
        // can contain any of this module's multi-character search tokens.
        let src = "fn f<'a>(x: &'a str) -> char { 'x' }";
        let stripped = strip_comments_and_strings(src);
        assert!(stripped.contains("'a"));
        assert!(stripped.contains("'x'"));
    }

    // ── find_function_body ──────────────────────────────────────────────

    #[test]
    fn finds_simple_function_body() {
        let src = "fn other() { let a = 1; }\nfn target() { let b = 2; }\nfn another() {}";
        let body = find_function_body(src, "target").unwrap();
        assert!(body.contains("let b = 2;"));
        assert!(!body.contains("let a = 1;"));
    }

    #[test]
    fn does_not_match_substring_function_names() {
        // `fn target_extra` must not be found when searching for `target`.
        let src = "fn target_extra() { let a = 1; }";
        assert!(find_function_body(src, "target").is_none());
    }

    #[test]
    fn stops_at_matching_brace_not_first_close() {
        let src = "fn target() { if true { nested(); } tail(); }\nfn after() { unrelated(); }";
        let body = find_function_body(src, "target").unwrap();
        assert!(body.contains("nested();"));
        assert!(body.contains("tail();"));
        assert!(!body.contains("unrelated();"));
    }

    #[test]
    fn returns_none_when_function_absent() {
        let src = "fn something_else() {}";
        assert!(find_function_body(src, "target").is_none());
    }

    // ── check_lease_callsite ─────────────────────────────────────────────

    #[test]
    fn lease_check_passes_when_clean() {
        let src = "fn revoke(&mut self) { slot.epoch = new_epoch; }";
        assert!(is_pass(check_lease_callsite(src)));
    }

    #[test]
    fn lease_check_fails_on_real_wrapping_add_in_revoke() {
        let src = "fn revoke(&mut self) { slot.epoch = slot.epoch.wrapping_add(1); }";
        assert!(!is_pass(check_lease_callsite(src)));
    }

    /// Required failure demonstration (RFC §Testing item 3): the forbidden
    /// token in a comment must NOT satisfy — nor break — the check. Since
    /// this check is a negative constraint (must NOT contain the pattern),
    /// a comment-only mention must PASS (there is no real anti-pattern in
    /// the code), which is only true once comments are actually stripped;
    /// under the pre-RFC implementation's shallow full-line-only stripping,
    /// a trailing same-line comment like this would have produced a false
    /// positive.
    #[test]
    fn lease_check_ignores_wrapping_add_mentioned_only_in_a_comment() {
        let src =
            "fn revoke(&mut self) { slot.epoch = new_epoch; } // old code used wrapping_add here";
        assert!(is_pass(check_lease_callsite(src)));
    }

    #[test]
    fn lease_check_fails_closed_when_function_missing() {
        let src = "fn totally_different() {}";
        assert!(!is_pass(check_lease_callsite(src)));
    }

    // ── check_cap_callsite ───────────────────────────────────────────────

    #[test]
    fn cap_check_passes_when_is_subset_of_in_mint_body() {
        let src =
            "fn mint(&mut self) { if !new_rights.is_subset_of(source.rights) { return Err(()); } }";
        assert!(is_pass(check_cap_callsite(src)));
    }

    /// Required failure demonstration: token present only in a comment.
    #[test]
    fn cap_check_fails_when_is_subset_of_only_in_comment() {
        let src = "fn mint(&mut self) { // should call is_subset_of\n    grant_anyway(); }";
        assert!(!is_pass(check_cap_callsite(src)));
    }

    /// Required failure demonstration: token present only in an unrelated
    /// function, not in the one this check is actually about.
    #[test]
    fn cap_check_fails_when_is_subset_of_only_in_unrelated_function() {
        let src = "fn mint(&mut self) { grant_anyway(); }\n\
                    fn copy(&mut self) { if a.is_subset_of(b) {} }";
        assert!(!is_pass(check_cap_callsite(src)));
    }

    #[test]
    fn cap_check_fails_closed_when_function_missing() {
        let src = "fn totally_different() {}";
        assert!(!is_pass(check_cap_callsite(src)));
    }

    // ── bcb_pattern_present ──────────────────────────────────────────────

    #[test]
    fn bcb_pattern_detected_in_real_code() {
        let src = "if a.generation > b.generation && a.valid { pick(a) }";
        assert!(bcb_pattern_present(src));
    }

    #[test]
    fn bcb_pattern_ignored_when_only_in_comment() {
        let src = "// compares .generation and .valid like the real selector\nfn f() {}";
        assert!(!bcb_pattern_present(src));
    }

    // ── strip_comments_only ──────────────────────────────────────────────

    #[test]
    fn strip_comments_only_keeps_string_contents() {
        let src = "let x = \"li a7, 21\"; // \"li a7, 22\" mentioned here too";
        let stripped = strip_comments_only(src);
        assert!(stripped.contains("\"li a7, 21\""));
        assert!(!stripped.contains("mentioned here too"));
    }

    // ── find_raw_syscall_sites ─────────────────────────────────────────────

    #[test]
    fn finds_a_real_syscall_site() {
        let src = "core::arch::asm!(\"li a7, 21\", \"ecall\", options(nostack));";
        let sites = find_raw_syscall_sites(src);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].1, 21);
        assert_eq!(sites[0].0, src.find("\"li a7, 21\"").unwrap());
    }

    #[test]
    fn ignores_a_syscall_number_mentioned_only_in_a_comment() {
        // Comments must be stripped by the caller first; this exercises
        // that find_raw_syscall_sites itself has no special comment logic
        // (the false positive fjell-init/fjell-proxy-text's real comments
        // hit is a stripping-order bug, not a parsing one).
        let raw = "// a7 is written by \"li a7, 22\". Declare as clobber.";
        let stripped = strip_comments_only(raw);
        assert!(find_raw_syscall_sites(&stripped).is_empty());
    }

    #[test]
    fn finds_multiple_sites_in_one_file() {
        let src = "\"li a7, 20\", \"ecall\" ... \"li a7, 21\", \"ecall\"";
        let sites = find_raw_syscall_sites(src);
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[1].1, 21);
    }

    // ── find_function_body_span ────────────────────────────────────────────

    #[test]
    fn body_span_matches_extracted_body_text() {
        let src = "fn target() { let a = 1; let b = 2; }";
        let (start, end) = find_function_body_span(src, "target").unwrap();
        assert_eq!(&src[start..end], " let a = 1; let b = 2; ");
    }

    // ── check_syscall_asm_callsite (Demonstration 1/2/3, RFC-v0.22-001) ────

    #[test]
    fn demonstration_2_flags_a0_as_plain_in_on_an_allowlisted_ipcreply_site() {
        // Reconstructs the exact shape fjell-measuredd::reply had before
        // this RFC fixed it — not a synthetic example.
        let src = "fn reply(tag: usize, w0: usize) { unsafe { core::arch::asm!(\"li a7, 23\", \"ecall\", in(\"a0\") 0usize, in(\"a1\") tag, in(\"a2\") w0); } }";
        let stripped = strip_comments_only(src);
        let (start, end) = find_function_body_span(&stripped, "reply").unwrap();
        let body = &stripped[start..end];
        let regs = registers_requiring_inlateout(23);
        assert!(regs.contains(&"a0"));
        assert!(body.contains("in(\"a0\")"));
    }

    #[test]
    fn demonstration_2_passes_once_a0_is_inlateout() {
        let src = "fn reply(tag: usize) { unsafe { core::arch::asm!(\"li a7, 23\", \"ecall\", inlateout(\"a0\") 0usize => _, in(\"a1\") tag); } }";
        let stripped = strip_comments_only(src);
        let (start, end) = find_function_body_span(&stripped, "reply").unwrap();
        let body = &stripped[start..end];
        assert!(!body.contains("in(\"a0\")"));
        assert!(body.contains("inlateout(\"a0\")"));
    }

    #[test]
    fn registers_requiring_inlateout_covers_ipccall_reply_words_not_ipcsend() {
        assert_eq!(registers_requiring_inlateout(22), &["a2", "a3", "a4", "a5"]);
        assert_eq!(registers_requiring_inlateout(23), &["a0"]);
        assert!(registers_requiring_inlateout(20).is_empty());
        assert!(registers_requiring_inlateout(21).is_empty());
    }

    /// `cargo test` runs test binaries with the crate's own directory as
    /// the working directory, not the workspace root that the real
    /// `cargo run -p fjell-tools -- callsite-audit` invocation always uses
    /// — so the two filesystem-touching tests below need an explicit,
    /// CWD-independent root instead of `check_syscall_asm_callsite`'s own
    /// production default (`.`).
    fn test_workspace_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn allowlist_entries_are_all_currently_resolvable() {
        // Demonstration 3 (the shipped tree passes): every named function
        // must actually exist where the allowlist says it does, or the
        // check fails closed with a specific reason rather than silently
        // matching nothing (the RFC-0.24-002 ci-proptest defect this
        // project has already been bitten by once).
        let root = test_workspace_root();
        for (path, fn_name) in ALLOWED_RAW_SYSCALL_SITES {
            let src = fs::read_to_string(root.join(path))
                .unwrap_or_else(|e| panic!("allowlist names {path}, unreadable: {e}"));
            let stripped = strip_comments_only(&src);
            assert!(
                find_function_body_span(&stripped, fn_name).is_some(),
                "allowlist names {path}::{fn_name}, not found in the current tree"
            );
        }
    }

    #[test]
    fn shipped_tree_has_no_syscall_callsite_violations() {
        // Demonstration 3: the tree as shipped by this RFC passes.
        let violations = check_syscall_asm_callsite(&test_workspace_root());
        assert!(
            violations.is_empty(),
            "unexpected SYSCALL-CALLSITE-001 violations on the shipped tree: {violations:#?}"
        );
    }

    // ── SYSCALL-CALLSITE-002 (RFC-0.28-004, D3) ────────────────────────────

    #[test]
    fn find_matching_paren_handles_nested_parens() {
        let src = "asm!(\"ecall\", in(\"a7\") f(x, y), options(nostack))";
        let open = src.find('(').unwrap();
        let close = find_matching_paren(src, open).unwrap();
        assert_eq!(close, src.len() - 1);
        assert_eq!(&src[open..=close], &src[open..]);
    }

    #[test]
    fn find_asm_block_spans_finds_one_block() {
        let src = "fn f() { unsafe { core::arch::asm!(\"ecall\", options(nostack)); } }";
        let spans = find_asm_block_spans(src);
        assert_eq!(spans.len(), 1);
        let (s, e) = spans[0];
        assert!(src[s..e].starts_with("core::arch::asm!("));
        assert!(src[s..e].ends_with(')'));
    }

    #[test]
    fn syscall_number_in_block_reads_literal_form() {
        let block = "core::arch::asm!(\"li a7, 21\", \"ecall\")";
        assert_eq!(syscall_number_in_block(block), Some(21));
    }

    #[test]
    fn syscall_number_in_block_reads_symbolic_form() {
        let block = "core::arch::asm!(\"ecall\", in(\"a7\") SyscallNumber::CapInspect as usize)";
        assert_eq!(syscall_number_in_block(block), Some(14));
    }

    #[test]
    fn syscall_number_in_block_none_for_generic_runtime_number() {
        // ecall2's own shape: the syscall number is a runtime parameter,
        // not a literal or a named SyscallNumber variant — deliberately
        // invisible to this check (the generic helpers stay unaudited
        // per-syscall; see the governing RFC's answer document §5(b)).
        let block = "core::arch::asm!(\"ecall\", in(\"a7\") nr, inlateout(\"a0\") a0 => r0)";
        assert_eq!(syscall_number_in_block(block), None);
    }

    #[test]
    fn demonstration_1_flags_a_new_undeclared_register() {
        // Reconstructs sys_ipc_call_words's real, live bug found by this
        // exact check: a5 entirely undeclared on an IpcCall block.
        let src = "fn f() { unsafe { core::arch::asm!(\"ecall\", in(\"a7\") SyscallNumber::IpcCall as usize, inlateout(\"a2\") w0 => _, inlateout(\"a3\") w1 => _, inlateout(\"a4\") w2 => _); } }";
        let stripped = strip_comments_only(src);
        let spans = find_asm_block_spans(&stripped);
        assert_eq!(spans.len(), 1);
        let (s, e) = spans[0];
        let block = &stripped[s..e];
        let nr = syscall_number_in_block(block).unwrap();
        assert_eq!(nr, 22);
        let required = fjell_syscall_internal_required_registers(nr);
        assert!(
            !block.contains("\"a5\""),
            "a5 must be entirely absent for this fixture"
        );
        assert!(required.contains(&"a5"));
    }

    #[test]
    fn demonstration_2_flags_a_plain_in_register() {
        let src = "fn f() { unsafe { core::arch::asm!(\"li a7, 14\", \"ecall\", inlateout(\"a0\") c => s, lateout(\"a1\") k, in(\"a2\") 0usize, lateout(\"a3\") b); } }";
        let stripped = strip_comments_only(src);
        let (s, e) = find_asm_block_spans(&stripped)[0];
        let block = &stripped[s..e];
        assert_eq!(syscall_number_in_block(block), Some(14));
        assert!(block.contains("in(\"a2\")"));
        assert!(!block.contains("inlateout(\"a2\")"));
    }

    #[test]
    fn shipped_tree_has_no_fjell_syscall_internal_violations() {
        // Demonstration 3: the tree as shipped by this RFC passes.
        let violations = check_fjell_syscall_internal_registers(&test_workspace_root());
        assert!(
            violations.is_empty(),
            "unexpected SYSCALL-CALLSITE-002 violations on the shipped tree: {violations:#?}"
        );
    }
}
