//! `capability-topology` — a service's own CSpace declaration and the
//! kernel's install must agree (RFC-0.35-001 R3/R4, E-072).
//!
//! Five services address a peer through eleven `CapHandle` slots they
//! declare in their own source; `crates/fjell-kernel/src/task/spawn.rs`
//! never installs a capability at any of them. Two of the five busy-spin on
//! the empty slot for the lifetime of every boot (`sys_ipc_recv` errors
//! rather than blocks on an unfilled handle). Nothing compared the two
//! declarations before this line — Gate 11's callsite conformance checks
//! LEASE/CAP/SYSCALL/BCB callsites, not this.
//!
//! **The eleven are enumerated explicitly, not discovered by scanning every
//! `CapHandle` in every service.** Slot 0 — every image's own endpoint,
//! installed generically by `spawn.rs`'s fallback — would false-positive on
//! a blind scan; it is not one of E-072's instances.
//!
//! **What this checks, for each declared slot:**
//! 1. *Sanity* — the service's own source still declares it
//!    (`CapHandle(<slot>)` appears in the named file), so this table cannot
//!    go stale against a service that has moved on.
//! 2. *Installed* — `spawn.rs` contains an `install_raw(<slot>, …)` call
//!    inside the brace-matched body of an `if image_id == … ImageId::<X>`
//!    condition naming this slot's image (handling a `||`-joined
//!    condition, as `PROXY_BRAILLE`/`SVC_PRESENTATION_FAULT` already share
//!    one).
//!
//! A slot this line's `waived_by` names is reported, not required to be
//! installed — whether to serve it is **RFC-0.35-002's decision**, and
//! installing authority a component does not yet exercise is the
//! least-privilege violation this project exists to refuse. The gate PASSes
//! when every declared slot is either installed or explicitly waived; it
//! FAILs on a slot that is neither — which is every one of the eleven,
//! before this line's two installs land.

use crate::read_file;
use std::process::ExitCode;

const NAME: &str = "capability-topology";
const SPAWN_PATH: &str = "crates/fjell-kernel/src/task/spawn.rs";

struct DeclaredSlot {
    service_file: &'static str,
    image: &'static str,
    slot: u32,
    purpose: &'static str,
    /// `Some(reason)` for a slot this line does not install — named so a
    /// future reader knows it is accounted for, not overlooked.
    waived_by: Option<&'static str>,
}

/// E-072's eleven instances, 2026-10-14. Updated in the same commit as
/// whichever line changes a service's disposition (RFC-0.35-002, for the
/// nine waived here).
const DECLARED_SLOTS: &[DeclaredSlot] = &[
    DeclaredSlot {
        service_file: "crates/services/fjell-secure-transportd/src/main.rs",
        image: "SECURE_TRANSPORTD",
        slot: 1,
        purpose: "smgr readiness (superseded by SERVICE_READY_SEND_SLOT)",
        waived_by: Some("E-072 survivor — not the update path (RFC-0.35-001 R5)"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-secure-transportd/src/main.rs",
        image: "SECURE_TRANSPORTD",
        slot: 2,
        purpose: "upgraded — the update path's metadata channel",
        waived_by: None,
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-secure-transportd/src/main.rs",
        image: "SECURE_TRANSPORTD",
        slot: 3,
        purpose: "diagnosticsd",
        waived_by: Some("RFC-0.35-002"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-secure-transportd/src/main.rs",
        image: "SECURE_TRANSPORTD",
        slot: 4,
        purpose: "attestd",
        waived_by: Some("RFC-0.35-002"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-diagnosticsd/src/main.rs",
        image: "DIAGNOSTICSD",
        slot: 1,
        purpose: "tools",
        waived_by: Some("RFC-0.35-002"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-diagnosticsd/src/main.rs",
        image: "DIAGNOSTICSD",
        slot: 2,
        purpose: "secure-transportd",
        waived_by: Some("RFC-0.35-002"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-diagnosticsd/src/main.rs",
        image: "DIAGNOSTICSD",
        slot: 3,
        purpose: "auditd",
        waived_by: Some("RFC-0.35-002"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-diagnosticsd/src/main.rs",
        image: "DIAGNOSTICSD",
        slot: 4,
        purpose: "measuredd",
        waived_by: Some("RFC-0.35-002"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-upgraded/src/main.rs",
        image: "UPGRADED",
        slot: 6,
        purpose: "secure-transportd — the update path's metadata channel",
        waived_by: None,
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-attestd/src/main.rs",
        image: "ATTESTD",
        slot: 5,
        purpose: "secure-transportd",
        waived_by: Some("RFC-0.35-002"),
    },
    DeclaredSlot {
        service_file: "crates/services/fjell-netd/src/main.rs",
        image: "NETD",
        slot: 1,
        purpose: "driver",
        waived_by: Some("RFC-0.35-002"),
    },
];

/// Extract the brace-matched body starting at the first `{` at or after
/// `from`. Mirrors `fjell-tools`' `callsite_audit::extract_braced_body` —
/// duplicated rather than shared, because the two tools do not depend on
/// each other and this is fifteen lines.
fn extract_braced_body(src: &str, from: usize) -> Option<&str> {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut body_start = None;
    let mut i = from;
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
                    return Some(&src[body_start?..i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Does `spawn_src` install a capability at `slot` inside a block whose `if`
/// condition names `ImageId::<image>` (alone or `||`-joined with others)?
///
/// Requires the full `image_id == fjell_abi::service::ImageId::<X>`
/// comparison, not a bare `ImageId::<X>` — `ATTESTD` et al. also appear as
/// `ep_obj`/`stack_pages` **match arms**, which are not conditions and have
/// no enclosing `if`-block brace to find; matching the bare name walked
/// into an unrelated block and false-positived on an unrelated
/// `install_raw` (caught by this check's own test before being trusted).
fn image_block_installs_slot(spawn_src: &str, image: &str, slot: u32) -> bool {
    let needle = format!("image_id == fjell_abi::service::ImageId::{image}");
    let mut search_from = 0;
    while let Some(rel) = spawn_src[search_from..].find(&needle) {
        let match_end = search_from + rel + needle.len();
        // Nothing but more `|| image_id == …` conditions can sit between a
        // real match here and the block's own opening brace — no condition
        // in this file contains a `{` — so the next `{` after the match is
        // the body to open, regardless of which `||`-joined arm matched.
        if let Some(open_rel) = spawn_src[match_end..].find('{') {
            let open_idx = match_end + open_rel;
            if let Some(body) = extract_braced_body(spawn_src, open_idx) {
                let pattern = "install_raw(\n";
                let mut p = 0;
                while let Some(call_rel) = body[p..].find(pattern) {
                    let after = p + call_rel + pattern.len();
                    let rest = body[after..].trim_start();
                    if let Some(num_end) = rest.find(|c: char| !c.is_ascii_digit()) {
                        if let Ok(n) = rest[..num_end].parse::<u32>() {
                            if n == slot && rest[num_end..].trim_start().starts_with(',') {
                                return true;
                            }
                        }
                    }
                    p = after;
                }
            }
        }
        search_from = match_end;
    }
    false
}

pub fn check() -> ExitCode {
    let Some(spawn_src) = read_file(NAME, SPAWN_PATH) else {
        return ExitCode::FAILURE;
    };

    let mut ok = true;
    let mut installed = 0;
    let mut waived = 0;
    let mut file_cache: Vec<(&str, String)> = Vec::new();

    for decl in DECLARED_SLOTS {
        let service_src = if let Some((_, s)) =
            file_cache.iter().find(|(f, _)| *f == decl.service_file)
        {
            s
        } else {
            let Some(s) = read_file(NAME, decl.service_file) else {
                return ExitCode::FAILURE;
            };
            file_cache.push((decl.service_file, s));
            &file_cache.last().unwrap().1
        };
        let declared_pattern = format!("CapHandle({})", decl.slot);
        if !service_src.contains(&declared_pattern) {
            eprintln!(
                "{NAME}: FAIL — {} no longer declares `{declared_pattern}` (slot {} for {}); \
                 this table has gone stale and must be updated or removed",
                decl.service_file, decl.slot, decl.purpose
            );
            ok = false;
            continue;
        }

        let is_installed = image_block_installs_slot(&spawn_src, decl.image, decl.slot);

        match (&decl.waived_by, is_installed) {
            (None, true) => {
                installed += 1;
            }
            (None, false) => {
                ok = false;
                eprintln!(
                    "{NAME}: FAIL — {} slot {} ({}) is declared but spawn.rs installs nothing \
                     there",
                    decl.image, decl.slot, decl.purpose
                );
            }
            (Some(reason), installed_flag) => {
                waived += 1;
                if installed_flag {
                    // A waived slot that turns out to be installed is not a
                    // failure — it means the next line already landed —
                    // but it is worth a plain line rather than silence.
                    println!(
                        "{NAME}: note — {} slot {} ({}) is installed; waiver ({reason}) can be \
                         retired",
                        decl.image, decl.slot, decl.purpose
                    );
                } else {
                    // Still printed, not silent: this is what lets one
                    // invocation show all eleven declared-but-uninstalled
                    // slots at once (R4's "show that run"), with the exit
                    // status carrying only the two this line owns.
                    println!(
                        "{NAME}: note — {} slot {} ({}) is declared but spawn.rs installs \
                         nothing there — waived to {reason}, not this line's business",
                        decl.image, decl.slot, decl.purpose
                    );
                }
            }
        }
    }

    if ok {
        println!(
            "{NAME}: PASS ({installed} installed, {waived} waived to a named line, \
             {} checked)",
            DECLARED_SLOTS.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("{NAME}: FAIL");
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_slots_cover_exactly_the_eleven_the_rfc_names() {
        assert_eq!(DECLARED_SLOTS.len(), 11);
        assert_eq!(
            DECLARED_SLOTS.iter().filter(|d| d.waived_by.is_none()).count(),
            2,
            "exactly two slots are this line's own business"
        );
    }

    #[test]
    fn extract_braced_body_finds_matching_close() {
        let src = "if x { a(); { b(); } c(); } else {}";
        let open = src.find('{').unwrap();
        let body = extract_braced_body(src, open).unwrap();
        assert_eq!(body, " a(); { b(); } c(); ");
    }

    #[test]
    fn image_block_installs_slot_finds_a_real_block() {
        let src = "if image_id == fjell_abi::service::ImageId::PROXY_TEXT {\n    \
            let _ = cs.install_raw(\n        1,\n        Capability {},\n    );\n}\n";
        assert!(image_block_installs_slot(src, "PROXY_TEXT", 1));
        assert!(!image_block_installs_slot(src, "PROXY_TEXT", 2));
        assert!(!image_block_installs_slot(src, "OTHER_IMAGE", 1));
    }

    #[test]
    fn image_block_installs_slot_ignores_a_match_arm_mention() {
        // Caught live: a bare `ImageId::ATTESTD` match-arm occurrence (no
        // `if`/condition at all) walked into an unrelated `if` block's
        // brace span and found an unrelated `install_raw(5, …)`, reporting
        // ATTESTD's slot 5 installed when nothing installs it.
        let src = "let ep_obj: u32 = match image_id {\n    \
            fjell_abi::service::ImageId::ATTESTD => 3,\n    \
            _ => 0,\n};\n\
            if image_id == fjell_abi::service::ImageId::PROXY_TEXT {\n    \
            let _ = cs.install_raw(\n        5,\n        Capability {},\n    );\n}\n";
        assert!(!image_block_installs_slot(src, "ATTESTD", 5));
    }

    #[test]
    fn image_block_installs_slot_handles_or_joined_condition() {
        let src = "if image_id == fjell_abi::service::ImageId::PROXY_BRAILLE\n    \
            || image_id == fjell_abi::service::ImageId::SVC_PRESENTATION_FAULT\n{\n    \
            let _ = cs.install_raw(\n        1,\n        Capability {},\n    );\n}\n";
        assert!(image_block_installs_slot(src, "SVC_PRESENTATION_FAULT", 1));
    }

    #[test]
    fn declared_slots_are_still_real_on_the_current_tree() {
        // `cargo test` runs with this crate's own directory as the working
        // directory, not the workspace root `check()` assumes in
        // production (`cargo xtask consistency-check` always runs from the
        // root) — compute the root from `CARGO_MANIFEST_DIR` instead.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("tools/fjell-consistency-check is two levels under the workspace root");
        for decl in DECLARED_SLOTS {
            let path = root.join(decl.service_file);
            let src = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let pattern = format!("CapHandle({})", decl.slot);
            assert!(
                src.contains(&pattern),
                "{} no longer declares {pattern} ({})",
                decl.service_file,
                decl.purpose
            );
        }
    }
}
