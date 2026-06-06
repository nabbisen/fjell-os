//! `fjell-unsafe-audit` — unsafe boundary inventory scanner (RFC v0.6-004).
//!
//! Usage:
//!   fjell-unsafe-audit [--root <path>] [--json] [--check]
//!
//! Exits 0 if every unsafe site has a `// SAFETY:` comment within 4 lines.
//! Exits 1 if any unsafe site is missing a SAFETY comment.
//! With `--json`, prints a JSON-lines inventory to stdout.

use std::{
    env, fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    process,
};

#[derive(Debug)]
struct UnsafeRecord {
    file:          String,
    line:          u32,
    kind:          UnsafeKind,
    has_safety:    bool,
    safety_text:   String,
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

fn scan_file(path: &Path, records: &mut Vec<UnsafeRecord>) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

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
            // Search preceding 4 lines for `// SAFETY:`.
            let search_from = idx.saturating_sub(4);
            let (has_safety, safety_text) = find_safety_comment(&lines[search_from..idx]);

            records.push(UnsafeRecord {
                file:        path.display().to_string(),
                line:        (idx + 1) as u32,
                kind,
                has_safety,
                safety_text,
            });
        }
    }
    Ok(())
}

fn find_safety_comment(preceding: &[&str]) -> (bool, String) {
    for line in preceding.iter().rev() {
        let t = line.trim();
        if t.starts_with("// SAFETY:") {
            let text = t.trim_start_matches("// SAFETY:").trim().to_string();
            return (true, text);
        }
        // Stop at blank lines or non-comment lines (except attribute-like lines).
        if t.is_empty() { break; }
        if !t.starts_with("//") && !t.starts_with('#') { break; }
    }
    (false, String::new())
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
                r#"{{"file":"{f}","line":{l},"kind":"{k}","has_safety":{s},"safety_text":"{t}"}}"#,
                f = r.file.replace('\\', "/"),
                l = r.line,
                k = r.kind.as_str(),
                s = r.has_safety,
                t = r.safety_text.replace('"', "\\\""),
            );
        }
    } else {
        println!("fjell-unsafe-audit  root={}", root.display());
        println!("  total unsafe sites : {total}");
        println!("  with SAFETY comment: {covered}");
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
            "// SAFETY: pointer is valid for the lifetime of the borrow.\nunsafe { *ptr }\n");
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
            "// SAFETY: caller ensures alignment.\npub unsafe fn raw_write(ptr: *mut u8) {}\n");
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
        let src = "// SAFETY: invariant holds.\n// other comment\n// another\n// and another\nunsafe { }\n";
        let f = TmpFile::write("audit_test_6.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1);
        assert!(recs[0].has_safety, "SAFETY 4 lines above should be found");
    }

    #[test]
    fn safety_five_lines_above_not_found() {
        let src = "// SAFETY: too far away.\n// a\n// b\n// c\n// d\nunsafe { }\n";
        let f = TmpFile::write("audit_test_7.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].has_safety, "SAFETY 5 lines above must not be found");
    }

    #[test]
    fn multiple_unsafe_sites_per_file() {
        let src = "// SAFETY: ok\nunsafe { a() }\nfn x() {}\nunsafe { b() }\n";
        let f = TmpFile::write("audit_test_8.rs", src);
        let mut recs = vec![];
        scan_file(&f.0, &mut recs).unwrap();
        assert_eq!(recs.len(), 2);
        assert!(recs[0].has_safety);
        assert!(!recs[1].has_safety, "second site has no SAFETY comment");
    }
}
