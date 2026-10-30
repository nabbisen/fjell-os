//! RFC-0.41-002 R5: **no task can receive on endpoint object 0** -- the invariant the
//! shared default grant used to break, and a check that cannot silently stop holding.
//!
//! Until RFC-0.41-002, `spawn.rs`'s `ep_obj` match ended `_ => 0`: every image without a
//! dedicated arm was handed slot 0 pointing at endpoint object 0 with `ALL_NON_META`
//! (which includes `RECV`), and `init` was handed the same by `main.rs`. Twenty images
//! held it; three received on it. Nothing sent to it, which is the only reason two
//! receivers on one object never took each other's messages.
//!
//! **What is checked, and why it is the right thing to check.** The kernel will not let
//! a task receive on an endpoint unless it holds a capability to it that carries `RECV`,
//! and a capability can be copied or minted down but never up. So "no task receives on
//! object 0" holds if and only if **no install anywhere gives anyone a `RECV`-carrying
//! capability to object 0**. That is a property of the installs -- `spawn.rs` and
//! `main.rs` -- and needs no reading of any service's source, which is why this check is
//! not a text scan of receive calls (a receive's slot is often a parameter or a field, and
//! a scanner that pretended to resolve those would be the worse defect). It reads:
//!
//! 1. the `ep_obj` table: every arm must name a real, nonzero object, and the **default
//!    arm must be `None`** -- a default that names an object is a shared object again;
//! 2. every other `kind: CapKind::Endpoint` install in `spawn.rs` and `main.rs`: one
//!    that resolves to object 0 must not carry `RECV` (`proxy-text`'s demonstration
//!    capability points at object 0 with `SEND | REPLY | INSPECT` and passes, by name).
//!
//! **What this does not say, so a reader does not assume otherwise.**
//! - It does **not** say "no two tasks receive on one object". That is not true of this
//!   tree and is not meant to be: the update path's metadata channel
//!   (`UPDATE_CHANNEL_EP_OBJECT`) and the `fleetd` <-> `diagnosticsd` tools channel are
//!   deliberate bidirectional rendezvous, each side holding `SEND | RECV`; and several
//!   one-way senders are installed with `ALL_NON_META`, which carries `RECV` they never
//!   use. A structural "two holders of `RECV`" rule would fail on both. Object 0 is the
//!   one object whose being shared was the defect.
//! - It does not see capabilities created at runtime by `sys_cap_install`. Nothing in
//!   the tree calls it, and `init` -- the only holder of `CapInstall` -- revokes it at
//!   bootstrap; if that changes, this check must say so rather than quietly missing it.
//! - A rights or object expression it cannot evaluate is a **failure**, not a pass: a
//!   check that skips what it cannot read would vouch for nothing.

use std::collections::HashMap;

/// What the check read, for the one-line account.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    /// Dedicated arms in the `ep_obj` table.
    pub arms: usize,
    /// Other `CapKind::Endpoint` installs read (not the table's own).
    pub installs: usize,
}

/// `//` comments removed, so a commented-out arm or a sentence mentioning `_ => 0` in a
/// comment is not read as code.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `const NAME: u32 = <digits>;` (optionally `pub`), across `srcs`.
fn u32_consts(srcs: &[&str]) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    for src in srcs {
        for line in strip_line_comments(src).lines() {
            let l = line.trim();
            let l = l.strip_prefix("pub ").unwrap_or(l);
            let Some(rest) = l.strip_prefix("const ") else {
                continue;
            };
            let Some((name, rest)) = rest.split_once(':') else {
                continue;
            };
            let Some((ty, val)) = rest.split_once('=') else {
                continue;
            };
            if ty.trim() == "u32" {
                if let Ok(n) = val.trim().trim_end_matches(';').trim().parse::<u32>() {
                    out.insert(name.trim().to_string(), n);
                }
            }
        }
    }
    out
}

/// A value expression: a decimal literal, or the last path segment of a constant.
fn value_of(expr: &str, consts: &HashMap<String, u32>) -> Option<u32> {
    let e = expr.trim().trim_end_matches(',').trim();
    if let Ok(n) = e.parse::<u32>() {
        return Some(n);
    }
    let name = e.rsplit("::").next()?.trim();
    consts.get(name).copied()
}

/// The text of a `const NAME ... = <definition>;`, searched across `srcs`.
fn const_definition<'a>(name: &str, srcs: &[&'a str]) -> Option<&'a str> {
    for src in srcs {
        let needle = format!("const {name}:");
        let mut from = 0;
        while let Some(rel) = src[from..].find(&needle) {
            let at = from + rel;
            // Whole-word: the char before must not be an identifier char.
            let before_ok = src[..at]
                .chars()
                .last()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            if before_ok {
                let after = &src[at..];
                if let Some(eq) = after.find('=') {
                    let def = &after[eq + 1..];
                    if let Some(end) = def.find(';') {
                        return Some(&def[..end]);
                    }
                }
            }
            from = at + needle.len();
        }
    }
    None
}

/// Does a rights expression carry `RECV`? Resolved through the constants that define
/// it (`Self::SEND.0 | Self::RECV.0`, aliases such as `ALL = ALL_NON_META`), down to the
/// bit. `Err` for a name it cannot find: a rights name this check has not read is not
/// assumed harmless.
fn carries_recv(expr: &str, defs: &[&str], depth: u8) -> Result<bool, String> {
    if depth > 6 {
        return Err(format!("rights `{expr}` nests too deeply to resolve"));
    }
    // Every `Self::X` / `CapRights::X` / bare UPPER_NAME in the expression.
    let mut names: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in expr.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' || c == ':' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                names.push(std::mem::take(&mut cur));
            }
        }
    }
    let mut any = false;
    for n in names {
        let last = n.rsplit("::").next().unwrap_or(&n).to_string();
        // Only SCREAMING_CASE names are rights; `Self`, `CapRights`, `0` are not.
        if last.is_empty()
            || !last
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            || last.chars().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        match last.as_str() {
            "RECV" | "ALL_NON_META" | "ALL_DEFINED" => return Ok(true),
            _ => {}
        }
        match const_definition(&last, defs) {
            Some(def) => {
                if carries_recv(def, defs, depth + 1)? {
                    return Ok(true);
                }
                any = true;
            }
            None => {
                return Err(format!(
                    "rights name `{last}` is not defined in any source this check reads"
                ));
            }
        }
    }
    let _ = any;
    Ok(false)
}

/// Brace-matched body after the first `{` at or after `from`.
fn braced(src: &str, from: usize) -> Option<&str> {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate().skip(from) {
        match b {
            b'{' => {
                depth += 1;
                if start.is_none() {
                    start = Some(i + 1);
                }
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start?..i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split a `match` body into `(pattern, expression)` arms at depth 0.
fn arms(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut pat = String::new();
    while i < chars.len() {
        // pattern up to `=>`
        if chars[i] == '=' && chars.get(i + 1) == Some(&'>') {
            i += 2;
            let mut depth = 0i32;
            let mut expr = String::new();
            while i < chars.len() {
                let c = chars[i];
                match c {
                    '{' | '(' | '[' => depth += 1,
                    '}' | ')' | ']' => depth -= 1,
                    _ => {}
                }
                if c == ',' && depth == 0 {
                    i += 1;
                    break;
                }
                expr.push(c);
                i += 1;
                // A `{ ... }` arm needs no trailing comma.
                if c == '}' && depth == 0 {
                    // consume an optional comma
                    let mut j = i;
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if chars.get(j) == Some(&',') {
                        i = j + 1;
                    }
                    break;
                }
            }
            out.push((pat.trim().to_string(), expr.trim().to_string()));
            pat.clear();
        } else {
            pat.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// The value inside `Some(...)` / `None`. `Err` if it is neither.
fn grant(expr: &str, consts: &HashMap<String, u32>) -> Result<Option<u32>, String> {
    let e = expr.trim();
    // A block arm `{ Some(X) }`.
    let e = e
        .strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .unwrap_or(e)
        .trim();
    if e == "None" {
        return Ok(None);
    }
    if let Some(inner) = e.strip_prefix("Some(").and_then(|r| r.strip_suffix(')')) {
        return value_of(inner, consts)
            .map(Some)
            .ok_or_else(|| format!("cannot evaluate the object `{inner}`"));
    }
    Err(format!("`{e}` is neither `None` nor `Some(<object>)`"))
}

/// The objects of the loop that encloses an install written `object_id,`: the second
/// element of each tuple in the nearest preceding `for (.., object_id) in [ .. ]`.
fn loop_objects(before: &str) -> Option<Vec<String>> {
    let f = before.rfind("object_id) in [")?;
    let arr = &before[f + "object_id) in [".len()..];
    let close = arr.find(']')?;
    let mut out = Vec::new();
    for t in arr[..close].split('(').skip(1) {
        let inner = t.split(')').next()?;
        let second = inner.split(',').nth(1)?.trim();
        if second.is_empty() {
            return None;
        }
        out.push(second.to_string());
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Judge the tree. Inputs are the texts of `spawn.rs`, the kernel's `main.rs`,
/// `fjell-abi`'s `service.rs` (the named object constants) and `fjell-cap`'s
/// `rights.rs` (what each rights name carries).
pub fn judge(
    spawn: &str,
    main_rs: &str,
    abi_service: &str,
    rights: &str,
) -> Result<Report, Vec<String>> {
    let mut why: Vec<String> = Vec::new();
    let spawn_c = strip_line_comments(spawn);
    let main_c = strip_line_comments(main_rs);
    let consts = u32_consts(&[spawn, abi_service]);
    let defs = [rights, spawn];

    // 1. The table.
    let mut n_arms = 0;
    let head = "let ep_obj: Option<u32> = match image_id {";
    match spawn_c.find(head) {
        None => why.push(format!(
            "spawn.rs has no `{head}`: this check cannot read the endpoint table, and a \
             table it cannot read is one it cannot vouch for"
        )),
        Some(at) => match braced(&spawn_c, at) {
            None => why.push("the `ep_obj` match is not brace-balanced".into()),
            Some(body) => {
                let mut saw_default = false;
                for (pat, expr) in arms(body) {
                    let which = pat.rsplit("::").next().unwrap_or(&pat).to_string();
                    match grant(&expr, &consts) {
                        Err(e) => why.push(format!("ep_obj arm `{which}`: {e}")),
                        Ok(g) => {
                            if pat == "_" {
                                saw_default = true;
                                if let Some(obj) = g {
                                    why.push(format!(
                                        "the default arm `_ => Some({obj})` hands every image \
                                         without a dedicated arm an endpoint capability: a \
                                         shared object (RFC-0.41-002 R3)"
                                    ));
                                }
                            } else {
                                n_arms += 1;
                                if g == Some(0) {
                                    why.push(format!(
                                        "ep_obj arm `{which}` points slot 0 at object 0, \
                                         where the shared default used to receive"
                                    ));
                                }
                            }
                        }
                    }
                }
                if !saw_default {
                    why.push(
                        "the `ep_obj` match has no `_ =>` arm; the default must be stated, and be `None`"
                            .into(),
                    );
                }
            }
        },
    }

    // 2. Every other Endpoint install.
    let mut n_installs = 0;
    for (file, src) in [("spawn.rs", &spawn_c), ("main.rs", &main_c)] {
        let mut from = 0;
        while let Some(rel) = src[from..].find("kind: CapKind::Endpoint") {
            let at = from + rel;
            from = at + 1;
            let line = src[..at].matches('\n').count() + 1;
            // The rest of this struct literal.
            let tail = &src[at..];
            let end = tail.find("state:").unwrap_or(tail.len().min(400));
            let lit = &tail[..end];
            let field = |name: &str| -> Option<String> {
                let k = format!("{name}:");
                let i = lit.find(&k)?;
                let rest = &lit[i + k.len()..];
                let stop = rest.find(',').unwrap_or(rest.len());
                Some(
                    rest[..stop]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            };
            // `object_id: X` -- or the loop form `for (slot, object_id) in [ (n, X), .. ]`
            // with a bare `object_id,` field, whose objects are the tuples' second elements.
            let objects: Option<Vec<String>> = match field("object_id") {
                Some(o) => Some(vec![o]),
                None if lit.contains("object_id,") => loop_objects(&src[..at]),
                None => None,
            };
            let (Some(objects), Some(rts)) = (objects, field("rights")) else {
                why.push(format!(
                    "{file}:{line}: an Endpoint install this check cannot read the object or rights of"
                ));
                continue;
            };
            if objects.len() == 1 && objects[0] == "ep_obj" {
                continue; // the table's own install, judged above
            }
            let mut to_zero = false;
            for obj in &objects {
                n_installs += 1;
                match value_of(obj, &consts) {
                    None => {
                        why.push(format!("{file}:{line}: cannot evaluate the object `{obj}`"));
                    }
                    Some(0) => to_zero = true,
                    Some(_) => {}
                }
            }
            if !to_zero {
                continue;
            }
            match carries_recv(&rts, &defs, 0) {
                Err(e) => why.push(format!("{file}:{line}: {e}")),
                Ok(true) => why.push(format!(
                    "{file}:{line}: an Endpoint install gives `{rts}` -- which carries RECV -- \
                     on object 0, the shared object nothing may receive on"
                )),
                Ok(false) => {}
            }
        }
    }

    if why.is_empty() {
        Ok(Report {
            arms: n_arms,
            installs: n_installs,
        })
    } else {
        Err(why)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RIGHTS: &str = "\
        pub const SEND: Self = CapRights(1 << 3);\n\
        pub const RECV: Self = CapRights(1 << 4);\n\
        pub const REPLY: Self = CapRights(1 << 6);\n\
        pub const INSPECT: Self = CapRights(1 << 10);\n\
        pub const ALL_NON_META: Self = CapRights((1 << 26) - 1);\n\
        pub const ALL: Self = Self::ALL_NON_META;\n\
        pub const PROXY_TEXT_DEMO: Self = CapRights(Self::SEND.0 | Self::REPLY.0 | Self::INSPECT.0);\n";

    const ABI: &str =
        "pub const NETD_EP_OBJECT: u32 = 21;\npub const BOOTCTL_EP_OBJECT: u32 = 12;\n";

    fn spawn_with(default_arm: &str, extra: &str) -> String {
        format!(
            "let ep_obj: Option<u32> = match image_id {{\n\
               ImageId::STORAGED => Some(1),\n\
               ImageId::NETD => Some(fjell_abi::service::NETD_EP_OBJECT),\n\
               ImageId::BOOTCTL => {{\n    Some(fjell_abi::service::BOOTCTL_EP_OBJECT)\n}}\n\
               {default_arm}\n\
             }};\n\
             if let Some(ep_obj) = ep_obj {{\n\
               let _ = cs.install_raw(0, Capability {{ kind: CapKind::Endpoint,\n\
                 object_id: ep_obj, rights: CapRights::ALL_NON_META, badge: 0, state: A }});\n\
             }}\n{extra}\n"
        )
    }

    fn demo_install() -> &'static str {
        "let _ = cs.install_raw(2, Capability { kind: CapKind::Endpoint,\n\
           object_id: 0, rights: CapRights::PROXY_TEXT_DEMO, badge: 0, state: A });\n"
    }

    #[test]
    fn the_fixed_shape_passes() {
        let s = spawn_with("_ => None,", demo_install());
        let r = judge(&s, "", ABI, RIGHTS).expect("must pass");
        assert_eq!(
            r,
            Report {
                arms: 3,
                installs: 1
            }
        );
    }

    /// The failing case R5 asks for: the default arm the line removed.
    #[test]
    fn the_old_shared_default_is_named_and_refused() {
        let s = spawn_with("_ => Some(0),", "");
        let e = judge(&s, "", ABI, RIGHTS).unwrap_err();
        assert!(e.iter().any(|m| m.contains("default arm")), "{e:?}");
    }

    #[test]
    fn a_default_that_names_any_object_is_refused() {
        let s = spawn_with("_ => Some(9),", "");
        assert!(judge(&s, "", ABI, RIGHTS).is_err());
    }

    #[test]
    fn a_dedicated_arm_pointing_at_object_zero_is_refused() {
        let s = spawn_with("_ => None,", "").replace("Some(1)", "Some(0)");
        let e = judge(&s, "", ABI, RIGHTS).unwrap_err();
        assert!(
            e.iter()
                .any(|m| m.contains("STORAGED") && m.contains("object 0")),
            "{e:?}"
        );
    }

    /// The two-receiver shape, built the other way: an explicit install that gives a
    /// second image a RECV-carrying capability to object 0.
    #[test]
    fn an_explicit_install_with_recv_on_object_zero_is_refused() {
        let extra = "let _ = cs.install_raw(5, Capability { kind: CapKind::Endpoint,\n\
                       object_id: 0, rights: CapRights::RECV, badge: 0, state: A });\n";
        let e = judge(&spawn_with("_ => None,", extra), "", ABI, RIGHTS).unwrap_err();
        assert!(e.iter().any(|m| m.contains("carries RECV")), "{e:?}");
    }

    #[test]
    fn all_non_meta_and_its_alias_carry_recv() {
        for r in ["CapRights::ALL_NON_META", "CapRights::ALL"] {
            let extra = format!(
                "let _ = cs.install_raw(5, Capability {{ kind: CapKind::Endpoint,\n\
                   object_id: 0, rights: {r}, badge: 0, state: A }});\n"
            );
            assert!(
                judge(&spawn_with("_ => None,", &extra), "", ABI, RIGHTS).is_err(),
                "{r}"
            );
        }
    }

    /// `main.rs` is read too: `init` was handed the same grant there.
    #[test]
    fn the_kernels_main_is_read_as_well() {
        let main = "let _ = cs.install_raw(0, Capability { kind: CapKind::Endpoint,\n\
                      object_id: 0, rights: CapRights::ALL_NON_META, badge: 0, state: A });\n";
        let e = judge(&spawn_with("_ => None,", ""), main, ABI, RIGHTS).unwrap_err();
        assert!(e.iter().any(|m| m.starts_with("main.rs:")), "{e:?}");
    }

    #[test]
    fn a_commented_out_default_is_not_code() {
        let s = spawn_with("_ => None, // was `_ => 0,` until RFC-0.41-002", "");
        assert!(judge(&s, "", ABI, RIGHTS).is_ok());
    }

    #[test]
    fn a_rights_name_it_cannot_resolve_is_a_failure_not_a_pass() {
        let extra = "let _ = cs.install_raw(5, Capability { kind: CapKind::Endpoint,\n\
                       object_id: 0, rights: CapRights::MYSTERY, badge: 0, state: A });\n";
        let e = judge(&spawn_with("_ => None,", extra), "", ABI, RIGHTS).unwrap_err();
        assert!(e.iter().any(|m| m.contains("MYSTERY")), "{e:?}");
    }

    /// The loop form, `for (slot, object_id) in [..]` with a bare `object_id,` field,
    /// is read through its array -- and an object 0 in it with RECV is refused.
    #[test]
    fn the_loop_form_is_read_through_its_array() {
        let ok = "for (slot, object_id) in [(1, 12), (2, 21)] {\n let _ = cs.install_raw(slot, Capability {\n\
                  kind: CapKind::Endpoint,\n object_id,\n rights: CapRights::SEND,\n state: A });\n}\n";
        assert!(judge(&spawn_with("_ => None,", ok), "", ABI, RIGHTS).is_ok());
        let bad = ok
            .replace("(2, 21)", "(2, 0)")
            .replace("CapRights::SEND", "CapRights::RECV");
        let e = judge(&spawn_with("_ => None,", &bad), "", ABI, RIGHTS).unwrap_err();
        assert!(e.iter().any(|m| m.contains("carries RECV")), "{e:?}");
    }

    #[test]
    fn a_missing_table_is_a_failure() {
        assert!(judge("fn nothing() {}", "", ABI, RIGHTS).is_err());
    }

    /// The real tree, run through the real judge: the fixed shape passes today.
    #[test]
    fn the_current_tree_has_no_receiver_on_object_zero() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("tools/fjell-consistency-check is two levels under the workspace root");
        let read = |p: &str| std::fs::read_to_string(root.join(p)).expect(p);
        let r = judge(
            &read("crates/fjell-kernel/src/task/spawn.rs"),
            &read("crates/fjell-kernel/src/main.rs"),
            &read("crates/fjell-abi/src/service.rs"),
            &read("crates/fjell-cap/src/rights.rs"),
        );
        assert!(r.is_ok(), "{r:?}");
    }
}
