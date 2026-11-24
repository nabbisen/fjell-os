//! RFC-0.47-001 R9: **`authority-funnel`** -- no production authority check outside the one funnel, and no
//! `SAFETY` comment claiming the funnel validated something in a function that never reaches it.
//!
//! `fjell_cap::enforcement`'s module doc called `require_cap` *"the **sole** production authority-checking
//! path"* for five milestones while four syscalls checked the lease before the right with hand-rolled code,
//! a second local funnel (`check_right`) tested one right and never the kind, a sixth path accepted a kind
//! nothing could hold, and one syscall decided authority by scanning the CSpace (E-126). Two careful readings
//! of that one file produced two different wrong counts of its call sites. **This is the instrument that
//! replaces reading.**
//!
//! ## What it judges, and how it derives
//!
//! **Population: every Rust source under `crates/fjell-kernel/src` that is tracked or untracked-and-not-
//! ignored** (`Scope::Working`), through the shared `fjell-repo-scope` function -- so a new file cannot hide,
//! not even before it is `git add`ed, and no syscall is named here.
//!
//! 1. **An authority check outside the funnel.** In *code only* (comments, strings and character literals
//!    blanked, so prose about a check is not a check) a token of the shape *a right tested*
//!    (`.rights.contains(`, `.rights.is_subset_of(`), *a lease tested* (`.check_lease(`, `.check_active(`) or
//!    *a kind tested* (`.kind ==`/`!=` against `CapKind::`, `match <x>.kind`) must sit inside a function on the
//!    [`CLASSIFIED`] list, **with a stated reason**, or the check fails naming the file, line and function.
//!    A classified entry that no longer covers a token is **stale and fails**, the way `repo-scope` retires its
//!    own: the list can only be as long as it is true.
//! 2. **A false `SAFETY` claim -- the half `unsafe-audit` cannot make**, because it checks that a comment
//!    *exists*, not that it is *true*. Every `SAFETY` comment whose text says `require_cap` validated
//!    something must sit in a function that **reaches a funnel entry**: one that calls `require_cap`,
//!    `require_cap_any_kind`, `require_held` or their `*_on_ct` kernel wrappers, **or calls a function that
//!    does** (the set is closed transitively over the kernel's own helpers, so `require_endpoint` is found
//!    without being named). Seven such comments were false when this check was first run on the tree.
//!
//! **A function whose body the scanner cannot delimit is a failure**, never a skip: a check that quietly
//! skips what it cannot read would vouch for nothing.
//!
//! **What it does not do.** It does not prove the funnel is *correct* (the host tests of `enforcement.rs` and
//! the boot scenarios do), and it judges only claims that name `require_cap`: the wider mismatch between a
//! `SAFETY` comment and its operation is a separate, larger finding the architect proposed as its own line.

use std::collections::{BTreeMap, BTreeSet};

/// A function allowed to contain an authority-shaped token, and why. **Keep it short.** An entry is
/// `(file path suffix, function name, reason)`.
pub struct Classified {
    pub file: &'static str,
    pub func: &'static str,
    pub reason: &'static str,
}

/// The only functions in the kernel allowed to test a right, a lease or a kind by hand.
pub const CLASSIFIED: &[Classified] = &[
    Classified {
        file: "cap/syscall.rs",
        func: "sys_ipc_reply",
        reason: "its authority is the pending reply edge (the call being served), not a capability handle, so \
                 the funnel has nothing to authorise; the lease check here is on the edge's own binding",
    },
    Classified {
        file: "lease/mod.rs",
        func: "check_active",
        reason: "the lease table's own implementation of the funnel's `LeaseChecker` trait: step 7 of the \
                 funnel calls through it, so this is the thing being consulted, not a second consultation",
    },
];

/// The funnel's own entries (`fjell_cap::enforcement`) and the kernel's three wrappers. Every other
/// funnel-reaching function is *derived* from these.
pub const FUNNEL_ROOTS: &[&str] = &[
    "require_cap",
    "require_cap_any_kind",
    "require_held",
    "require_cap_on_ct",
    "require_cap_any_kind_on_ct",
    "require_held_on_ct",
];

/// One kernel source file.
pub struct Source {
    pub path: String,
    pub text: String,
}

/// What a green run covered.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    pub functions: usize,
    pub tokens: usize,
    pub classified_used: usize,
    pub safety_claims: usize,
    pub funnel_reaching: usize,
}

pub use fjell_rust_scan::{Func, code_only, functions, test_module_ranges};

/// The innermost function whose body contains `line`.
fn enclosing<'a>(funcs: &'a [Func], line: usize) -> Option<&'a Func> {
    funcs
        .iter()
        .filter(|f| f.start_line <= line && line <= f.end_line)
        .min_by_key(|f| f.end_line - f.start_line)
}

/// Does `body` (code-only) call `name(`?
fn calls(body: &str, name: &str) -> bool {
    let pat = format!("{name}(");
    let mut from = 0;
    while let Some(i) = body[from..].find(&pat) {
        let at = from + i;
        let before = body[..at].chars().next_back();
        if !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            return true;
        }
        from = at + pat.len();
    }
    false
}

/// An authority-shaped token on a code-only line, if any.
fn token_on(line: &str) -> Option<&'static str> {
    const DOTTED: &[&str] = &[
        ".rights.contains(",
        ".rights.is_subset_of(",
        ".check_lease(",
        ".check_active(",
    ];
    for t in DOTTED {
        if line.contains(t) {
            return Some(t);
        }
    }
    if line.contains(".kind") && line.contains("CapKind::") {
        let tests = line.contains("==") || line.contains("!=") || line.contains("matches!");
        if tests {
            return Some(".kind tested against CapKind::");
        }
    }
    if line.contains("match") && line.contains(".kind") && line.trim_end().ends_with('{') {
        return Some("match on .kind");
    }
    None
}

/// The text of the contiguous `//` comment block that starts at `lines[idx]` (a line containing `SAFETY`).
fn comment_block(lines: &[&str], idx: usize) -> String {
    let mut text = String::new();
    let mut k = idx;
    while k < lines.len() && lines[k].trim_start().starts_with("//") {
        text.push_str(lines[k]);
        text.push(' ');
        k += 1;
    }
    text
}

/// Pure core. `Ok` says what was covered; `Err` carries every problem, each naming its subject.
pub fn evaluate(sources: &[Source], classified: &[Classified]) -> Result<Report, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    if sources.is_empty() {
        return Err(vec![
            "no kernel source files in the population -- a check over nothing vouches for nothing"
                .to_string(),
        ]);
    }

    struct Parsed<'a> {
        src: &'a Source,
        code: String,
        funcs: Vec<Func>,
    }
    let mut parsed: Vec<Parsed> = Vec::new();
    for s in sources {
        let code = code_only(&s.text);
        match functions(&code) {
            Ok(funcs) => parsed.push(Parsed {
                src: s,
                code,
                funcs,
            }),
            Err(e) => problems.push(format!("{}: {e}", s.path)),
        }
    }

    // funnel-reaching set, closed transitively over every function in the population
    let mut reaching: BTreeSet<String> = FUNNEL_ROOTS.iter().map(|s| s.to_string()).collect();
    loop {
        let mut grew = false;
        for p in &parsed {
            for f in &p.funcs {
                if reaching.contains(&f.name) {
                    continue;
                }
                let body = &p.code[f.body.0..=f.body.1];
                if reaching.iter().any(|r| calls(body, r)) {
                    reaching.insert(f.name.clone());
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }

    let mut functions_seen = 0usize;
    let mut tokens = 0usize;
    let mut used: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut claims = 0usize;
    for p in &parsed {
        functions_seen += p.funcs.len();
        let code_lines: Vec<&str> = p.code.lines().collect();
        let raw_lines: Vec<&str> = p.src.text.lines().collect();
        let tests = test_module_ranges(&p.code);
        let in_tests = |line: usize| tests.iter().any(|(a, b)| *a <= line && line <= *b);

        // 1. authority checks outside the funnel
        for (idx, l) in code_lines.iter().enumerate() {
            let Some(tok) = token_on(l) else { continue };
            let line = idx + 1;
            if in_tests(line) {
                continue;
            }
            tokens += 1;
            let func = enclosing(&p.funcs, line).map(|f| f.name.as_str());
            let entry = classified
                .iter()
                .find(|c| p.src.path.ends_with(c.file) && Some(c.func) == func);
            match entry {
                Some(c) => {
                    *used.entry((c.file, c.func)).or_insert(0) += 1;
                }
                None => problems.push(format!(
                    "{}:{line}: `{tok}` in `{}` is an authority check outside the funnel -- route it through \
                     `require_cap`, `require_cap_any_kind` or `require_held` (or, if it is not an authority \
                     decision, classify the function in `authority_funnel::CLASSIFIED` with a reason)",
                    p.src.path,
                    func.unwrap_or("<no enclosing function>")
                )),
            }
        }

        // 2. SAFETY comments that claim `require_cap` validated something
        for (idx, l) in raw_lines.iter().enumerate() {
            if !(l.trim_start().starts_with("//") && l.contains("SAFETY")) {
                continue;
            }
            let block = comment_block(&raw_lines, idx);
            if !block.contains("require_cap") {
                continue;
            }
            let line = idx + 1;
            if in_tests(line) {
                continue;
            }
            claims += 1;
            match enclosing(&p.funcs, line) {
                None => problems.push(format!(
                    "{}:{line}: a SAFETY comment claims `require_cap` validated something, but it is not inside \
                     any function",
                    p.src.path
                )),
                Some(f) => {
                    let body = &p.code[f.body.0..=f.body.1];
                    if !reaching.iter().any(|r| calls(body, r)) {
                        problems.push(format!(
                            "{}:{line}: a SAFETY comment claims `require_cap` validated something, but `{}` never \
                             reaches the funnel (it calls none of the funnel's entries, directly or through \
                             another kernel function) -- the claim is false",
                            p.src.path, f.name
                        ));
                    }
                }
            }
        }
    }

    // a classified entry that covers nothing is stale
    let mut classified_used = 0usize;
    for c in classified {
        if used.contains_key(&(c.file, c.func)) {
            classified_used += 1;
        } else {
            problems.push(format!(
                "stale classified entry `{}::{}`: no authority-shaped token remains in it, so it is retired \
                 (the list can only be as long as it is true)",
                c.file, c.func
            ));
        }
    }

    if problems.is_empty() {
        Ok(Report {
            files: sources.len(),
            functions: functions_seen,
            tokens,
            classified_used,
            safety_claims: claims,
            funnel_reaching: reaching.len(),
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(path: &str, text: &str) -> Source {
        Source {
            path: path.to_string(),
            text: text.to_string(),
        }
    }

    fn run(text: &str, classified: &[Classified]) -> Result<Report, Vec<String>> {
        evaluate(
            &[src("crates/fjell-kernel/src/x/handler.rs", text)],
            classified,
        )
    }

    const FUNNEL_CALLER: &str = "fn sys_a(tf: &mut T) { if let Err(e) = require_cap_on_ct(ct, 0, h, K, R, None) { return; } }\n";

    #[test]
    fn a_handler_that_goes_through_the_funnel_is_green() {
        assert!(run(FUNNEL_CALLER, &[]).is_ok());
    }

    #[test]
    fn an_inline_rights_check_is_red_and_named() {
        let t =
            "fn sys_b(cap: &C) {\n    if !cap.rights.contains(CapRights::COPY) { return; }\n}\n";
        let e = run(t, &[]).unwrap_err();
        assert!(
            e.iter().any(|p| p.contains("handler.rs:2")
                && p.contains("sys_b")
                && p.contains(".rights.contains(")),
            "{e:?}"
        );
    }

    #[test]
    fn an_inline_lease_check_and_an_inline_kind_check_are_red() {
        let lease = "fn sys_c(cap: &C, lt: &L) { cap.check_lease(lt)?; }\n";
        assert!(run(lease, &[]).is_err());
        let kind = "fn sys_d(cap: &C) -> bool {\n    cap.kind != CapKind::Endpoint\n}\n";
        assert!(run(kind, &[]).is_err());
        let m = "fn sys_e(cap: &C) {\n    match cap.kind {\n        _ => {}\n    }\n}\n";
        assert!(run(m, &[]).is_err());
    }

    #[test]
    fn prose_about_a_check_is_not_a_check() {
        let t = "// cap.rights.contains(X) is what the funnel does\nfn f() {\n    let s = \"cap.check_lease(lt)\";\n    /* .rights.contains( */\n}\n";
        assert!(run(t, &[]).is_ok(), "{:?}", run(t, &[]));
    }

    #[test]
    fn a_classified_function_is_allowed_and_a_stale_entry_is_red() {
        let t = "fn sys_ipc_reply(lt: &L) { lt.check_active(a, b); }\n";
        let c = [Classified {
            file: "x/handler.rs",
            func: "sys_ipc_reply",
            reason: "edge",
        }];
        assert!(run(t, &c).is_ok());
        // the token is gone: the entry is stale
        let stale = run(FUNNEL_CALLER, &c).unwrap_err();
        assert!(
            stale.iter().any(|p| p.contains("stale classified entry")),
            "{stale:?}"
        );
        // the entry names another function: the token is not covered
        let other = [Classified {
            file: "x/handler.rs",
            func: "something_else",
            reason: "r",
        }];
        assert!(run(t, &other).is_err());
    }

    #[test]
    fn a_hand_rolled_check_in_a_brand_new_file_cannot_hide() {
        let a = src("crates/fjell-kernel/src/old.rs", FUNNEL_CALLER);
        let b = src(
            "crates/fjell-kernel/src/brand_new.rs",
            "fn g(c: &C) { if c.rights.contains(R::X) {} }\n",
        );
        let e = evaluate(&[a, b], &[]).unwrap_err();
        assert!(e.iter().any(|p| p.contains("brand_new.rs")), "{e:?}");
    }

    // 2. SAFETY claims ---------------------------------------------------------------------------
    #[test]
    fn a_safety_claim_in_a_function_that_never_reaches_the_funnel_is_red() {
        let t = "fn sys_f() {\n    // SAFETY: category=x capability handle is validated by require_cap before this call.\n    unsafe { g() }\n}\n";
        let e = run(t, &[]).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("handler.rs:2")
                    && p.contains("`sys_f` never reaches the funnel")),
            "{e:?}"
        );
    }

    #[test]
    fn the_same_claim_in_a_function_that_calls_the_funnel_is_green() {
        let t = "fn sys_g() {\n    let _ = require_cap_any_kind_on_ct(ct, 0, h, R);\n    // SAFETY: category=x the handle is validated by require_cap above.\n    unsafe { g() }\n}\n";
        assert!(run(t, &[]).is_ok(), "{:?}", run(t, &[]));
    }

    #[test]
    fn reaching_the_funnel_through_a_kernel_helper_counts_and_the_helper_is_not_named_here() {
        let t = "fn require_endpoint() { let _ = require_cap_on_ct(a, b, c, d, e, f); }\n\
                 fn sys_h() {\n    require_endpoint();\n    // SAFETY: category=x validated by require_cap before this call.\n    unsafe { g() }\n}\n";
        assert!(run(t, &[]).is_ok(), "{:?}", run(t, &[]));
        // but a helper that does not reach it does not count
        let u = "fn helper() { other(); }\nfn sys_i() {\n    helper();\n    // SAFETY: category=x validated by require_cap.\n    unsafe { g() }\n}\n";
        assert!(run(u, &[]).is_err());
    }

    #[test]
    fn a_multi_line_safety_comment_is_read_whole() {
        let t = "fn sys_j() {\n    // SAFETY: category=x single hart;\n    // the capability was validated by require_cap above.\n    unsafe { g() }\n}\n";
        assert!(run(t, &[]).is_err());
    }

    #[test]
    fn a_safety_comment_that_does_not_claim_the_funnel_is_not_judged() {
        let t = "fn sys_k() {\n    // SAFETY: category=kernel-global-mutable single hart; read only.\n    unsafe { g() }\n}\n";
        assert!(run(t, &[]).is_ok());
    }

    #[test]
    fn host_unit_tests_are_not_production_authority_decisions() {
        let t = "fn prod() {}\n#[cfg(test)]\nmod tests {\n    fn t(c: &C) { c.rights.contains(R::X); lt.check_active(a, b); }\n}\n";
        assert!(run(t, &[]).is_ok(), "{:?}", run(t, &[]));
        // but the same code outside a test module is not excused
        let u = "fn prod(c: &C) { c.rights.contains(R::X); }\n";
        assert!(run(u, &[]).is_err());
    }

    // robustness ---------------------------------------------------------------------------------
    #[test]
    fn unbalanced_braces_are_a_failure_not_a_skip() {
        let e = run("fn broken() {\n    if x {\n}\n", &[]).unwrap_err();
        assert!(e.iter().any(|p| p.contains("cannot be delimited")), "{e:?}");
    }

    #[test]
    fn braces_inside_strings_and_chars_do_not_confuse_the_delimiter() {
        let t = "fn ok() {\n    let a = \"}}}\";\n    let b = '{';\n    let c: &'static str = r#\"{{\"#;\n}\n";
        assert!(run(t, &[]).is_ok(), "{:?}", run(t, &[]));
    }

    #[test]
    fn an_empty_population_is_an_error() {
        assert!(evaluate(&[], &[]).is_err());
    }

    // the real tree ------------------------------------------------------------------------------
    #[test]
    fn the_real_kernel_has_no_authority_check_outside_the_funnel() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let paths =
            fjell_repo_scope::files_with_extension(&root, fjell_repo_scope::Scope::Working, "rs")
                .expect("repository scope");
        let sources: Vec<Source> = paths
            .iter()
            .filter(|p| p.to_string_lossy().contains("crates/fjell-kernel/src/"))
            .map(|p| Source {
                path: p.to_string_lossy().to_string(),
                text: std::fs::read_to_string(p).expect("readable"),
            })
            .collect();
        let r = evaluate(&sources, CLASSIFIED);
        assert!(r.is_ok(), "{r:#?}");
    }
}
