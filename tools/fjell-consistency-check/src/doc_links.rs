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
//!
//! ## What counts as a link: code is not markup
//!
//! A `[label](url)` inside a fenced code block or an inline code span is
//! sample text. No renderer makes it clickable, so it cannot break for a
//! reader, and checking it reports a failure over something that is not a
//! link. Until 2026-10-21 the extractor was a raw byte scan with no such
//! awareness: `rfcs/done/000-rfc-lifecycle-policy.md`, whose subject is
//! cross-reference syntax, illustrates that syntax in ```markdown fences
//! and so put seven non-links in front of the check. The policy document
//! was correct; the instrument over-claimed its domain. This failed
//! closed — it refused a tree that was in fact clean — but a check that
//! cannot say what it inspects is not evidence either way.
//!
//! Indented (four-space) code blocks are deliberately *not* treated as
//! literal: a four-space indent is also how a list continuation is
//! written, so honouring it would hide real links in list items. An
//! unterminated code span likewise stays live, which keeps both
//! omissions on the reporting side.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const KNOWN_BROKEN_PATH: &str = "tests/doc-links/known-broken.txt";
const EXCLUDE_DIRS: &[&str] = &["target", ".git", ".git-exclude"];

const NAME: &str = "doc-links";

pub fn check() -> ExitCode {
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    if let Err(e) = walk_markdown(Path::new("."), &mut files) {
        eprintln!("{NAME}: FAIL — cannot walk repository tree: {e}");
        return ExitCode::FAILURE;
    }
    let refs: Vec<(&Path, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_path(), c.as_str()))
        .collect();

    let Some(known_broken_src) = crate::read_file(NAME, KNOWN_BROKEN_PATH) else {
        return ExitCode::FAILURE;
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
/// The published book's source directory.
const BOOK_ROOT: &str = "docs/src/";

pub fn run_check(files: &[(&Path, &str)], known_broken: &BTreeSet<(String, String)>) -> ExitCode {
    let mut broken = Vec::new();
    let mut recorded = 0usize;
    let mut total_links = 0usize;
    let mut matched: BTreeSet<(String, String)> = BTreeSet::new();

    for (path, content) in files {
        let dir = path.parent().unwrap_or(Path::new("."));
        for link in extract_links(content) {
            if is_external_or_anchor(&link) {
                continue;
            }
            total_links += 1;
            let target_link = link.split('#').next().unwrap_or(&link);
            let resolved = normalise(&dir.join(target_link));
            let file_label = normalise_label(path);
            // RFC-0.33-004 (E-052): a page of the published book must not reach
            // outside it by relative path. It resolves on disk — so the existence
            // check below passes — and 404s on the site, where `../../../rfcs/…`
            // escapes the site root. A citation to a repository file is an
            // absolute repository URL. (Pages outside the book, the RFCs among
            // them, are read on disk and keep their relative links.)
            if file_label.starts_with(BOOK_ROOT)
                && !resolved
                    .to_string_lossy()
                    .replace('\\', "/")
                    .starts_with(BOOK_ROOT)
            {
                broken.push(format!(
                    "{file_label}: link {link:?} leaves the book (resolves to {}): it works on \
                     disk and 404s on the published site — cite a repository file as an absolute \
                     `<repository url>/blob/main/<path>` URL (E-052)",
                    resolved.display()
                ));
                continue;
            }
            if resolved.exists() {
                continue;
            }
            let key = (file_label.clone(), link.clone());
            if known_broken.contains(&key) {
                recorded += 1;
                matched.insert(key);
                continue;
            }
            broken.push(format!(
                "{file_label}: broken link {link:?} (resolves to {})",
                resolved.display()
            ));
        }
    }

    // An allow-list entry that no longer matches a broken link is not
    // harmless: it would silently excuse that link if it ever broke again.
    // Until 2026-09-15 this check only consulted the list for links that were
    // already broken, so a fixed link's entry stayed forever and nothing could
    // say so — the list failed open. Found when both recorded entries turned
    // out to be mechanically fixable.
    for (file, link) in known_broken {
        if !matched.contains(&(file.clone(), link.clone())) {
            broken.push(format!(
                "{KNOWN_BROKEN_PATH}: lists `{file} -> {link}`, which is not a broken link — \
                 remove the entry, or it will excuse that link if it breaks again"
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
/// trailing `"title"` (`(url "title")`), and skipping any link that falls
/// inside a region Markdown renders literally (see the module header).
fn extract_links(src: &str) -> Vec<String> {
    let literal = literal_regions(src);
    let mut links = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if literal.iter().any(|&(s, e)| i >= s && i < e) {
                i += 1;
                continue;
            }
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

/// An opening or closing code fence: `(fence char, run length, rest of line)`.
/// Up to three leading spaces are permitted, per CommonMark; a backtick
/// fence's info string may not itself contain a backtick.
fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let c = trimmed.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let run = trimmed.chars().take_while(|&x| x == c).count();
    if run < 3 {
        return None;
    }
    let info = &trimmed[run..];
    if c == '`' && info.contains('`') {
        return None;
    }
    Some((c, run, info))
}

/// Byte ranges of `src` that Markdown renders literally: fenced code blocks
/// (including their fence lines) and inline code spans. A fence left open at
/// end of input runs to the end, as a renderer treats it.
fn literal_regions(src: &str) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut open: Option<(char, usize, usize)> = None; // (char, run, region start)
    let mut offset = 0usize;
    for line_nl in src.split_inclusive('\n') {
        let line = line_nl.trim_end_matches('\n').trim_end_matches('\r');
        let line_end = offset + line_nl.len();
        match open {
            Some((oc, orun, start)) => {
                if let Some((c, run, rest)) = fence_marker(line) {
                    if c == oc && run >= orun && rest.trim().is_empty() {
                        regions.push((start, line_end));
                        open = None;
                    }
                }
            }
            None => {
                if let Some((c, run, _)) = fence_marker(line) {
                    open = Some((c, run, offset));
                } else {
                    code_span_ranges(line, offset, &mut regions);
                }
            }
        }
        offset = line_end;
    }
    if let Some((_, _, start)) = open {
        regions.push((start, src.len()));
    }
    regions
}

/// Inline code spans on one line: a run of N backticks is closed by the next
/// run of exactly N. An unmatched run is literal text, so scanning resumes
/// after it and anything following stays live.
fn code_span_ranges(line: &str, base: usize, out: &mut Vec<(usize, usize)>) {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'`' {
            let n = b[i..].iter().take_while(|&&c| c == b'`').count();
            let mut j = i + n;
            let mut closed = None;
            while j < b.len() {
                if b[j] == b'`' {
                    let m = b[j..].iter().take_while(|&&c| c == b'`').count();
                    if m == n {
                        closed = Some(j + m);
                        break;
                    }
                    j += m;
                } else {
                    j += 1;
                }
            }
            match closed {
                Some(end) => {
                    out.push((base + i, base + end));
                    i = end;
                }
                None => i += n,
            }
            continue;
        }
        i += 1;
    }
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
        // file `rfcs/ERRATA.md` shape by pointing at a file guaranteed
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

    /// The allow-list must not fail open: an entry whose link has been fixed
    /// would otherwise excuse that link, silently, if it ever broke again.
    #[test]
    fn stale_known_broken_entry_fails() {
        let files = [(Path::new("Cargo.toml"), "[x](src/main.rs)")];
        let mut known = BTreeSet::new();
        known.insert(("Cargo.toml".to_string(), "src/main.rs".to_string()));
        assert_eq!(run_check(&files, &known), ExitCode::FAILURE);
    }

    /// Same, for an entry whose link has been removed from the file entirely.
    #[test]
    fn known_broken_entry_for_a_link_no_longer_present_fails() {
        let files = [(Path::new("Cargo.toml"), "no links here")];
        let mut known = BTreeSet::new();
        known.insert(("Cargo.toml".to_string(), "does/not/exist.md".to_string()));
        assert_eq!(run_check(&files, &known), ExitCode::FAILURE);
    }

    // ── RFC-0.33-004 (E-052): a book page may not link out of the book ───────

    #[test]
    fn a_book_page_linking_out_of_the_book_is_refused() {
        // The target exists on disk (this is the whole problem), so only the
        // boundary rule can refuse it.
        let files = [(Path::new("docs/src/a/page.md"), "[x](../../../Cargo.toml)")];
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::FAILURE);
    }

    #[test]
    fn a_page_outside_the_book_keeps_its_relative_links() {
        let files = [(Path::new("rfcs/a/page.md"), "[x](../../Cargo.toml)")];
        // resolves to ./Cargo.toml relative to this crate's cwd
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::SUCCESS);
    }

    // ── Code is not markup: a link inside literal text is not a link ─────────
    //
    // Each case below is paired with a control: the same non-existent target
    // outside the literal region must still fail, or the test would pass for
    // the wrong reason (the extractor having stopped working at all).

    #[test]
    fn a_link_inside_a_fenced_block_is_not_a_link() {
        let src = "text\n```markdown\n[x](does/not/exist.md)\n```\nmore\n";
        assert_eq!(extract_links(src), Vec::<String>::new());
        let files = [(Path::new("Cargo.toml"), src)];
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::SUCCESS);
    }

    #[test]
    fn control_the_same_link_outside_the_fence_still_fails() {
        let src = "text\n```markdown\nnothing\n```\n[x](does/not/exist.md)\n";
        assert_eq!(extract_links(src), vec!["does/not/exist.md".to_string()]);
        let files = [(Path::new("Cargo.toml"), src)];
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::FAILURE);
    }

    #[test]
    fn a_fence_reopens_so_links_after_a_closed_block_are_live() {
        let src = "```\n[a](no/a.md)\n```\n[b](no/b.md)\n```\n[c](no/c.md)\n```\n";
        assert_eq!(extract_links(src), vec!["no/b.md".to_string()]);
    }

    #[test]
    fn tilde_fences_and_longer_runs_delimit_too() {
        let src = "~~~\n[a](no/a.md)\n~~~\n````\n[b](no/b.md)\n````\n";
        assert_eq!(extract_links(src), Vec::<String>::new());
    }

    /// A shorter run does not close a longer fence, so the block continues.
    #[test]
    fn a_shorter_run_does_not_close_a_longer_fence() {
        let src = "````\n```\n[a](no/a.md)\n````\n[b](no/b.md)\n";
        assert_eq!(extract_links(src), vec!["no/b.md".to_string()]);
    }

    /// An unclosed fence runs to end of input, as a renderer treats it.
    #[test]
    fn an_unclosed_fence_swallows_the_rest_of_the_file() {
        let src = "```\n[a](no/a.md)\n[b](no/b.md)\n";
        assert_eq!(extract_links(src), Vec::<String>::new());
    }

    /// A fence's own info string is not scanned, but indentation past three
    /// spaces is not a fence at all.
    #[test]
    fn four_space_indent_is_not_a_fence() {
        let src = "    ```\n[a](no/a.md)\n";
        assert_eq!(extract_links(src), vec!["no/a.md".to_string()]);
    }

    #[test]
    fn a_link_inside_an_inline_code_span_is_not_a_link() {
        let src = "write it as `[x](does/not/exist.md)` in the file\n";
        assert_eq!(extract_links(src), Vec::<String>::new());
        let files = [(Path::new("Cargo.toml"), src)];
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::SUCCESS);
    }

    #[test]
    fn control_a_link_beside_a_code_span_is_still_a_link() {
        let src = "`code` then [x](does/not/exist.md) here\n";
        assert_eq!(extract_links(src), vec!["does/not/exist.md".to_string()]);
        let files = [(Path::new("Cargo.toml"), src)];
        assert_eq!(run_check(&files, &BTreeSet::new()), ExitCode::FAILURE);
    }

    /// An unmatched backtick is literal text: everything after it stays live,
    /// so this omission falls on the reporting side.
    #[test]
    fn an_unmatched_backtick_does_not_mask_the_rest_of_the_line() {
        let src = "a ` stray tick then [x](no/x.md)\n";
        assert_eq!(extract_links(src), vec!["no/x.md".to_string()]);
    }

    /// The real document that exposed the blind spot: example cross-reference
    /// syntax, in a fence, in the policy that documents cross-references.
    #[test]
    fn the_lifecycle_policys_example_syntax_is_not_checked() {
        let src = "Use a relative path:\n\n```markdown\n                   See [RFC 010](../done/010-revoke-tokens.md) for the prior work.\n                   ```\n";
        assert_eq!(extract_links(src), Vec::<String>::new());
    }
}
