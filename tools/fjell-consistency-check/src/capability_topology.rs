//! `capability-topology` — a service's own CSpace declaration and the
//! kernel's install must agree (RFC-0.35-001 R3/R4, E-072).
//!
//! **The universe is discovered, not enumerated.** Every
//! `crates/services/*/src/main.rs` is scanned for its own
//! `const NAME: CapHandle = CapHandle(<slot>)` declarations — the shape
//! every one of E-072's eleven instances takes. A hand-maintained list of
//! "the eleven" would be a **third** independent declaration of the
//! topology, alongside the service's own source and `spawn.rs`'s installs,
//! and a twelfth instance added anywhere would simply not appear in it
//! (caught in review: a planted `CAP_ARCH_PROBE_12TH` slot passed silently
//! against the enumerated list this module used before this revision).
//! `WAIVERS` below is now what its name says — slots this line does not
//! install, each with a reason — not the set this check looks for.
//!
//! **Two slots are excluded from discovery entirely, not waived**, because
//! this module cannot evaluate them at all, not because their outcome is
//! deferred:
//! - **slot 0**, for every service — every image's own identity endpoint,
//!   installed unconditionally for every `image_id` by `spawn.rs`'s
//!   `_ => 0` fallback arm, outside any per-image condition this module's
//!   `image_block_installs_slot` can even name.
//! - **`fjell-init`**, entirely — `init` is spawned directly in
//!   `crates/fjell-kernel/src/main.rs`, never through `spawn.rs`'s
//!   per-image blocks this module reads (confirmed, RFC-0.35-001 R1's own
//!   control: `init`'s CSpace install is a different site). Its declared
//!   `UART_RX_EP` (slot 8) **is** installed — `main.rs:1293`'s own
//!   `install_raw(8, …)` inside `init`'s special-cased spawn path — this
//!   module simply has no way to check it, and says so here rather than
//!   mis-reporting it as waived-but-uninstalled or silently passing it.
//!
//! **What this checks, for each discovered slot:**
//! 1. *Installed* — `spawn.rs` contains an `install_raw(<slot>, …)` call
//!    inside the brace-matched body of an `if image_id == … ImageId::<X>`
//!    condition naming this slot's image (handling a `||`-joined
//!    condition, as `PROXY_BRAILLE`/`SVC_PRESENTATION_FAULT` already share
//!    one).
//! 2. *Or waived* — `WAIVERS` names this exact (image, slot) with a reason.
//!    Whether to serve a waived slot is **RFC-0.35-002's decision** —
//!    installing authority a component does not yet exercise is the
//!    least-privilege violation this project exists to refuse.
//!
//! The gate PASSes when every discovered slot is either installed or
//! explicitly waived; it FAILs on a slot that is neither — a new,
//! unclassified declared-and-uninstalled slot included, the moment it is
//! declared, with no list to update first.

use crate::read_file;
use std::process::ExitCode;

const NAME: &str = "capability-topology";
const SPAWN_PATH: &str = "crates/fjell-kernel/src/task/spawn.rs";
const SERVICES_DIR: &str = "crates/services";

/// Spawned outside `spawn.rs`'s per-image blocks entirely (see the module
/// doc) — excluded from discovery, not waived: this module cannot evaluate
/// its installs at all, true or false.
const EXCLUDED_SERVICE_DIRS: &[&str] = &["fjell-init"];

struct Waiver {
    image: &'static str,
    slot: u32,
    purpose: &'static str,
    reason: &'static str,
}

/// E-072's nine survivors, 2026-10-14, named with the reason this line does
/// not install them. Updated in the same commit as whichever line changes
/// a service's disposition (RFC-0.35-002, for all nine here).
const WAIVERS: &[Waiver] = &[
    Waiver {
        image: "SECURE_TRANSPORTD",
        slot: 1,
        purpose: "smgr readiness (superseded by SERVICE_READY_SEND_SLOT)",
        reason: "E-072 survivor — not the update path (RFC-0.35-001 R5)",
    },
    Waiver {
        image: "SECURE_TRANSPORTD",
        slot: 3,
        purpose: "diagnosticsd",
        reason: "RFC-0.35-002",
    },
    Waiver {
        image: "SECURE_TRANSPORTD",
        slot: 4,
        purpose: "attestd",
        reason: "RFC-0.35-002",
    },
    Waiver {
        image: "DIAGNOSTICSD",
        slot: 1,
        purpose: "tools",
        reason: "RFC-0.35-002",
    },
    Waiver {
        image: "DIAGNOSTICSD",
        slot: 2,
        purpose: "secure-transportd",
        reason: "RFC-0.35-002",
    },
    Waiver {
        image: "DIAGNOSTICSD",
        slot: 3,
        purpose: "auditd",
        reason: "RFC-0.35-002",
    },
    Waiver {
        image: "DIAGNOSTICSD",
        slot: 4,
        purpose: "measuredd",
        reason: "RFC-0.35-002",
    },
    Waiver {
        image: "ATTESTD",
        slot: 5,
        purpose: "secure-transportd",
        reason: "RFC-0.35-002, E-074 (push_attestation has no caller)",
    },
    Waiver {
        image: "NETD",
        slot: 1,
        purpose: "driver",
        reason: "RFC-0.35-002, E-074 (no reference outside its own const)",
    },
];

#[derive(Debug)]
struct DiscoveredSlot {
    service_dir: String,
    const_name: String,
    image: String,
    slot: u32,
}

/// `crates/services/<dir>` → the `ImageId::<X>` this service spawns as —
/// the same derivation `spawn.rs`'s own per-image conditions and every
/// `ImageId` variant already use (`fjell-svc-presentation-fault` →
/// `SVC_PRESENTATION_FAULT`, `fjell-secure-transportd` →
/// `SECURE_TRANSPORTD`): strip the crate's `fjell-` prefix, uppercase,
/// hyphens to underscores.
fn image_name_for(service_dir: &str) -> String {
    service_dir
        .strip_prefix("fjell-")
        .unwrap_or(service_dir)
        .to_uppercase()
        .replace('-', "_")
}

/// Scan `src` for every `const NAME: CapHandle = CapHandle(<slot>)`
/// declaration (optionally `fjell_cap`-qualified on either or both sides) —
/// the shape a service uses to name a peer's endpoint slot in its own
/// CSpace. A call-site conversion like `CapHandle(SLOT_OWN_EP)` or
/// `CapHandle::new(n, 0)` does not match: this is a **declaration** of a
/// slot number as this service's own name for it, not a syscall argument.
fn find_cap_handle_consts(src: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(const_rel) = rest.find("const ") {
        let after_const = &rest[const_rel + "const ".len()..];
        let Some(colon_rel) = after_const.find(':') else {
            break;
        };
        let name = after_const[..colon_rel].trim();
        let after_name = &after_const[colon_rel + 1..];
        let Some(eq_rel) = after_name.find('=') else {
            rest = after_name;
            continue;
        };
        let ty = after_name[..eq_rel].trim();
        let ty = ty.strip_prefix("fjell_cap::").unwrap_or(ty);
        let after_eq = after_name[eq_rel + 1..].trim_start();
        let after_eq = after_eq.strip_prefix("fjell_cap::").unwrap_or(after_eq);
        if ty == "CapHandle"
            && !name.is_empty()
            && name.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            if let Some(num_src) = after_eq.strip_prefix("CapHandle(") {
                if let Some(close_rel) = num_src.find(')') {
                    let digits = num_src[..close_rel].trim();
                    if let Ok(slot) = digits.parse::<u32>() {
                        out.push((name.to_string(), slot));
                    }
                }
            }
        }
        rest = after_name;
    }
    out
}

/// Every `<services_dir>/*/src/main.rs` directory name, sorted, so output
/// order is stable across runs and platforms. `services_dir` is a
/// parameter (not the `SERVICES_DIR` constant) solely so this crate's own
/// tests can point it at an absolute path and avoid `set_current_dir` —
/// production always calls this with `SERVICES_DIR`.
fn service_dirs(services_dir: &str) -> Option<Vec<String>> {
    let entries = crate::read_dir_named(NAME, services_dir)?;
    let mut dirs: Vec<String> = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else { continue };
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if EXCLUDED_SERVICE_DIRS.contains(&name.as_str()) {
            continue;
        }
        dirs.push(name);
    }
    dirs.sort();
    Some(dirs)
}

fn discover_slots(services_dir: &str) -> Option<Vec<DiscoveredSlot>> {
    let mut out = Vec::new();
    for service_dir in service_dirs(services_dir)? {
        let path = format!("{services_dir}/{service_dir}/src/main.rs");
        let src = read_file(NAME, &path)?;
        let image = image_name_for(&service_dir);
        for (const_name, slot) in find_cap_handle_consts(&src) {
            if slot == 0 {
                // Every image's own identity endpoint — spawn.rs's
                // unconditional fallback, not a per-image install this
                // module can name a condition for. See the module doc.
                continue;
            }
            out.push(DiscoveredSlot {
                service_dir: service_dir.clone(),
                const_name,
                image: image.clone(),
                slot,
            });
        }
    }
    Some(out)
}

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
    let Some(discovered) = discover_slots(SERVICES_DIR) else {
        return ExitCode::FAILURE;
    };

    let mut ok = true;
    let mut installed = 0;
    let mut waived = 0;

    for decl in &discovered {
        let waiver = WAIVERS
            .iter()
            .find(|w| w.image == decl.image && w.slot == decl.slot);
        let is_installed = image_block_installs_slot(&spawn_src, &decl.image, decl.slot);

        match (waiver, is_installed) {
            (None, true) => {
                installed += 1;
            }
            (None, false) => {
                ok = false;
                eprintln!(
                    "{NAME}: FAIL — {} slot {} (`{}`, {}) is declared but spawn.rs installs \
                     nothing there, and no waiver names it",
                    decl.image, decl.slot, decl.const_name, decl.service_dir
                );
            }
            (Some(w), true) => {
                waived += 1;
                // A waived slot that turns out to be installed is not a
                // failure — it means the next line already landed —
                // but it is worth a plain line rather than silence.
                println!(
                    "{NAME}: note — {} slot {} ({}) is installed; waiver ({}) can be retired",
                    decl.image, decl.slot, w.purpose, w.reason
                );
            }
            (Some(w), false) => {
                waived += 1;
                // Still printed, not silent: this is what lets one
                // invocation show every declared-but-uninstalled slot at
                // once (R4's "show that run"), with the exit status
                // carrying only the ones no waiver names.
                println!(
                    "{NAME}: note — {} slot {} ({}) is declared but spawn.rs installs \
                     nothing there — waived to {}, not this line's business",
                    decl.image, decl.slot, w.purpose, w.reason
                );
            }
        }
    }

    if ok {
        println!(
            "{NAME}: PASS ({installed} installed, {waived} waived to a named line, \
             {} checked)",
            discovered.len()
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
    fn image_name_for_matches_existing_image_id_variants() {
        assert_eq!(image_name_for("fjell-secure-transportd"), "SECURE_TRANSPORTD");
        assert_eq!(
            image_name_for("fjell-svc-presentation-fault"),
            "SVC_PRESENTATION_FAULT"
        );
        assert_eq!(image_name_for("fjell-netd"), "NETD");
    }

    #[test]
    fn find_cap_handle_consts_matches_plain_and_qualified_forms() {
        let src = "const CAP_SXT_EP: fjell_cap::CapHandle = fjell_cap::CapHandle(5);\n\
                   const CAP_DRV_EP: CapHandle = CapHandle(1);\n\
                   const CAP_NETDEV: CapHandle = CapHandle(0);\n";
        let found = find_cap_handle_consts(src);
        assert_eq!(
            found,
            vec![
                ("CAP_SXT_EP".to_string(), 5),
                ("CAP_DRV_EP".to_string(), 1),
                ("CAP_NETDEV".to_string(), 0),
            ]
        );
    }

    #[test]
    fn find_cap_handle_consts_ignores_call_sites_not_declarations() {
        let src = "let _ = sys_reboot(CapHandle(SLOT_REBOOT), 0);\n\
                   let h = CapHandle::new(34, 0);\n\
                   const SLOT_OWN_EP: u32 = 0;\n";
        assert!(find_cap_handle_consts(src).is_empty());
    }

    #[test]
    fn find_cap_handle_consts_catches_a_new_undeclared_instance() {
        // The shape of the review's own demonstration (D2): a slot this
        // module had never heard of, added to a service that already has
        // others. Discovery must see it without any list being touched.
        let src = "const CAP_ARCH_PROBE_12TH: CapHandle = CapHandle(7);\n";
        assert_eq!(
            find_cap_handle_consts(src),
            vec![("CAP_ARCH_PROBE_12TH".to_string(), 7)]
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
    fn discovery_on_the_current_tree_matches_the_known_eleven() {
        // `cargo test` runs with this crate's own directory as the working
        // directory, not the workspace root `check()` assumes in
        // production (`cargo xtask consistency-check` always runs from the
        // root) — pass the computed root's own services dir in, rather
        // than `set_current_dir` (which races every other test in this
        // binary run in parallel on the same process-wide cwd).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("tools/fjell-consistency-check is two levels under the workspace root");
        let services_dir = root.join(SERVICES_DIR);
        let discovered = discover_slots(&services_dir.to_string_lossy())
            .expect("discovery must succeed on the real tree");
        assert_eq!(
            discovered.len(),
            11,
            "expected exactly E-072's eleven non-zero, non-init slots: {discovered:#?}",
        );
        let unwaived = discovered
            .iter()
            .filter(|d| !WAIVERS.iter().any(|w| w.image == d.image && w.slot == d.slot))
            .count();
        assert_eq!(unwaived, 2, "exactly two slots are this line's own business");
    }
}
