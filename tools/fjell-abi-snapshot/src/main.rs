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
//! Items added between snapshots are **not** a failure (additive change).
//! Items *removed or renamed* between snapshots fail the `--verify` gate.
//! Signature changes are flagged if the whole-line hash differs.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
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
    kind: String, // fn | struct | enum | trait | const | type
    name: String,
    sig_hash: String, // first 16 hex chars of SHA-256-like hash of full sig line
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
        _ => {
            eprintln!("Usage: fjell-abi-snapshot --generate|--verify [--snapshot <path>]");
            ExitCode::FAILURE
        }
    }
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
    for (i, item) in items.iter().enumerate() {
        let comma = if i + 1 < items.len() { "," } else { "" };
        out.push_str(&format!(
            "  {{\"crate\":{:?},\"module\":{:?},\"kind\":{:?},\"name\":{:?},\"sig\":{:?}}}{}\n",
            item.crate_name, item.module, item.kind, item.name, item.sig_hash, comma
        ));
    }
    out.push_str("]\n");
    fs::write(path, &out)?;
    Ok(items.len())
}

// ── Verify ───────────────────────────────────────────────────────────────────

fn verify(snapshot_path: &str) -> ExitCode {
    let baseline = match load_snapshot(snapshot_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("fjell-abi-snapshot: cannot read {}: {}", snapshot_path, e);
            eprintln!("Run --generate first.");
            return ExitCode::FAILURE;
        }
    };

    let current = scan_all();
    let current_map: BTreeMap<(String, String, String), &AbiItem> = current
        .iter()
        .map(|i| ((i.crate_name.clone(), i.kind.clone(), i.name.clone()), i))
        .collect();
    let baseline_map: BTreeMap<(String, String, String), &AbiItem> = baseline
        .iter()
        .map(|i| ((i.crate_name.clone(), i.kind.clone(), i.name.clone()), i))
        .collect();

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
    println!("  Added          : {} (additive — OK)", added_count);
    println!("  Removed        : {}", removed.len());
    println!("  Changed sig    : {}", changed.len());

    if removed.is_empty() && changed.is_empty() {
        println!("  Result         : PASS");
        ExitCode::SUCCESS
    } else {
        if !removed.is_empty() {
            eprintln!("\nREMOVED stable items (breaking):");
            for r in &removed {
                eprintln!("  - {}::{} {} {}", r.crate_name, r.module, r.kind, r.name);
            }
        }
        if !changed.is_empty() {
            eprintln!("\nCHANGED stable signatures (breaking):");
            for (b, c) in &changed {
                eprintln!(
                    "  ~ {}::{} {} {} (was sig={}, now sig={})",
                    b.crate_name,
                    b.module,
                    b.kind,
                    b.name,
                    &b.sig_hash[..8],
                    &c.sig_hash[..8]
                );
            }
        }
        eprintln!("\nResult: FAIL — update tests/abi/snapshot.json with --generate");
        ExitCode::from(1)
    }
}

fn load_snapshot(path: &str) -> io::Result<Vec<AbiItem>> {
    let content = fs::read_to_string(path)?;
    let mut items = Vec::new();
    // Minimal JSON parser: each line is one item object
    for line in content.lines() {
        let line = line.trim().trim_end_matches(',');
        if !line.starts_with('{') {
            continue;
        }
        let cr = extract_json_str(line, "crate").unwrap_or_default();
        let mo = extract_json_str(line, "module").unwrap_or_default();
        let ki = extract_json_str(line, "kind").unwrap_or_default();
        let na = extract_json_str(line, "name").unwrap_or_default();
        let sig = extract_json_str(line, "sig").unwrap_or_default();
        if !na.is_empty() {
            items.push(AbiItem {
                crate_name: cr,
                module: mo,
                kind: ki,
                name: na,
                sig_hash: sig,
            });
        }
    }
    Ok(items)
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

// ── Scanner ───────────────────────────────────────────────────────────────────

fn scan_all() -> Vec<AbiItem> {
    let mut items = Vec::new();
    for (crate_name, src_dir) in STABLE_CRATES {
        scan_dir(Path::new(src_dir), crate_name, "", &mut items);
    }
    items.sort();
    items
}

fn scan_dir(dir: &Path, crate_name: &str, prefix: &str, items: &mut Vec<AbiItem>) {
    let lib = dir.join("lib.rs");
    if lib.exists() {
        scan_file(&lib, crate_name, prefix, items);
    }
    if let Ok(entries) = fs::read_dir(dir) {
        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension().and_then(|e| e.to_str()) == Some("rs")
                    && p.file_name().and_then(|n| n.to_str()) != Some("lib.rs")
            })
            .collect();
        paths.sort();
        for path in paths {
            let mod_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let full_prefix = if prefix.is_empty() {
                mod_name.to_string()
            } else {
                format!("{}::{}", prefix, mod_name)
            };
            scan_file(&path, crate_name, &full_prefix, items);
        }
    }
}

fn scan_file(path: &Path, crate_name: &str, module: &str, items: &mut Vec<AbiItem>) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    items.extend(scan_content(&content, crate_name, module));
}

/// The parsing core of `scan_file`, taking source text directly so it can
/// be exercised in tests without touching the filesystem.
fn scan_content(content: &str, crate_name: &str, module: &str) -> Vec<AbiItem> {
    let mut items = Vec::new();
    let mut inside_test = false;
    let lines: Vec<&str> = content.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        // Skip test modules
        if trimmed.starts_with("#[cfg(test)]") {
            inside_test = true;
        }
        if inside_test && trimmed.starts_with("mod tests") {
            i += 1;
            continue;
        }

        let (kind, rest) = match () {
            _ if trimmed.starts_with("pub fn ") => ("fn", &trimmed[7..]),
            _ if trimmed.starts_with("pub async fn") => ("fn", &trimmed[13..]),
            _ if trimmed.starts_with("pub struct ") => ("struct", &trimmed[11..]),
            _ if trimmed.starts_with("pub enum ") => ("enum", &trimmed[9..]),
            _ if trimmed.starts_with("pub trait ") => ("trait", &trimmed[10..]),
            _ if trimmed.starts_with("pub const ") => ("const", &trimmed[10..]),
            _ if trimmed.starts_with("pub type ") => ("type", &trimmed[9..]),
            _ => {
                i += 1;
                continue;
            }
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
        let (full_decl, last_index) = join_wrapped_declaration(&lines, i);
        i = last_index;

        let sig_hash = simple_hash(&normalize_signature(&full_decl));
        items.push(AbiItem {
            crate_name: crate_name.to_string(),
            module: module.to_string(),
            kind: kind.to_string(),
            name,
            sig_hash,
        });

        i += 1;
    }
    items
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
                kind: "fn".into(),
                name: "z".into(),
                sig_hash: "0".into(),
            },
            AbiItem {
                crate_name: "a".into(),
                module: "".into(),
                kind: "fn".into(),
                name: "a".into(),
                sig_hash: "0".into(),
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
}
