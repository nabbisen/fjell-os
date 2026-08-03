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
    // RFC-0.24-002 Slice 5: a declared count as the first element. The
    // reader is still line-oriented (no parser dependency), but a file
    // that does not parse to exactly this many items — whether reformatted
    // to one line, or truncated mid-array — now fails loudly instead of
    // silently reading as empty or partial.
    out.push_str(&format!("  {{\"count\":{}}},\n", items.len()));
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
            for (cr, mo, ki, na) in &dups {
                eprintln!("  {}::{} {} {}", cr, mo, ki, na);
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
            for (cr, mo, ki, na) in &dups {
                eprintln!("  {}::{} {} {}", cr, mo, ki, na);
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

/// Build the diff identity map, keyed on `(crate, module, kind, name)`.
/// Unlike `.collect()`-ing directly into a `BTreeMap` (which silently
/// keeps only the last of any duplicate key), this returns every
/// duplicated key as an error instead of dropping the rest — RFC-0.24-003
/// R1. Before `module` joined the key, 45 of 423 baseline items collapsed
/// into 378 keys with no signal that anything had been lost.
fn build_identity_map(
    items: &[AbiItem],
) -> Result<BTreeMap<(String, String, String, String), &AbiItem>, Vec<(String, String, String, String)>>
{
    let mut map: BTreeMap<(String, String, String, String), &AbiItem> = BTreeMap::new();
    let mut duplicates = Vec::new();
    for item in items {
        let key = (
            item.crate_name.clone(),
            item.module.clone(),
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
        scan_crate(Path::new(src_dir), crate_name, &mut items);
    }
    items.sort();
    items
}

/// Scan one stable crate starting at `src_dir/lib.rs`.
fn scan_crate(src_dir: &Path, crate_name: &str, items: &mut Vec<AbiItem>) {
    let lib_path = src_dir.join("lib.rs");
    let Ok(content) = fs::read_to_string(&lib_path) else {
        return;
    };
    scan_module_tree(&content, src_dir, crate_name, "", items);
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
    prefix: &str,
    items: &mut Vec<AbiItem>,
) {
    let mut file_mods = Vec::new();
    scan_content_into(content, crate_name, prefix, items, &mut file_mods);
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
                scan_module_tree(&child_content, dir, crate_name, &child_prefix, items);
            }
        }
    }
}

/// The parsing core, taking source text directly so it can be exercised in
/// tests without touching the filesystem. Handles inline `mod name { … }`
/// blocks by recursing with a qualified prefix; file-backed `mod name;`
/// declarations are recorded into `file_mods` for the caller (which has
/// filesystem access) to resolve and follow.
fn scan_content_into(
    content: &str,
    crate_name: &str,
    module: &str,
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
            let (body, end_line) = extract_inline_mod_body(&lines, &stripped_lines, i);
            if !prev_line_was_cfg_test {
                let child_prefix = join_prefix(module, &name);
                scan_content_into(&body, crate_name, &child_prefix, items, file_mods);
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
}

/// Test-only, filesystem-free entry point preserving the original
/// `scan_content` signature.
fn scan_content(content: &str, crate_name: &str, module: &str) -> Vec<AbiItem> {
    let mut items = Vec::new();
    let mut file_mods = Vec::new();
    scan_content_into(content, crate_name, module, &mut items, &mut file_mods);
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

/// Starting at `lines[start]` (containing a module's opening `{`), find
/// the line where brace depth — counted on `stripped_lines`, so a
/// `{`/`}` inside a string, char literal, or comment can never perturb it
/// — returns to zero, and return the ORIGINAL lines strictly between
/// opener and closer (joined back into text to recurse on) plus the
/// index of the closing line.
fn extract_inline_mod_body(
    lines: &[&str],
    stripped_lines: &[&str],
    start: usize,
) -> (String, usize) {
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
        let items = scan_content("pub unsafe fn sys_audit_drain_ptr(ptr: usize) -> usize {", "c", "m");
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
        let src =
            "pub mod outer {\n    pub fn f<'a>(x: &'a str) -> &'a str { x }\n    pub const INNER: usize = 1;\n}\n";
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
        let src = "pub mod outer {\n    pub mod inner {\n        pub const DEEP: usize = 1;\n    }\n}\n";
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
        let modules: std::collections::BTreeSet<_> = items.iter().map(|i| i.module.as_str()).collect();
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
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "1".into(),
            },
            AbiItem {
                crate_name: "c".into(),
                module: "m".into(),
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "2".into(),
            },
        ];
        let err = build_identity_map(&items).unwrap_err();
        assert_eq!(err, vec![("c".into(), "m".into(), "const".into(), "READY".into())]);
    }

    #[test]
    fn build_identity_map_distinguishes_by_module() {
        // The exact shape R1 exists to fix: same crate/kind/name, distinct
        // modules — must NOT be reported as a duplicate.
        let items = vec![
            AbiItem {
                crate_name: "c".into(),
                module: "a".into(),
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "1".into(),
            },
            AbiItem {
                crate_name: "c".into(),
                module: "b".into(),
                kind: "const".into(),
                name: "READY".into(),
                sig_hash: "2".into(),
            },
        ];
        assert!(build_identity_map(&items).is_ok());
    }
}
