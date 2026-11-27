//! RFC-0.50-002: **`syscall-doc`** -- the published syscall tables are compared with `SyscallNumber`.
//!
//! `syscall-surface` (RFC-v0.22-001 Slice 1, made a rule by RFC-0.49-002) reads three source files and a TOML and **no document**,
//! while `doc-counts` declined the question in writing on the grounds that `syscall-surface` covered it. The Syscall Reference and
//! `kernel.md`'s group table therefore drifted for twenty milestones -- seven live syscalls missing from the first, eight from the
//! second, a count of 26 against 35 -- with nothing reading them. This reads them.
//!
//! **Two tables, three directions each.**
//!
//! * `docs/src/api/syscalls.md` -- rows `| <number> | \`Name\` | <wrappers> | <group> |`. Fails on **a row naming no variant**, **a variant
//!   no row names**, **a row whose number disagrees with the enum**, and a variant named twice.
//! * `docs/src/external-design/kernel.md` -- the `| Group | Syscalls |` table, which names syscalls by **snake-case wrapper name** and has
//!   no numbers. Fails on a name that is no variant's snake-case, a variant no entry names, a duplicate.
//!
//! **Wrapper aliases are parsed as aliases, never as syscalls.** An *"(also reached via `x`)"* parenthetical (kernel.md) or *"(also reached
//! via `sys_x`)"* (syscalls.md) names a **different wrapper of the same syscall**. Each alias must name a `pub fn sys_x` that exists in
//! `crates/fjell-syscall/src/lib.rs` **and must reach the row's own syscall** -- its body contains `SyscallNumber::<Name>`, or the literal
//! `li a7, <number>`, or calls another `sys_*` wrapper that does (followed to depth 4). That is stronger than a count: a count that came out
//! right could not tell `ipc_call_words` from `ipc_recv_msg` swapped, and could not see `ipc_try_send`, which names a wrapper that does not
//! exist (the pre-0.27 name, renamed at E-022).
//!
//! **The wrapper column is checked narrowly** (D4): every `sys_*` the Syscall Reference's third column names must exist as a `pub fn`; and a
//! row that says it has **no wrapper** (`—`) must not have one -- `sys_cap_delete` has existed since 0.47 while the page said otherwise.
//!
//! **Nothing found is a failure.** A table whose rows cannot be found (its header renamed, the section removed) is FAIL, not a pass over
//! nothing.
//!
//! **Not checked:** the prose around the tables (which is why RFC-0.50-002 D3 removes the hand-typed counts from it), the *Group* column's
//! assignment, what a wrapper's signature is, and the service tables.

use crate::syscall_table::parse_declared;
use std::collections::{BTreeMap, BTreeSet};

pub struct Inputs<'a> {
    /// `crates/fjell-abi/src/syscall.rs`
    pub abi: &'a str,
    /// `docs/src/api/syscalls.md`
    pub reference: &'a str,
    /// `docs/src/external-design/kernel.md`
    pub kernel_doc: &'a str,
    /// `crates/fjell-syscall/src/lib.rs`
    pub wrappers: &'a str,
}

#[derive(Debug)]
pub struct Report {
    pub declared: usize,
    pub reference_rows: usize,
    pub kernel_entries: usize,
    pub aliases_resolved: usize,
    pub wrappers_checked: usize,
}

/// `IpcCall` -> `ipc_call`.
pub fn snake(pascal: &str) -> String {
    let mut out = String::new();
    for (i, c) in pascal.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The backtick-delimited spans of `s`, in order.
fn spans(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(a) = rest.find('`') {
        let after = &rest[a + 1..];
        let Some(b) = after.find('`') else { break };
        out.push(&after[..b]);
        rest = &after[b + 1..];
    }
    out
}

/// Split a cell into `(plain-text with aliases removed, alias spans)`. An alias group is `(also reached via ...)`.
fn split_aliases(cell: &str) -> (String, Vec<String>) {
    let marker = "(also reached via";
    let mut plain = String::new();
    let mut aliases = Vec::new();
    let mut rest = cell;
    while let Some(a) = rest.find(marker) {
        plain.push_str(&rest[..a]);
        let after = &rest[a + marker.len()..];
        let end = after.find(')').unwrap_or(after.len());
        for sp in spans(&after[..end]) {
            aliases.push(sp.to_string());
        }
        rest = &after[(end + 1).min(after.len())..];
    }
    plain.push_str(rest);
    (plain, aliases)
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

struct RefRow {
    number: u32,
    name: String,
    wrappers: String,
}

fn parse_reference(src: &str) -> Vec<RefRow> {
    let mut rows = Vec::new();
    for line in src.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        let c = cells(line);
        if c.len() < 3 {
            continue;
        }
        let Ok(number) = c[0].parse::<u32>() else { continue };
        let name = c[1].trim_matches('`');
        if name.is_empty()
            || c[1] == name
            || !name.chars().next().is_some_and(|x| x.is_ascii_uppercase())
        {
            continue;
        }
        rows.push(RefRow {
            number,
            name: name.to_string(),
            wrappers: c[2].clone(),
        });
    }
    rows
}

struct KernelEntry {
    canonical: String,
    aliases: Vec<String>,
}

/// The `| Group | Syscalls |` table of `kernel.md`. `None` when the header is not found.
fn parse_kernel_table(src: &str) -> Option<Vec<KernelEntry>> {
    let mut in_table = false;
    let mut out = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        if !in_table {
            if t.starts_with('|') && cells(t).len() == 2 && cells(t)[0] == "Group" && cells(t)[1] == "Syscalls" {
                in_table = true;
            }
            continue;
        }
        if !t.starts_with('|') {
            break;
        }
        let c = cells(t);
        if c.len() < 2 || c[0].starts_with("---") || c[0].starts_with(':') {
            continue;
        }
        // Group the aliases with the canonical span that precedes their parenthetical.
        let mut rest = c[1].as_str();
        let marker = "(also reached via";
        loop {
            let (head, tail) = match rest.find(marker) {
                Some(a) => (&rest[..a], Some(&rest[a..])),
                None => (rest, None),
            };
            // the canonical spans in `head`: all but we attach the alias group to the LAST one
            let mut sp: Vec<&str> = spans(head);
            if let Some(tail) = tail {
                let after = &tail[marker.len()..];
                let end = after.find(')').unwrap_or(after.len());
                let al: Vec<String> = spans(&after[..end]).iter().map(|s| s.to_string()).collect();
                let last = sp.pop();
                for s in sp {
                    out.push(KernelEntry { canonical: s.to_string(), aliases: vec![] });
                }
                if let Some(l) = last {
                    out.push(KernelEntry { canonical: l.to_string(), aliases: al });
                }
                rest = &after[(end + 1).min(after.len())..];
            } else {
                for s in sp {
                    out.push(KernelEntry { canonical: s.to_string(), aliases: vec![] });
                }
                break;
            }
        }
    }
    in_table.then_some(out)
}

/// `pub fn <name>` body: the text from the declaration to the next `\npub fn ` (or the end).
fn body_of<'a>(wrappers: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("pub fn {name}");
    let mut from = 0;
    while let Some(p) = wrappers[from..].find(&needle) {
        let start = from + p;
        let after = start + needle.len();
        // whole identifier: next char must not continue it
        if wrappers[after..].chars().next().is_some_and(|c| c == '(' || c == '<') {
            let end = wrappers[after..].find("\npub fn ").map_or(wrappers.len(), |e| after + e);
            return Some(&wrappers[start..end]);
        }
        from = after;
    }
    None
}

/// Does wrapper `name` reach syscall `variant` (number `num`)? Direct evidence, or through another `sys_*` it calls (depth <= 4).
fn reaches(wrappers: &str, name: &str, variant: &str, num: u32, depth: usize, seen: &mut BTreeSet<String>) -> bool {
    if depth == 0 || !seen.insert(name.to_string()) {
        return false;
    }
    let Some(body) = body_of(wrappers, name) else { return false };
    if body.contains(&format!("SyscallNumber::{variant}")) || body.contains(&format!("li a7, {num}\"")) {
        return true;
    }
    let mut i = 0;
    let b = body.as_bytes();
    while let Some(p) = body[i..].find("sys_") {
        let s = i + p;
        let mut e = s;
        while e < b.len() && (b[e].is_ascii_alphanumeric() || b[e] == b'_') {
            e += 1;
        }
        let callee = &body[s..e];
        // `name` is the declaration itself on its first occurrence; only a call (`callee(`) recurses
        if callee != name && body[e..].starts_with('(') && reaches(wrappers, callee, variant, num, depth - 1, seen) {
            return true;
        }
        i = e.max(s + 4);
    }
    false
}

pub fn evaluate(i: &Inputs<'_>) -> Result<Report, Vec<String>> {
    let mut problems = Vec::new();
    let declared = parse_declared(i.abi);
    if declared.is_empty() {
        return Err(vec!["no `SyscallNumber` variants found in the ABI source".into()]);
    }
    let by_name: BTreeMap<&str, u32> = declared.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    let by_snake: BTreeMap<String, (&str, u32)> = declared
        .iter()
        .map(|(n, v)| (snake(n), (n.as_str(), *v)))
        .collect();

    // ── the Syscall Reference ────────────────────────────────────────────────────
    let rows = parse_reference(i.reference);
    if rows.is_empty() {
        problems.push("docs/src/api/syscalls.md: no syscall rows found (`| <number> | `Name` | … |`): a table whose header was renamed or removed is a failure, not a pass over nothing".into());
    }
    let mut seen_ref: BTreeSet<&str> = BTreeSet::new();
    let mut wrappers_checked = 0usize;
    let mut aliases_resolved = 0usize;
    for r in &rows {
        if !seen_ref.insert(r.name.as_str()) {
            problems.push(format!("docs/src/api/syscalls.md: `{}` has two rows", r.name));
        }
        match by_name.get(r.name.as_str()) {
            None => {
                problems.push(format!("docs/src/api/syscalls.md: row `{}` names no `SyscallNumber` variant", r.name));
                continue;
            }
            Some(n) if *n != r.number => problems.push(format!(
                "docs/src/api/syscalls.md: row `{}` says {} and the enum says {}",
                r.name, r.number, n
            )),
            Some(_) => {}
        }
        let num = by_name[r.name.as_str()];
        let (plain, aliases) = split_aliases(&r.wrappers);
        let named: Vec<&str> = spans(&plain).into_iter().filter(|s| s.starts_with("sys_")).collect();
        for w in &named {
            wrappers_checked += 1;
            if body_of(i.wrappers, w).is_none() {
                problems.push(format!(
                    "docs/src/api/syscalls.md: row `{}` names wrapper `{w}`, which is not a `pub fn` in crates/fjell-syscall/src/lib.rs",
                    r.name
                ));
            }
        }
        for a in &aliases {
            check_alias("docs/src/api/syscalls.md", &r.name, num, a, i.wrappers, &mut problems, &mut aliases_resolved);
        }
        if named.is_empty() && aliases.is_empty() && r.wrappers.trim_start().starts_with('—') {
            // "no wrapper": there must not be one under the obvious name
            let canonical = format!("sys_{}", snake(&r.name));
            if body_of(i.wrappers, &canonical).is_some() {
                problems.push(format!(
                    "docs/src/api/syscalls.md: row `{}` says it has no wrapper, and `pub fn {canonical}` exists in crates/fjell-syscall/src/lib.rs",
                    r.name
                ));
            }
        }
    }
    for (n, v) in &declared {
        if !seen_ref.contains(n.as_str()) {
            problems.push(format!(
                "docs/src/api/syscalls.md: `{n}` ({v}) is declared in the ABI and no row names it"
            ));
        }
    }

    // ── kernel.md ────────────────────────────────────────────────────────────────
    let kernel = parse_kernel_table(i.kernel_doc);
    let mut kernel_entries = 0usize;
    match kernel {
        None => problems.push("docs/src/external-design/kernel.md: the `| Group | Syscalls |` table was not found: a renamed or removed table is a failure, not a pass over nothing".into()),
        Some(entries) => {
            if entries.is_empty() {
                problems.push("docs/src/external-design/kernel.md: the `| Group | Syscalls |` table has no entries".into());
            }
            let mut seen_k: BTreeSet<String> = BTreeSet::new();
            for e in &entries {
                kernel_entries += 1;
                if !seen_k.insert(e.canonical.clone()) {
                    problems.push(format!("docs/src/external-design/kernel.md: `{}` is listed twice", e.canonical));
                }
                let Some((variant, num)) = by_snake.get(&e.canonical) else {
                    problems.push(format!(
                        "docs/src/external-design/kernel.md: entry `{}` is no `SyscallNumber` variant's snake-case name",
                        e.canonical
                    ));
                    continue;
                };
                for a in &e.aliases {
                    check_alias(
                        "docs/src/external-design/kernel.md",
                        variant,
                        *num,
                        &format!("sys_{a}"),
                        i.wrappers,
                        &mut problems,
                        &mut aliases_resolved,
                    );
                }
            }
            for (n, v) in &declared {
                if !seen_k.contains(&snake(n)) {
                    problems.push(format!(
                        "docs/src/external-design/kernel.md: `{}` (`{n}`, {v}) is declared in the ABI and no group-table entry names it",
                        snake(n)
                    ));
                }
            }
        }
    }

    if problems.is_empty() {
        Ok(Report {
            declared: declared.len(),
            reference_rows: rows.len(),
            kernel_entries,
            aliases_resolved,
            wrappers_checked,
        })
    } else {
        Err(problems)
    }
}

fn check_alias(
    doc: &str,
    variant: &str,
    num: u32,
    wrapper: &str,
    wrappers_src: &str,
    problems: &mut Vec<String>,
    resolved: &mut usize,
) {
    if body_of(wrappers_src, wrapper).is_none() {
        problems.push(format!(
            "{doc}: `{variant}` lists `{wrapper}` as an alias, and no `pub fn {wrapper}` exists in crates/fjell-syscall/src/lib.rs (an alias that is not a wrapper)"
        ));
        return;
    }
    let mut seen = BTreeSet::new();
    if reaches(wrappers_src, wrapper, variant, num, 4, &mut seen) {
        *resolved += 1;
    } else {
        problems.push(format!(
            "{doc}: `{variant}` lists `{wrapper}` as an alias, and that wrapper does not reach `SyscallNumber::{variant}` ({num}) -- directly, by `li a7, {num}`, or through another wrapper"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABI: &str = "pub enum SyscallNumber {\n    Yield = 0,\n    IpcCall = 22,\n    IpcRecv = 21,\n    CapDelete = 12,\n}\n";
    const WRAPPERS: &str = "pub fn sys_yield() {\n    ecall0(SyscallNumber::Yield as usize);\n}\npub fn sys_ipc_call(ep: u32) {\n    ecall(SyscallNumber::IpcCall as usize);\n}\npub fn sys_ipc_call_words(ep: u32) {\n    asm!(\"ecall\", in(\"a7\") SyscallNumber::IpcCall as usize);\n}\npub fn sys_ipc_recv(ep: u32) {\n    ecall(SyscallNumber::IpcRecv as usize);\n}\npub fn sys_ipc_recv_msg(ep: u32) {\n    asm!(\"li a7, 21\", \"ecall\");\n}\npub fn sys_cap_delete(c: u32) {\n    ecall1(SyscallNumber::CapDelete as usize, 0);\n}\npub fn sys_wrap_yield() {\n    sys_yield();\n}\n";
    const REFERENCE: &str = "# Reference\n\n| # | Syscall number | `fjell-syscall` wrapper(s) | Group |\n|---|---|---|---|\n| 0 | `Yield` | `sys_yield` | Scheduler |\n| 12 | `CapDelete` | `sys_cap_delete` | Capability |\n| 21 | `IpcRecv` | `sys_ipc_recv` (also reached via `sys_ipc_recv_msg`) | IPC |\n| 22 | `IpcCall` | `sys_ipc_call` (also reached via `sys_ipc_call_words`) | IPC |\n";
    const KERNEL: &str = "text\n\n| Group | Syscalls |\n|---|---|\n| Scheduler | `yield` |\n| IPC | `ipc_call` (also reached via `ipc_call_words`), `ipc_recv` (also reached via `ipc_recv_msg`) |\n| Capability | `cap_delete` |\n\nafter\n";

    fn run(reference: &str, kernel: &str, wrappers: &str) -> Result<Report, Vec<String>> {
        evaluate(&Inputs { abi: ABI, reference, kernel_doc: kernel, wrappers })
    }

    #[test]
    fn a_consistent_world_passes_and_counts_what_it_read() {
        let r = run(REFERENCE, KERNEL, WRAPPERS).unwrap();
        assert_eq!((r.declared, r.reference_rows, r.kernel_entries), (4, 4, 4));
        assert_eq!(r.aliases_resolved, 4, "two in each table; a literal `li a7, 21` counts");
    }

    /// Direction 1: a row naming no variant (a retired syscall left in the page).
    #[test]
    fn a_row_naming_no_variant_fails() {
        let doc = REFERENCE.to_string() + "| 111 | `DmaShare` | — | Hardware |\n";
        let err = run(&doc, KERNEL, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("`DmaShare` names no `SyscallNumber` variant")), "{err:?}");
        let k = KERNEL.replace("| `cap_delete` |", "| `cap_delete`, `dma_share` |");
        let err = run(REFERENCE, &k, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("`dma_share` is no `SyscallNumber` variant")), "{err:?}");
    }

    /// Direction 2: a variant no row names -- the seven the real page lacked.
    #[test]
    fn a_variant_no_row_names_fails_in_both_tables() {
        let doc = REFERENCE.replace("| 12 | `CapDelete` | `sys_cap_delete` | Capability |\n", "");
        let err = run(&doc, KERNEL, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("`CapDelete` (12) is declared in the ABI and no row names it")), "{err:?}");
        let k = KERNEL.replace("| Capability | `cap_delete` |\n", "");
        let err = run(REFERENCE, &k, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("`cap_delete`") && e.contains("no group-table entry names it")), "{err:?}");
    }

    /// Direction 3: a row whose number disagrees with the enum.
    #[test]
    fn a_row_with_the_wrong_number_fails() {
        let doc = REFERENCE.replace("| 12 | `CapDelete`", "| 13 | `CapDelete`");
        let err = run(&doc, KERNEL, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("`CapDelete` says 13 and the enum says 12")), "{err:?}");
    }

    /// Direction 4: an alias that is not a wrapper (`ipc_try_send`), and an alias that reaches a DIFFERENT syscall.
    #[test]
    fn an_alias_that_is_not_a_wrapper_or_reaches_another_syscall_fails() {
        let k = KERNEL.replace("`ipc_call_words`", "`ipc_try_send`");
        let err = run(REFERENCE, &k, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("`sys_ipc_try_send`") && e.contains("an alias that is not a wrapper")), "{err:?}");
        // swapped: ipc_call claims ipc_recv_msg, which issues IpcRecv
        let k = KERNEL.replace("`ipc_call` (also reached via `ipc_call_words`), `ipc_recv` (also reached via `ipc_recv_msg`)", "`ipc_call` (also reached via `ipc_recv_msg`), `ipc_recv` (also reached via `ipc_call_words`)");
        let err = run(REFERENCE, &k, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("does not reach `SyscallNumber::IpcCall`")), "{err:?}");
    }

    #[test]
    fn an_alias_reaches_its_syscall_through_another_wrapper() {
        let doc = REFERENCE.replace("| 0 | `Yield` | `sys_yield` |", "| 0 | `Yield` | `sys_yield` (also reached via `sys_wrap_yield`) |");
        assert!(run(&doc, KERNEL, WRAPPERS).is_ok());
    }

    /// The wrapper column: a named wrapper must exist, and "no wrapper" must be true (`sys_cap_delete` existed while the page said not).
    #[test]
    fn the_wrapper_column_is_checked_narrowly() {
        let doc = REFERENCE.replace("`sys_cap_delete`", "`sys_cap_remove`");
        let err = run(&doc, KERNEL, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("wrapper `sys_cap_remove`")), "{err:?}");
        let doc = REFERENCE.replace("`sys_cap_delete`", "— (dispatched; no wrapper)");
        let err = run(&doc, KERNEL, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("says it has no wrapper") && e.contains("pub fn sys_cap_delete")), "{err:?}");
        // and a true "no wrapper" passes
        let w = WRAPPERS.replace("pub fn sys_cap_delete", "pub fn sys_cap_delete_renamed");
        assert!(run(&doc, KERNEL, &w).is_ok());
    }

    #[test]
    fn duplicates_fail() {
        let doc = REFERENCE.to_string() + "| 0 | `Yield` | `sys_yield` | Scheduler |\n";
        assert!(run(&doc, KERNEL, WRAPPERS).unwrap_err().iter().any(|e| e.contains("`Yield` has two rows")));
        let k = KERNEL.replace("| Scheduler | `yield` |", "| Scheduler | `yield`, `yield` |");
        assert!(run(REFERENCE, &k, WRAPPERS).unwrap_err().iter().any(|e| e.contains("`yield` is listed twice")));
    }

    /// N3: a renamed header is a failure, not a pass over nothing.
    #[test]
    fn a_table_that_cannot_be_found_is_a_failure_not_a_pass() {
        let k = KERNEL.replace("| Group | Syscalls |", "| Kind | Calls |");
        let err = run(REFERENCE, &k, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("table was not found")), "{err:?}");
        let err = run("# empty\n", KERNEL, WRAPPERS).unwrap_err();
        assert!(err.iter().any(|e| e.contains("no syscall rows found")), "{err:?}");
    }

    #[test]
    fn snake_case_and_alias_splitting() {
        assert_eq!(snake("PlatformInfoGet"), "platform_info_get");
        assert_eq!(snake("Yield"), "yield");
        let (plain, al) = split_aliases("`sys_ipc_recv` (also reached via `sys_ipc_recv_msg`, `sys_x`) tail");
        assert_eq!(al, ["sys_ipc_recv_msg", "sys_x"]);
        assert!(plain.contains("sys_ipc_recv") && !plain.contains("sys_ipc_recv_msg"), "{plain}");
    }

    // ── the real documents, in the repository ───────────────────────────────────
    fn real(rel: &str) -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn the_real_documents_agree_with_the_enum() {
        let (abi, reference, kernel, w) = (
            real("crates/fjell-abi/src/syscall.rs"),
            real("docs/src/api/syscalls.md"),
            real("docs/src/external-design/kernel.md"),
            real("crates/fjell-syscall/src/lib.rs"),
        );
        let r = evaluate(&Inputs { abi: &abi, reference: &reference, kernel_doc: &kernel, wrappers: &w });
        assert!(r.is_ok(), "{:#?}", r.err());
    }
}
