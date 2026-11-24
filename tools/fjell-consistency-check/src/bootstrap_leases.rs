//! RFC-0.48-001 R6: **`bootstrap-leases`** -- no boot-time capability is granted without a lease, and what is exempt is
//! named, reasoned, and kept true.
//!
//! Until 0.48 every capability the kernel installed at boot carried `lease: None`, and `Capability::check_lease` returns
//! `Ok(())` for an unbound capability **for ever** -- so nothing could take a boot grant back (E-093). `spawn()` now
//! creates a *boot lease* per task and binds every capability the task holds in one pass, so no grant literal needs a
//! lease and **a grant added tomorrow cannot forget the bind**. This check is what keeps that true.
//!
//! **Population: every `Capability { ... }` literal under `crates/fjell-kernel/src`** outside `#[cfg(test)]`, through the
//! shared `fjell-repo-scope`, in code only (comments and strings blanked). Nothing here names a grant.
//!
//! 1. **A literal that is `lease: None` must be in a function on [`CLASSIFIED_FUNCTIONS`]**, each with a reason -- and
//!    **`spawn_with_lease` is on it only because the check also requires that function to contain the bind pass**
//!    (`cap.lease = Some(` and a `boot_lease`): delete the pass and every grant in it turns red at once.
//! 2. **A literal that is not `lease: None` is not this check's business** (a runtime install is bound by construction).
//! 3. **The one exception list the bind pass carries** (`unbound_fixture_slots`) is parsed from the source and each image it
//!    names must be on [`CLASSIFIED_UNBOUND_IMAGES`] with a reason; an entry here with no image in the source is stale.
//! 4. **A stale classified function fails** -- no `lease: None` literal remains in it -- so the list is only as long as it is
//!    true, the way `repo-scope` retires its own.
//! 5. **The stated limit stays true or is restated.** *Lease revocation does not reclaim an MMIO mapping, because
//!    `MmioUnmap` is declared in the ABI and never dispatched.* The syscall expectations name it undispatched; **if it ever
//!    is dispatched this fails**, and the claim in RFC-0.48-001 and `spawn.rs` has to be rewritten, not left standing
//!    after it stopped being the reason.
//!
//! 6. **Task slots are never reused.** *A lease's owner is a task-table **index** (`TaskId::new(tidx, 0)`, generation pinned
//!    to 0 on both sides of `sys_lease_revoke`'s comparison), and a boot lease outlives its owner's death.* That is safe
//!    only while a dead task's index is never handed to a new task -- and `TaskTable::remove`, the one function that
//!    returns a slot, has **no caller**. Nothing fails when a property true *by a function being dead* stops being true,
//!    so this fails instead: any `.remove(` / `::remove(` call in the kernel, or any assignment that clears a `.task`
//!    slot outside `TaskTable::remove` itself, is red, naming the orphan answer it protects. **It is a stopgap and says
//!    so:** the remedy, whenever slot reuse is actually wanted, is a generation-aware owner (store and compare the full
//!    `TaskId`) -- RFC-0.48-001, "Who can revoke a boot lease once its owner is dead". The guard is deliberately broad
//!    (an unrelated `.remove(` is red too): the kernel has none today, and a guard that tried to tell which `remove` it is
//!    would be the narrow search this project keeps correcting.
//!
//! **What it does not do:** it does not show a capability is *revoked* (the `svc` boot profile does, through the funnel),
//! only that no grant escapes the bind. A function it cannot delimit is a **failure**, never a skip.

use crate::authority_funnel::{code_only, functions, test_module_ranges};

/// A function allowed to hold a `lease: None` capability literal, and why.
pub struct ClassifiedFunction {
    pub file: &'static str,
    pub func: &'static str,
    pub reason: &'static str,
    /// If set, the function's body must contain this text -- the thing that makes the exemption true.
    pub must_contain: &'static [&'static str],
}

pub const CLASSIFIED_FUNCTIONS: &[ClassifiedFunction] = &[
    ClassifiedFunction {
        file: "task/spawn.rs",
        func: "spawn_with_lease",
        reason: "the per-image grants are bound to the task's boot lease in one pass at the end of this function; the \
                 literals are `lease: None` only until that pass",
        must_contain: &["cap.lease = Some(", "boot_lease", "LeaseBinding"],
    },
    ClassifiedFunction {
        file: "main.rs",
        func: "kmain",
        reason: "`init`, the root that creates every other task's boot lease: it is built directly rather than by `spawn()`, \
                 and a lease whose owner is also its subject revokes nothing (RFC-0.48-001 D4)",
        must_contain: &[],
    },
];

/// An image `unbound_fixture_slots` may exempt, and why.
pub struct ClassifiedImage {
    pub image: &'static str,
    pub reason: &'static str,
}

pub const CLASSIFIED_UNBOUND_IMAGES: &[ClassifiedImage] = &[
    ClassifiedImage {
        image: "NEG_TEST",
        reason: "a fixture whose job is to bind capabilities to leases it creates; a capability binds once (E-090), so its \
                 slot-0 source must be one boot has not bound",
    },
    ClassifiedImage {
        image: "SAMPLE_SERVICE",
        reason: "the blocked-IPC fixture, which copies slot 0 and binds the copy to a lease it creates",
    },
];

pub struct Source {
    pub path: String,
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    pub literals: usize,
    pub unbound_literals: usize,
    pub classified_used: usize,
    pub fixture_images: usize,
    /// Slot-releasing sites found outside `TaskTable::remove` (always 0 in an `Ok` report).
    pub slot_release_sites: usize,
}

/// Byte spans of `Capability { ... }` literals in code-only text: the brace-matched block after the type name.
fn capability_literals(code: &str) -> Vec<(usize, usize)> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = code[from..].find("Capability {") {
        let at = from + i;
        from = at + 1;
        // a whole word: `Capability` not `FooCapability`, and not the struct definition or an impl header
        if at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_') {
            continue;
        }
        let before = code[..at].trim_end();
        if before.ends_with("struct") || before.ends_with("impl") || before.ends_with("for") {
            continue;
        }
        let open = at + "Capability ".len();
        let mut depth = 0i32;
        for (k, c) in b.iter().enumerate().skip(open) {
            match c {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        out.push((at, k));
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn line_of(code: &str, pos: usize) -> usize {
    code[..pos].matches('\n').count() + 1
}

/// The `ImageId::X` names inside `fn unbound_fixture_slots`, from its (code-only) body.
fn fixture_images(code: &str) -> Option<Vec<String>> {
    let funcs = functions(code).ok()?;
    let f = funcs.iter().find(|f| f.name == "unbound_fixture_slots")?;
    let body = &code[f.body.0..=f.body.1];
    let mut out = Vec::new();
    for part in body.split("ImageId::").skip(1) {
        let name: String = part
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    Some(out)
}

/// Sites in non-test kernel code that return a task slot to reuse, other than `TaskTable::remove` itself:
/// `(path, line, what)`. Text over code-only source (comments and strings blanked), so a mention in prose is not a site.
fn slot_release_sites(sources: &[Source]) -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    for s in sources {
        let code = code_only(&s.text);
        let tests = test_module_ranges(&code);
        // A function that cannot be delimited is reported by the main loop; here it only means no exemption applies.
        let funcs = functions(&code).unwrap_or_default();
        let in_table_remove = |line: usize| {
            s.path.ends_with("task/tcb.rs")
                && funcs
                    .iter()
                    .any(|f| f.name == "remove" && f.start_line <= line && line <= f.end_line)
        };
        let mut hit = |pos: usize, what: String| {
            let line = line_of(&code, pos);
            if tests.iter().any(|(x, y)| *x <= line && line <= *y) || in_table_remove(line) {
                return;
            }
            out.push((s.path.clone(), line, what));
        };
        for needle in [".remove(", "::remove("] {
            let mut from = 0;
            while let Some(i) = code[from..].find(needle) {
                let at = from + i;
                from = at + 1;
                hit(at, format!("`{needle}` is called"));
            }
        }
        // Clearing a slot's task by hand: `.task.take(`, `.task.replace(`, `.task = <anything but Some(..)>`, or a
        // `mem::{take,replace,swap}` whose arguments name a `.task`.
        let mut from = 0;
        while let Some(i) = code[from..].find(".task") {
            let at = from + i;
            from = at + 1;
            let rest = &code[at + ".task".len()..];
            if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                continue; // `.task_foo`, `.tasks`
            }
            let rest = rest.trim_start();
            if rest.starts_with(".take(") || rest.starts_with(".replace(") {
                hit(at, "a slot's `.task` is taken out by hand".into());
            } else if rest.starts_with('=')
                && !rest.starts_with("==")
                && !rest[1..].trim_start().starts_with("Some(")
            {
                hit(
                    at,
                    "a slot's `.task` is assigned something other than `Some(..)`".into(),
                );
            }
        }
        let mut from = 0;
        while let Some(i) = code[from..].find("mem::") {
            let at = from + i;
            from = at + 1;
            let window = &code[at..code.len().min(at + 80)];
            let call = ["take(", "replace(", "swap("]
                .iter()
                .any(|c| window["mem::".len()..].starts_with(c));
            if call && window.contains(".task") {
                hit(at, "a slot's `.task` is moved out with `mem::`".into());
            }
        }
    }
    out
}

/// Pure core. `undispatched` is the syscall expectations' explicit list.
pub fn evaluate(sources: &[Source], undispatched: &[String]) -> Result<Report, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    if sources.is_empty() {
        return Err(vec![
            "no kernel source files in the population -- a check over nothing vouches for nothing"
                .into(),
        ]);
    }
    let (mut literals, mut unbound_literals) = (0usize, 0usize);
    let mut used: Vec<(&str, &str)> = Vec::new();
    let mut fixtures: Option<Vec<String>> = None;

    for s in sources {
        let code = code_only(&s.text);
        let funcs = match functions(&code) {
            Ok(f) => f,
            Err(e) => {
                problems.push(format!("{}: {e}", s.path));
                continue;
            }
        };
        let tests = test_module_ranges(&code);
        if s.path.ends_with("task/spawn.rs") {
            fixtures = fixture_images(&code);
        }
        for (a, b) in capability_literals(&code) {
            let line = line_of(&code, a);
            if tests.iter().any(|(x, y)| *x <= line && line <= *y) {
                continue;
            }
            literals += 1;
            let block = &code[a..=b];
            if !block
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .contains("lease: None")
            {
                continue;
            }
            unbound_literals += 1;
            let func = funcs
                .iter()
                .filter(|f| f.start_line <= line && line <= f.end_line)
                .min_by_key(|f| f.end_line - f.start_line);
            let entry = func.and_then(|f| {
                CLASSIFIED_FUNCTIONS
                    .iter()
                    .find(|c| s.path.ends_with(c.file) && c.func == f.name)
            });
            match (func, entry) {
                (Some(f), Some(c)) => {
                    let body = &code[f.body.0..=f.body.1];
                    if let Some(missing) = c.must_contain.iter().find(|m| !body.contains(*m)) {
                        problems.push(format!(
                            "{}:{line}: `lease: None` in `{}`, which is classified only because it binds its grants -- but its body \
                             no longer contains `{missing}`: the bind pass is gone, so every grant in it is now permanent",
                            s.path, f.name
                        ));
                    }
                    if !used.contains(&(c.file, c.func)) {
                        used.push((c.file, c.func));
                    }
                }
                (f, None) => problems.push(format!(
                    "{}:{line}: a boot capability literal is `lease: None` in `{}` and is not bound to a lease -- nothing can \
                     take it back (E-093). Bind it (add it to `spawn()`'s grants, which are bound in one pass) or classify the \
                     function in `bootstrap_leases::CLASSIFIED_FUNCTIONS` with a reason",
                    s.path,
                    f.map_or("<no enclosing function>", |f| f.name.as_str())
                )),
                (None, Some(_)) => unreachable!("an entry is only found through a function"),
            }
        }
    }

    // 3. the fixture exemption list
    let named = fixtures.clone().unwrap_or_default();
    if fixtures.is_none() {
        problems.push("`unbound_fixture_slots` was not found in task/spawn.rs: the exemption the bind pass carries cannot be read".into());
    }
    for img in &named {
        if !CLASSIFIED_UNBOUND_IMAGES.iter().any(|c| c.image == img) {
            problems.push(format!(
                "`unbound_fixture_slots` exempts `ImageId::{img}` from the bind pass, and it is not in \
                 `bootstrap_leases::CLASSIFIED_UNBOUND_IMAGES` with a reason"
            ));
        }
    }
    for c in CLASSIFIED_UNBOUND_IMAGES {
        if fixtures.is_some() && !named.iter().any(|n| n == c.image) {
            problems.push(format!(
                "stale classified image `{}`: `unbound_fixture_slots` no longer names it",
                c.image
            ));
        }
    }

    // 4. stale classified functions
    for c in CLASSIFIED_FUNCTIONS {
        if !used.contains(&(c.file, c.func)) {
            problems.push(format!(
                "stale classified function `{}::{}`: no `lease: None` capability literal remains in it, so it is retired",
                c.file, c.func
            ));
        }
    }

    // 5. the stated limit
    if !undispatched.iter().any(|n| n == "MmioUnmap") {
        problems.push(
            "`MmioUnmap` is no longer undispatched: RFC-0.48-001's statement that lease revocation does not reclaim an MMIO \
             mapping is true *because* nothing unmaps one. Restate the claim (spawn.rs, RFC-0.48-001, the book) before this passes"
                .into(),
        );
    }

    // 6. task slots are never reused
    let slot_sites = slot_release_sites(sources);
    for (path, line, what) in &slot_sites {
        problems.push(format!(
            "{path}:{line}: {what}. RFC-0.48-001: a boot lease outlives its owner's death, and a lease's owner is a task-table \
             *index* (`TaskId::new(tidx, 0)`, generation ignored), so an orphaned boot lease is safe **only because a dead \
             task's slot is never reused**. This returns a task slot to reuse: a new occupant of that index would become the \
             owner of every boot lease its predecessor spawned. Make lease ownership generation-aware (store and compare the \
             full `TaskId`) first, then retire this guard -- or, if this is not the task table, say so by renaming it"
        ));
    }

    if problems.is_empty() {
        Ok(Report {
            files: sources.len(),
            literals,
            unbound_literals,
            classified_used: used.len(),
            fixture_images: named.len(),
            slot_release_sites: slot_sites.len(),
        })
    } else {
        Err(problems)
    }
}

/// `undispatched = [ "A", "B", ... ]` out of `tests/syscall/expected.toml`.
pub fn parse_undispatched(toml: &str) -> Vec<String> {
    let Some(i) = toml.find("undispatched") else {
        return Vec::new();
    };
    let rest = &toml[i..];
    let Some(open) = rest.find('[') else {
        return Vec::new();
    };
    let Some(close) = rest[open..].find(']') else {
        return Vec::new();
    };
    rest[open + 1..open + close]
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .flat_map(|l| l.split(','))
        .map(|t| t.trim().trim_matches('"').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNDISP: &[&str] = &["MmioUnmap"];
    fn undisp() -> Vec<String> {
        UNDISP.iter().map(|s| s.to_string()).collect()
    }
    fn src(path: &str, text: &str) -> Source {
        Source {
            path: path.into(),
            text: text.into(),
        }
    }

    const SPAWN_OK: &str = "fn unbound_fixture_slots(i: ImageId) -> &'static [usize] { match i { ImageId::NEG_TEST | ImageId::SAMPLE_SERVICE => &[0], _ => &[] } }\n\
        fn spawn_with_lease(boot_lease: L) {\n    let _ = Capability { kind: K, lease: None };\n    cap.lease = Some(fjell_cap::slot::LeaseBinding { lease_id: boot_lease });\n}\n";
    const MAIN_OK: &str = "fn kmain() { let _ = Capability { kind: K, lease: None }; }\n";

    fn base() -> Vec<Source> {
        vec![
            src("crates/fjell-kernel/src/task/spawn.rs", SPAWN_OK),
            src("crates/fjell-kernel/src/main.rs", MAIN_OK),
        ]
    }

    #[test]
    fn the_control_is_green() {
        let r = evaluate(&base(), &undisp()).unwrap();
        assert_eq!(
            (r.unbound_literals, r.classified_used, r.fixture_images),
            (2, 2, 2)
        );
    }

    #[test]
    fn an_unbound_literal_in_a_new_function_is_red_and_named() {
        let mut v = base();
        v.push(src(
            "crates/fjell-kernel/src/trap/new.rs",
            "fn sneaky() {\n    let _ = Capability { kind: K, lease: None };\n}\n",
        ));
        let e = evaluate(&v, &undisp()).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("new.rs:2") && p.contains("sneaky") && p.contains("E-093")),
            "{e:?}"
        );
    }

    #[test]
    fn a_bound_or_runtime_literal_is_not_this_checks_business() {
        let mut v = base();
        v.push(src(
            "crates/fjell-kernel/src/cap/x.rs",
            "fn f() { let _ = Capability { kind: K, lease: Some(b) }; }\n",
        ));
        assert!(evaluate(&v, &undisp()).is_ok());
    }

    #[test]
    fn deleting_the_bind_pass_reddens_every_grant_in_the_function() {
        let spawn = SPAWN_OK.replace(
            "cap.lease = Some(fjell_cap::slot::LeaseBinding { lease_id: boot_lease });",
            "",
        );
        let mut v = base();
        v[0] = src("crates/fjell-kernel/src/task/spawn.rs", &spawn);
        let e = evaluate(&v, &undisp()).unwrap_err();
        assert!(e.iter().any(|p| p.contains("bind pass is gone")), "{e:?}");
    }

    #[test]
    fn a_stale_classified_function_is_red() {
        let mut v = base();
        v[1] = src("crates/fjell-kernel/src/main.rs", "fn kmain() {}\n");
        let e = evaluate(&v, &undisp()).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("stale classified function") && p.contains("kmain")),
            "{e:?}"
        );
    }

    #[test]
    fn a_new_fixture_exemption_needs_a_reason_and_a_stale_one_is_red() {
        let grown = SPAWN_OK.replace(
            "ImageId::NEG_TEST |",
            "ImageId::NEG_TEST | ImageId::AUDITD |",
        );
        let mut v = base();
        v[0] = src("crates/fjell-kernel/src/task/spawn.rs", &grown);
        let e = evaluate(&v, &undisp()).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("ImageId::AUDITD") && p.contains("CLASSIFIED_UNBOUND_IMAGES")),
            "{e:?}"
        );
        let shrunk = SPAWN_OK.replace("ImageId::NEG_TEST | ", "");
        v[0] = src("crates/fjell-kernel/src/task/spawn.rs", &shrunk);
        let e = evaluate(&v, &undisp()).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("stale classified image") && p.contains("NEG_TEST")),
            "{e:?}"
        );
    }

    #[test]
    fn test_modules_and_comments_are_not_grants() {
        let mut v = base();
        v.push(src(
            "crates/fjell-kernel/src/cap/t.rs",
            "// Capability { lease: None }\nfn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = Capability { kind: K, lease: None }; }\n}\n",
        ));
        assert!(
            evaluate(&v, &undisp()).is_ok(),
            "{:?}",
            evaluate(&v, &undisp())
        );
    }

    #[test]
    fn dispatching_mmio_unmap_makes_the_stated_limit_false_and_the_check_says_so() {
        let e = evaluate(&base(), &[]).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("MmioUnmap") && p.contains("Restate the claim")),
            "{e:?}"
        );
    }

    const TCB_OK: &str = "impl TaskTable {\n    pub fn insert(&mut self, t: Task) { slot.task = Some(t); }\n    pub fn remove(&mut self, id: TaskId) -> Option<Task> {\n        let task = slot.task.take()?;\n        Some(task)\n    }\n}\n";

    fn with(extra: &str) -> Vec<Source> {
        let mut v = base();
        v.push(src("crates/fjell-kernel/src/task/tcb.rs", TCB_OK));
        if !extra.is_empty() {
            v.push(src("crates/fjell-kernel/src/task/other.rs", extra));
        }
        v
    }

    #[test]
    fn the_slot_guard_control_is_green_with_remove_defined_and_uncalled() {
        let r = evaluate(&with(""), &undisp()).unwrap();
        assert_eq!(r.slot_release_sites, 0);
    }

    #[test]
    fn a_caller_of_remove_is_red_and_names_the_orphan_answer_it_protects() {
        for call in [
            "fn reap(t: &mut TaskTable, id: TaskId) { let _ = t.remove(id); }\n",
            "fn reap(id: TaskId) { let _ = TaskTable::remove(&mut T, id); }\n",
        ] {
            let e = evaluate(&with(call), &undisp()).unwrap_err();
            assert!(
                e.iter().any(|p| p.contains("other.rs:1")
                    && p.contains("never reused")
                    && p.contains("generation-aware")),
                "{e:?}"
            );
        }
    }

    #[test]
    fn clearing_a_slot_by_hand_is_red_too_not_only_the_named_function() {
        for body in [
            "fn f(s: &mut Slot) { let _ = s.task.take(); }\n",
            "fn f(s: &mut Slot) { s.task = None; }\n",
            "fn f(s: &mut Slot) { let _ = core::mem::take(&mut s.task); }\n",
            "fn f(s: &mut Slot) { let _ = core::mem::replace(&mut s.task, None); }\n",
        ] {
            let e = evaluate(&with(body), &undisp()).unwrap_err();
            assert!(e.iter().any(|p| p.contains("other.rs:1")), "{body}: {e:?}");
        }
    }

    #[test]
    fn what_the_slot_guard_must_not_flag() {
        // insert's `= Some(..)`, a comparison, a longer field name, a comment, a string, and a test module.
        let ok = "fn f(s: &mut Slot) { s.task = Some(t); let _ = s.task == None; let _ = s.tasks.len(); s.task_id = 3; }\n\
                  // t.remove(id) in prose\n\
                  fn g() { let _ = \"x.remove(y)\"; }\n\
                  #[cfg(test)]\nmod tests {\n    fn t(x: &mut V) { x.remove(0); }\n}\n";
        assert!(evaluate(&with(ok), &undisp()).is_ok());
    }

    #[test]
    fn a_clear_inside_remove_in_the_wrong_file_is_not_exempt() {
        // The exemption is `TaskTable::remove` in task/tcb.rs, not any function named `remove`.
        let e = evaluate(
            &with("fn remove(s: &mut Slot) { let _ = s.task.take(); }\n"),
            &undisp(),
        )
        .unwrap_err();
        assert!(e.iter().any(|p| p.contains("other.rs:1")), "{e:?}");
    }

    #[test]
    fn the_undispatched_list_parses_with_comments_and_trailing_commas() {
        let t = "x = 1\nundispatched = [\n    # why\n    \"MmioUnmap\",\n    \"Foo\", # c\n]\n";
        assert_eq!(
            parse_undispatched(t),
            vec!["MmioUnmap".to_string(), "Foo".to_string()]
        );
    }

    #[test]
    fn an_empty_population_is_an_error() {
        assert!(evaluate(&[], &undisp()).is_err());
    }

    #[test]
    fn the_real_kernel_binds_every_boot_grant_or_names_why_not() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let paths =
            fjell_repo_scope::files_with_extension(&root, fjell_repo_scope::Scope::Working, "rs")
                .expect("scope");
        let sources: Vec<Source> = paths
            .iter()
            .filter(|p| p.to_string_lossy().contains("crates/fjell-kernel/src/"))
            .map(|p| Source {
                path: p.to_string_lossy().to_string(),
                text: std::fs::read_to_string(p).expect("read"),
            })
            .collect();
        let toml = std::fs::read_to_string(root.join("tests/syscall/expected.toml"))
            .expect("expected.toml");
        let r = evaluate(&sources, &parse_undispatched(&toml));
        assert!(r.is_ok(), "{r:#?}");
    }
}
