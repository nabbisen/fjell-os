//! `endpoint-envelope` -- the install envelope is the union of the broker's `Endpoint` grants
//! (0.46 small item C).
//!
//! `fjell_cap::install::ENDPOINT_ENVELOPE` is documented as *"exactly the union the broker's policy
//! grants for the `Endpoint` class"*. Three places spell that union out and nothing compared them:
//!
//! * `crates/fjell-cap/src/install.rs` -- the constant the kernel masks an install with (and that
//!   `spawn.rs` hands the broker, by *reference*: it is checked below that no literal crept in);
//! * `crates/services/fjell-cap-broker/src/main.rs` -- the **authoritative** policy table;
//! * `crates/fjell-tools/src/policy_eval.rs` -- a hand-mirrored copy of that table for host tests.
//!
//! (The handoff counted five files. Two of them are not copies: `spawn.rs` only references the
//! constant and `fjell-neg-test` only mentions `EP_RW` in a comment.)
//!
//! Drift produces an `Allow` in the production table that dies `PermissionDenied` at runtime:
//! fail-closed, and exactly the confusing-UX failure this project weighs with correctness. The check:
//! the union of the rights of every `Allow` rule whose resource is `Endpoint` (or a wildcard) in the
//! broker equals the envelope, and so does the mirror's.
//!
//! **Not a text match on numbers.** Each side is *evaluated*: the broker's `const NAME: u64 = expr;`
//! lines and `CapRights`' `pub const NAME: Self = CapRights(expr);` lines are read into a symbol
//! table, and every rights expression is evaluated against it. An expression it cannot evaluate is a
//! **failure**, not a pass -- a check that skips what it cannot read vouches for nothing (the lesson
//! `object-zero` already carries). It also checks the broker's own `RIGHT_<N>` bit constants against
//! `CapRights::<N>`: the union is only meaningful if the broker's bits *are* the kernel's.
//!
//! **What this does not cover**, stated so a reader does not assume otherwise: it compares rights,
//! not objects or requesters; it reads the mirror's `Rule { .. }` blocks only for the `Endpoint`
//! class and says nothing about the mirror's other rules (it has 16 rules where the broker has 32 --
//! drift this check does not claim to find); and it cannot see a table built by code rather than
//! written as a `const` list.

use std::collections::BTreeMap;
use std::process::ExitCode;

const NAME: &str = "endpoint-envelope";
const INSTALL_PATH: &str = "crates/fjell-cap/src/install.rs";
const RIGHTS_PATH: &str = "crates/fjell-cap/src/rights.rs";
const BROKER_PATH: &str = "crates/services/fjell-cap-broker/src/main.rs";
const MIRROR_PATH: &str = "crates/fjell-tools/src/policy_eval.rs";
const SPAWN_PATH: &str = "crates/fjell-kernel/src/task/spawn.rs";

type Symbols = BTreeMap<String, u64>;

// ── A tiny evaluator: identifiers, numbers, `|`, `+`, `-`, `<<`, parentheses ──────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Num(u64),
    Id(String),
    Or,
    Plus,
    Minus,
    Shl,
    L,
    R,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '|' => {
                out.push(Tok::Or);
                i += 1;
            }
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            '(' => {
                out.push(Tok::L);
                i += 1;
            }
            ')' => {
                out.push(Tok::R);
                i += 1;
            }
            '<' if b.get(i + 1) == Some(&'<') => {
                out.push(Tok::Shl);
                i += 2;
            }
            c if c.is_ascii_digit() => {
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_') {
                    j += 1;
                }
                let raw: String = b[i..j].iter().filter(|c| **c != '_').collect();
                let digits = raw
                    .trim_end_matches("u64")
                    .trim_end_matches("u32")
                    .trim_end_matches("usize");
                let v = if let Some(h) = digits.strip_prefix("0x") {
                    u64::from_str_radix(h, 16)
                } else {
                    digits.parse::<u64>()
                }
                .map_err(|_| format!("cannot read the number `{raw}`"))?;
                out.push(Tok::Num(v));
                i = j;
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut j = i;
                while j < b.len()
                    && (b[j].is_ascii_alphanumeric() || b[j] == '_' || b[j] == ':' || b[j] == '.')
                {
                    j += 1;
                }
                let mut id: String = b[i..j].iter().collect();
                // `CapRights::SEND.0` and `Self::SEND.0` both mean the bit named SEND.
                if let Some(rest) = id.strip_suffix(".0") {
                    id = rest.to_string();
                }
                let id = id.rsplit("::").next().unwrap_or(&id).to_string();
                out.push(Tok::Id(id));
                i = j;
            }
            other => return Err(format!("cannot read `{other}` in a rights expression")),
        }
    }
    Ok(out)
}

fn eval(expr: &str, syms: &Symbols) -> Result<u64, String> {
    let toks = lex(expr)?;
    let mut pos = 0;
    let v = parse_or(&toks, &mut pos, syms)?;
    if pos != toks.len() {
        return Err(format!("trailing tokens in `{}`", expr.trim()));
    }
    Ok(v)
}

fn parse_or(t: &[Tok], p: &mut usize, s: &Symbols) -> Result<u64, String> {
    let mut v = parse_add(t, p, s)?;
    while t.get(*p) == Some(&Tok::Or) {
        *p += 1;
        v |= parse_add(t, p, s)?;
    }
    Ok(v)
}

fn parse_add(t: &[Tok], p: &mut usize, s: &Symbols) -> Result<u64, String> {
    let mut v = parse_shl(t, p, s)?;
    loop {
        match t.get(*p) {
            Some(Tok::Plus) => {
                *p += 1;
                v = v.wrapping_add(parse_shl(t, p, s)?);
            }
            Some(Tok::Minus) => {
                *p += 1;
                v = v.wrapping_sub(parse_shl(t, p, s)?);
            }
            _ => return Ok(v),
        }
    }
}

fn parse_shl(t: &[Tok], p: &mut usize, s: &Symbols) -> Result<u64, String> {
    let mut v = parse_atom(t, p, s)?;
    while t.get(*p) == Some(&Tok::Shl) {
        *p += 1;
        let n = parse_atom(t, p, s)?;
        v = v
            .checked_shl(n as u32)
            .ok_or_else(|| format!("shift by {n} overflows"))?;
    }
    Ok(v)
}

fn parse_atom(t: &[Tok], p: &mut usize, s: &Symbols) -> Result<u64, String> {
    match t.get(*p) {
        Some(Tok::Num(n)) => {
            *p += 1;
            Ok(*n)
        }
        Some(Tok::Id(id)) => {
            *p += 1;
            s.get(id)
                .copied()
                .ok_or_else(|| format!("`{id}` is not a constant this check can read"))
        }
        Some(Tok::L) => {
            *p += 1;
            let v = parse_or(t, p, s)?;
            if t.get(*p) != Some(&Tok::R) {
                return Err("unbalanced parenthesis".to_string());
            }
            *p += 1;
            Ok(v)
        }
        other => Err(format!("unexpected {other:?}")),
    }
}

// ── Reading the sources ──────────────────────────────────────────────────────────────────────────

/// `const NAME: u64 = expr;` lines (the broker's rights constants), evaluated in order, iterating
/// until nothing more resolves so declaration order does not matter.
fn broker_consts(src: &str) -> Symbols {
    let mut pending: Vec<(String, String)> = Vec::new();
    for line in src.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("const ")
            && let Some((name, rhs)) = rest.split_once(": u64 =")
            && let Some(expr) = rhs.trim().strip_suffix(';')
        {
            pending.push((name.trim().to_string(), expr.trim().to_string()));
        }
    }
    resolve(pending)
}

/// `pub const NAME: Self = CapRights(expr);` lines in `rights.rs`.
fn caprights_consts(src: &str) -> Symbols {
    let mut pending: Vec<(String, String)> = Vec::new();
    for line in src.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("pub const ")
            && let Some((name, rhs)) = rest.split_once(": Self = CapRights(")
            && let Some(expr) = rhs.trim().strip_suffix(");")
        {
            pending.push((name.trim().to_string(), expr.trim().to_string()));
        }
    }
    resolve(pending)
}

fn resolve(mut pending: Vec<(String, String)>) -> Symbols {
    let mut syms = Symbols::new();
    loop {
        let before = pending.len();
        pending.retain(|(name, expr)| match eval(expr, &syms) {
            Ok(v) => {
                syms.insert(name.clone(), v);
                false
            }
            Err(_) => true,
        });
        if pending.len() == before {
            return syms;
        }
    }
}

/// The value of `pub const ENDPOINT_ENVELOPE: CapRights = CapRights( ... );`.
fn envelope(install_src: &str, rights: &Symbols) -> Result<u64, String> {
    let start = install_src
        .find("pub const ENDPOINT_ENVELOPE: CapRights = CapRights(")
        .ok_or("install.rs no longer declares `ENDPOINT_ENVELOPE: CapRights = CapRights(..)`")?;
    let after = &install_src[start..];
    let open = after
        .find("CapRights(")
        .map(|i| i + "CapRights(".len())
        .unwrap_or(0);
    let close = after
        .find(");")
        .ok_or("ENDPOINT_ENVELOPE has no closing `);`")?;
    eval(after[open..close].trim().trim_end_matches(','), rights)
        .map_err(|e| format!("ENDPOINT_ENVELOPE: {e}"))
}

/// One rule block's fields, as text.
struct RuleText {
    resource: String,
    kind: String,
    rights: String,
}

/// Every `<opener> { ... }` block in `src`, with the three fields read as text. `res_key`,
/// `kind_key`, `rights_key` are the field names (the broker and its mirror spell them differently).
fn rule_blocks(
    src: &str,
    opener: &str,
    res_key: &str,
    kind_key: &str,
    rights_key: &str,
) -> Vec<RuleText> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find(opener) {
        let after = &rest[i + opener.len()..];
        let end = after.find("}").unwrap_or(after.len());
        let body = &after[..end];
        let field = |key: &str| -> String {
            body.lines()
                .find_map(|l| {
                    let l = l.trim();
                    l.strip_prefix(key)
                        .and_then(|r| r.trim_start().strip_prefix(':'))
                        .map(|v| v.trim().trim_end_matches(',').trim().to_string())
                })
                .unwrap_or_default()
        };
        // `rights:` may be followed by a trailing comment on the same line (`/* MINT */`).
        let strip_comments = |s: String| -> String {
            let mut s = s;
            while let (Some(a), Some(b)) = (s.find("/*"), s.find("*/")) {
                s.replace_range(a..b + 2, "");
            }
            s.split("//")
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(',')
                .trim()
                .to_string()
        };
        out.push(RuleText {
            resource: field(res_key),
            kind: field(kind_key),
            rights: strip_comments(field(rights_key)),
        });
        rest = &after[end..];
    }
    out
}

/// The union of the rights of every `Allow` rule whose resource is the `Endpoint` class or a
/// wildcard, and how many such rules there were. `Err` names a rule it could not evaluate.
fn endpoint_union(
    rules: &[RuleText],
    is_endpoint: impl Fn(&str) -> bool,
    is_allow: impl Fn(&str) -> bool,
    is_wild: impl Fn(&str) -> bool,
    syms: &Symbols,
) -> Result<(u64, usize), String> {
    let mut union = 0u64;
    let mut n = 0usize;
    for r in rules {
        if !is_allow(&r.kind) {
            continue;
        }
        if !(is_endpoint(&r.resource) || is_wild(&r.resource)) {
            continue;
        }
        let v = eval(&r.rights, syms)
            .map_err(|e| format!("an Endpoint-class Allow rule's rights `{}`: {e}", r.rights))?;
        union |= v;
        n += 1;
    }
    Ok((union, n))
}

// ── The judgement ────────────────────────────────────────────────────────────────────────────────

pub struct Sources<'a> {
    pub install: &'a str,
    pub rights: &'a str,
    pub broker: &'a str,
    pub mirror: &'a str,
    pub spawn: &'a str,
}

fn names(bits: u64, rights: &Symbols) -> String {
    let mut v: Vec<&str> = rights
        .iter()
        .filter(|(_, b)| b.count_ones() == 1 && bits & **b != 0)
        .map(|(n, _)| n.as_str())
        .collect();
    v.sort();
    v.join("|")
}

pub fn judge(s: &Sources) -> Result<String, Vec<String>> {
    let mut why = Vec::new();
    let rights = caprights_consts(s.rights);
    if rights.get("SEND").is_none() {
        return Err(vec![format!(
            "could not read `CapRights::SEND` from {RIGHTS_PATH}: the right constants are not in the shape this check reads"
        )]);
    }
    let env = match envelope(s.install, &rights) {
        Ok(v) => v,
        Err(e) => return Err(vec![e]),
    };

    // The broker's bit constants must be the kernel's bits.
    let bconsts = broker_consts(s.broker);
    let mut compared = 0usize;
    for (name, v) in &bconsts {
        if let Some(short) = name.strip_prefix("RIGHT_")
            && let Some(k) = rights.get(short)
        {
            compared += 1;
            if k != v {
                why.push(format!(
                    "{BROKER_PATH}: `{name}` = {v:#x} but `CapRights::{short}` = {k:#x}: the broker's bit is not the kernel's"
                ));
            }
        }
    }
    if compared < 10 {
        why.push(format!(
            "only {compared} of the broker's `RIGHT_*` constants could be matched to a `CapRights` constant; the comparison below would rest on a table this check barely read"
        ));
    }

    // The broker's table (authoritative).
    let brules = rule_blocks(s.broker, "PolicyRule {", "resource", "kind", "rights");
    let bres = endpoint_union(
        &brules,
        |r| r.contains("ResourceClass::Endpoint"),
        |k| k.contains("PolicyKind::Allow"),
        |r| r == "WILDCARD",
        &bconsts,
    );
    let broker_union = match bres {
        Ok((u, n)) if n > 0 => Some((u, n)),
        Ok(_) => {
            why.push(format!("{BROKER_PATH} has no Endpoint-class Allow rule this check could read: a union of nothing proves nothing"));
            None
        }
        Err(e) => {
            why.push(format!("{BROKER_PATH}: {e}"));
            None
        }
    };
    if let Some((u, n)) = broker_union
        && u != env
    {
        why.push(format!(
            "the union of the broker's {n} Endpoint-class Allow rules is {u:#x} ({}) but ENDPOINT_ENVELOPE is {env:#x} ({}): an Allow outside the envelope would die PermissionDenied at runtime, and an envelope wider than the table is authority nothing grants",
            names(u, &rights),
            names(env, &rights)
        ));
    }

    // The mirror.
    let mut msyms = Symbols::new();
    for (n, v) in [
        ("RIGHT_SEND", "SEND"),
        ("RIGHT_RECV", "RECV"),
        ("RIGHT_CALL", "CALL"),
        ("RIGHT_REPLY", "REPLY"),
        ("RIGHT_COPY", "COPY"),
    ] {
        if let Some(k) = rights.get(v) {
            msyms.insert(n.to_string(), *k);
        }
    }
    // The mirror's own constants (`const EP_RW: u64 = ...;`, indented inside a module).
    let mirror_consts = broker_consts(s.mirror);
    for (n, v) in &mirror_consts {
        msyms.insert(n.clone(), *v);
    }
    let mrules = rule_blocks(s.mirror, "Rule {", "res", "kind", "rights");
    let mirror_n = match endpoint_union(
        &mrules,
        |r| r == "RC_ENDPOINT",
        |k| k == "Kind::Allow",
        |r| r == "WILDCARD",
        &msyms,
    ) {
        Ok((u, n)) if n > 0 => {
            if u != env {
                why.push(format!(
                    "the union of the {MIRROR_PATH} mirror's {n} Endpoint-class Allow rules is {u:#x} ({}) but ENDPOINT_ENVELOPE is {env:#x} ({}): the host-test mirror has drifted from the table it claims to mirror",
                    names(u, &rights),
                    names(env, &rights)
                ));
            }
            n
        }
        Ok(_) => {
            why.push(format!(
                "{MIRROR_PATH} has no Endpoint-class Allow rule this check could read"
            ));
            0
        }
        Err(e) => {
            why.push(format!("{MIRROR_PATH}: {e}"));
            0
        }
    };

    // spawn.rs must hand the broker the constant, not a literal copy of it.
    if !s.spawn.contains("install::ENDPOINT_ENVELOPE") {
        why.push(format!(
            "{SPAWN_PATH} no longer references `install::ENDPOINT_ENVELOPE`: the broker's CapInstall grant must use the constant, not a copy of its value"
        ));
    }

    if !why.is_empty() {
        return Err(why);
    }
    let (u, n) = broker_union.expect("no problems, so the broker's union was read");
    Ok(format!(
        "{NAME}: PASS (the broker's {n} Endpoint-class Allow rules union to {u:#x} = ENDPOINT_ENVELOPE ({}); \
         the host-test mirror's {mirror_n} such rules agree; {compared} broker RIGHT_* constants equal their CapRights bits; \
         spawn.rs hands the broker the constant. NOT covered: objects and requesters, the mirror's other rules, \
         and any table built by code rather than a const list)",
        names(env, &rights)
    ))
}

pub fn check() -> ExitCode {
    let read = |p: &str| crate::read_file(NAME, p);
    let (Some(install), Some(rights), Some(broker), Some(mirror), Some(spawn)) = (
        read(INSTALL_PATH),
        read(RIGHTS_PATH),
        read(BROKER_PATH),
        read(MIRROR_PATH),
        read(SPAWN_PATH),
    ) else {
        return ExitCode::FAILURE;
    };
    match judge(&Sources {
        install: &install,
        rights: &rights,
        broker: &broker,
        mirror: &mirror,
        spawn: &spawn,
    }) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            for w in &why {
                eprintln!("{NAME}: FAIL — {w}");
            }
            eprintln!("{NAME}: FAIL");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real(path: &str) -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root");
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    struct Real {
        install: String,
        rights: String,
        broker: String,
        mirror: String,
        spawn: String,
    }

    fn load() -> Real {
        Real {
            install: real(INSTALL_PATH),
            rights: real(RIGHTS_PATH),
            broker: real(BROKER_PATH),
            mirror: real(MIRROR_PATH),
            spawn: real(SPAWN_PATH),
        }
    }

    fn run(r: &Real) -> Result<String, Vec<String>> {
        judge(&Sources {
            install: &r.install,
            rights: &r.rights,
            broker: &r.broker,
            mirror: &r.mirror,
            spawn: &r.spawn,
        })
    }

    #[test]
    fn the_evaluator_reads_the_shapes_the_sources_use() {
        let mut s = Symbols::new();
        s.insert("A".into(), 1 << 3);
        s.insert("B".into(), 1 << 4);
        assert_eq!(eval("A | B", &s).unwrap(), 0b11000);
        assert_eq!(eval("CapRights::A.0 | Self::B.0", &s).unwrap(), 0b11000);
        assert_eq!(eval("(1u64 << 26) - 1", &s).unwrap(), (1 << 26) - 1);
        assert_eq!(eval("0xFF_u64", &s).unwrap(), 0xFF);
        assert!(eval("A | NOPE", &s).unwrap_err().contains("NOPE"));
        assert!(eval("A | (B", &s).is_err());
        assert!(eval("A $ B", &s).is_err());
    }

    #[test]
    fn the_real_tree_agrees() {
        let line = run(&load()).unwrap_or_else(|e| panic!("{e:#?}"));
        assert!(
            line.contains("= ENDPOINT_ENVELOPE") && line.contains("NOT covered"),
            "{line}"
        );
        // The five rights of EP_RW and MINT, by name.
        assert!(line.contains("CALL|COPY|MINT|RECV|REPLY|SEND"), "{line}");
    }

    /// Replace the first occurrence of `from` with `to` in `s`, requiring that it occurs.
    fn edit(s: &mut String, from: &str, to: &str) {
        assert!(s.contains(from), "control: `{from}` is not in the source");
        *s = s.replacen(from, to, 1);
    }

    #[test]
    fn a_broker_allow_wider_than_the_envelope_is_red() {
        let mut r = load();
        // The NEG_TEST rule is the only Endpoint rule with a bare right; widen it with INSPECT.
        edit(
            &mut r.broker,
            "rights: RIGHT_SEND,\n        object: Some(",
            "rights: RIGHT_SEND | RIGHT_INSPECT,\n        object: Some(",
        );
        let e = run(&r).unwrap_err();
        assert!(
            e.iter()
                .any(|w| w.contains("ENDPOINT_ENVELOPE") && w.contains("INSPECT")),
            "{e:#?}"
        );
    }

    #[test]
    fn an_envelope_wider_than_the_table_is_red() {
        let mut r = load();
        edit(
            &mut r.install,
            "| CapRights::MINT.0,",
            "| CapRights::MINT.0 | CapRights::INSPECT.0,",
        );
        let e = run(&r).unwrap_err();
        assert!(
            e.iter()
                .any(|w| w.contains("authority nothing grants") || w.contains("drifted")),
            "{e:#?}"
        );
    }

    #[test]
    fn a_narrower_envelope_is_red() {
        let mut r = load();
        edit(&mut r.install, "        | CapRights::MINT.0,", "        ,");
        assert!(run(&r).is_err());
    }

    #[test]
    fn a_drifted_mirror_is_red() {
        let mut r = load();
        edit(
            &mut r.mirror,
            "rights: EP_RW | (1 << 8), /* MINT */",
            "rights: EP_RW,",
        );
        let e = run(&r).unwrap_err();
        assert!(e.iter().any(|w| w.contains("mirror")), "{e:#?}");
    }

    #[test]
    fn a_brokers_bit_that_is_not_the_kernels_is_red() {
        let mut r = load();
        edit(
            &mut r.broker,
            "const RIGHT_COPY: u64 = 1 << 7;",
            "const RIGHT_COPY: u64 = 1 << 9;",
        );
        let e = run(&r).unwrap_err();
        assert!(
            e.iter()
                .any(|w| w.contains("RIGHT_COPY") && w.contains("not the kernel's")),
            "{e:#?}"
        );
    }

    #[test]
    fn a_rights_expression_it_cannot_read_is_a_failure_not_a_pass() {
        let mut r = load();
        edit(
            &mut r.broker,
            "rights: RIGHT_SEND,\n        object: Some(",
            "rights: unknown_fn(),\n        object: Some(",
        );
        let e = run(&r).unwrap_err();
        assert!(e.iter().any(|w| w.contains("unknown_fn")), "{e:#?}");
    }

    #[test]
    fn a_literal_copy_in_spawn_rs_is_red() {
        let mut r = load();
        edit(
            &mut r.spawn,
            "install::ENDPOINT_ENVELOPE",
            "install::SOMETHING_ELSE",
        );
        let e = run(&r).unwrap_err();
        assert!(
            e.iter()
                .any(|w| w.contains("spawn.rs") || w.contains("spawn")),
            "{e:#?}"
        );
    }

    #[test]
    fn no_endpoint_rule_at_all_proves_nothing() {
        let mut r = load();
        r.broker = r.broker.replace(
            "ResourceClass::Endpoint as u16",
            "ResourceClass::Config as u16",
        );
        let e = run(&r).unwrap_err();
        assert!(
            e.iter()
                .any(|w| w.contains("proves nothing") || w.contains("no Endpoint-class")),
            "{e:#?}"
        );
    }
}
