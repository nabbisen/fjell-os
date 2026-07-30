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

fn walk_rs(root: &str) -> std::io::Result<Vec<std::path::PathBuf>> {
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
    inner(std::path::Path::new(root), &mut out)?;
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
}
