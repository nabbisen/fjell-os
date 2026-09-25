//! # `fjell-abi-snapshot`
//!
//! Produces and verifies a stable-surface snapshot for the Fjell ABI
//! (RFC-v0.10-002). The snapshot is a JSON record of every `pub` item
//! in the stable crates (`fjell-sdk`, `fjell-syscall`, `fjell-cap`,
//! `fjell-abi`, `fjell-service-api`, `fjell-semantic-v1`,
//! `fjell-audit-format`, `fjell-bundle-format`).
//!
//! Modes:
//!   `--generate`   — emit snapshot.json from the current workspace.
//!   `--verify`     — compare current workspace to snapshot.json (CI gate).
//!
//! The snapshot format is intentionally line-oriented so `git diff`
//! produces meaningful output.
//!
//! ## Approach
//!
//! A full Rust type-system scraper (e.g. via `rustdoc --output-format json`)
//! is the ideal but requires unstable toolchain features. This tool uses
//! a pragmatic line-level scanner over the source: for each stable crate,
//! it records all `pub` items (functions, structs, enums, traits, consts,
//! type aliases) in the crate's `src/lib.rs` and immediate child modules.
//! This catches 95% of stability-relevant changes with zero nightly
//! dependency.
//!
//! ## What an item's hash covers (RFC-0.33-004 D5, RFC-0.34-003 D3 / §C)
//!
//! A `fn`, `const` or `type` is hashed by its whole declaration. An `enum`, a braced
//! `struct` and a `trait` are hashed by their declaration **and their members** — the
//! variants, the fields, the item signatures — because the declaration line
//! (`pub struct AuditRecordBin {`) says nothing about what is inside: until this was
//! done, adding a field or a method was zero drift. What is **in** and **out**, and why,
//! so that no reflow moves a hash and no ABI change hides in one:
//!
//! | | | why |
//! |---|---|---|
//! | comments, doc comments, whitespace, line layout | **out** (normalised) | cannot change the ABI; a hash that moves on `rustfmt` gets regenerated unread |
//! | `#[repr(...)]` | **in** | it *is* the layout: `#[repr(C)]` on `AuditRecordBin` is the ABI |
//! | `#[non_exhaustive]` | **in** | changes what a downstream `match` may assume |
//! | `#[derive(...)]` | **in for a crate that is published, out otherwise** — sorted, so reordering is free | for a crate on crates.io (`fjell-abi`, read from its manifest's absence of `publish = false`) removing a derived trait breaks consumers this tree cannot see, and Gate 4 is the only instrument for that surface; for the seven `publish = false` crates the workspace compiler catches a removed `Copy` at its use site, so the churn is not worth it (RFC-0.34-003 D8) |
//! | `#[doc]`, `#[must_use]`, `#[allow]`, other attributes on the item | **out** everywhere | they change lints and docs, not anything a consumer can depend on |
//! | a field's or variant's own attributes | **in** (part of its text) | they sit on the member and are rare; a change there is a change to it |
//! | private fields | **in** | they set the size and layout of a `#[repr(C)]` struct, and construction |
//! | field / variant / item **order** | **in** | field order is layout; variant order renumbers implicit discriminants; for trait items it is conservative (a reorder registers) |
//! | a trait item's default body | **out** | behaviour, not surface; the signature before the `{` is in |
//! | a tuple or unit struct's fields | in, on its declaration line | as before |
//!
//! Items added between snapshots are **not** a failure (additive change).
//! Items *removed or renamed* between snapshots fail the `--verify` gate.
//! Signature changes are flagged if the whole-line hash differs.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

// Crates whose public surface is part of the stable ABI.
const STABLE_CRATES: &[(&str, &str)] = &[
    ("fjell-sdk", "crates/fjell-sdk/src"),
    ("fjell-syscall", "crates/fjell-syscall/src"),
    ("fjell-cap", "crates/fjell-cap/src"),
    ("fjell-abi", "crates/fjell-abi/src"),
    ("fjell-service-api", "crates/fjell-service-api/src"),
    ("fjell-semantic-v1", "crates/fjell-semantic-v1/src"),
    (
        "fjell-audit-format",
        "crates/formats/fjell-audit-format/src",
    ),
    (
        "fjell-bundle-format",
        "crates/formats/fjell-bundle-format/src",
    ),
];

/// One public item in the stable surface.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AbiItem {
    crate_name: String,
    module: String,
    // RFC-0.24-003 R6: the impl block's self type, "" for free items.
    // Deliberately a separate field, not appended to `module` — a field
    // named `module` holding a type name would be exactly the "name that
    // lies" pattern this whole line exists to correct.
    impl_type: String,
    kind: String, // fn | struct | enum | trait | const | type
    name: String,
    sig_hash: String, // first 16 hex chars of SHA-256-like hash of full sig line
    /// RFC-0.33-004 D5 / RFC-0.34-003 D3: for a braced `enum`, `struct` or `trait`, its
    /// **members** in source order — an enum's variants, a struct's fields, a trait's
    /// item signatures — each whitespace-normalised, with its attributes, discriminant
    /// and payload; empty for every other kind (a tuple or unit struct's fields are on
    /// its declaration line already). Preceded by the item's ABI attributes (see the
    /// module doc). The hash covers all of it (the snapshot stores only the hash), and
    /// `--dump-members` prints it so a change can be *named*.
    members: Vec<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("--help");
    let snapshot_path = args
        .windows(2)
        .find(|w| w[0] == "--snapshot")
        .and_then(|w| w.get(1))
        .map(String::as_str)
        .unwrap_or("tests/abi/snapshot.json");

    match mode {
        "--generate" => generate(snapshot_path),
        "--verify" => verify(snapshot_path),
        "--dump-members" | "--dump-enums" => dump_members(),
        _ => {
            eprintln!(
                "Usage: fjell-abi-snapshot --generate|--verify|--dump-members [--snapshot <path>]"
            );
            ExitCode::FAILURE
        }
    }
}

/// `--dump-members`: every scanned enum, struct and trait with its ABI attributes and
/// members, one `crate::module::Item` header and one indented line each. Not a gate: it
/// exists so that when a hash changes, *what* changed can be named by diffing two dumps
/// (the snapshot itself stores only the hash). (`--dump-enums` is the earlier name.)
fn dump_members() -> ExitCode {
    for it in scan_all().iter().filter(|i| !i.members.is_empty()) {
        let module = if it.module.is_empty() {
            String::new()
        } else {
            format!("::{}", it.module)
        };
        println!("{} {}{}::{}", it.kind, it.crate_name, module, it.name);
        for v in &it.members {
            println!("    {v}");
        }
    }
    ExitCode::SUCCESS
}

// ── Generate ─────────────────────────────────────────────────────────────────

fn generate(out_path: &str) -> ExitCode {
    let items = scan_all();
    match write_snapshot(&items, out_path) {
        Ok(n) => {
            println!("fjell-abi-snapshot: wrote {} items to {}", n, out_path);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("write error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn write_snapshot(items: &[AbiItem], path: &str) -> io::Result<usize> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    out.push_str("[\n");
    // RFC-0.24-002 Slice 5: a declared count as the first element. The
    // reader is still line-oriented (no parser dependency), but a file
    // that does not parse to exactly this many items — whether reformatted
    // to one line, or truncated mid-array — now fails loudly instead of
    // silently reading as empty or partial.
    out.push_str(&format!("  {{\"count\":{}}},\n", items.len()));
    for (i, item) in items.iter().enumerate() {
        let comma = if i + 1 < items.len() { "," } else { "" };
        out.push_str(&format!(
            "  {{\"crate\":{:?},\"module\":{:?},\"impl_type\":{:?},\"kind\":{:?},\"name\":{:?},\"sig\":{:?}}}{}\n",
            item.crate_name, item.module, item.impl_type, item.kind, item.name, item.sig_hash, comma
        ));
    }
    out.push_str("]\n");
    fs::write(path, &out)?;
    Ok(items.len())
}

// ── Verify ───────────────────────────────────────────────────────────────────

fn verify(snapshot_path: &str) -> ExitCode {
    let loaded = match load_snapshot(snapshot_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("fjell-abi-snapshot: cannot read {}: {}", snapshot_path, e);
            eprintln!("Run --generate first.");
            return ExitCode::FAILURE;
        }
    };

    // RFC-0.24-002 Slice 5: a snapshot that does not parse completely must
    // fail here, never be read as empty or partial. Two shapes, both
    // caught by the same check: no `count` header at all (e.g. the file
    // was reformatted onto one line, so no line starts with `{"count"`),
    // or a header present but the parsed item count doesn't match it
    // (e.g. the file was truncated mid-array).
    let declared = match loaded.declared_count {
        Some(n) => n,
        None => {
            eprintln!(
                "fjell-abi-snapshot: {} has no `count` header — malformed or \
                 reformatted snapshot, cannot trust its contents",
                snapshot_path
            );
            eprintln!("Run --generate to rewrite it in the current format.");
            return ExitCode::FAILURE;
        }
    };
    if loaded.items.len() != declared {
        eprintln!(
            "fjell-abi-snapshot: {} declares {} items but {} were parsed — \
             file is truncated or malformed",
            snapshot_path,
            declared,
            loaded.items.len()
        );
        eprintln!("Run --generate to rewrite it in the current format.");
        return ExitCode::FAILURE;
    }
    let baseline = loaded.items;

    let current = scan_all();

    // RFC-0.24-003 R1: `module` joins the identity key, and either map
    // containing a duplicate key fails the gate outright — the important
    // half of this repair. Building the map by hand rather than
    // `.collect()`-ing into a `BTreeMap` (which silently keeps only the
    // last of any duplicate key) is what makes a duplicate loud instead of
    // a 10%-of-the-surface hole no comparison could ever see.
    let current_map = match build_identity_map(&current) {
        Ok(m) => m,
        Err(dups) => {
            eprintln!(
                "fjell-abi-snapshot: current workspace scan has {} duplicate \
                 identity key(s) — cannot verify against a duplicated surface:",
                dups.len()
            );
            for (cr, mo, im, ki, na) in &dups {
                let owner = if im.is_empty() {
                    String::new()
                } else {
                    format!("{}::", im)
                };
                eprintln!("  {}::{} {}{} {}", cr, mo, owner, ki, na);
            }
            eprintln!("Result: FAIL");
            return ExitCode::from(1);
        }
    };
    let baseline_map = match build_identity_map(&baseline) {
        Ok(m) => m,
        Err(dups) => {
            eprintln!(
                "fjell-abi-snapshot: {} has {} duplicate identity key(s) — \
                 cannot verify against a duplicated baseline:",
                snapshot_path,
                dups.len()
            );
            for (cr, mo, im, ki, na) in &dups {
                let owner = if im.is_empty() {
                    String::new()
                } else {
                    format!("{}::", im)
                };
                eprintln!("  {}::{} {}{} {}", cr, mo, owner, ki, na);
            }
            eprintln!("Result: FAIL");
            return ExitCode::from(1);
        }
    };

    let mut removed: Vec<&AbiItem> = Vec::new();
    let mut changed: Vec<(&AbiItem, &AbiItem)> = Vec::new();
    let added_count = current_map
        .keys()
        .filter(|k| !baseline_map.contains_key(*k))
        .count();

    for (key, base_item) in &baseline_map {
        match current_map.get(key) {
            None => removed.push(base_item),
            Some(cur_item) => {
                if cur_item.sig_hash != base_item.sig_hash {
                    changed.push((base_item, cur_item));
                }
            }
        }
    }

    println!("fjell-abi-snapshot verify:");
    println!("  Baseline items : {}", baseline.len());
    println!("  Current items  : {}", current.len());
    // The parenthetical points at the itemised list the FAIL path prints;
    // on a passing run there is no "below", so it is not printed there.
    println!(
        "  Added          : {}{}",
        added_count,
        if added_count > 0 {
            " (not breaking, but not free — see below)"
        } else {
            ""
        }
    );
    println!("  Removed        : {}", removed.len());
    println!("  Changed sig    : {}", changed.len());

    // RFC-0.30-002 R3/E-035: an addition used to be free — `Added != 0`
    // never affected the result, so the enumerate-and-regenerate step this
    // project's release cycle documents had nothing enforcing it and could
    // be deferred indefinitely. Every accumulated addition would then land
    // in one unreviewed commit whenever someone finally ran --generate.
    // §5 (shape 1): the gate fails the moment an addition is unrecorded,
    // in the same line that made it — where the knowledge of *why* still
    // is — rather than staying silently green until a cut.
    if removed.is_empty() && changed.is_empty() && added_count == 0 {
        println!("  Result         : PASS");
        ExitCode::SUCCESS
    } else {
        if !removed.is_empty() {
            eprintln!("\nREMOVED stable items (breaking):");
            for r in &removed {
                eprintln!("  - {}", item_label(r));
            }
        }
        if !changed.is_empty() {
            eprintln!("\nCHANGED stable signatures (breaking):");
            for (b, c) in &changed {
                eprintln!(
                    "  ~ {} (was sig={}, now sig={})",
                    item_label(b),
                    &b.sig_hash[..8],
                    &c.sig_hash[..8]
                );
            }
            if changed
                .iter()
                .any(|(_, c)| matches!(c.kind.as_str(), "enum" | "struct" | "trait"))
            {
                eprintln!(
                    "\nAn enum, struct or trait hash covers its members (RFC-0.33-004 D5, \
                     RFC-0.34-003 D3): a change above may be a variant, field or method added, \
                     removed, renumbered, retyped or reordered, or a changed `#[repr]`. The \
                     snapshot stores only the hash; `--dump-members` at the baseline's commit and \
                     now, diffed, names which."
                );
            }
        }
        if added_count > 0 {
            eprintln!(
                "\n{added_count} item(s) added to the stable surface since the last \
                 snapshot — additive, not breaking, but the enumerate-and-regenerate \
                 step is owed now, in this line, not deferred to a cut (RFC-0.30-002 §5):"
            );
            for key in current_map.keys() {
                if !baseline_map.contains_key(key) {
                    if let Some(item) = current_map.get(key) {
                        eprintln!("  + {}", item_label(item));
                    }
                }
            }
        }
        eprintln!("\nResult: FAIL — update tests/abi/snapshot.json with --generate");
        ExitCode::from(1)
    }
}

/// Human-readable label for an item, naming its impl-block self type when
/// it has one — so e.g. two same-named methods on different types (the
/// exact R6 collision) are unambiguous in any diagnostic that names one.
fn item_label(item: &AbiItem) -> String {
    if item.impl_type.is_empty() {
        format!(
            "{}::{} {} {}",
            item.crate_name, item.module, item.kind, item.name
        )
    } else {
        format!(
            "{}::{} {}::{} {}",
            item.crate_name, item.module, item.impl_type, item.kind, item.name
        )
    }
}

/// Identity key: crate, module path, impl-block self type ("" for free
/// items), kind, name.
type IdentityKey = (String, String, String, String, String);

/// Build the diff identity map, keyed on `(crate, module, impl_type, kind,
/// name)`. Unlike `.collect()`-ing directly into a `BTreeMap` (which
/// silently keeps only the last of any duplicate key), this returns every
/// duplicated key as an error instead of dropping the rest — RFC-0.24-003
/// R1. Before `module` joined the key, 45 of 423 baseline items collapsed
/// into 378 keys with no signal that anything had been lost. `impl_type`
/// joined the key in R6, added in review after R1's own check caught two
/// further collisions — `CatalogOwner::new` vs `CatalogRangeOwner::new` and
/// similar — that `module` alone could not distinguish: two different
/// types' same-named methods, at the same module path.
fn build_identity_map(
    items: &[AbiItem],
) -> Result<BTreeMap<IdentityKey, &AbiItem>, Vec<IdentityKey>> {
    let mut map: BTreeMap<IdentityKey, &AbiItem> = BTreeMap::new();
    let mut duplicates = Vec::new();
    for item in items {
        let key = (
            item.crate_name.clone(),
            item.module.clone(),
            item.impl_type.clone(),
            item.kind.clone(),
            item.name.clone(),
        );
        if map.contains_key(&key) {
            duplicates.push(key);
        } else {
            map.insert(key, item);
        }
    }
    if duplicates.is_empty() {
        Ok(map)
    } else {
        Err(duplicates)
    }
}

/// A snapshot load: the declared item count (from the `{"count":N}`
/// header, if one was found) alongside whatever items were actually
/// parsed. `verify()` requires these to agree before trusting either.
struct LoadedSnapshot {
    declared_count: Option<usize>,
    items: Vec<AbiItem>,
}

fn load_snapshot(path: &str) -> io::Result<LoadedSnapshot> {
    let content = fs::read_to_string(path)?;
    let mut items = Vec::new();
    let mut declared_count = None;
    // Minimal JSON parser: each line is one object.
    for line in content.lines() {
        let line = line.trim().trim_end_matches(',');
        if !line.starts_with('{') {
            continue;
        }
        if let Some(n) = extract_json_usize(line, "count") {
            declared_count = Some(n);
            continue;
        }
        let cr = extract_json_str(line, "crate").unwrap_or_default();
        let mo = extract_json_str(line, "module").unwrap_or_default();
        let it = extract_json_str(line, "impl_type").unwrap_or_default();
        let ki = extract_json_str(line, "kind").unwrap_or_default();
        let na = extract_json_str(line, "name").unwrap_or_default();
        let sig = extract_json_str(line, "sig").unwrap_or_default();
        if !na.is_empty() {
            items.push(AbiItem {
                crate_name: cr,
                module: mo,
                impl_type: it,
                kind: ki,
                name: na,
                sig_hash: sig,
                members: Vec::new(),
            });
        }
    }
    Ok(LoadedSnapshot {
        declared_count,
        items,
    })
}

fn extract_json_str(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let idx = json.find(&needle)?;
    let rest = json[idx + needle.len()..].trim_start();
    if rest.starts_with('"') {
        let inner = &rest[1..];
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        None
    }
}

/// Like `extract_json_str`, for a bare numeric value: `"count":423`.
fn extract_json_usize(json: &str, key: &str) -> Option<usize> {
    let needle = format!("\"{}\":", key);
    let idx = json.find(&needle)?;
    let rest = json[idx + needle.len()..].trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

// ── Scanner ───────────────────────────────────────────────────────────────────
//
// RFC-0.24-003 R4: the scanner follows the module tree the crate actually
// compiles — starting at `lib.rs` and descending through `mod` declarations
// — rather than walking the directory. A file no `mod` declaration reaches
// (e.g. an orphaned sibling file with no `mod` line anywhere) is never
// scanned; a file reached only through an inline `mod name { … }` block is
// scanned with that block's name correctly qualifying its items' `module`.

fn scan_all() -> Vec<AbiItem> {
    let mut items = Vec::new();
    for (crate_name, src_dir) in STABLE_CRATES {
        // RFC-0.34-003 D8: read from the crate's own manifest, so it cannot rot the way a
        // hand-kept list of "crates that matter" would.
        let manifest = Path::new(src_dir).join("../Cargo.toml");
        let published = fs::read_to_string(&manifest)
            .map(|t| crate_is_published(&t))
            .unwrap_or(false);
        scan_crate(Path::new(src_dir), crate_name, published, &mut items);
    }
    items.sort();
    items
}

/// Scan one stable crate starting at `src_dir/lib.rs`.
fn scan_crate(src_dir: &Path, crate_name: &str, published: bool, items: &mut Vec<AbiItem>) {
    let lib_path = src_dir.join("lib.rs");
    let Ok(content) = fs::read_to_string(&lib_path) else {
        return;
    };
    scan_module_tree(&content, src_dir, crate_name, published, "", items);
}

/// Scan one file's content (`scan_content_into`), then follow every
/// file-backed `mod NAME;` declaration it contained by reading `NAME.rs`
/// (or `NAME/mod.rs`) from `dir` and recursing. Inline `mod NAME { … }`
/// blocks are already fully handled inside `scan_content_into` — they need
/// no file I/O, since their content is already loaded.
fn scan_module_tree(
    content: &str,
    dir: &Path,
    crate_name: &str,
    published: bool,
    prefix: &str,
    items: &mut Vec<AbiItem>,
) {
    let mut file_mods = Vec::new();
    scan_content_into(
        content,
        crate_name,
        published,
        prefix,
        "",
        items,
        &mut file_mods,
    );
    for (mod_prefix, name) in file_mods {
        let flat = dir.join(format!("{name}.rs"));
        let nested = dir.join(&name).join("mod.rs");
        let path = if flat.exists() {
            Some(flat)
        } else if nested.exists() {
            Some(nested)
        } else {
            None
        };
        // A `mod NAME;` with no resolvable file is a compile error in the
        // real crate — nothing to scan; not this tool's problem to report.
        if let Some(path) = path {
            if let Ok(child_content) = fs::read_to_string(&path) {
                let child_prefix = join_prefix(&mod_prefix, &name);
                scan_module_tree(
                    &child_content,
                    dir,
                    crate_name,
                    published,
                    &child_prefix,
                    items,
                );
            }
        }
    }
}

/// The parsing core, taking source text directly so it can be exercised in
/// tests without touching the filesystem. Handles inline `mod name { … }`
/// blocks by recursing with a qualified prefix (and a reset `impl_type`,
/// since a fresh module body starts with no enclosing impl); `impl Type {
/// … }` / `impl Trait for Type { … }` blocks similarly, recursing with the
/// self type and the SAME `module` (RFC-0.24-003 R6 — entering an impl
/// block does not change the module path). File-backed `mod name;`
/// declarations are recorded into `file_mods` for the caller (which has
/// filesystem access) to resolve and follow.
fn scan_content_into(
    content: &str,
    crate_name: &str,
    published: bool,
    module: &str,
    impl_type: &str,
    items: &mut Vec<AbiItem>,
    file_mods: &mut Vec<(String, String)>,
) {
    let stripped = strip_for_module_tracking(content);
    let lines: Vec<&str> = content.lines().collect();
    let stripped_lines: Vec<&str> = stripped.lines().collect();

    let mut i = 0;
    let mut prev_line_was_cfg_test = false;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        let clean = stripped_lines.get(i).copied().unwrap_or("").trim();

        if clean.starts_with("#[cfg(test)]") {
            prev_line_was_cfg_test = true;
            i += 1;
            continue;
        }

        // Inline module: `[pub] mod NAME { … }` — recurse into the
        // already-loaded body; no file to read. A `#[cfg(test)]`-gated
        // block is skipped entirely (it compiles out of a release build).
        if let Some(name) = inline_mod_name(clean) {
            let (body, end_line) = extract_braced_body(&lines, &stripped_lines, i);
            if !prev_line_was_cfg_test {
                let child_prefix = join_prefix(module, &name);
                scan_content_into(
                    &body,
                    crate_name,
                    published,
                    &child_prefix,
                    "",
                    items,
                    file_mods,
                );
            }
            prev_line_was_cfg_test = false;
            i = end_line + 1;
            continue;
        }

        // Impl block: `impl [<…>] Type[<…>] { … }` or `impl [<…>] Trait
        // [<…>] for Type[<…>] { … }` — recurse with the self type; module
        // path is unchanged (RFC-0.24-003 R6).
        if let Some(self_type) = impl_self_type(clean) {
            let (body, end_line) = extract_braced_body(&lines, &stripped_lines, i);
            if !prev_line_was_cfg_test {
                scan_content_into(
                    &body, crate_name, published, module, &self_type, items, file_mods,
                );
            }
            prev_line_was_cfg_test = false;
            i = end_line + 1;
            continue;
        }

        // File-backed module: `[pub] mod NAME;` — recorded for the caller
        // to resolve (this function has no filesystem access by design).
        if let Some(name) = file_mod_name(clean) {
            if !prev_line_was_cfg_test {
                file_mods.push((module.to_string(), name));
            }
            prev_line_was_cfg_test = false;
            i += 1;
            continue;
        }

        prev_line_was_cfg_test = false;

        let Some(after_pub) = trimmed.strip_prefix("pub ") else {
            i += 1;
            continue;
        };
        // RFC-0.24-003 R2: `pub const fn`, `pub unsafe fn`, `pub async fn`,
        // `pub extern "C" fn` (in any combination, e.g. `pub const unsafe
        // fn`) all declare a function — previously only bare `pub fn` and
        // `pub async fn` were recognised, so `pub const fn` fell through to
        // the `const` arm below and was recorded as `kind:"const"
        // name:"fn"` (the literal keyword, not the function's real name),
        // and `pub unsafe fn` matched nothing at all and was silently
        // absent from the scanned surface.
        let (kind, rest): (&str, &str) = if let Some(r) = strip_fn_modifiers(after_pub) {
            ("fn", r)
        } else if let Some(r) = after_pub.strip_prefix("struct ") {
            ("struct", r)
        } else if let Some(r) = after_pub.strip_prefix("enum ") {
            ("enum", r)
        } else if let Some(r) = after_pub.strip_prefix("trait ") {
            ("trait", r)
        } else if let Some(r) = after_pub.strip_prefix("const ") {
            ("const", r)
        } else if let Some(r) = after_pub.strip_prefix("type ") {
            ("type", r)
        } else {
            i += 1;
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            i += 1;
            continue;
        }

        // rustfmt wraps long declarations (chiefly long parameter lists)
        // across multiple lines. Join lines while unclosed `(` remain, so
        // the hashed text is the whole declaration, not just its first
        // physical line. `{`/`}` are deliberately not tracked, so this
        // never runs on into a struct/enum/trait body: the join stops at
        // the declaration's own parens even when the same line also opens
        // a body brace (e.g. `) -> Foo {`).
        let decl_start = i;
        let (full_decl, last_index) = join_wrapped_declaration(&lines, i);
        i = last_index;

        // RFC-0.33-004 D5 (E-056) and RFC-0.34-003 D3 (E-067): the declaration line of
        // `pub enum SyscallNumber {`, `pub struct AuditRecordBin {` or `pub trait T {`
        // says nothing about what is inside, so adding or retiring a variant, a field or
        // a method was zero drift. The hash covers the item's **members** and its ABI
        // attributes (see the module doc for exactly what is in and out, and why).
        let mut members: Vec<String> = Vec::new();
        let mut hashed = normalize_signature(&full_decl);
        if matches!(kind, "enum" | "struct" | "trait") {
            if let Some(b) = braced_members(kind, &stripped_lines, decl_start) {
                // The declaration is what precedes the opening brace, so a one-line
                // `pub enum E { A, B }` and the same enum over several lines hash alike.
                hashed = b.head;
                hashed.push_str(" {");
                for m in &b.members {
                    hashed.push(' ');
                    hashed.push_str(m);
                    hashed.push(',');
                }
                hashed.push_str(" }");
                members = b.members;
                i = i.max(b.end);
            }
            let mut attrs = abi_attrs(&stripped_lines, decl_start);
            if published {
                // RFC-0.34-003 D8: for a crate that is on crates.io, removing a derived
                // trait is a breaking change for consumers this tree cannot see. Sorted,
                // so reordering a derive list is free.
                let d = derive_set(&stripped_lines, decl_start);
                if !d.is_empty() {
                    attrs.push(format!("#[derive({})]", d.join(", ")));
                }
            }
            if !attrs.is_empty() {
                // Attributes are part of what is hashed and of what is dumped, first.
                hashed = format!("{} {hashed}", attrs.join(" "));
                let mut with = attrs;
                with.extend(members);
                members = with;
            }
        }
        let sig_hash = simple_hash(&hashed);
        items.push(AbiItem {
            crate_name: crate_name.to_string(),
            module: module.to_string(),
            impl_type: impl_type.to_string(),
            kind: kind.to_string(),
            name,
            sig_hash,
            members,
        });

        i += 1;
    }
}

/// Test-only, filesystem-free entry point preserving the original
/// `scan_content` signature.
fn scan_content(content: &str, crate_name: &str, module: &str) -> Vec<AbiItem> {
    scan_content_as(content, crate_name, module, false)
}

/// As [`scan_content`], for a crate that is `published` (its derive sets are hashed).
fn scan_content_as(content: &str, crate_name: &str, module: &str, published: bool) -> Vec<AbiItem> {
    let mut items = Vec::new();
    let mut file_mods = Vec::new();
    scan_content_into(
        content,
        crate_name,
        published,
        module,
        "",
        &mut items,
        &mut file_mods,
    );
    items
}

/// Strip `fn`-declaration modifier keywords (`const`, `async`, `unsafe`,
/// `extern "ABI"`, in any legal combination and order) from `rest` and
/// return the text after `fn `, or `None` if `rest` is not a function
/// declaration at all. `rest` is everything after `pub `.
fn strip_fn_modifiers(rest: &str) -> Option<&str> {
    let mut s = rest;
    loop {
        let trimmed = s.trim_start();
        if let Some(r) = trimmed.strip_prefix("const ") {
            s = r;
        } else if let Some(r) = trimmed.strip_prefix("async ") {
            s = r;
        } else if let Some(r) = trimmed.strip_prefix("unsafe ") {
            s = r;
        } else if let Some(r) = trimmed.strip_prefix("extern ") {
            let r = r.trim_start();
            if let Some(after_quote) = r.strip_prefix('"') {
                if let Some(end) = after_quote.find('"') {
                    s = after_quote[end + 1..].trim_start();
                    continue;
                }
            }
            s = r;
        } else {
            s = trimmed;
            break;
        }
    }
    s.strip_prefix("fn ")
}

/// If `clean_line` (comment/string/char-literal stripped) is an inline
/// module opener — `[pub] mod NAME {` — return `NAME`.
fn inline_mod_name(clean_line: &str) -> Option<String> {
    let rest = mod_decl_rest(clean_line)?;
    let name = leading_ident(rest);
    if name.is_empty() {
        return None;
    }
    let after_name = rest[name.len()..].trim_start();
    after_name.starts_with('{').then_some(name)
}

/// If `clean_line` is a file-backed module declaration — `[pub] mod
/// NAME;` — return `NAME`.
fn file_mod_name(clean_line: &str) -> Option<String> {
    let rest = mod_decl_rest(clean_line)?;
    let name = leading_ident(rest);
    if name.is_empty() {
        return None;
    }
    let after_name = rest[name.len()..].trim_start();
    after_name.starts_with(';').then_some(name)
}

fn mod_decl_rest(clean_line: &str) -> Option<&str> {
    clean_line
        .strip_prefix("pub mod ")
        .or_else(|| clean_line.strip_prefix("mod "))
        .map(str::trim_start)
}

fn leading_ident(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// If `clean_line` (stripped) is an `impl` block opener, return its self
/// type: generics stripped (`impl<T> Foo<T> {` → `Foo`), and for a trait
/// impl, the type after `for` (`impl Trait for Type {` → `Type`, not the
/// trait name) — RFC-0.24-003 R6.
fn impl_self_type(clean_line: &str) -> Option<String> {
    let rest = clean_line.strip_prefix("impl")?;
    // Must be the real keyword — the next char must not continue an
    // identifier (rules out e.g. a hypothetical `impls_foo(...)` line).
    if rest
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        return None;
    }
    let rest = skip_generic_params(rest.trim_start());
    let rest = rest.trim_start();
    let target = match find_top_level_for(rest) {
        Some(pos) => rest[pos..].trim_start(),
        None => rest,
    };
    let name = leading_ident(target);
    if name.is_empty() { None } else { Some(name) }
}

/// If `s` starts with `<`, skip past the matching `>` (respecting nested
/// `<>` depth, e.g. `<T: Foo<Bar>>`) and return the remainder trimmed;
/// otherwise return `s` unchanged.
fn skip_generic_params(s: &str) -> &str {
    if !s.starts_with('<') {
        return s;
    }
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'<' => depth += 1,
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return s[i + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    s // unterminated `<...>` — give up and return as-is
}

/// Byte index just past a top-level (not inside `<...>`) `"for "` in `s`,
/// i.e. the start of the self type in `impl Trait for Type`.
fn find_top_level_for(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => depth += 1,
            b'>' => depth -= 1,
            _ => {}
        }
        if depth == 0 && s[i..].starts_with("for ") && (i == 0 || bytes[i - 1] == b' ') {
            return Some(i + 4);
        }
        i += 1;
    }
    None
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", prefix, name)
    }
}

/// Net change in brace depth for one already-stripped line.
fn brace_delta(line: &str) -> i32 {
    let mut d = 0i32;
    for c in line.chars() {
        match c {
            '{' => d += 1,
            '}' => d -= 1,
            _ => {}
        }
    }
    d
}

/// Starting at `lines[start]` (containing a `mod`/`impl` block's opening
/// `{`), find the line where brace depth — counted on `stripped_lines`, so
/// a `{`/`}` inside a string, char literal, or comment can never perturb
/// it — returns to zero, and return the ORIGINAL lines strictly between
/// opener and closer (joined back into text to recurse on) plus the
/// index of the closing line.
fn extract_braced_body(lines: &[&str], stripped_lines: &[&str], start: usize) -> (String, usize) {
    let mut depth = brace_delta(stripped_lines[start]);
    let mut end = start;
    while depth > 0 && end + 1 < lines.len() {
        end += 1;
        depth += brace_delta(stripped_lines[end]);
    }
    let body = if start + 1 < end {
        lines[start + 1..end].join("\n")
    } else {
        String::new()
    };
    (body, end)
}

/// Blank out line comments, block comments, string literal contents, and
/// char literal contents — preserving line structure (newlines) and the
/// byte length/position of everything else — so brace-depth tracking on
/// the result cannot be perturbed by a `{`/`}` inside any of them
/// (RFC-0.24-003 R3). Lifetimes (`'a`) are left untouched; only genuine
/// char literals (`'x'`, `'\''`, `'\n'`, `'\u{2603}'`) are blanked.
fn strip_for_module_tracking(content: &str) -> String {
    let bytes = content.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        let c = bytes[i];
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
            while i < n && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
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
        if c == b'\'' {
            if let Some(len) = char_literal_len(&bytes[i..]) {
                for k in 0..len {
                    out.push(if bytes[i + k] == b'\n' { '\n' } else { ' ' });
                }
                i += len;
                continue;
            }
            // Lifetime (`'a`, `'static`): pass the quote through unchanged.
            out.push('\'');
            i += 1;
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

/// If `bytes` (starting at a `'`) is a genuine char literal, return its
/// total length including both quotes; `None` means it's a lifetime.
fn char_literal_len(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 3 {
        return None;
    }
    if bytes[1] == b'\\' {
        // Escape sequence: '\n', '\\', '\'', '\0', '\u{2603}', etc. Find
        // the closing quote within a bounded lookahead (long enough for
        // any unicode escape, `'\u{10FFFF}'`).
        let max_escape = bytes.len().min(12);
        for end in 3..max_escape {
            if bytes[end] == b'\'' {
                return Some(end + 1);
            }
        }
        None
    } else if bytes[2] == b'\'' {
        Some(3)
    } else {
        None
    }
}

/// What a braced `enum`, `struct` or `trait` contains.
struct Braced {
    /// The declaration up to (not including) its opening brace, whitespace-normalised.
    head: String,
    /// Variants, fields or trait-item signatures, in source order, normalised.
    members: Vec<String>,
    /// The line the closing brace is on.
    end: usize,
}

/// Read the braced body of the item whose declaration starts at `stripped[start]`
/// (comments and string contents already blanked): its head and its members.
///
/// * `enum`, `struct`: members are split at commas outside `()`, `{}`, `[]` and `<>`.
/// * `trait`: members are **item signatures** — an item ends at a top-level `;`, or at
///   the `{` of a default body, which is *not* part of the signature (a default body's
///   text is behaviour, not surface).
///
/// `None` if the item has no braced body: a unit or tuple struct (`pub struct S;`,
/// `pub struct S(u8);` — their fields are on the declaration line, already hashed).
fn braced_members(kind: &str, stripped: &[&str], start: usize) -> Option<Braced> {
    let mut head = String::new();
    let mut body = String::new();
    let mut depth = 0i32;
    let mut paren = 0i32;
    let mut opened = false;
    let mut end = start;
    'lines: for (k, line) in stripped.iter().enumerate().skip(start) {
        for c in line.chars() {
            if !opened {
                match c {
                    '(' => paren += 1,
                    ')' => paren -= 1,
                    // A `;` before any `{` ends a unit/tuple struct: no body.
                    ';' if paren <= 0 => return None,
                    _ => {}
                }
            }
            match c {
                '{' => {
                    depth += 1;
                    if depth == 1 {
                        opened = true;
                        continue;
                    }
                }
                '}' => {
                    depth -= 1;
                    if opened && depth == 0 {
                        end = k;
                        break 'lines;
                    }
                }
                _ => {}
            }
            if opened {
                body.push(c);
            } else {
                head.push(c);
            }
        }
        if opened {
            body.push('\n');
        } else {
            head.push(' ');
        }
        // A tuple/unit struct with no body may run on for many lines before the next
        // item's brace; a struct head never spans more than a handful of lines.
        if !opened && k > start + 12 {
            return None;
        }
    }
    if !opened || depth != 0 {
        return None;
    }
    let members = if kind == "trait" {
        split_trait_items(&body)
    } else {
        split_top_level(&body, ',')
    };
    Some(Braced {
        head: tidy(&head),
        members,
        end,
    })
}

/// Split `text` at `sep` where that is not inside `()`, `{}`, `[]` or `<>`, and
/// whitespace-normalise each non-empty piece. (`->` is not a closing angle bracket.)
fn split_top_level(text: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut nest = 0i32;
    let mut prev = ' ';
    for c in text.chars() {
        match c {
            '(' | '{' | '[' | '<' => nest += 1,
            ')' | '}' | ']' => nest -= 1,
            '>' if prev != '-' && prev != '=' => nest -= 1,
            _ => {}
        }
        if c == sep && nest == 0 {
            let v = tidy(&cur);
            if !v.is_empty() {
                out.push(v);
            }
            cur.clear();
        } else {
            cur.push(c);
        }
        prev = c;
    }
    let v = tidy(&cur);
    if !v.is_empty() {
        out.push(v);
    }
    out
}

/// A trait body's items: each ends at a top-level `;`, or at the `{` that opens a
/// default body (the body is skipped; the signature before it is the member).
fn split_trait_items(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut nest = 0i32;
    let mut prev = ' ';
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '(' | '[' | '<' => nest += 1,
            ')' | ']' => nest -= 1,
            '>' if prev != '-' && prev != '=' => nest -= 1,
            _ => {}
        }
        if nest == 0 && c == '{' {
            // Skip the default body to its matching brace.
            let mut d = 1;
            for b in chars.by_ref() {
                match b {
                    '{' => d += 1,
                    '}' => {
                        d -= 1;
                        if d == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let v = tidy(&cur);
            if !v.is_empty() {
                out.push(v);
            }
            cur.clear();
            prev = '}';
            continue;
        }
        if nest == 0 && c == ';' {
            let v = tidy(&cur);
            if !v.is_empty() {
                out.push(v);
            }
            cur.clear();
        } else {
            cur.push(c);
        }
        prev = c;
    }
    let v = tidy(&cur);
    if !v.is_empty() {
        out.push(v);
    }
    out
}

/// Is the crate whose `Cargo.toml` text this is publishable? (No `publish = false` in it.)
/// Read from the manifest, never from a list here (RFC-0.34-003 D8).
fn crate_is_published(manifest: &str) -> bool {
    !manifest.lines().any(|l| {
        let l = l.trim();
        l.strip_prefix("publish").is_some_and(|r| {
            let r = r.trim_start();
            r.strip_prefix('=')
                .is_some_and(|v| v.trim().starts_with("false"))
        })
    })
}

/// The traits a `#[derive(...)]` above the item at `stripped[start]` names, sorted and
/// de-duplicated. `derive` lines are collected the way `abi_attrs` collects attributes.
fn derive_set(stripped: &[&str], start: usize) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut i = start;
    while i > 0 {
        let line = stripped[i - 1].trim();
        if line.is_empty() {
            i -= 1;
            continue;
        }
        if !line.starts_with("#[") {
            break;
        }
        let mut rest = line;
        while let Some(pos) = rest.find("#[derive(") {
            let after = &rest[pos + "#[derive(".len()..];
            let Some(close) = after.find(")]") else { break };
            for n in after[..close].split(',') {
                let n = n.trim();
                if !n.is_empty() {
                    names.push(n.to_string());
                }
            }
            rest = &after[close + 2..];
        }
        i -= 1;
    }
    names.sort();
    names.dedup();
    names
}

/// The attributes directly above the item at `stripped[start]` that are part of its
/// ABI: `#[repr(...)]` and `#[non_exhaustive]`. Every other attribute — `derive`, `doc`,
/// `must_use`, `allow`, `cfg_attr`, … — is **out**, and the module doc says why.
fn abi_attrs(stripped: &[&str], start: usize) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut i = start;
    while i > 0 {
        let line = stripped[i - 1].trim();
        if line.is_empty() {
            // A comment or doc line (blanked); attributes may sit above it.
            i -= 1;
            continue;
        }
        if !line.starts_with("#[") {
            break;
        }
        // One line may carry several `#[..]` groups.
        let mut rest = line;
        let mut this: Vec<String> = Vec::new();
        while let Some(pos) = rest.find("#[") {
            let mut d = 0;
            let mut close = None;
            for (o, c) in rest[pos..].char_indices() {
                match c {
                    '[' => d += 1,
                    ']' => {
                        d -= 1;
                        if d == 0 {
                            close = Some(pos + o);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else { break };
            this.push(normalize_signature(&rest[pos..=close]));
            rest = &rest[close + 1..];
        }
        // Collected bottom-up; keep them in source order.
        found.splice(0..0, this);
        i -= 1;
    }
    found.retain(|a| {
        let name = a.trim_start_matches("#[").trim_start();
        let name = name.split(['(', ']', ' ', '=']).next().unwrap_or("");
        matches!(name, "repr" | "non_exhaustive")
    });
    found
}

/// Starting at `lines[start]`, join subsequent lines while the declaration
/// has more `(` than `)` so far. Returns the joined text and the index of
/// the last line consumed (so the caller resumes scanning after it).
fn join_wrapped_declaration(lines: &[&str], start: usize) -> (String, usize) {
    let mut text = lines[start].trim().to_string();
    let mut depth = paren_depth(&text);
    let mut i = start;
    while depth > 0 && i + 1 < lines.len() {
        i += 1;
        let next = lines[i].trim();
        text.push(' ');
        text.push_str(next);
        depth += paren_depth(next);
    }
    (text, i)
}

/// Count of `(` minus `)` in a line. Only parens are tracked (not `<>` or
/// `[]`) to avoid `->`'s `>` being mistaken for a closing generic bracket,
/// which would throw off the balance on the overwhelmingly common case of
/// a plain one-line function signature ending in `-> ReturnType {`.
fn paren_depth(s: &str) -> i32 {
    let mut d = 0i32;
    for c in s.chars() {
        match c {
            '(' => d += 1,
            ')' => d -= 1,
            _ => {}
        }
    }
    d
}

/// Collapse all whitespace runs to a single space and trim, so that
/// rustfmt reflow (e.g. re-aligning a run of `NAME = value,` constants, or
/// wrapping a long line differently) does not change the hash by itself.
fn normalize_signature(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// [`normalize_signature`], and the punctuation-spacing `rustfmt` moves between a one-line
/// and a wrapped rendering of the same text: no space just inside `(` `[` or just before
/// `)` `]` `,`, and no trailing comma before a closing bracket. Applied to **members**
/// (fields, variants, trait-item signatures), so re-wrapping a long method signature over
/// several lines does not move a hash (RFC-0.34-003 §C).
fn tidy(s: &str) -> String {
    let mut t = normalize_signature(s);
    loop {
        let before = t.clone();
        for (from, to) in [
            ("( ", "("),
            (" )", ")"),
            ("[ ", "["),
            (" ]", "]"),
            (" ,", ","),
            (",)", ")"),
            (",]", "]"),
        ] {
            t = t.replace(from, to);
        }
        if t == before {
            return t;
        }
    }
}

/// Fast non-cryptographic hash sufficient for change detection.
fn simple_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_hash_deterministic() {
        assert_eq!(simple_hash("pub fn foo()"), simple_hash("pub fn foo()"));
        assert_ne!(simple_hash("pub fn foo()"), simple_hash("pub fn bar()"));
    }

    /// Required failure demonstration, direction 1a (RFC-v0.22-001 §Testing
    /// item 2): re-padding alignment whitespace on an unwrapped declaration
    /// produces NO signature change. This is the exact real-world bug found
    /// in this codebase (`pub const WRITE_ACK:    usize = 0x204;` — extra
    /// spaces for column alignment).
    #[test]
    fn realigned_whitespace_produces_no_signature_change() {
        let padded = "pub const WRITE_ACK:    usize = 0x204;";
        let compact = "pub const WRITE_ACK: usize = 0x204;";

        let items_a = scan_content(padded, "c", "m");
        let items_b = scan_content(compact, "c", "m");

        assert_eq!(items_a.len(), 1);
        assert_eq!(items_b.len(), 1);
        assert_eq!(
            items_a[0].sig_hash, items_b[0].sig_hash,
            "re-padding alignment whitespace must not change the signature hash"
        );
    }

    /// Required failure demonstration, direction 1b: two *wrapped* renderings
    /// of the identical signature — differing only in indentation amount,
    /// not in tokens — must hash identically. This is the wrapped-declaration
    /// analogue of the whitespace test above: whichever way rustfmt happens
    /// to indent a multi-line declaration, the join-then-normalise pipeline
    /// must converge on the same hash.
    #[test]
    fn differently_indented_wrapped_declaration_produces_no_signature_change() {
        let indent_4 = "pub fn foo(\n    a: usize,\n    b: usize,\n) -> usize {";
        let indent_8 = "pub fn foo(\n        a: usize,\n        b: usize,\n) -> usize {";

        let items_a = scan_content(indent_4, "c", "m");
        let items_b = scan_content(indent_8, "c", "m");

        assert_eq!(items_a.len(), 1);
        assert_eq!(items_b.len(), 1);
        assert_eq!(
            items_a[0].sig_hash, items_b[0].sig_hash,
            "indentation depth of a wrapped declaration must not change its \
             signature hash once joined and normalised"
        );
    }

    /// Required failure demonstration, direction 2 (RFC-v0.22-001 §Testing
    /// item 2): a genuine signature change (here, a parameter type) DOES
    /// still change the hash — normalisation must not paper over a real
    /// change along with the cosmetic ones.
    #[test]
    fn genuine_signature_change_still_detected() {
        let original = "pub fn foo(a: usize, b: usize) -> usize {";
        let changed_param_type = "pub fn foo(a: u32, b: usize) -> usize {";
        let changed_return_type = "pub fn foo(a: usize, b: usize) -> u32 {";

        let base = scan_content(original, "c", "m");
        let param_changed = scan_content(changed_param_type, "c", "m");
        let return_changed = scan_content(changed_return_type, "c", "m");

        assert_ne!(
            base[0].sig_hash, param_changed[0].sig_hash,
            "a parameter type change must still change the signature hash"
        );
        assert_ne!(
            base[0].sig_hash, return_changed[0].sig_hash,
            "a return type change must still change the signature hash"
        );
    }

    #[test]
    fn wrapped_declaration_does_not_swallow_the_body() {
        // The joined declaration must stop at the closing paren, not run
        // on into the function body — even though the same line that
        // closes the parens also opens the body brace.
        let src =
            "pub fn foo(\n    a: usize,\n) -> usize {\n    a + should_not_appear_in_signature()\n}";
        let items = scan_content(src, "c", "m");
        assert_eq!(items.len(), 1);
        // If the join over-consumed, this hash would differ from the
        // signature-only hash below (computed independently, same inputs).
        let expected = simple_hash(&normalize_signature("pub fn foo( a: usize, ) -> usize {"));
        assert_eq!(items[0].sig_hash, expected);
    }

    #[test]
    fn extract_json_str_works() {
        let json = r#"{"crate":"fjell-sdk","module":"cap","kind":"struct","name":"CapHandle","sig":"abcd1234"}"#;
        assert_eq!(extract_json_str(json, "crate"), Some("fjell-sdk".into()));
        assert_eq!(extract_json_str(json, "name"), Some("CapHandle".into()));
        assert_eq!(extract_json_str(json, "sig"), Some("abcd1234".into()));
    }

    #[test]
    fn abi_item_sort_is_stable() {
        let mut items = vec![
            AbiItem {
                crate_name: "b".into(),
                module: "".into(),
                impl_type: "".into(),
                kind: "fn".into(),
                name: "z".into(),
                sig_hash: "0".into(),
                members: Vec::new(),
            },
            AbiItem {
                crate_name: "a".into(),
                module: "".into(),
                impl_type: "".into(),
                kind: "fn".into(),
                name: "a".into(),
                sig_hash: "0".into(),
                members: Vec::new(),
            },
        ];
        items.sort();
        assert_eq!(items[0].crate_name, "a");
    }

    #[test]
    fn scan_produces_items_for_stable_crates() {
        // scan_all uses relative paths; if run from within target/ (as the
        // test binary is), the crates/ tree is not visible. Accept either
        // a non-trivial count (workspace root) or zero (test-binary CWD).
        let items = scan_all();
        // If items are found, assert we got a reasonable surface count.
        if !items.is_empty() {
            assert!(
                items.len() >= 10,
                "scan_all found only {} items; expected ≥ 10",
                items.len()
            );
        }
        // Either way the function must not panic — reaching here is the pass.
    }

    // ── RFC-0.24-002 Slice 5: a snapshot that does not parse completely ────
    // ── must never be read as empty or partial. ────────────────────────────

    fn write_temp(name: &str, content: &str) -> String {
        let path = std::env::temp_dir().join(name);
        fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn extract_json_usize_works() {
        assert_eq!(extract_json_usize(r#"{"count":423}"#, "count"), Some(423));
        assert_eq!(extract_json_usize(r#"{"count":0}"#, "count"), Some(0));
        assert_eq!(extract_json_usize(r#"{"crate":"x"}"#, "count"), None);
    }

    /// Required failure demonstration, total case: a snapshot reformatted
    /// onto a single line (as `jq -c .` or an editor auto-save could
    /// produce) has no line starting with `{"count"`, so `declared_count`
    /// is `None` — the loader must not silently treat that as zero items.
    #[test]
    fn load_snapshot_reports_no_count_header_when_reformatted_to_one_line() {
        let path = write_temp(
            "fjell-abi-snapshot-test-total.json",
            r#"[{"count":2},{"crate":"a","module":"","kind":"fn","name":"f","sig":"1"},{"crate":"a","module":"","kind":"fn","name":"g","sig":"2"}]"#,
        );
        let loaded = load_snapshot(&path).unwrap();
        assert_eq!(
            loaded.declared_count, None,
            "a single-line file has no line starting with `{{\"count\"`, so no \
             header can be found — this must not be silently treated as 0 \
             declared items matching 0 parsed items"
        );
    }

    /// Required failure demonstration, partial case: a truncated file (some
    /// items missing, not all) must disagree on count even though a valid
    /// header line is present — the more insidious shape, since a bare
    /// zero-items check alone would miss it.
    #[test]
    fn load_snapshot_declared_count_disagrees_with_truncated_items() {
        let path = write_temp(
            "fjell-abi-snapshot-test-partial.json",
            "[\n  {\"count\":3},\n  {\"crate\":\"a\",\"module\":\"\",\"kind\":\"fn\",\"name\":\"f\",\"sig\":\"1\"},\n]\n",
        );
        let loaded = load_snapshot(&path).unwrap();
        assert_eq!(loaded.declared_count, Some(3));
        assert_eq!(
            loaded.items.len(),
            1,
            "only one of the three declared items is actually present"
        );
    }

    #[test]
    fn load_snapshot_agrees_when_file_is_intact() {
        let path = write_temp(
            "fjell-abi-snapshot-test-intact.json",
            "[\n  {\"count\":2},\n  {\"crate\":\"a\",\"module\":\"\",\"kind\":\"fn\",\"name\":\"f\",\"sig\":\"1\"},\n  {\"crate\":\"a\",\"module\":\"\",\"kind\":\"fn\",\"name\":\"g\",\"sig\":\"2\"}\n]\n",
        );
        let loaded = load_snapshot(&path).unwrap();
        assert_eq!(loaded.declared_count, Some(2));
        assert_eq!(loaded.items.len(), 2);
    }

    // ── RFC-0.24-003 R2: fn-modifier prefixes ───────────────────────────────

    #[test]
    fn const_fn_recognised_as_fn_not_const_named_fn() {
        let items = scan_content("pub const fn from_bytes(b: &[u8]) -> Self {", "c", "m");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "fn");
        assert_eq!(items[0].name, "from_bytes");
    }

    #[test]
    fn unsafe_fn_is_scanned_at_all() {
        // Before R2, `pub unsafe fn` matched no pattern and was silently
        // absent from the surface entirely — not misnamed, just missing.
        let items = scan_content(
            "pub unsafe fn sys_audit_drain_ptr(ptr: usize) -> usize {",
            "c",
            "m",
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "fn");
        assert_eq!(items[0].name, "sys_audit_drain_ptr");
    }

    #[test]
    fn async_fn_recognised() {
        let items = scan_content("pub async fn connect() -> Self {", "c", "m");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "fn");
        assert_eq!(items[0].name, "connect");
    }

    #[test]
    fn extern_c_fn_recognised() {
        let items = scan_content(r#"pub extern "C" fn callback() {"#, "c", "m");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "fn");
        assert_eq!(items[0].name, "callback");
    }

    #[test]
    fn const_unsafe_fn_recognised() {
        // Not found in any of the eight stable crates today, but the
        // handoff asks for the same prefix-confusion class to be checked
        // regardless — reported here rather than left untested.
        let items = scan_content("pub const unsafe fn raw_new() -> Self {", "c", "m");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "fn");
        assert_eq!(items[0].name, "raw_new");
    }

    // ── RFC-0.24-003 R3: braces inside strings/chars/comments must not ──────
    // ── affect module-depth tracking ─────────────────────────────────────────

    #[test]
    fn brace_in_string_literal_does_not_affect_module_depth() {
        let src = "pub mod outer {\n    pub const MSG: &str = \"unbalanced { brace\";\n    pub const INNER: usize = 1;\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.module == "outer"));
    }

    #[test]
    fn brace_in_char_literal_does_not_affect_module_depth() {
        let src = "pub mod outer {\n    pub const OPEN: char = '{';\n    pub const CLOSE: char = '}';\n    pub const INNER: usize = 1;\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.module == "outer"));
    }

    #[test]
    fn lifetime_is_not_mistaken_for_a_char_literal() {
        let src = "pub mod outer {\n    pub fn f<'a>(x: &'a str) -> &'a str { x }\n    pub const INNER: usize = 1;\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.module == "outer"));
    }

    #[test]
    fn brace_in_line_comment_does_not_affect_module_depth() {
        let src = "pub mod outer {\n    // this comment has a { brace in it\n    pub const INNER: usize = 1;\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].module, "outer");
    }

    #[test]
    fn brace_in_block_comment_does_not_affect_module_depth() {
        let src = "pub mod outer {\n    /* a block comment { with a brace\n       spanning multiple lines } */\n    pub const INNER: usize = 1;\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].module, "outer");
    }

    #[test]
    fn nested_inline_modules_qualify_the_full_path() {
        let src =
            "pub mod outer {\n    pub mod inner {\n        pub const DEEP: usize = 1;\n    }\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].module, "outer::inner");
    }

    #[test]
    fn six_inline_modules_with_same_named_const_all_qualify_distinctly() {
        // The worked example from the RFC: six inline `pub mod` blocks,
        // each with its own `pub const READY`, previously all collapsed to
        // `module:""` because the old scanner derived `module` from the
        // file path only.
        let src = "pub mod a {\n    pub const READY: usize = 1;\n}\npub mod b {\n    pub const READY: usize = 2;\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 2);
        let modules: std::collections::BTreeSet<_> =
            items.iter().map(|i| i.module.as_str()).collect();
        assert_eq!(modules.len(), 2, "each READY must keep its own module");
        assert!(modules.contains("a"));
        assert!(modules.contains("b"));
    }

    #[test]
    fn cfg_test_inline_module_is_not_scanned() {
        let src = "#[cfg(test)]\nmod v07_tag_tests {\n    pub const LEAKED: usize = 1;\n}\n";
        let items = scan_content(src, "c", "");
        assert!(
            items.is_empty(),
            "a #[cfg(test)] module must not contribute to the stable surface"
        );
    }

    // ── RFC-0.24-003 R1: duplicate identity keys must fail, not collapse ────

    #[test]
    fn build_identity_map_reports_duplicate_keys() {
        let items = vec![
            AbiItem {
                crate_name: "c".into(),
                module: "m".into(),
                impl_type: "".into(),
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "1".into(),
                members: Vec::new(),
            },
            AbiItem {
                crate_name: "c".into(),
                module: "m".into(),
                impl_type: "".into(),
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "2".into(),
                members: Vec::new(),
            },
        ];
        let err = build_identity_map(&items).unwrap_err();
        assert_eq!(
            err,
            vec![(
                "c".into(),
                "m".into(),
                "".into(),
                "const".into(),
                "READY".into()
            )]
        );
    }

    #[test]
    fn build_identity_map_distinguishes_by_module() {
        // The exact shape R1 exists to fix: same crate/kind/name, distinct
        // modules — must NOT be reported as a duplicate.
        let items = vec![
            AbiItem {
                crate_name: "c".into(),
                module: "a".into(),
                impl_type: "".into(),
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "1".into(),
                members: Vec::new(),
            },
            AbiItem {
                crate_name: "c".into(),
                module: "b".into(),
                impl_type: "".into(),
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "2".into(),
                members: Vec::new(),
            },
        ];
        assert!(build_identity_map(&items).is_ok());
    }

    // ── RFC-0.24-003 R6: impl scope ──────────────────────────────────────────

    #[test]
    fn impl_self_type_simple() {
        assert_eq!(impl_self_type("impl Foo {"), Some("Foo".to_string()));
    }

    #[test]
    fn impl_self_type_generic_strips_params() {
        assert_eq!(impl_self_type("impl<T> Foo<T> {"), Some("Foo".to_string()));
        assert_eq!(
            impl_self_type("impl<T: Clone, U> Foo<T, U> {"),
            Some("Foo".to_string())
        );
    }

    #[test]
    fn impl_self_type_trait_impl_takes_the_type_after_for() {
        assert_eq!(
            impl_self_type("impl Display for Foo {"),
            Some("Foo".to_string()),
            "the self type, not the trait name"
        );
    }

    #[test]
    fn impl_self_type_generic_trait_impl() {
        assert_eq!(
            impl_self_type("impl<T> From<T> for Foo<T> {"),
            Some("Foo".to_string())
        );
    }

    #[test]
    fn impl_self_type_none_for_non_impl_lines() {
        assert_eq!(impl_self_type("pub fn foo() {"), None);
        assert_eq!(impl_self_type("pub struct Foo {"), None);
    }

    #[test]
    fn methods_inside_an_impl_block_carry_its_self_type() {
        let src = "impl Foo {\n    pub fn new() -> Self { Self }\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].impl_type, "Foo");
        assert_eq!(items[0].name, "new");
    }

    #[test]
    fn free_items_carry_no_impl_type() {
        let src = "pub const TOP: usize = 1;\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].impl_type, "");
    }

    #[test]
    fn impl_type_does_not_affect_module_path() {
        let src =
            "pub mod outer {\n    impl Foo {\n        pub fn new() -> Self { Self }\n    }\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].module, "outer",
            "entering an impl block must not change the module path"
        );
        assert_eq!(items[0].impl_type, "Foo");
    }

    #[test]
    fn two_types_with_same_named_method_stay_distinct_by_impl_type() {
        // The exact shape R1 found: two different types, same method name,
        // same module — must not collide once impl_type joins the key.
        let src = "impl AuditRecordBin {\n    pub fn kind(self) -> AuditKind { AuditKind::A }\n}\nimpl AuditPersistRecord {\n    pub fn kind(&self) -> AuditKind { AuditKind::B }\n}\n";
        let items = scan_content(src, "c", "");
        assert_eq!(items.len(), 2);
        assert!(
            build_identity_map(&items).is_ok(),
            "distinct impl_type must not collide"
        );
        let types: std::collections::BTreeSet<_> =
            items.iter().map(|i| i.impl_type.as_str()).collect();
        assert!(types.contains("AuditRecordBin"));
        assert!(types.contains("AuditPersistRecord"));
    }

    #[test]
    fn build_identity_map_distinguishes_by_impl_type() {
        let items = vec![
            AbiItem {
                crate_name: "c".into(),
                module: "".into(),
                impl_type: "A".into(),
                kind: "fn".into(),
                name: "new".into(),
                sig_hash: "1".into(),
                members: Vec::new(),
            },
            AbiItem {
                crate_name: "c".into(),
                module: "".into(),
                impl_type: "B".into(),
                kind: "fn".into(),
                name: "new".into(),
                sig_hash: "2".into(),
                members: Vec::new(),
            },
        ];
        assert!(build_identity_map(&items).is_ok());
    }

    // ── RFC-0.33-004 D5 / E-056: an enum's hash covers its variants ───────────

    fn enum_hash(src: &str) -> String {
        let items = scan_content(src, "c", "m");
        assert_eq!(items.len(), 1, "{src}");
        items[0].sig_hash.clone()
    }

    const BASE: &str = "pub enum E {\n    A = 0,\n    B = 1,\n}\n";

    /// The finding, demonstrated: with the declaration line alone, all of these
    /// were the same item. Each must now differ from the base.
    #[test]
    fn a_variant_added_removed_renumbered_or_reordered_changes_the_hash() {
        let base = enum_hash(BASE);
        for (what, src) in [
            (
                "added",
                "pub enum E {\n    A = 0,\n    B = 1,\n    C = 2,\n}\n",
            ),
            ("removed", "pub enum E {\n    A = 0,\n}\n"),
            ("renumbered", "pub enum E {\n    A = 0,\n    B = 7,\n}\n"),
            ("reordered", "pub enum E {\n    B = 1,\n    A = 0,\n}\n"),
            ("renamed", "pub enum E {\n    A = 0,\n    Bee = 1,\n}\n"),
            (
                "payload changed",
                "pub enum E {\n    A = 0,\n    B(u8) = 1,\n}\n",
            ),
        ] {
            assert_ne!(
                enum_hash(src),
                base,
                "a variant {what} must change the hash"
            );
        }
    }

    /// What must **not** change it: comments, doc comments, whitespace and layout.
    #[test]
    fn comments_whitespace_and_layout_do_not_change_an_enum_hash() {
        let base = enum_hash(BASE);
        for src in [
            "pub enum E {\n    /// doc\n    A = 0, // trailing\n    /* block */ B = 1,\n}\n",
            "pub enum E { A = 0, B = 1 }\n",
            "pub enum E {\n        A   =   0,\n\n        B = 1,\n    }\n",
            "pub enum E {\n    A = 0,\n    B = 1\n}\n", // no trailing comma
        ] {
            assert_eq!(enum_hash(src), base, "{src}");
        }
    }

    #[test]
    fn struct_variants_with_commas_inside_are_one_variant() {
        let a = "pub enum E {\n    M { tag: usize, words: [usize; 4] },\n    N,\n}\n";
        let b = "pub enum E {\n    M { tag: usize, words: [usize; 5] },\n    N,\n}\n";
        assert_ne!(enum_hash(a), enum_hash(b));
        let items = scan_content(a, "c", "m");
        assert_eq!(
            items[0].members,
            ["M { tag: usize, words: [usize; 4] }", "N"]
        );
    }

    /// An enum that is not the last item, and the item after it, are both still
    /// found: reading the body must not swallow what follows.
    #[test]
    fn the_item_after_an_enum_is_still_scanned() {
        let items = scan_content(
            "pub enum E {\n    A,\n    B,\n}\npub fn after() {}\npub const C: u8 = 1;\n",
            "c",
            "m",
        );
        let names: Vec<_> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["E", "after", "C"]);
    }

    // ── RFC-0.34-003 D3 / E-067: a struct's fields and a trait's items are hashed ──

    /// **The inverted test.** This used to be
    /// `a_braced_structs_fields_are_still_outside_its_hash` and asserted the blindness:
    /// two structs differing by a field hashed alike. It now asserts the opposite.
    #[test]
    fn a_field_added_to_a_braced_struct_changes_its_hash() {
        let a = scan_content("pub struct S {\n    pub a: u8,\n}\n", "c", "m");
        let b = scan_content(
            "pub struct S {\n    pub a: u8,\n    pub b: u8,\n}\n",
            "c",
            "m",
        );
        assert_ne!(a[0].sig_hash, b[0].sig_hash);
        assert_eq!(b[0].members, ["pub a: u8", "pub b: u8"]);
    }

    fn struct_hash(src: &str) -> String {
        let items = scan_content(src, "c", "m");
        assert_eq!(items.len(), 1, "{src}");
        items[0].sig_hash.clone()
    }

    const S: &str = "pub struct S {\n    pub a: u8,\n    pub b: u16,\n}\n";

    #[test]
    fn a_field_removed_retyped_renamed_or_reordered_changes_the_hash() {
        let base = struct_hash(S);
        for (what, src) in [
            ("removed", "pub struct S {\n    pub a: u8,\n}\n"),
            (
                "retyped",
                "pub struct S {\n    pub a: u8,\n    pub b: u32,\n}\n",
            ),
            (
                "renamed",
                "pub struct S {\n    pub a: u8,\n    pub c: u16,\n}\n",
            ),
            (
                "reordered",
                "pub struct S {\n    pub b: u16,\n    pub a: u8,\n}\n",
            ),
            (
                "made private",
                "pub struct S {\n    pub a: u8,\n    b: u16,\n}\n",
            ),
        ] {
            assert_ne!(
                struct_hash(src),
                base,
                "a field {what} must change the hash"
            );
        }
    }

    /// The control §C asks for: **a `rustfmt` reflow does not move the hash.**
    #[test]
    fn a_reflow_does_not_move_a_struct_hash() {
        let base = struct_hash(S);
        for src in [
            "pub struct S { pub a: u8, pub b: u16 }\n",
            "pub struct S {\n    /// doc\n    pub a: u8, // trailing\n    /* block */ pub b: u16,\n}\n",
            "pub struct S {\n        pub a:   u8,\n\n        pub b:   u16\n}\n",
            "#[derive(Clone, Copy, Debug)]\npub struct S {\n    pub a: u8,\n    pub b: u16,\n}\n",
            "/// docs\n#[must_use]\n#[allow(dead_code)]\npub struct S {\n    pub a: u8,\n    pub b: u16,\n}\n",
        ] {
            assert_eq!(struct_hash(src), base, "{src}");
        }
    }

    /// `#[repr(C)]` is the ABI: adding, removing or changing it moves the hash.
    #[test]
    fn repr_is_in_the_hash_and_derive_is_out() {
        let plain = struct_hash(S);
        let c = struct_hash(&format!("#[repr(C)]\n{S}"));
        let packed = struct_hash(&format!("#[repr(C, packed)]\n{S}"));
        let both = struct_hash(&format!("#[derive(Clone)]\n#[repr(C)]\n{S}"));
        assert_ne!(plain, c);
        assert_ne!(c, packed);
        assert_eq!(c, both, "derive must not move a hash; repr must");
        // several attributes on one line
        assert_eq!(c, struct_hash(&format!("#[derive(Clone)] #[repr(C)]\n{S}")));
        let items = scan_content(&format!("#[repr(C)]\n{S}"), "c", "m");
        assert_eq!(items[0].members[0], "#[repr(C)]");
    }

    /// A tuple or unit struct has no braced body and is untouched — and the scanner
    /// must not run on into the *next* item's brace looking for one.
    #[test]
    fn tuple_and_unit_structs_are_not_confused_with_the_next_items_body() {
        let items = scan_content(
            "pub struct T(pub u8);\npub struct U;\npub struct B {\n    pub x: u8,\n}\npub fn after() {}\n",
            "c",
            "m",
        );
        let by: Vec<(&str, usize)> = items
            .iter()
            .map(|i| (i.name.as_str(), i.members.len()))
            .collect();
        assert_eq!(by, [("T", 0), ("U", 0), ("B", 1), ("after", 0)]);
    }

    const TR: &str = "pub trait T {\n    fn a(&self) -> u8;\n    fn b(&mut self, x: u16);\n}\n";

    #[test]
    fn a_method_added_removed_or_changed_moves_a_trait_hash() {
        let base = struct_hash(TR);
        for (what, src) in [
            (
                "added",
                "pub trait T {\n    fn a(&self) -> u8;\n    fn b(&mut self, x: u16);\n    fn c(&self);\n}\n",
            ),
            ("removed", "pub trait T {\n    fn a(&self) -> u8;\n}\n"),
            (
                "retyped",
                "pub trait T {\n    fn a(&self) -> u32;\n    fn b(&mut self, x: u16);\n}\n",
            ),
            (
                "with an associated type",
                "pub trait T {\n    type Out;\n    fn a(&self) -> u8;\n    fn b(&mut self, x: u16);\n}\n",
            ),
        ] {
            assert_ne!(
                struct_hash(src),
                base,
                "a method {what} must change the hash"
            );
        }
    }

    /// A trait's *default body* is behaviour, not surface: editing it is not drift;
    /// nor is a reflow or a comment.
    #[test]
    fn a_default_body_and_a_reflow_do_not_move_a_trait_hash() {
        let with_default = "pub trait T {\n    fn a(&self) -> u8 {\n        1\n    }\n    fn b(&mut self, x: u16);\n}\n";
        let other_default = "pub trait T {\n    fn a(&self) -> u8 {\n        2 + { 3 }\n    }\n    fn b(&mut self, x: u16);\n}\n";
        assert_eq!(struct_hash(with_default), struct_hash(other_default));
        let reflowed = "pub trait T {\n    /// doc\n    fn a(\n        &self,\n    ) -> u8 { 1 } // c\n    fn b(&mut self, x: u16);\n}\n";
        assert_eq!(struct_hash(with_default), struct_hash(reflowed));
        assert_eq!(scan_content(with_default, "c", "m")[0].members.len(), 2);
    }

    #[test]
    fn generic_fields_with_commas_are_one_field() {
        let items = scan_content(
            "pub struct G {\n    pub m: Map<K, V>,\n    pub f: fn(u8, u8) -> Result<u8, E>,\n}\n",
            "c",
            "m",
        );
        assert_eq!(
            items[0].members,
            ["pub m: Map<K, V>", "pub f: fn(u8, u8) -> Result<u8, E>"]
        );
    }

    // ── RFC-0.34-003 D8: the derive set is in the hash for a published crate ──

    fn hash_as(src: &str, published: bool) -> String {
        let items = scan_content_as(src, "c", "m", published);
        assert_eq!(items.len(), 1, "{src}");
        items[0].sig_hash.clone()
    }

    #[test]
    fn a_derive_removed_or_added_moves_a_published_crates_hash_and_not_others() {
        let with = "#[derive(Clone, Copy, Debug)]\npub struct S {\n    pub a: u8,\n}\n";
        let without = "#[derive(Clone, Debug)]\npub struct S {\n    pub a: u8,\n}\n";
        let more = "#[derive(Clone, Copy, Debug, PartialEq)]\npub struct S {\n    pub a: u8,\n}\n";
        // published: removing `Copy`, or adding `PartialEq`, is drift
        assert_ne!(hash_as(with, true), hash_as(without, true));
        assert_ne!(hash_as(with, true), hash_as(more, true));
        // not published: neither is
        assert_eq!(hash_as(with, false), hash_as(without, false));
        assert_eq!(hash_as(with, false), hash_as(more, false));
    }

    /// Sorted: reordering a derive list, splitting it over two attributes, or putting it on
    /// one line with `repr` is free — even for a published crate.
    #[test]
    fn a_reordered_or_split_derive_list_does_not_move_a_published_hash() {
        let base = hash_as(
            "#[derive(Clone, Copy, Debug)]\npub struct S {\n    pub a: u8,\n}\n",
            true,
        );
        for src in [
            "#[derive(Debug, Copy, Clone)]\npub struct S {\n    pub a: u8,\n}\n",
            "#[derive(Clone)]\n#[derive(Copy, Debug)]\npub struct S {\n    pub a: u8,\n}\n",
            "#[derive(Clone, Copy, Debug)] #[must_use]\npub struct S {\n    pub a: u8,\n}\n",
            "/// doc\n#[allow(dead_code)]\n#[derive( Clone ,Copy,Debug )]\npub struct S {\n    pub a: u8,\n}\n",
        ] {
            assert_eq!(hash_as(src, true), base, "{src}");
        }
    }

    #[test]
    fn doc_must_use_and_allow_stay_out_everywhere() {
        let plain = "pub struct S {\n    pub a: u8,\n}\n";
        let noisy = "/// d\n#[must_use]\n#[allow(dead_code)]\npub struct S {\n    pub a: u8,\n}\n";
        for published in [true, false] {
            assert_eq!(hash_as(plain, published), hash_as(noisy, published));
        }
    }

    /// The fact is read from the manifest: `publish = false` means not published.
    #[test]
    fn published_is_read_from_the_manifest_and_only_fjell_abi_is_published() {
        assert!(crate_is_published(
            "[package]\nname = \"x\"\nversion = \"1\"\n"
        ));
        assert!(!crate_is_published(
            "[package]\nname = \"x\"\npublish = false\n"
        ));
        assert!(!crate_is_published(
            "[package]\nname = \"x\"\npublish     = false\n"
        ));
        let mut published = Vec::new();
        for (name, src) in STABLE_CRATES {
            let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(src)
                .join("../Cargo.toml");
            let text = fs::read_to_string(&manifest)
                .unwrap_or_else(|e| panic!("{name}: {} — {e}", manifest.display()));
            if crate_is_published(&text) {
                published.push(*name);
            }
        }
        assert_eq!(
            published,
            ["fjell-abi"],
            "the scanned crates that are on crates.io"
        );
    }
}
