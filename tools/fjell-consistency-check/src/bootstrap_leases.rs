//! RFC-0.48-001 R6: **`bootstrap-leases`** -- no boot-time capability is granted without a lease, and what is exempt is
//! named, reasoned, and kept true.
//!
//! Until 0.48 every capability the kernel installed at boot carried `lease: None`, and `Capability::check_lease` returns
//! `Ok(())` for an unbound capability **for ever** -- so nothing could take a boot grant back (E-093). `spawn()` now
//! creates a *boot lease* per task and binds every capability the task holds in one pass, so no grant literal needs a
//! lease and **a grant added tomorrow cannot forget the bind**. This check is what keeps that true.
//!
//! **Population: every `Capability { ... }` literal under `crates/fjell-kernel/src` and the three crates extracted from it (`SCANNED_SRC`)** outside `#[cfg(test)]`, through the
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
//! 5. **Device mappings are unmapped by exactly the callers the stated limit allows -- and lease revocation is not one.**
//!    *Lease revocation withdraws authority exercised per call; it reclaims neither a DMA frame nor an MMIO mapping.* Until
//!    0.49 that was true **because `MmioUnmap` was declared and dispatched nowhere** (nothing could unmap a device page), and
//!    this check failed the day it was dispatched. Since RFC-0.49-002 it is true for a different reason -- unmapping is the
//!    holder's act (`sys_mmio_unmap`, `sys_dma_revoke`), the task's death, and the rollback of a map that failed -- and the
//!    check guards **that**: every call in non-test kernel code to a device-unmap primitive ([`DEVICE_UNMAP_SINKS`]) must sit
//!    in a function on [`CLASSIFIED_DEVICE_UNMAP_CALLERS`] with a reason, and a stale entry fails. The day
//!    `LeaseTable::revoke` (or anything else) acquires such a call it is red, and the claim in RFC-0.48-001 / RFC-0.49-002 and
//!    limitations row 12 has to be restated rather than left standing after it stopped being the reason.
//!
//! 6. **No production code compares a task by index with a literal generation 0** (RFC-0.49-001 D3). *A task slot's
//!    generation advances when a task is destroyed and its slot reused, so `TaskId::new(index, 0)` stops naming the task in
//!    that slot -- and used as an owner comparison it lets a **new occupant of slot n inherit what its predecessor owned**.*
//!    This premise replaces 0.48's ("no caller of `TaskTable::remove`", a property that was true by a function being dead,
//!    which nothing could notice stop being true): reuse is now real, and what keeps it sound is that identity is a real
//!    `TaskId` wherever it is compared. **The count is produced here, not written down:** every `TaskId::new(_, 0)` /
//!    `TaskId { .., generation: 0 }` in non-test kernel code is found by the scanner and must sit in a function on
//!    [`CLASSIFIED_TASK_ID_SITES`] with a reason (the idle task and `init` built at boot as the first occupants of their
//!    slots; the DMA table's empty-entry sentinel; the one fallback id for "no task is current"). A stale entry fails too.
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
    /// Call sites of device-unmap primitives, every one in a classified function (RFC-0.49-002 D7).
    pub device_unmap_sites: usize,
    /// Production `TaskId` literals with generation 0, every one in a classified function (RFC-0.49-001 D3).
    pub task_id_zero_sites: usize,
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

/// The device-unmap primitives (RFC-0.49-002 D7): the page-table unmap, the DMA table's verified unmap and its revoke/release
/// methods, and the mapping table's task release. **A call to any of these takes a device mapping back.**
pub const DEVICE_UNMAP_SINKS: &[&str] = &[
    "unmap_page(",
    "unmap_dma_region_verified(",
    "revoke_entry(",
    "revoke_by_pa(",
    "revoke_by_id(",
    "release_task(",
];

/// A function allowed to call a device-unmap primitive, and why.
pub struct ClassifiedDeviceUnmapCaller {
    pub file: &'static str,
    pub func: &'static str,
    pub reason: &'static str,
}

pub const CLASSIFIED_DEVICE_UNMAP_CALLERS: &[ClassifiedDeviceUnmapCaller] = &[
    ClassifiedDeviceUnmapCaller {
        file: "trap/syscall.rs",
        func: "sys_mmio_unmap",
        reason: "the holder's explicit, capability-checked, verified unmap of its own device mapping (RFC-0.49-002)",
    },
    ClassifiedDeviceUnmapCaller {
        file: "trap/syscall.rs",
        func: "sys_mmio_map",
        reason: "the rollback of a map that failed part-way: it unmaps the pages it had just mapped, and releases its own record",
    },
    ClassifiedDeviceUnmapCaller {
        file: "trap/syscall.rs",
        func: "sys_dma_revoke",
        reason: "the holder's explicit DMA revoke (RFC 052): the owner's own region, by physical address",
    },
    ClassifiedDeviceUnmapCaller {
        file: "trap/syscall.rs",
        func: "sys_dma_alloc",
        reason: "the rollback of a DMA allocation that failed after its page was mapped",
    },
    ClassifiedDeviceUnmapCaller {
        file: "main.rs",
        func: "unmap_dma_region_verified",
        reason: "the verified DMA unmap itself: it reads the PTE, unmaps, and requires the same frame back",
    },
    ClassifiedDeviceUnmapCaller {
        file: "main.rs",
        func: "revoke_entry",
        reason: "the DMA table's one revoke of an entry: it calls the verified unmap",
    },
    ClassifiedDeviceUnmapCaller {
        file: "main.rs",
        func: "revoke_by_pa",
        reason: "the DMA table's revoke by physical address: called by `sys_dma_revoke` only",
    },
    ClassifiedDeviceUnmapCaller {
        file: "main.rs",
        func: "revoke_by_id",
        reason: "the DMA table's revoke by region id, `#[allow(dead_code)]` and called by nothing today; it shares `revoke_entry`",
    },
    ClassifiedDeviceUnmapCaller {
        file: "trap/dispatch.rs",
        func: "schedule_next",
        reason: "a task's death (exit or fault): its DMA regions and device-mapping records are released",
    },
    ClassifiedDeviceUnmapCaller {
        file: "task/destroy.rs",
        func: "destroy_inner",
        reason: "a task destroyed by `TaskKill` that had not ended itself: the death path's releases (RFC-0.49-001)",
    },
];

/// `(path, line, enclosing function, sink)` for every call to a device-unmap primitive in non-test kernel code, **outside the
/// primitive's own definition** (a function named like the sink is the definition, not a caller).
fn device_unmap_call_sites(sources: &[Source]) -> Vec<(String, usize, String, &'static str)> {
    let mut out = Vec::new();
    for s in sources {
        let code = code_only(&s.text);
        let tests = test_module_ranges(&code);
        let funcs = functions(&code).unwrap_or_default();
        for sink in DEVICE_UNMAP_SINKS {
            let bare = sink.trim_end_matches('(');
            let mut from = 0;
            while let Some(i) = code[from..].find(sink) {
                let at = from + i;
                from = at + 1;
                // a whole identifier: not `foo_unmap_page(` -- and not the definition `fn unmap_page(`
                if at > 0 {
                    let prev = code.as_bytes()[at - 1];
                    if prev.is_ascii_alphanumeric() || prev == b'_' {
                        continue;
                    }
                }
                if code[..at].trim_end().ends_with("fn") {
                    continue;
                }
                let line = line_of(&code, at);
                if tests.iter().any(|(x, y)| *x <= line && line <= *y) {
                    continue;
                }
                let func = funcs
                    .iter()
                    .filter(|f| f.start_line <= line && line <= f.end_line)
                    .min_by_key(|f| f.end_line - f.start_line)
                    .map_or("<no enclosing function>".to_string(), |f| f.name.clone());
                if func == bare {
                    continue; // inside a function of the sink's own name: its definition (e.g. `Vspace::unmap_page`)
                }
                out.push((s.path.clone(), line, func, *sink));
            }
        }
    }
    out
}

/// A function allowed to build a task id with a literal generation `0`, and why (RFC-0.49-001 D3).
pub struct ClassifiedTaskIdSite {
    pub file: &'static str,
    pub func: &'static str,
    pub reason: &'static str,
}

/// The source trees this check reads: the kernel, **and the three crates extracted from it** (RFC-0.50-001). The lease table -- where
/// an owner is compared with a caller's whole `TaskId` -- now lives in `fjell-lease`; a check that kept reading only the kernel would
/// have been silently emptied of the one place premise 6 is about.
pub const SCANNED_SRC: &[&str] = &[
    "crates/fjell-kernel/src/",
    "crates/fjell-lease/src/",
    "crates/fjell-task/src/",
    "crates/fjell-user-ptr/src/",
];

pub const CLASSIFIED_TASK_ID_SITES: &[ClassifiedTaskIdSite] = &[
    ClassifiedTaskIdSite {
        file: "main.rs",
        func: "kmain",
        reason: "the idle task (slot 0) and `init` (slot 1) are built directly at boot as the FIRST occupants of their slots, \
                 so generation 0 is their real generation; the idle task is never destroyed",
    },
    ClassifiedTaskIdSite {
        file: "main.rs",
        func: "free",
        reason: "`DmaRegionEntry::free`'s sentinel: index 0xFFFF marks an empty entry and names no task",
    },
    ClassifiedTaskIdSite {
        file: "fjell-lease/src/lib.rs",
        func: "empty",
        reason: "an `Empty` lease slot's placeholder owner: the slot is not `Active`, so its owner is never read until `allocate` \
                 overwrites it. (In `fjell-lease` since 0.50 -- RFC-0.50-001 moved the table out of the kernel; the lease table is \
                 where owners are compared, so this check reads that crate too and does not let the move take it out of scope.)",
    },
    ClassifiedTaskIdSite {
        file: "trap/dispatch.rs",
        func: "current_task_id",
        reason: "the fallback when no task is current -- the idle task, never destroyed, generation 0; unreachable from a syscall",
    },
];

/// Is the `n`th top-level argument of the call whose `(` is at `open` in `code` the literal `0`? (`n` is 1-based.)
fn nth_arg_is_zero(code: &str, open: usize, n: usize) -> bool {
    let b = code.as_bytes();
    let (mut depth, mut arg, mut start) = (0i32, 1usize, open + 1);
    for (k, c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return arg == n && code[start..k].trim() == "0";
                }
            }
            b',' if depth == 1 => {
                if arg == n {
                    return code[start..k].trim() == "0";
                }
                arg += 1;
                start = k + 1;
            }
            _ => {}
        }
    }
    false
}

/// Production sites that build a task id with a literal generation `0`: `(path, line, enclosing function)`. Text over
/// code-only source (comments and strings blanked), outside `#[cfg(test)]`.
fn task_id_zero_sites(sources: &[Source]) -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    for s in sources {
        let code = code_only(&s.text);
        let tests = test_module_ranges(&code);
        let funcs = functions(&code).unwrap_or_default();
        let mut found: Vec<usize> = Vec::new();
        let mut from = 0;
        while let Some(i) = code[from..].find("TaskId::new(") {
            let at = from + i;
            from = at + 1;
            let open = at + "TaskId::new".len();
            if nth_arg_is_zero(&code, open, 2) {
                found.push(at);
            }
        }
        // the struct-literal form: `TaskId { index: .., generation: 0 }`
        let mut from = 0;
        while let Some(i) = code[from..].find("TaskId {") {
            let at = from + i;
            from = at + 1;
            let rest: String = code[at..].chars().take(120).collect();
            if let Some(g) = rest.find("generation:") {
                let tail = rest[g + "generation:".len()..].trim_start();
                if tail.starts_with('0')
                    && !tail[1..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
                {
                    found.push(at);
                }
            }
        }
        for at in found {
            let line = line_of(&code, at);
            if tests.iter().any(|(x, y)| *x <= line && line <= *y) {
                continue;
            }
            let func = funcs
                .iter()
                .filter(|f| f.start_line <= line && line <= f.end_line)
                .min_by_key(|f| f.end_line - f.start_line)
                .map_or("<no enclosing function>".to_string(), |f| f.name.clone());
            out.push((s.path.clone(), line, func));
        }
    }
    out
}

/// Pure core.
pub fn evaluate(sources: &[Source]) -> Result<Report, Vec<String>> {
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

    // 5. device mappings are unmapped only by the classified callers
    let unmap_sites = device_unmap_call_sites(sources);
    let mut unmap_used: Vec<(&str, &str)> = Vec::new();
    for (path, line, func, sink) in &unmap_sites {
        match CLASSIFIED_DEVICE_UNMAP_CALLERS
            .iter()
            .find(|c| path.ends_with(c.file) && c.func == func)
        {
            Some(c) => {
                if !unmap_used.contains(&(c.file, c.func)) {
                    unmap_used.push((c.file, c.func));
                }
            }
            None => problems.push(format!(
                "{path}:{line}: `{sink}` is called in `{func}`, which is not a classified caller of a device-unmap primitive. \
                 RFC-0.49-002 D7: lease revocation withdraws per-call authority and reclaims no DMA frame and no MMIO mapping; \
                 unmapping is the holder's act, the task's death, and the rollback of a failed map. A new caller is either one of \
                 those (classify it in `bootstrap_leases::CLASSIFIED_DEVICE_UNMAP_CALLERS` with a reason) or it makes the claim \
                 false -- in which case restate it (RFC-0.49-002, `v1-limitations.md` row 12, `spawn.rs`) before this passes"
            )),
        }
    }
    for c in CLASSIFIED_DEVICE_UNMAP_CALLERS {
        if !unmap_used.contains(&(c.file, c.func)) {
            problems.push(format!(
                "stale classified device-unmap caller `{}::{}`: it no longer calls a device-unmap primitive, so it is retired",
                c.file, c.func
            ));
        }
    }

    // 6. no production comparison of a task by index with a literal generation 0
    let zero_sites = task_id_zero_sites(sources);
    let mut classified_used: Vec<(&str, &str)> = Vec::new();
    for (path, line, func) in &zero_sites {
        match CLASSIFIED_TASK_ID_SITES
            .iter()
            .find(|c| path.ends_with(c.file) && c.func == func)
        {
            Some(c) => {
                if !classified_used.contains(&(c.file, c.func)) {
                    classified_used.push((c.file, c.func));
                }
            }
            None => problems.push(format!(
                "{path}:{line}: a task id is built from an index with a literal generation 0, in `{func}`. RFC-0.49-001 D3: a task \
                 slot's generation advances when a task is destroyed, so this id no longer names the slot's occupant -- and used to \
                 compare an owner it lets a new occupant of the slot inherit what its predecessor owned. Use the task's real id \
                 (`TaskTable::id_at`, `current_task_id`) or, for a lookup by a queue's index, `TaskTable::get_by_index`; or classify \
                 the function in `bootstrap_leases::CLASSIFIED_TASK_ID_SITES` with a reason"
            )),
        }
    }
    for c in CLASSIFIED_TASK_ID_SITES {
        if !classified_used.contains(&(c.file, c.func)) {
            problems.push(format!(
                "stale classified task-id site `{}::{}`: no literal-generation-0 task id remains in it, so it is retired",
                c.file, c.func
            ));
        }
    }

    if problems.is_empty() {
        Ok(Report {
            files: sources.len(),
            literals,
            unbound_literals,
            classified_used: used.len(),
            fixture_images: named.len(),
            device_unmap_sites: unmap_sites.len(),
            task_id_zero_sites: zero_sites.len(),
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
            path: path.into(),
            text: text.into(),
        }
    }

    const SPAWN_OK: &str = "fn unbound_fixture_slots(i: ImageId) -> &'static [usize] { match i { ImageId::NEG_TEST | ImageId::SAMPLE_SERVICE => &[0], _ => &[] } }\n\
        fn spawn_with_lease(boot_lease: L) {\n    let _ = Capability { kind: K, lease: None };\n    cap.lease = Some(fjell_cap::slot::LeaseBinding { lease_id: boot_lease });\n}\n";
    const MAIN_OK: &str = "fn kmain() { let _ = Capability { kind: K, lease: None }; let idle = TaskId::new(0, 0); let init = TaskId::new(1, 0); }\nimpl E { const fn free() -> Self { E { owner: TaskId::new(0xFFFF, 0) } } }\n";
    const DISPATCH_OK: &str =
        "pub fn current_task_id() -> TaskId { sched.current().unwrap_or(TaskId::new(0, 0)) }\n";
    const LEASE_OK: &str = "impl LeaseObject { const fn empty() -> Self { LeaseObject { owner: TaskId { index: 0, generation: 0 } } } }\n";

    /// One source per classified device-unmap caller's file, each caller a function that calls a sink.
    fn unmap_sources() -> Vec<Source> {
        let mut files: Vec<(&str, String)> = Vec::new();
        for c in CLASSIFIED_DEVICE_UNMAP_CALLERS {
            let body = format!("fn {}() {{ unmap_page(1); }}\n", c.func);
            match files.iter_mut().find(|(f, _)| *f == c.file) {
                Some((_, text)) => text.push_str(&body),
                None => files.push((c.file, body)),
            }
        }
        files
            .into_iter()
            .map(|(f, t)| {
                // an entry in an extracted crate (`fjell-lease/src/lib.rs`) is not under the kernel's `src/`
                let path = if f.starts_with("fjell-") {
                    format!("crates/{f}")
                } else {
                    format!("crates/fjell-kernel/src/{f}")
                };
                src(&path, &t)
            })
            .collect()
    }

    fn base() -> Vec<Source> {
        let mut v = vec![
            src("crates/fjell-kernel/src/task/spawn.rs", SPAWN_OK),
            src("crates/fjell-kernel/src/main.rs", MAIN_OK),
            src("crates/fjell-kernel/src/trap/dispatch.rs", DISPATCH_OK),
            src("crates/fjell-lease/src/lib.rs", LEASE_OK),
        ];
        v.extend(unmap_sources());
        v
    }

    #[test]
    fn the_control_is_green() {
        let r = evaluate(&base()).unwrap();
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
        let e = evaluate(&v).unwrap_err();
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
        assert!(evaluate(&v).is_ok());
    }

    #[test]
    fn deleting_the_bind_pass_reddens_every_grant_in_the_function() {
        let spawn = SPAWN_OK.replace(
            "cap.lease = Some(fjell_cap::slot::LeaseBinding { lease_id: boot_lease });",
            "",
        );
        let mut v = base();
        v[0] = src("crates/fjell-kernel/src/task/spawn.rs", &spawn);
        let e = evaluate(&v).unwrap_err();
        assert!(e.iter().any(|p| p.contains("bind pass is gone")), "{e:?}");
    }

    #[test]
    fn a_stale_classified_function_is_red() {
        let mut v = base();
        v[1] = src("crates/fjell-kernel/src/main.rs", "fn kmain() {}\n");
        let e = evaluate(&v).unwrap_err();
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
        let e = evaluate(&v).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("ImageId::AUDITD") && p.contains("CLASSIFIED_UNBOUND_IMAGES")),
            "{e:?}"
        );
        let shrunk = SPAWN_OK.replace("ImageId::NEG_TEST | ", "");
        v[0] = src("crates/fjell-kernel/src/task/spawn.rs", &shrunk);
        let e = evaluate(&v).unwrap_err();
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
        assert!(evaluate(&v).is_ok(), "{:?}", evaluate(&v));
    }

    #[test]
    fn an_empty_population_is_an_error() {
        assert!(evaluate(&[]).is_err());
    }

    #[test]
    fn the_real_kernel_binds_every_boot_grant_or_names_why_not() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let paths =
            fjell_repo_scope::files_with_extension(&root, fjell_repo_scope::Scope::Working, "rs")
                .expect("scope");
        let sources: Vec<Source> = paths
            .iter()
            .filter(|p| {
                let p = p.to_string_lossy();
                SCANNED_SRC.iter().any(|d| p.contains(d))
            })
            .map(|p| Source {
                path: p.to_string_lossy().to_string(),
                text: std::fs::read_to_string(p).expect("read"),
            })
            .collect();
        let r = evaluate(&sources);
        assert!(r.is_ok(), "{r:#?}");
    }

    // ── premise 6: no production task id is built from an index with a literal generation 0 ─────────────────────────────

    /// The base population plus one extra file.
    fn zero_base(extra_path: &str, extra: &str) -> Vec<Source> {
        let mut v = base();
        if !extra.is_empty() {
            v.push(src(extra_path, extra));
        }
        v
    }

    /// RFC-0.50-001: the lease table moved out of the kernel, and this check moved with it. A literal-generation-0 task id in
    /// `fjell-lease` (where an owner is compared with a caller's whole id) is as red as one in the kernel's `lease/mod.rs` was.
    #[test]
    fn a_literal_generation_zero_in_an_extracted_crate_is_as_red_as_one_in_the_kernel() {
        for path in [
            "crates/fjell-lease/src/lib.rs",
            "crates/fjell-task/src/tcb.rs",
            "crates/fjell-user-ptr/src/lib.rs",
        ] {
            let e = evaluate(&zero_base(path, "fn pick() -> TaskId { TaskId::new(3, 0) }\n")).unwrap_err();
            assert!(
                e.iter().any(|p| p.contains(path) && p.contains("literal generation 0")),
                "{path}: {e:?}"
            );
        }
    }

    #[test]
    fn the_task_id_control_is_green_and_the_count_comes_from_the_scanner() {
        let r = evaluate(&zero_base("", "")).unwrap();
        assert_eq!(
            r.task_id_zero_sites, 5,
            "idle, init, the DMA sentinel, the lease placeholder, the fallback"
        );
    }

    #[test]
    fn a_new_generation_zero_task_id_is_red_and_names_the_function() {
        for body in [
            "fn owner_check(tidx: u16) -> bool { owner == TaskId::new(tidx, 0) }\n",
            "fn lookup(t: &T) { t.get(fjell_abi::task::TaskId::new(7, 0)); }\n",
            "fn lit() -> TaskId { TaskId { index: 3, generation: 0 } }\n",
            "fn spaced() -> TaskId { TaskId::new( idx as u16 ,\n    0 ) }\n",
        ] {
            let e =
                evaluate(&zero_base("crates/fjell-kernel/src/cap/syscall.rs", body)).unwrap_err();
            assert!(
                e.iter().any(|p| p.contains("cap/syscall.rs:1")
                    && p.contains("literal generation 0")
                    && p.contains("TaskTable::id_at")),
                "{body}: {e:?}"
            );
        }
    }

    #[test]
    fn what_the_task_id_premise_must_not_flag() {
        let ok = "fn real(g: u16) -> TaskId { TaskId::new(5, g) }\n\
                  fn also() -> TaskId { TaskId::new(5, 10) }\n\
                  fn expr() -> TaskId { TaskId::new(5, gen + 0) }\n\
                  // TaskId::new(1, 0) in prose\n\
                  fn s() { let _ = \"TaskId::new(1, 0)\"; }\n\
                  fn sf() -> TaskId { TaskId { index: 3, generation: g0 } }\n\
                  #[cfg(test)]\nmod tests {\n    fn t() -> TaskId { TaskId::new(1, 0) }\n}\n";
        assert!(evaluate(&zero_base("crates/fjell-kernel/src/cap/syscall.rs", ok)).is_ok());
    }

    #[test]
    fn a_stale_classified_task_id_site_is_red() {
        let mut v = zero_base("", "");
        v[2] = src(
            "crates/fjell-kernel/src/trap/dispatch.rs",
            "pub fn current_task_id() -> TaskId { sched.current().unwrap() }\n",
        );
        let e = evaluate(&v).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("stale classified task-id site")
                    && p.contains("current_task_id")),
            "{e:?}"
        );
    }

    #[test]
    fn a_classified_function_name_in_the_wrong_file_is_not_exempt() {
        let e = evaluate(&zero_base(
            "crates/fjell-kernel/src/cap/syscall.rs",
            "pub fn current_task_id() -> TaskId { TaskId::new(2, 0) }\n",
        ))
        .unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("cap/syscall.rs") && p.contains("literal generation 0")),
            "{e:?}"
        );
    }

    // ── premise 5: device mappings are unmapped only by the classified callers ──────────────────────────────────────────

    #[test]
    fn the_device_unmap_control_is_green_and_counts_its_sites() {
        let r = evaluate(&base()).unwrap();
        assert_eq!(r.device_unmap_sites, CLASSIFIED_DEVICE_UNMAP_CALLERS.len());
    }

    #[test]
    fn lease_revocation_calling_a_device_unmap_primitive_is_red_and_says_restate_the_claim() {
        for call in [
            "unmap_page(root, va)",
            "crate::unmap_dma_region_verified(owner, va, pa, 1)",
            "dma_table().revoke_by_pa(task, pa)",
            "crate::mmio_mappings().release_task(id)",
        ] {
            let mut v = base();
            v.push(src(
                "crates/fjell-kernel/src/lease/mod.rs",
                &format!(
                    "impl LeaseTable {{ pub fn revoke(&mut self, id: LeaseId) {{ {call}; }} }}\n"
                ),
            ));
            let e = evaluate(&v).unwrap_err();
            assert!(
                e.iter().any(|p| p.contains("lease/mod.rs")
                    && p.contains("`revoke`")
                    && p.contains("restate it")),
                "{call}: {e:?}"
            );
        }
    }

    #[test]
    fn a_sink_that_is_only_a_longer_name_or_a_definition_or_prose_is_not_a_call() {
        let ok = "fn my_unmap_page(x: u32) {}\n\
                  fn unmap_page(x: u32) { inner(); }\n\
                  // unmap_page(root, va) is described here\n\
                  fn s() { let _ = \"release_task(1)\"; }\n\
                  #[cfg(test)]\nmod tests {\n    fn t() { unmap_page(1); }\n}\n";
        let mut v = base();
        v.push(src("crates/fjell-kernel/src/mm/other.rs", ok));
        assert!(evaluate(&v).is_ok());
    }

    #[test]
    fn a_stale_classified_device_unmap_caller_is_red() {
        let mut v = base();
        for s in v.iter_mut() {
            if s.path.ends_with("trap/syscall.rs") {
                s.text = s.text.replace(
                    "fn sys_mmio_unmap() { unmap_page(1); }",
                    "fn sys_mmio_unmap() {}",
                );
            }
        }
        let e = evaluate(&v).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("stale classified device-unmap caller")
                    && p.contains("sys_mmio_unmap")),
            "{e:?}"
        );
    }
}
