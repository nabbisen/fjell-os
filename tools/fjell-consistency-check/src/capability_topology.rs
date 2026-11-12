//! `capability-topology` — a service's own CSpace declaration and the
//! kernel's install must agree (RFC-0.35-001 R3/R4, E-072).
//!
//! **The universe is discovered, not enumerated -- anywhere under `crates/`.**
//! Every `.rs` file beneath `crates/`, at any depth, is scanned for
//! `const NAME: CapHandle = CapHandle(<slot>)` declarations -- the shape
//! every one of E-072's eleven instances takes. Each hit is attributed to the
//! crate that contains it (the nearest ancestor directory holding a
//! `Cargo.toml`) and from there to the `ImageId` it spawns as, read from
//! `fjell-abi`'s own list of images. A hand-maintained list of "the eleven" --
//! or of directories to search, which `SERVICES_DIR = "crates/services"` was --
//! would be a **third** independent declaration of the topology, alongside the
//! service's own source and `spawn.rs`'s installs, and a declaration written
//! anywhere it does not name would simply not appear in it. Two caught in
//! review: a planted `CAP_ARCH_PROBE_12TH` passed silently against the
//! enumerated list this module used first, and nine of the nineteen
//! declarations in the tree (`crates/drivers/`) were never scanned because the
//! search was one hard-coded directory (RFC-0.43-002, E-117).
//! A declaration in a crate that is **not** a spawnable image (no `ImageId` to
//! attribute it to) is a failure, not a skip: a slot this check cannot judge is
//! one it cannot vouch for.
//! `WAIVERS` below is what its name says -- slots this line does not
//! install, each with a reason -- not the set this check looks for.
//!
//! **One service is excluded from discovery, not waived**, because this module
//! cannot evaluate it at all, not because its outcome is deferred:
//! - **`fjell-init`**, entirely -- `init` is spawned directly in
//!   `crates/fjell-kernel/src/main.rs`, never through `spawn.rs`'s
//!   per-image blocks this module reads (confirmed, RFC-0.35-001 R1's own
//!   control: `init`'s CSpace install is a different site). Its declared
//!   `UART_RX_EP` (slot 8) **is** installed -- `main.rs:1293`'s own
//!   `install_raw(8, …)` inside `init`'s special-cased spawn path -- this
//!   module simply has no way to check it, and says so here rather than
//!   mis-reporting it as waived-but-uninstalled or silently passing it.
//!
//! **Slot 0 is judged, not excluded** (RFC-0.43-002 R2). It used to be skipped
//! for every service on the grounds that `spawn.rs`'s `_ => 0` fallback
//! installed it unconditionally for every image -- true until RFC-0.41-002 R3
//! deleted that fallback. Now `spawn.rs`'s `ep_obj` table maps an image to an
//! endpoint object or to `None`, and **an image has slot 0 installed if and only
//! if its arm grants `Some(..)`**; an image with no arm gets nothing there, so a
//! declared slot 0 can be wrong. This module reads those arms
//! (`object_zero::ep_obj_installs`) and fails closed on a shape it cannot
//! attribute.
//!
//! **MMIO capabilities install at `<base> + region_idx`**, from a second table
//! (`mmio_regions_for_service`), in a loop -- not in any `if image_id == …` block.
//! They are read the same way (`object_zero::mmio_slots_by_image`, base read from
//! the install itself), so `fjell-driver-uart`'s correct `CapHandle(32)` (region 1)
//! is shown installed rather than waived.
//!
//! **What this checks, for each discovered slot:**
//! 1. *Installed* — one of: `spawn.rs` contains an `install_raw(<slot>, …)`
//!    call inside the brace-matched body of an `if image_id == … ImageId::<X>`
//!    condition naming this slot's image (handling a `||`-joined condition, as
//!    `PROXY_BRAILLE`/`SVC_PRESENTATION_FAULT` already share one); or the slot is
//!    0 and the image's `ep_obj` arm grants an object; or the slot is
//!    `<base> + r` for a region `r` in the image's `mmio_regions_for_service` arm.
//! 2. *Or waived* — `WAIVERS` names this exact (image, slot) with a reason.
//!    Whether to serve a waived slot is **RFC-0.35-002's decision** —
//!    installing authority a component does not yet exercise is the
//!    least-privilege violation this project exists to refuse.
//!
//! The gate PASSes when every discovered slot is either installed or
//! explicitly waived; it FAILs on a slot that is neither — a new,
//! unclassified declared-and-uninstalled slot included, the moment it is
//! declared, with no list to update first.
//!
//! **What discovery does not see, stated so a reader does not assume
//! otherwise** (architect probes, 2026-10-14, each run against this module
//! and each passing silently before the first was fixed):
//! - ~~a slot declared outside `main.rs`~~ — **fixed**: every `.rs` under a
//!   service's `src/`, recursively, is now scanned. The probe that found
//!   this planted `CapHandle(9)` in `secure-transportd`'s `channel.rs`.
//! - **a slot whose number is a named constant** rather than a decimal
//!   literal — `CapHandle(SOME_SLOT)` is not matched, because resolving it
//!   would mean evaluating constants, and a text-scanning checker that
//!   pretended to do that would be the worse defect. Every one of E-072's
//!   eleven instances is a literal, and so is every slot in the tree today;
//!   if that changes, this check must grow a way to say so rather than
//!   quietly missing it.
//! - **a binding that is not a `const` with a SCREAMING_CASE name** — a
//!   `let` or a lowercase constant is not matched, for the same reason and
//!   with the same obligation.
//!
//! These are limits of a text scanner, not deferred decisions. They are
//! here because the sentence *"the universe is discovered"* is true of the
//! shape every declaration in this tree takes and **not** of every shape
//! Rust permits, and the difference is exactly the kind of gap this line
//! exists to close.

use crate::read_file;
use std::process::ExitCode;

const NAME: &str = "capability-topology";
const SPAWN_PATH: &str = "crates/fjell-kernel/src/task/spawn.rs";
/// The root of the scan: every crate in the workspace (RFC-0.43-002 R1). Not a services
/// directory and not a list of directories.
const CRATES_DIR: &str = "crates";
const ABI_SERVICE_PATH: &str = "crates/fjell-abi/src/service.rs";

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

/// E-072's remaining survivors, named with the reason this line does not
/// install them. Updated in the same commit as whichever line changes a
/// service's disposition. RFC-0.35-002 retired five by deleting the
/// declaration outright: `SECURE_TRANSPORTD` slots 3 and 4, and
/// `DIAGNOSTICSD` slot 3, closed E-068/E-069 (the protocols they addressed
/// were retracted, not merely left unserved); `ATTESTD` slot 5 and `NETD`
/// slot 1 closed E-074 the same way (built, never wired to a caller —
/// deleted rather than waived further). None of those five is discovered
/// any more, so none would be matched here even if left in — removed
/// anyway, so this list does not claim a slot exists that does not.
/// RFC-0.37-001 retired a sixth **differently**: `DIAGNOSTICSD` slot 1
/// (`tools`) still exists and is still discovered — `fleetd` is now its
/// first real caller, so the slot is **installed**, not waived, and simply
/// no longer needs an entry here. **A waiver names where the slot's
/// disposition lives, and that name has to keep meaning something.**
/// RFC-0.39-002 satisfied a seventh: `DIAGNOSTICSD` slot 4 (`measuredd`) is now an
/// installed `CALL` capability, so its waiver is **gone** -- not reworded. A
/// waiver that has been satisfied is not a waiver with new text.
/// Re-check these whenever a line named here closes.
const WAIVERS: &[Waiver] = &[
    Waiver {
        image: "DIAGNOSTICSD",
        slot: 2,
        purpose: "secure-transportd",
        reason: "E-069 closed: transportd serves one endpoint and never this \
                 one; `push_bundle` fails closed on the absence and the caller \
                 replies PUSH_FAULT, so nothing reads a false value",
    },
    // RFC-0.43-002 R4. Slot 0 is where a capability delivered **at runtime by
    // cap-broker** arrives; there is no spawn-time install to find, so "declared and
    // not installed by spawn.rs" is the expected state, not a mismatch. `SECURE_TRANSPORTD`
    // has no `ep_obj` arm, which is correct: it receives nothing at spawn.
    // **cap-broker's `SxtSession = 9` is a `ResourceClass` discriminant, not a slot** -- a
    // reader who sees `9` beside this waiver must not take it for a slot mismatch.
    Waiver {
        image: "SECURE_TRANSPORTD",
        slot: 0,
        purpose: "cap-broker",
        reason: "slot 0 is where a capability delivered at runtime by cap-broker \
                 is meant to arrive (`CAP_SESSION`); there is no spawn-time install \
                 to find. That delivery is not live yet: the constant is \
                 `#[allow(dead_code)]` and read nowhere. cap-broker's `SxtSession = 9` \
                 is a ResourceClass discriminant, not a slot",
    },
];

#[derive(Debug)]
struct DiscoveredSlot {
    /// The declaring crate's directory name (`fjell-driver-virtio-net`).
    service_dir: String,
    const_name: String,
    image: String,
    slot: u32,
    /// Where it was declared, for the report.
    file: String,
}

/// What discovery found: the slots it could attribute to an image, and the declarations
/// it could not (a crate that is not a spawnable image). The second list is a failure.
#[derive(Debug, Default)]
struct Discovery {
    slots: Vec<DiscoveredSlot>,
    unattributed: Vec<String>,
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
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
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

/// Every `ImageId` name declared in `fjell-abi`'s `service.rs`
/// (`pub const NAME: ImageId = ImageId(..)`) -- the authoritative list of what can be spawned.
fn image_ids(abi_service: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in abi_service.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("pub const ") {
            if let Some((name, ty)) = rest.split_once(':') {
                if ty.trim_start().starts_with("ImageId") {
                    out.push(name.trim().to_string());
                }
            }
        }
    }
    out
}

/// Walk `dir` recursively and collect every `.rs` file, each with the directory name of the
/// crate that contains it (the nearest ancestor holding a `Cargo.toml`), in sorted order.
/// `crate_name` is the enclosing crate on entry. Read failures report through
/// `read_dir_named`, the module's one traversal helper, and abort discovery.
fn rs_files_with_crate(
    dir: &str,
    crate_name: Option<&str>,
    out: &mut Vec<(String, Option<String>)>,
) -> bool {
    let Some(entries) = crate::read_dir_named(NAME, dir) else {
        return false;
    };
    let mut files: Vec<String> = Vec::new();
    let mut subdirs: Vec<String> = Vec::new();
    let mut has_manifest = false;
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            subdirs.push(name);
        } else if name == "Cargo.toml" {
            has_manifest = true;
        } else if name.ends_with(".rs") {
            files.push(name);
        }
    }
    let here_crate: Option<String> = if has_manifest {
        std::path::Path::new(dir)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    } else {
        crate_name.map(str::to_string)
    };
    files.sort();
    subdirs.sort();
    for f in files {
        out.push((format!("{dir}/{f}"), here_crate.clone()));
    }
    for d in subdirs {
        if !rs_files_with_crate(&format!("{dir}/{d}"), here_crate.as_deref(), out) {
            return false;
        }
    }
    true
}

/// Discover every `CapHandle` slot declaration under `crates_dir`. `images` is the list of
/// spawnable `ImageId` names. `crates_dir` is a parameter (not the `CRATES_DIR` constant)
/// solely so this crate's own tests can point it at an absolute path.
fn discover_slots(crates_dir: &str, images: &[String]) -> Option<Discovery> {
    let mut files = Vec::new();
    if !rs_files_with_crate(crates_dir, None, &mut files) {
        return None;
    }
    let mut out = Discovery::default();
    for (path, krate) in files {
        let src = read_file(NAME, &path)?;
        for (const_name, slot) in find_cap_handle_consts(&src) {
            let Some(krate) = krate.as_deref() else {
                out.unattributed.push(format!(
                    "`{const_name}` = CapHandle({slot}) in {path}, which is under no crate"
                ));
                continue;
            };
            if EXCLUDED_SERVICE_DIRS.contains(&krate) {
                continue;
            }
            let image = image_name_for(krate);
            if !images.contains(&image) {
                out.unattributed.push(format!(
                    "`{const_name}` = CapHandle({slot}) in {path}: crate `{krate}` is not a \
                     spawnable image (no `ImageId::{image}`), so no install can be matched to it"
                ));
                continue;
            }
            out.slots.push(DiscoveredSlot {
                service_dir: krate.to_string(),
                const_name,
                image,
                slot,
                file: path.clone(),
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

/// How one discovered slot is accounted for.
#[derive(Debug, PartialEq, Eq)]
enum Disposition {
    Installed(&'static str),
    Waived,
    WaivedButInstalled,
    Defect,
}

/// The judgement of every discovered slot, with the lines to print. Pure over its inputs.
#[derive(Debug, Default)]
struct Verdict {
    installed: usize,
    waived: usize,
    /// One line per slot: what was found, so the population is visible and not just counted.
    population: Vec<String>,
    /// Lines for `eprintln!` -- the declarations that are neither installed nor waived.
    failures: Vec<String>,
    notes: Vec<String>,
}

/// Judge `slots` against `spawn.rs` (and `fjell-abi`'s object constants). `Err` if a table
/// this needs cannot be read -- never a silent pass.
fn judge_slots(
    slots: &[DiscoveredSlot],
    spawn_src: &str,
    abi_service: &str,
) -> Result<Verdict, String> {
    let ep_images = crate::object_zero::ep_obj_installs(spawn_src, abi_service)?;
    let mmio = crate::object_zero::mmio_slots_by_image(spawn_src)?;
    let mut v = Verdict::default();
    for decl in slots {
        let waiver = WAIVERS
            .iter()
            .find(|w| w.image == decl.image && w.slot == decl.slot);
        let how: Option<&'static str> =
            if image_block_installs_slot(spawn_src, &decl.image, decl.slot) {
                Some("`if image_id ==` block")
            } else if decl.slot == 0 && ep_images.iter().any(|i| *i == decl.image) {
                Some("`ep_obj` arm")
            } else if mmio
                .iter()
                .any(|(i, slots)| *i == decl.image && slots.contains(&decl.slot))
            {
                Some("MMIO `<base> + region_idx`")
            } else {
                None
            };
        let disp = match (waiver, how) {
            (None, Some(h)) => Disposition::Installed(h),
            (None, None) => Disposition::Defect,
            (Some(_), Some(_)) => Disposition::WaivedButInstalled,
            (Some(_), None) => Disposition::Waived,
        };
        let line = format!(
            "{} slot {} `{}` ({}): {}",
            decl.image,
            decl.slot,
            decl.const_name,
            decl.file,
            match &disp {
                Disposition::Installed(h) => format!("installed ({h})"),
                Disposition::Waived => "waived".to_string(),
                Disposition::WaivedButInstalled => "waived AND installed".to_string(),
                Disposition::Defect =>
                    "DEFECT -- declared, installed nowhere, not waived".to_string(),
            }
        );
        v.population.push(line);
        match (waiver, disp) {
            (_, Disposition::Installed(_)) => v.installed += 1,
            (_, Disposition::Defect) => v.failures.push(format!(
                "{} slot {} (`{}`, {}) is declared but spawn.rs installs nothing there, \
                 and no waiver names it",
                decl.image, decl.slot, decl.const_name, decl.service_dir
            )),
            (Some(w), Disposition::WaivedButInstalled) => {
                v.waived += 1;
                // A waived slot that turns out to be installed is not a failure -- it
                // means the next line already landed -- but it is worth a plain line.
                v.notes.push(format!(
                    "note — {} slot {} ({}) is installed; waiver ({}) can be retired",
                    decl.image, decl.slot, w.purpose, w.reason
                ));
            }
            (Some(w), Disposition::Waived) => {
                v.waived += 1;
                v.notes.push(format!(
                    "note — {} slot {} ({}) is declared, spawn.rs installs nothing there, \
                     and that is accounted for: {}",
                    decl.image, decl.slot, w.purpose, w.reason
                ));
            }
            (None, _) => {}
        }
    }
    Ok(v)
}

pub fn check() -> ExitCode {
    let Some(spawn_src) = read_file(NAME, SPAWN_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(abi_service) = read_file(NAME, ABI_SERVICE_PATH) else {
        return ExitCode::FAILURE;
    };
    let images = image_ids(&abi_service);
    if images.is_empty() {
        eprintln!(
            "{NAME}: FAIL — {ABI_SERVICE_PATH} declares no `ImageId`; nothing can be attributed"
        );
        return ExitCode::FAILURE;
    }
    let Some(discovery) = discover_slots(CRATES_DIR, &images) else {
        return ExitCode::FAILURE;
    };
    let discovered = &discovery.slots;

    let mut ok = true;
    for u in &discovery.unattributed {
        ok = false;
        eprintln!("{NAME}: FAIL — {u}");
    }
    let (installed, waived) = match judge_slots(discovered, &spawn_src, &abi_service) {
        Err(why) => {
            eprintln!("{NAME}: FAIL — {why}");
            return ExitCode::FAILURE;
        }
        Ok(v) => {
            if std::env::var_os("CAPABILITY_TOPOLOGY_POPULATION").is_some() {
                for l in &v.population {
                    println!("{NAME}: population — {l}");
                }
            }
            for n in &v.notes {
                println!("{NAME}: {n}");
            }
            for f in &v.failures {
                ok = false;
                eprintln!("{NAME}: FAIL — {f}");
            }
            (v.installed, v.waived)
        }
    };

    // RFC-0.41-002 R5: slot 0 is now judged per image above (RFC-0.43-002); this is the
    // separate, tree-wide invariant that made it dangerous --
    // an endpoint object more than one task could receive on, object 0 -- is judged
    // here, from the installs (`object_zero`).
    let abi_src = read_file(NAME, "crates/fjell-abi/src/service.rs");
    let main_src = read_file(NAME, "crates/fjell-kernel/src/main.rs");
    let rights_src = read_file(NAME, "crates/fjell-cap/src/rights.rs");
    let object_zero = match (abi_src, main_src, rights_src) {
        (Some(abi), Some(main), Some(rights)) => {
            Some(crate::object_zero::judge(&spawn_src, &main, &abi, &rights))
        }
        _ => None,
    };
    let zero_note = match object_zero {
        None => {
            ok = false;
            String::new()
        }
        Some(Ok(r)) => format!(
            "; object 0: no capability to receive on it among {} dedicated arms and {} other \
             endpoint installs, and the default arm grants none",
            r.arms, r.installs
        ),
        Some(Err(why)) => {
            ok = false;
            for w in why {
                eprintln!("{NAME}: FAIL — object 0 — {w}");
            }
            String::new()
        }
    };

    if ok {
        println!(
            "{NAME}: PASS ({installed} installed, {waived} waived to a named line, \
             {} checked{zero_note})",
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
        assert_eq!(
            image_name_for("fjell-secure-transportd"),
            "SECURE_TRANSPORTD"
        );
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

    /// A fixture workspace: `crates/<group>/<crate>/{Cargo.toml,src/...}`.
    fn fixture(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fjell-cap-topology-{tag}-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&root);
        for (rel, body) in files {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).expect("fixture dir");
            std::fs::write(&p, body).expect("fixture file");
        }
        root
    }

    fn imgs(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn discovery_reads_every_rs_file_under_a_crate_not_only_main() {
        // An architect probe on 2026-10-14 planted `CapHandle(9)` in
        // `fjell-secure-transportd`'s `channel.rs` and this check passed with it there,
        // because discovery read `main.rs` alone. Fixtured so the walk cannot narrow again.
        let root = fixture(
            "walk",
            &[
                ("crates/services/fjell-probe-svc/Cargo.toml", ""),
                (
                    "crates/services/fjell-probe-svc/src/main.rs",
                    "const CAP_A: CapHandle = CapHandle(1);\n",
                ),
                (
                    "crates/services/fjell-probe-svc/src/channel.rs",
                    "pub const CAP_B: fjell_cap::CapHandle = fjell_cap::CapHandle(9);\n",
                ),
                (
                    "crates/services/fjell-probe-svc/src/inner/deep.rs",
                    "pub const CAP_C: CapHandle = CapHandle(12);\n",
                ),
            ],
        );
        let d = discover_slots(
            &root.join("crates").to_string_lossy(),
            &imgs(&["PROBE_SVC"]),
        )
        .expect("discovery");
        std::fs::remove_dir_all(&root).ok();
        let mut slots: Vec<u32> = d.slots.iter().map(|x| x.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![1, 9, 12], "{d:#?}");
        assert!(d.unattributed.is_empty());
    }

    /// RFC-0.43-002 R1: the universe is every crate under `crates/`, not a directory name.
    /// A crate laid out somewhere nobody listed -- a new group, a nested one -- is seen.
    #[test]
    fn discovery_reaches_a_declaration_wherever_it_is_under_crates() {
        let root = fixture(
            "anywhere",
            &[
                ("crates/services/fjell-a/Cargo.toml", ""),
                (
                    "crates/services/fjell-a/src/main.rs",
                    "const X_A: CapHandle = CapHandle(1);\n",
                ),
                ("crates/drivers/fjell-b/Cargo.toml", ""),
                (
                    "crates/drivers/fjell-b/src/main.rs",
                    "const X_B: CapHandle = CapHandle(2);\n",
                ),
                // a group that did not exist when this check was written
                ("crates/brand-new-group/nested/fjell-c/Cargo.toml", ""),
                (
                    "crates/brand-new-group/nested/fjell-c/src/lib.rs",
                    "const X_C: CapHandle = CapHandle(3);\n",
                ),
            ],
        );
        let d = discover_slots(
            &root.join("crates").to_string_lossy(),
            &imgs(&["A", "B", "C"]),
        )
        .expect("discovery");
        std::fs::remove_dir_all(&root).ok();
        let mut got: Vec<(String, u32, String)> = d
            .slots
            .iter()
            .map(|x| (x.image.clone(), x.slot, x.service_dir.clone()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                ("A".to_string(), 1, "fjell-a".to_string()),
                ("B".to_string(), 2, "fjell-b".to_string()),
                ("C".to_string(), 3, "fjell-c".to_string()),
            ]
        );
    }

    /// A declaration in a crate that is not a spawnable image is reported, not skipped.
    #[test]
    fn a_declaration_in_a_non_image_crate_is_unattributed_not_ignored() {
        let root = fixture(
            "unattributed",
            &[
                ("crates/libs/fjell-not-an-image/Cargo.toml", ""),
                (
                    "crates/libs/fjell-not-an-image/src/lib.rs",
                    "const Y: CapHandle = CapHandle(4);\n",
                ),
            ],
        );
        let d = discover_slots(
            &root.join("crates").to_string_lossy(),
            &imgs(&["SOMETHING_ELSE"]),
        )
        .expect("discovery");
        std::fs::remove_dir_all(&root).ok();
        assert!(d.slots.is_empty());
        assert_eq!(d.unattributed.len(), 1, "{d:#?}");
        assert!(d.unattributed[0].contains("not a spawnable image"));
    }

    #[test]
    fn fjell_init_stays_excluded() {
        let root = fixture(
            "init",
            &[
                ("crates/services/fjell-init/Cargo.toml", ""),
                (
                    "crates/services/fjell-init/src/main.rs",
                    "        const UART_RX_EP: CapHandle = CapHandle(8);\n",
                ),
            ],
        );
        let d = discover_slots(&root.join("crates").to_string_lossy(), &imgs(&["INIT"]))
            .expect("discovery");
        std::fs::remove_dir_all(&root).ok();
        assert!(d.slots.is_empty() && d.unattributed.is_empty(), "{d:#?}");
    }

    #[test]
    fn image_ids_reads_the_abi_list() {
        let abi = "    pub const STORAGED: ImageId = ImageId(0x01);\n\
                   pub const DRIVER_UART: ImageId = ImageId(0x1b); // c\n\
                   pub const NETD_EP_OBJECT: u32 = 3;\n\
                   pub const SERVICE_READY_SEND_SLOT: u32 = 20;\n";
        assert_eq!(image_ids(abi), vec!["STORAGED", "DRIVER_UART"]);
    }

    const SPAWN: &str = "\
let ep_obj: Option<u32> = match image_id {\n\
    fjell_abi::service::ImageId::STORAGED => Some(1),\n\
    fjell_abi::service::ImageId::A | fjell_abi::service::ImageId::B => Some(2),\n\
    fjell_abi::service::ImageId::EXPLICIT_NONE => None,\n\
    fjell_abi::service::ImageId::BLOCKY => {\n        Some(3)\n    }\n\
    _ => None,\n};\n\
let mmio_regions_for_service: Option<&[usize]> = match image_id {\n\
    fjell_abi::service::ImageId::DRIVER_UART => {\n        static UART0: &[usize] = &[1];\n        Some(UART0)\n    }\n\
    fjell_abi::service::ImageId::MANY => {\n        static ALL: &[usize] = &[0, 3, 4];\n        Some(ALL)\n    }\n\
    _ => None,\n};\n\
if let Some(regions) = mmio_regions_for_service {\n    for &region_idx in regions {\n        let _ = cs.install_raw(\n            31 + region_idx,\n            Capability {},\n        );\n    }\n}\n";

    /// R2: an image with a `Some` arm has slot 0; one with none, or an explicit `None`, does not.
    #[test]
    fn ep_obj_arms_are_attributed_to_images() {
        let got = crate::object_zero::ep_obj_installs(SPAWN, "").expect("attributable");
        assert_eq!(got, vec!["STORAGED", "A", "B", "BLOCKY"]);
        assert!(!got.contains(&"EXPLICIT_NONE".to_string()));
        assert!(!got.contains(&"NEVER_MENTIONED".to_string()));
    }

    /// Fail closed on a shape it cannot attribute -- and name it.
    #[test]
    fn ep_obj_shapes_it_cannot_attribute_fail_closed() {
        let guard =
            "let ep_obj: Option<u32> = match image_id {\n X if cond => Some(1),\n _ => None,\n};";
        assert!(
            crate::object_zero::ep_obj_installs(guard, "")
                .unwrap_err()
                .contains("guard or binding")
        );
        let notimg =
            "let ep_obj: Option<u32> = match image_id {\n some_var => Some(1),\n _ => None,\n};";
        assert!(
            crate::object_zero::ep_obj_installs(notimg, "")
                .unwrap_err()
                .contains("not `ImageId::<NAME>`")
        );
        let dflt = "let ep_obj: Option<u32> = match image_id {\n _ => Some(0),\n};";
        assert!(
            crate::object_zero::ep_obj_installs(dflt, "")
                .unwrap_err()
                .contains("default arm")
        );
        assert!(
            crate::object_zero::ep_obj_installs("fn nothing() {}", "")
                .unwrap_err()
                .contains("no `let ep_obj")
        );
    }

    /// MMIO slots are `<base> + region`, the base read from the install, not restated.
    #[test]
    fn mmio_slots_are_attributed_with_the_base_read_from_the_install() {
        let got = crate::object_zero::mmio_slots_by_image(SPAWN).expect("attributable");
        assert_eq!(
            got,
            vec![
                ("DRIVER_UART".to_string(), vec![32]),
                ("MANY".to_string(), vec![31, 34, 35]),
            ]
        );
        // A different base is followed, not assumed.
        let rebased = SPAWN.replace("31 + region_idx", "40 + region_idx");
        let got = crate::object_zero::mmio_slots_by_image(&rebased).unwrap();
        assert_eq!(got[0].1, vec![41]);
        // Formula not recognised -> fail closed.
        let odd = SPAWN.replace("31 + region_idx", "region_idx * 2");
        assert!(
            crate::object_zero::mmio_slots_by_image(&odd)
                .unwrap_err()
                .contains("formula not recognised")
        );
    }

    fn slot(image: &str, slot: u32) -> DiscoveredSlot {
        DiscoveredSlot {
            service_dir: format!("fjell-{}", image.to_lowercase()),
            const_name: "C".into(),
            image: image.into(),
            slot,
            file: "f.rs".into(),
        }
    }

    /// The control on the new axis: declared slot 0, image with no arm -> a defect, not a skip.
    #[test]
    fn slot_zero_is_judged_not_excluded() {
        let slots = [
            slot("STORAGED", 0),
            slot("NO_ARM_IMAGE", 0),
            slot("EXPLICIT_NONE", 0),
        ];
        let v = judge_slots(&slots, SPAWN, "").expect("judges");
        assert_eq!(v.installed, 1);
        assert_eq!(v.failures.len(), 2, "{v:#?}");
        assert!(v.failures.iter().any(|f| f.contains("NO_ARM_IMAGE slot 0")));
        assert!(
            v.failures
                .iter()
                .any(|f| f.contains("EXPLICIT_NONE slot 0"))
        );
    }

    #[test]
    fn mmio_declarations_are_installed_not_defects() {
        let slots = [slot("DRIVER_UART", 32), slot("MANY", 34), slot("MANY", 33)];
        let v = judge_slots(&slots, SPAWN, "").expect("judges");
        assert_eq!(v.installed, 2, "{v:#?}");
        assert_eq!(v.failures.len(), 1);
        assert!(v.failures[0].contains("MANY slot 33"));
    }

    /// A table it cannot read is an `Err`, never a pass.
    #[test]
    fn unreadable_tables_are_an_error_not_a_pass() {
        assert!(judge_slots(&[slot("X", 0)], "fn nothing() {}", "").is_err());
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
    fn discovery_on_the_current_tree_finds_the_whole_population() {
        // `cargo test` runs with this crate's own directory as the working directory, not
        // the workspace root `check()` assumes in production -- pass the computed root's
        // crates dir in, rather than `set_current_dir` (which races every other test).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("tools/fjell-consistency-check is two levels under the workspace root");
        let abi = std::fs::read_to_string(root.join(ABI_SERVICE_PATH)).expect("service.rs");
        let d = discover_slots(&root.join(CRATES_DIR).to_string_lossy(), &image_ids(&abi))
            .expect("discovery must succeed on the real tree");
        assert!(d.unattributed.is_empty(), "{:#?}", d.unattributed);
        // Nineteen `const .. CapHandle(<slot>)` declarations existed under `crates/` when
        // RFC-0.43-002 widened the scan: ten in services, nine in drivers -- nine the old
        // scan never saw. Six of the nine (`fjell-driver-virtio-net`'s) were then deleted
        // with the paths that used them (RFC-0.43-003, E-118: the image never had those
        // capabilities and exited at the first one it used). `fjell-init`'s one is excluded
        // by design. That leaves twelve judged: nine in services, three in
        // `fjell-driver-uart`. If a crate adds or loses a declaration this number moves, and
        // the change is made on purpose. What must stay true is that the driver crates are
        // *in the universe*: planting a constant there is caught (the RFC-0.43-002 control).
        assert_eq!(d.slots.len(), 12, "{:#?}", d.slots);
        let drivers = d
            .slots
            .iter()
            .filter(|s| s.file.contains("/drivers/"))
            .count();
        assert_eq!(
            drivers, 3,
            "the driver crates' declarations must be in the universe"
        );
        assert!(
            d.slots.iter().any(|s| s.slot == 0),
            "slot 0 must be discovered, not skipped"
        );
    }
}
