//! RFC-0.27-001: the `doc-links` subcheck (S4).
//!
//! Every relative link in a tracked `.md` file must resolve to an existing
//! path. The 2026-08-03 instrument audit found 13 broken; a fresh sweep for
//! this RFC found 14 (E-016's own text anticipated the count moving). 12
//! were mechanical (a renamed file, a missing `../`, a `.md` extension
//! where the real artefact is `.txt`) and are fixed in this same change.
//! The remaining 2 need a decision about where a superseded ADR's content
//! actually lives now (RFC 045's renumbering) — recorded in
//! `tests/doc-links/known-broken.txt` with the reason, per the handoff's
//! "fix what is mechanical, record what is not."
//!
//! ## Scope: a filesystem walk, not `git ls-files`
//!
//! Consistent with every other subcheck in this tool (none shells out to
//! git — see `errata_tracking`'s design note on the same question), this
//! walks the directory tree directly, excluding `target/`, `.git/`, and
//! `.git-exclude/` (the last is this project's private, gitignored review
//! workflow directory — not part of the tracked documentation set at all).
//! Verified equivalent to `git ls-files '*.md'` against the current tree
//! (380 files, both ways) before relying on it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const KNOWN_BROKEN_PATH: &str = "tests/doc-links/known-broken.txt";
const EXCLUDE_DIRS: &[&str] = &["target", ".git", ".git-exclude"];

pub fn check() -> ExitCode {
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    if let Err(e) = walk_markdown(Path::new("."), &mut files) {
        eprintln!("consistency-check: cannot walk repository tree: {e}");
        return ExitCode::FAILURE;
    }
    let refs: Vec<(&Path, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_path(), c.as_str()))
        .collect();

    let known_broken_src = match fs::read_to_string(KNOWN_BROKEN_PATH) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("consistency-check: cannot read {KNOWN_BROKEN_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let known_broken = parse_known_broken(&known_broken_src);

    run_check(&refs, &known_broken)
}

fn walk_markdown(dir: &Path, out: &mut Vec<(PathBuf, String)>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if EXCLUDE_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk_markdown(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            if let Ok(content) = fs::read_to_string(&path) {
                out.push((path, content));
            }
        }
    }
    Ok(())
}

/// `(containing file, link as written)` pairs allowed to stay broken.
fn parse_known_broken(src: &str) -> BTreeSet<(String, String)> {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(" -> "))
        .map(|(f, l)| (f.trim().to_string(), l.trim().to_string()))
        .collect()
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
/// `files` is `(path, content)`; paths are compared to `known_broken` after
/// normalising to forward-slash, repo-root-relative form.
pub fn run_check(files: &[(&Path, &str)], known_broken: &BTreeSet<(String, String)>) -> ExitCode {
    let mut broken = Vec::new();
    let mut recorded = 0usize;
    let mut total_links = 0usize;

    for (path, content) in files {
        let dir = path.parent().unwrap_or(Path::new("."));
        for link in extract_links(content) {
            if is_external_or_anchor(&link) {
                continue;
            }
            total_links += 1;
            let target_link = link.split('#').next().unwrap_or(&link);
            let resolved = normalise(&dir.join(target_link));
            if resolved.exists() {
                continue;
            }
            let file_label = normalise_label(path);
            if known_broken.contains(&(file_label.clone(), link.clone())) {
                recorded += 1;
                continue;
            }
            broken.push(format!(
                "{file_label}: broken link {link:?} (resolves to {})",
                resolved.display()
            ));
        }
    }

    if broken.is_empty() {
        println!(
            "doc-links: PASS ({total_links} relative links checked, {recorded} recorded as known-broken)"
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("doc-links: FAIL");
        for b in &broken {
            eprintln!("  {b}");
        }
        ExitCode::FAILURE
    }
}

/// Strip a leading `./` for a stable, comparable label (`known-broken.txt`
/// entries are written without it).
fn normalise_label(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

/// Collapse `.`/`..` components without requiring the path to exist (unlike
/// `Path::canonicalize`, which would fail on the very paths this check
/// needs to report as missing).
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn is_external_or_anchor(link: &str) -> bool {
    link.is_empty()
        || link.starts_with('#')
        || link.starts_with("http://")
        || link.starts_with("https://")
        || link.starts_with("mailto:")
        || link.starts_with("data:")
}

/// Extract every Markdown `[label](url)` link target, dropping an optional
/// trailing `"title"` (`(url "title")`).
fn extract_links(src: &str) -> Vec<String> {
    let mut links = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(close_bracket) = src[i..].find(']') {
                let after = i + close_bracket + 1;
                if src.as_bytes().get(after) == Some(&b'(') {
                    if let Some(close_paren_rel) = src[after..].find(')') {
                        let inner = &src[after + 1..after + close_paren_rel];
                        let url = inner.split_whitespace().next().unwrap_or("");
                        links.push(url.to_string());
                        i = after + close_paren_rel + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_link() {
        let src = "See [the doc](path/to/file.md) for details.";
        assert_eq!(extract_links(src), vec!["path/to/file.md".to_string()]);
    }

    #[test]
    fn extracts_link_with_title() {
        let src = "[label](path/to/file.md \"a title\")";
        assert_eq!(extract_links(src), vec!["path/to/file.md".to_string()]);
    }

    #[test]
    fn ignores_external_and_anchor_links() {
        assert!(is_external_or_anchor("https://example.com"));
        assert!(is_external_or_anchor("#section"));
        assert!(is_external_or_anchor("mailto:a@b.com"));
        assert!(!is_external_or_anchor("../foo.md"));
    }

    #[test]
    fn resolving_link_that_exists_passes() {
        let files = [(Path::new("docs/a.md"), "[x](b.md)")];
        // b.md resolved relative to docs/ is docs/b.md — use the real repo
        // file `docs/rfcs/ERRATA.md` shape by pointing at a file guaranteed
        // to exist relative to the crate's own manifest instead.
        let files2 = [(Path::new("Cargo.toml"), "[x](src/main.rs)")];
        let _ = files; // fixture above illustrates shape; not asserted on disk
        assert_eq!(run_check(&files2, &BTreeSet::new()), ExitCode::SUCCESS);
    }

    /// Required failure demonstration: a link to a path that does not exist.
    #[test]
    fn broken_link_fails() {
        let files = [(Path::new("Cargo.toml"), "[x](does/not/exist.md)")];
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::FAILURE);
    }

    #[test]
    fn known_broken_link_is_recorded_not_failed() {
        let files = [(Path::new("Cargo.toml"), "[x](does/not/exist.md)")];
        let mut known = BTreeSet::new();
        known.insert(("Cargo.toml".to_string(), "does/not/exist.md".to_string()));
        assert_eq!(run_check(&files, &known), ExitCode::SUCCESS);
    }

    #[test]
    fn fragment_only_target_is_checked_without_the_fragment() {
        // `file.md#section` must resolve `file.md`, not the literal string.
        let files = [(Path::new("Cargo.toml"), "[x](src/main.rs#somewhere)")];
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::SUCCESS);
    }

    #[test]
    fn parse_known_broken_reads_arrow_format() {
        let src = "# comment\n\nfoo.md -> ../bar.md\nbaz.md -> qux.md\n";
        let parsed = parse_known_broken(src);
        assert!(parsed.contains(&("foo.md".to_string(), "../bar.md".to_string())));
        assert!(parsed.contains(&("baz.md".to_string(), "qux.md".to_string())));
        assert_eq!(parsed.len(), 2);
    }
}
