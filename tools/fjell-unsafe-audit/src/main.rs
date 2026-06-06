//! `fjell-unsafe-audit` — unsafe boundary inventory scanner (RFC v0.6-004).
//!
//! Usage:
//!   fjell-unsafe-audit [--root <path>] [--json] [--check]
//!
//! Exits 0 if every unsafe site has a `// SAFETY: category=raw-pointer-deref` comment within 4 lines.
//! Exits 1 if any unsafe site is missing a SAFETY comment.
//! With `--json`, prints a JSON-lines inventory to stdout.

use std::{
    env, fs,
    io,
    path::{Path, PathBuf},
    process,
};

/// Known unsafe site categories (RFC-v0.7.5-001 / W-H-05).
///
/// Every unsafe block MUST name its category in the SAFETY comment:
///   `// SAFETY: category=<known-name>`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnsafeCategory {
    RawPointerDeref,
    PageTableMutation,
    CsrAsm,
    MmioAccess,
    PhysIdMapAssumption,
    KernelGlobalMutable,
    UserCopy,
    Unknown,      // category= present but not a recognised name
    Missing,      // no category= tag at all
}

impl UnsafeCategory {
    fn from_str(s: &str) -> Self {
        match s.trim() {
            "raw-pointer-deref"       => Self::RawPointerDeref,
            "page-table-mutation"     => Self::PageTableMutation,
            "csr-asm"                 => Self::CsrAsm,
            "mmio-access"             => Self::MmioAccess,
            "phys-id-map-assumption"  => Self::PhysIdMapAssumption,
            "kernel-global-mutable"   => Self::KernelGlobalMutable,
            "user-copy"               => Self::UserCopy,
            _                         => Self::Unknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::RawPointerDeref      => "raw-pointer-deref",
            Self::PageTableMutation    => "page-table-mutation",
            Self::CsrAsm               => "csr-asm",
            Self::MmioAccess           => "mmio-access",
            Self::PhysIdMapAssumption  => "phys-id-map-assumption",
            Self::KernelGlobalMutable  => "kernel-global-mutable",
            Self::UserCopy             => "user-copy",
            Self::Unknown              => "unknown",
            Self::Missing              => "missing",
        }
    }

    fn is_valid(self) -> bool {
        !matches!(self, Self::Unknown | Self::Missing)
    }
}

#[derive(Debug)]
struct UnsafeRecord {
    file:          String,
    line:          u32,
    kind:          UnsafeKind,
    has_safety:    bool,
    safety_text:   String,
    category:      UnsafeCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnsafeKind {
    Block,
    Fn,
    Impl,
    Trait,
}

impl UnsafeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Fn    => "fn",
            Self::Impl  => "impl",
            Self::Trait => "trait",
        }
    }
}

// ── Scanner ───────────────────────────────────────────────────────────────────

/// RFC 060 — strip string-literal regions and line comments from a single
/// source line so the substring scanner does not flag `"unsafe {"` inside a
/// Rust string or a `// comment about unsafe {` as a real unsafe site.
/// Line-local; multi-line strings and `/* … */` block comments are out of
/// scope.
fn strip_string_literals(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        // Line comment: `// …` — drop the rest of the line.
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            break;
        }
        // Raw string: r"…" or r#"…"# (any number of #)
        if c == 'r' && i + 1 < bytes.len() && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < bytes.len() && bytes[j] == b'#' { hashes += 1; j += 1; }
            if j < bytes.len() && bytes[j] == b'"' {
                // skip body until closing `"` followed by `hashes` `#`s
                j += 1;
                while j < bytes.len() {
                    if bytes[j] == b'"' {
                        let mut k = j + 1;
                        let mut matched = 0;
                        while matched < hashes && k < bytes.len() && bytes[k] == b'#' {
                            matched += 1; k += 1;
                        }
                        if matched == hashes { j = k; break; }
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
            // not a real raw string, fall through
        }
        // Normal string: "…"
        if c == '"' {
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\\' && j + 1 < bytes.len() { j += 2; continue; }
                if bytes[j] == b'"' { j += 1; break; }
                j += 1;
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn scan_file(path: &Path, records: &mut Vec<UnsafeRecord>) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        // RFC 060: classify on a copy with string literals stripped so the
        // scanner's own pattern strings and test-fixture strings do not
        // register as unsafe sites.
        let stripped = strip_string_literals(line);
        let trimmed = stripped.trim();

        let kind = if trimmed.contains("unsafe {") || trimmed.starts_with("unsafe {") {
            Some(UnsafeKind::Block)
        } else if trimmed.contains("unsafe fn ") {
            Some(UnsafeKind::Fn)
        } else if trimmed.contains("unsafe impl ") {
            Some(UnsafeKind::Impl)
        } else if trimmed.contains("unsafe trait ") {
            Some(UnsafeKind::Trait)
        } else {
            None
        };

        if let Some(kind) = kind {
            // Search preceding 12 lines for `// SAFETY:` and `category=`.
            let search_from = idx.saturating_sub(12);
            let (has_safety, safety_text) = find_safety_comment(&lines[search_from..idx]);
            let category = if has_safety {
                extract_category(&safety_text)
            } else {
                UnsafeCategory::Missing
            };

            records.push(UnsafeRecord {
                file:        path.display().to_string(),
                line:        (idx + 1) as u32,
                kind,
                has_safety,
                safety_text,
                category,
            });
        }
    }
    Ok(())
}

fn find_safety_comment(preceding: &[&str]) -> (bool, String) {
    // Scan backwards looking for // SAFETY:
    let mut in_safety = false;
    let mut safety_lines: Vec<&str> = Vec::new();

    for line in preceding.iter().rev() {
        let t = line.trim();
        if t.starts_with("// SAFETY:") {
            safety_lines.push(t);
            in_safety = true;
            break;
        }
        // Continuation comment lines before the SAFETY tag
        if in_safety && t.starts_with("//") {
            safety_lines.push(t);
            continue;
        }
        if t.is_empty() || (!t.starts_with("//") && !t.starts_with('#')) {
            break;
        }
    }

    if !in_safety && safety_lines.is_empty() {
        // Try simpler scan
        for line in preceding.iter().rev() {
            let t = line.trim();
            if t.contains("// SAFETY:") || t.starts_with("// SAFETY:") {
                let text = t.to_string();
                return (true, text);
            }
            if t.is_empty() { break; }
            if !t.starts_with("//") && !t.starts_with('#') { break; }
        }
        return (false, String::new());
    }

    safety_lines.reverse();
    let text = safety_lines.join(" ");
    (true, text)
}

/// Extract category from a SAFETY comment (RFC-v0.7.5-001).
fn extract_category(safety_text: &str) -> UnsafeCategory {
    // Look for "category=<name>" anywhere in the text
    if let Some(pos) = safety_text.find("category=") {
        let rest = &safety_text[pos + "category=".len()..];
        let name = rest.split(|c: char| c.is_whitespace() || c == ',').next().unwrap_or("");
        return UnsafeCategory::from_str(name);
    }
    UnsafeCategory::Missing
}

// ── Directory walk ────────────────────────────────────────────────────────────

fn walk(dir: &Path, records: &mut Vec<UnsafeRecord>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path  = entry.path();
        if path.is_dir() {
            // Skip target/, .git/, node_modules/.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "target" | ".git" | "node_modules") { continue; }
            walk(&path, records)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            scan_file(&path, records)?;
        }
    }
    Ok(())
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut root    = PathBuf::from(".");
    let mut json    = false;
    let mut check   = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--root"  => { i += 1; if i < args.len() { root = PathBuf::from(&args[i]); } }
            "--json"  => json  = true,
            "--check" => check = true,
            _         => {}
        }
        i += 1;
    }

    let mut records = Vec::new();
    if let Err(e) = walk(&root, &mut records) {
        eprintln!("fjell-unsafe-audit: walk error: {e}");
        process::exit(2);
    }

    let total    = records.len();
    let missing  = records.iter().filter(|r| !r.has_safety).count();
    let covered  = total - missing;

    if json {
        for r in &records {
            println!(
                r#"{{"file":"{f}","line":{l},"kind":"{k}","has_safety":{s},"category":"{cat}","safety_text":"{t}"}}"#,
                f = r.file.replace('\\', "/"),
                l = r.line,
                k = r.kind.as_str(),
                s = r.has_safety,
                    cat = r.category.as_str(),
                t = r.safety_text.replace('"', "\\\""),
            );
        }
    } else {
        println!("fjell-unsafe-audit  root={}", root.display());
        println!("  total unsafe sites : {total}");
        println!("  with SAFETY comment: {covered}");
        let valid_cats = records.iter().filter(|r| r.has_safety && r.category.is_valid()).count();
        let missing_cats = records.iter().filter(|r| r.has_safety && !r.category.is_valid()).count();
        println!("  with valid category tag: {valid_cats}");
        if missing_cats > 0 {
            println!("  MISSING/UNKNOWN category: {missing_cats}");
        }
        println!("  missing comment    : {missing}");
        if missing > 0 {
            println!();
            println!("MISSING SAFETY comments:");
            for r in records.iter().filter(|r| !r.has_safety) {
                println!("  {}:{} [{}]", r.file, r.line, r.kind.as_str());
            }
        }
    }

    if check && missing > 0 {
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile_helper::TmpFile;

    // Minimal file-write helper (no external dep).
    mod tempfile_helper {
        use std::{fs, path::PathBuf};
        pub struct TmpFile(pub PathBuf);
        impl TmpFile {
            pub fn write(name: &str, content: &str) -> Self {
                let p = std::env::temp_dir().join(name);
                fs::write(&p, content).unwrap();
                Self(p)
            }
        }
        impl Drop for TmpFile {
            fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
        }
    }

    #[test]
    fn detects_unsafe_block_with_safety() {
        let f = TmpFile::write("audit_test_1.rs",
            "// SAFETY: category=raw-pointer-deref pointer is valid for the lifetime of the borrow.\nunsafe { *ptr }\n");
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1);
        assert!(recs[0].has_safety, "should detect SAFETY comment");
    }

    #[test]
    fn detects_unsafe_block_missing_safety() {
        let f = TmpFile::write("audit_test_2.rs", "unsafe { *ptr }\n");
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].has_safety, "no SAFETY comment → missing");
    }

    #[test]
    fn detects_unsafe_fn() {
        let f = TmpFile::write("audit_test_3.rs",
            "// SAFETY: category=raw-pointer-deref caller ensures alignment.\npub unsafe fn raw_write(ptr: *mut u8) {}\n");
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].kind, UnsafeKind::Fn);
        assert!(recs[0].has_safety);
    }

    #[test]
    fn skips_non_rust_files() {
        let p = std::env::temp_dir().join("audit_test_4.txt");
        std::fs::write(&p, "unsafe { }").unwrap();
        let mut recs = vec![];
        scan_file(&p, &mut recs).unwrap();
        // scan_file itself doesn't filter extension — it's called after walk filter.
        // Walk skips non-.rs files; test that scanning a .txt still "works" (finds
        // the pattern but the walk would never call it).
        let _ = recs;
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn empty_file_produces_no_records() {
        let f = TmpFile::write("audit_test_5.rs", "// pure safe Rust\nfn main() {}\n");
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 0);
    }

    #[test]
    fn safety_comment_up_to_four_lines_above() {
        let src = "// SAFETY: category=raw-pointer-deref invariant holds.\n// other comment\n// another\n// and another\nunsafe { }\n";
        let f = TmpFile::write("audit_test_6.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1);
        assert!(recs[0].has_safety, "SAFETY 4 lines above should be found");
    }

    #[test]
    fn safety_far_above_within_window_found() {
        // Production lookback window is 12 lines; SAFETY 5 lines above
        // is within the window and must be found.
        let src = "// SAFETY: category=raw-pointer-deref within window.\n// a\n// b\n// c\n// d\nunsafe { }\n";
        let f = TmpFile::write("audit_test_7.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1);
        assert!(recs[0].has_safety, "SAFETY 5 lines above should be found (12-line window)");
    }

    #[test]
    fn multiple_unsafe_sites_per_file() {
        let src = "// SAFETY: category=raw-pointer-deref ok\nunsafe { a() }\nfn x() {}\nunsafe { b() }\n";
        let f = TmpFile::write("audit_test_8.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 2);
        assert!(recs[0].has_safety);
        assert!(!recs[1].has_safety, "second site has no SAFETY comment");
    }

    // RFC 060: `unsafe {` inside a string literal must not be classified.
    #[test]
    fn unsafe_inside_string_literal_not_counted() {
        let src = "let s = \"unsafe { foo }\";\n// SAFETY: category=raw-pointer-deref real one\nunsafe { real() }\n";
        let f = TmpFile::write("audit_test_9.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1, "only the real unsafe is counted, not the string");
        assert!(recs[0].has_safety);
        assert_eq!(recs[0].line, 3);
    }

    // RFC 060: `unsafe fn` inside string also ignored; raw strings handled.
    #[test]
    fn unsafe_inside_raw_string_not_counted() {
        let src = "let s = r#\"unsafe fn x() {}\"#;\nfn ordinary() {}\n";
        let f = TmpFile::write("audit_test_10.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 0);
    }
}
