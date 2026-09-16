//! RFC-0.32-003 D8 — the four subchecks that hold the documentation's shape.
//!
//! E-050's claim is that 76 of the 135 files under the book root are in no
//! book, that book pages point at documents the book does not contain, and
//! that four directory names exist twice. Each of those is a property of the
//! tree, so each gets an instrument rather than a convention:
//!
//!   - `summary-completeness`       — every page is reachable from SUMMARY.md
//!   - `prose-in-the-book`          — no prose under `docs/` outside `src/`
//!   - `no-stub-pages`              — no page that is merely a pointer
//!   - `unique-doc-directory-names` — no leaf name used twice
//!
//! **Everything here is derived from the tree.** The one hand-written list is
//! `ROOT_PAGES`, and it is the exception D13 declares rather than a
//! convenience: those four files are what a visitor to the repository sees
//! first, so they stay at the root and out of the book.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

pub const DOCS_DIR: &str = "docs";
pub const BOOK_SRC: &str = "docs/src";
pub const SUMMARY_PATH: &str = "docs/src/SUMMARY.md";

/// The repository-root pages that are deliberately not book chapters (D13).
///
/// This is the only hand-written file list in this module. Each is here
/// because a visitor finds it before they find the book — three of them by
/// convention that hosting platforms render automatically, one because it is
/// the project's terms:
const ROOT_PAGES: &[(&str, &str)] = &[
    (
        "README.md",
        "the repository's front page, rendered by the host",
    ),
    (
        "CHANGELOG.md",
        "the release history a visitor checks before cloning",
    ),
    (
        "ROADMAP.md",
        "the surviving roadmap (D13); the book's copy is merged into it",
    ),
    (
        "TERMS_OF_USE.md",
        "the project's terms, which must be findable without the book",
    ),
];

/// `docs/book/` is mdBook's output: generated, git-ignored (D9), and never
/// prose in its own right. Walking it would make every check report the book
/// twice.
const GENERATED_DIRS: &[&str] = &["book"];

/// A page at or under `docs/src` whose body is this small *and* which points
/// at a document outside the book is a pointer, not a page (D5).
///
/// The cut is derived, not chosen: measured over every page under `docs/src`
/// that links outside it, the sizes are 226, 815, then 1566, 1831, 1916,
/// 2025 and 2912 bytes. The gap between 815 and 1566 is where "this page is
/// its link" stops and "this page has links" begins. It is still a judgement,
/// and it is one number in one place so it can be moved.
const STUB_MAX_BYTES: usize = 1024;

// ── shared derivation ────────────────────────────────────────────────────────

/// Every `.md` under `dir`, excluding generated output, repo-root-relative
/// and forward-slashed.
fn markdown_under(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        if path.is_dir() {
            if GENERATED_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    Ok(())
}

/// Directories under `dir`, excluding generated output.
fn directories_under(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        if GENERATED_DIRS.contains(&name.as_str()) {
            continue;
        }
        out.push(path.clone());
        directories_under(&path, out)?;
    }
    Ok(())
}

/// Resolve a relative link target against the page that wrote it, without
/// touching the filesystem — `..` is applied lexically so a target that
/// escapes `docs/src` is recognised whether or not it exists.
fn resolve_lexically(from_page: &Path, target: &str) -> PathBuf {
    let target = target.split('#').next().unwrap_or(target);
    let base = from_page.parent().unwrap_or(Path::new("."));
    let mut parts: Vec<String> = Vec::new();
    for c in base.join(target).components() {
        match c {
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir => {}
            other => parts.push(other.as_os_str().to_string_lossy().to_string()),
        }
    }
    PathBuf::from(parts.join("/"))
}

/// Every markdown link target in `src` that is not external.
fn local_link_targets(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
            if let Some(end) = src[i + 2..].find(')') {
                let target = &src[i + 2..i + 2 + end];
                if !target.starts_with('#') && !target.starts_with("mailto:") {
                    out.push(target.trim().to_string());
                }
                i += 2 + end;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// A link into this repository's own tree written as a full URL — the shape
/// `docs/src/release/v1-non-goals.md` uses, which a relative-path check
/// cannot see. Returns the repository path it points at.
fn repo_url_path(target: &str) -> Option<String> {
    let rest = target.strip_prefix("https://github.com/")?;
    let mut parts = rest.splitn(5, '/');
    let _owner = parts.next()?;
    let _repo = parts.next()?;
    let kind = parts.next()?;
    if kind != "blob" && kind != "tree" {
        return None;
    }
    let _git_ref = parts.next()?;
    Some(parts.next()?.split('#').next()?.to_string())
}

// ── summary-completeness ─────────────────────────────────────────────────────

const SUMMARY_NAME: &str = "summary-completeness";

pub fn summary_completeness() -> ExitCode {
    let Some(summary) = crate::read_file(SUMMARY_NAME, SUMMARY_PATH) else {
        return ExitCode::FAILURE;
    };
    let pages = match markdown_under(Path::new(BOOK_SRC)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{SUMMARY_NAME}: FAIL — cannot walk {BOOK_SRC}: {e}");
            return ExitCode::FAILURE;
        }
    };
    run_summary_check(&summary, &pages)
}

/// Core comparison, pure in its inputs.
///
/// `SUMMARY.md` is excluded from the pages that must be listed: it is the
/// navigation, not a chapter, and requiring it to list itself would be a
/// rule about the instrument rather than the book. That single exclusion is
/// why this reports 134 pages where E-050 counts 135 files.
pub fn run_summary_check(summary_src: &str, pages: &[PathBuf]) -> ExitCode {
    let summary_path = PathBuf::from(SUMMARY_PATH);
    let chapters: BTreeSet<PathBuf> = pages
        .iter()
        .filter(|p| **p != summary_path)
        .cloned()
        .collect();

    let mut listed: BTreeSet<PathBuf> = BTreeSet::new();
    for target in local_link_targets(summary_src) {
        if target.ends_with(".md") {
            listed.insert(resolve_lexically(&summary_path, &target));
        }
    }

    let missing: Vec<&PathBuf> = chapters.iter().filter(|p| !listed.contains(*p)).collect();
    let dangling: Vec<&PathBuf> = listed.iter().filter(|p| !chapters.contains(*p)).collect();

    if missing.is_empty() && dangling.is_empty() {
        println!(
            "{SUMMARY_NAME}: PASS ({} page(s) under {BOOK_SRC}, all reachable from SUMMARY.md; \
             {} entr(ies), all resolving)",
            chapters.len(),
            listed.len()
        );
        return ExitCode::SUCCESS;
    }

    if !missing.is_empty() {
        eprintln!(
            "{SUMMARY_NAME}: FAIL — {} of {} page(s) under {BOOK_SRC} are in no book:",
            missing.len(),
            chapters.len()
        );
        // Grouped by directory: 41 ADRs listed one per line is a wall, and
        // the shape of this failure is which *sections* are missing.
        let mut by_dir: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for p in &missing {
            let dir = p
                .parent()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default();
            by_dir.entry(dir).or_default().push(
                p.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
        }
        for (dir, files) in &by_dir {
            eprintln!("    {dir}/  ({} page(s))", files.len());
            for f in files.iter().take(4) {
                eprintln!("      {f}");
            }
            if files.len() > 4 {
                eprintln!("      … and {} more", files.len() - 4);
            }
        }
    }
    for p in &dangling {
        eprintln!(
            "{SUMMARY_NAME}: FAIL — SUMMARY.md lists {}, which does not exist",
            p.display()
        );
    }
    ExitCode::FAILURE
}

// ── prose-in-the-book ────────────────────────────────────────────────────────

const PROSE_NAME: &str = "prose-in-the-book";

pub fn prose_in_the_book() -> ExitCode {
    let all = match markdown_under(Path::new(DOCS_DIR)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{PROSE_NAME}: FAIL — cannot walk {DOCS_DIR}: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The repository root, one level only: `releases/` and `rfcs/` hold
    // documents by decision (D12), but a `.md` dropped at the root itself is
    // prose outside the book that is not one of D13's four.
    let mut root_md: Vec<String> = Vec::new();
    match fs::read_dir(".") {
        Ok(entries) => {
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_file() && name.ends_with(".md") {
                    root_md.push(name);
                }
            }
        }
        Err(e) => {
            eprintln!("{PROSE_NAME}: FAIL — cannot read the repository root: {e}");
            return ExitCode::FAILURE;
        }
    }
    root_md.sort();
    run_prose_check(&all, &root_md)
}

/// D12: `docs/` holds the book and nothing else. Any `.md` under `docs/` that
/// is not under `docs/src` is prose living where no reader will find it.
pub fn run_prose_check(docs_markdown: &[PathBuf], root_markdown: &[String]) -> ExitCode {
    let src = Path::new(BOOK_SRC);
    let outside: Vec<&PathBuf> = docs_markdown
        .iter()
        .filter(|p| !p.starts_with(src))
        .collect();

    // D1's declared exception, checked rather than assumed: a root page that
    // is not one of the four is prose nobody navigates to.
    let allowed: BTreeSet<&str> = ROOT_PAGES.iter().map(|(n, _)| *n).collect();
    let stray_root: Vec<&String> = root_markdown
        .iter()
        .filter(|n| !allowed.contains(n.as_str()))
        .collect();

    if outside.is_empty() && stray_root.is_empty() {
        println!(
            "{PROSE_NAME}: PASS (every one of the {} markdown file(s) under {DOCS_DIR} is a book page; \
             {} repository-root page(s), all declared)",
            docs_markdown.len(),
            root_markdown.len()
        );
        return ExitCode::SUCCESS;
    }

    for name in &stray_root {
        eprintln!(
            "{PROSE_NAME}: FAIL — {name} is prose at the repository root and is not one of D13's \
             four declared pages; it belongs under {BOOK_SRC} and in SUMMARY.md"
        );
    }
    if outside.is_empty() {
        return ExitCode::FAILURE;
    }

    eprintln!(
        "{PROSE_NAME}: FAIL — {} markdown file(s) under {DOCS_DIR} are outside the book:",
        outside.len()
    );
    let mut by_dir: BTreeMap<String, usize> = BTreeMap::new();
    for p in &outside {
        let dir = p
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        *by_dir.entry(dir).or_default() += 1;
    }
    for (dir, n) in &by_dir {
        eprintln!("    {dir}/  ({n} file(s))");
    }
    eprintln!(
        "  Every maintained page belongs under {BOOK_SRC} and in SUMMARY.md (D1); \
         records belong in releases/, the RFC corpus in rfcs/ (D12)."
    );
    ExitCode::FAILURE
}

// ── no-stub-pages ────────────────────────────────────────────────────────────

const STUB_NAME: &str = "no-stub-pages";

pub fn no_stub_pages() -> ExitCode {
    let pages = match markdown_under(Path::new(BOOK_SRC)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{STUB_NAME}: FAIL — cannot walk {BOOK_SRC}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut loaded: Vec<(PathBuf, String)> = Vec::new();
    for p in pages {
        match fs::read_to_string(&p) {
            Ok(s) => loaded.push((p, s)),
            Err(e) => {
                eprintln!("{STUB_NAME}: FAIL — cannot read {}: {e}", p.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let refs: Vec<(&Path, &str)> = loaded
        .iter()
        .map(|(p, s)| (p.as_path(), s.as_str()))
        .collect();
    run_stub_check(&refs)
}

/// D5: a page is the document. A short page whose content is a pointer to a
/// document outside the book is neither — and in the built site the pointer
/// cannot even resolve, because nothing outside `docs/src` is copied into
/// `docs/book`.
///
/// Both spellings of "outside" count: a relative path that climbs out of
/// `docs/src`, and a full `https://github.com/owner/repo/blob/ref/path` URL
/// into this repository's own tree. The second is not a theoretical case —
/// it is how `docs/src/release/v1-non-goals.md` points at its real document,
/// and a relative-path-only rule reports that page as clean.
pub fn run_stub_check(pages: &[(&Path, &str)]) -> ExitCode {
    let src = Path::new(BOOK_SRC);
    let mut stubs: Vec<(String, Vec<String>)> = Vec::new();

    for (path, content) in pages {
        if path.file_name().is_some_and(|f| f == "SUMMARY.md") {
            continue;
        }
        if content.len() > STUB_MAX_BYTES {
            continue;
        }
        let mut escaping = Vec::new();
        for target in local_link_targets(content) {
            let outside = if let Some(repo_path) = repo_url_path(&target) {
                !Path::new(&repo_path).starts_with(src)
            } else if target.starts_with("http") {
                false
            } else {
                !resolve_lexically(path, &target).starts_with(src)
            };
            if outside {
                escaping.push(target);
            }
        }
        if !escaping.is_empty() {
            stubs.push((path.to_string_lossy().to_string(), escaping));
        }
    }

    if stubs.is_empty() {
        println!(
            "{STUB_NAME}: PASS ({} page(s) checked; none is a pointer to a document outside the book)",
            pages.len()
        );
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "{STUB_NAME}: FAIL — {} page(s) are pointers, not pages:",
        stubs.len()
    );
    for (page, targets) in &stubs {
        eprintln!("    {page}");
        for t in targets {
            eprintln!("      -> {t}   (outside {BOOK_SRC}; not copied into the built site)");
        }
    }
    eprintln!("  D5: replace the page with the document it points at, or delete it.");
    ExitCode::FAILURE
}

// ── unique-doc-directory-names ───────────────────────────────────────────────

const UNIQUE_NAME: &str = "unique-doc-directory-names";

pub fn unique_doc_directory_names() -> ExitCode {
    let mut docs_dirs = Vec::new();
    if let Err(e) = directories_under(Path::new(DOCS_DIR), &mut docs_dirs) {
        eprintln!("{UNIQUE_NAME}: FAIL — cannot walk {DOCS_DIR}: {e}");
        return ExitCode::FAILURE;
    }
    let mut root_dirs = Vec::new();
    match fs::read_dir(".") {
        Ok(entries) => {
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() && !name.starts_with('.') && name != DOCS_DIR {
                    root_dirs.push(name);
                }
            }
        }
        Err(e) => {
            eprintln!("{UNIQUE_NAME}: FAIL — cannot read the repository root: {e}");
            return ExitCode::FAILURE;
        }
    }
    docs_dirs.sort();
    root_dirs.sort();
    run_unique_check(&docs_dirs, &root_dirs)
}

/// D6: no two directories under `docs/`, and none against a repository-root
/// directory, share a leaf name. A name that means two things is why
/// `docs/src/assurance/` (the audits) and `verification/` (proof source) were
/// read as the same place.
pub fn run_unique_check(docs_dirs: &[PathBuf], root_dirs: &[String]) -> ExitCode {
    let mut by_leaf: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for d in docs_dirs {
        let Some(leaf) = d.file_name().map(|f| f.to_string_lossy().to_string()) else {
            continue;
        };
        by_leaf
            .entry(leaf)
            .or_default()
            .push(d.to_string_lossy().to_string());
    }

    let mut problems: Vec<String> = Vec::new();
    for (leaf, paths) in &by_leaf {
        if paths.len() > 1 {
            problems.push(format!(
                "  `{leaf}` names {} directories: {}",
                paths.len(),
                paths.join(", ")
            ));
        }
        if root_dirs.contains(leaf) {
            problems.push(format!(
                "  `{leaf}` names both {} and the repository-root `{leaf}/`",
                paths.join(", ")
            ));
        }
    }

    if problems.is_empty() {
        println!(
            "{UNIQUE_NAME}: PASS ({} director(ies) under {DOCS_DIR}, every leaf name distinct, \
             and distinct from the {} repository-root director(ies))",
            docs_dirs.len(),
            root_dirs.len()
        );
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "{UNIQUE_NAME}: FAIL — {} duplicate director(y/ies) name(s):",
        problems.len()
    );
    for p in &problems {
        eprintln!("{p}");
    }
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The subchecks run with the repository root as the working directory
    /// (that is how Gate 12 invokes them); `cargo test` runs from the package
    /// directory, so this anchors to the manifest rather than to the CWD.
    #[test]
    fn the_declared_root_pages_are_at_the_root() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("tools/<crate>/ is two levels below the repository root");
        for (name, why) in ROOT_PAGES {
            assert!(
                repo_root.join(name).is_file(),
                "{name} is the declared D1 exception ({why}) but is not at {}",
                repo_root.display()
            );
        }
    }

    #[test]
    fn lexical_resolution_climbs_out_of_the_book() {
        let page = Path::new("docs/src/release/v1-readiness.md");
        assert_eq!(
            resolve_lexically(page, "../../release/v1-readiness.md"),
            PathBuf::from("docs/release/v1-readiness.md")
        );
        assert_eq!(
            resolve_lexically(page, "./sibling.md"),
            PathBuf::from("docs/src/release/sibling.md")
        );
    }

    #[test]
    fn a_full_repository_url_is_recognised_as_a_path() {
        assert_eq!(
            repo_url_path(
                "https://github.com/nabbisen/fjell-os/blob/main/docs/release/v1-non-goals.md"
            ),
            Some("docs/release/v1-non-goals.md".to_string())
        );
        assert_eq!(repo_url_path("https://example.com/whatever"), None);
        assert_eq!(repo_url_path("../relative.md"), None);
    }

    #[test]
    fn summary_check_names_an_unlisted_page_and_a_dangling_entry() {
        let pages = vec![
            PathBuf::from("docs/src/SUMMARY.md"),
            PathBuf::from("docs/src/listed.md"),
            PathBuf::from("docs/src/unlisted.md"),
        ];
        assert_eq!(
            run_summary_check("- [L](./listed.md)\n- [U](./unlisted.md)\n", &pages),
            ExitCode::SUCCESS
        );
        // an unlisted page fails
        assert_ne!(
            run_summary_check("- [L](./listed.md)\n", &pages),
            ExitCode::SUCCESS
        );
        // an entry resolving to nothing fails
        assert_ne!(
            run_summary_check(
                "- [L](./listed.md)\n- [U](./unlisted.md)\n- [G](./ghost.md)\n",
                &pages
            ),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn prose_check_passes_only_when_everything_is_under_src() {
        let declared: Vec<String> = ROOT_PAGES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(
            run_prose_check(&[PathBuf::from("docs/src/a.md")], &declared),
            ExitCode::SUCCESS
        );
        assert_ne!(
            run_prose_check(
                &[
                    PathBuf::from("docs/src/a.md"),
                    PathBuf::from("docs/release/b.md")
                ],
                &declared
            ),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn prose_check_rejects_an_undeclared_root_page() {
        let mut roots: Vec<String> = ROOT_PAGES.iter().map(|(n, _)| n.to_string()).collect();
        roots.push("STRAY-GUIDE.md".to_string());
        assert_ne!(
            run_prose_check(&[PathBuf::from("docs/src/a.md")], &roots),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn stub_check_catches_both_spellings_and_leaves_real_pages_alone() {
        let relative = Path::new("docs/src/release/v1-readiness.md");
        let absolute = Path::new("docs/src/release/v1-non-goals.md");
        let inside = Path::new("docs/src/release/real.md");

        assert_ne!(
            run_stub_check(&[(
                relative,
                "# T\n\nSee [it](../../release/v1-readiness.md).\n"
            )]),
            ExitCode::SUCCESS
        );
        assert_ne!(
            run_stub_check(&[(
                absolute,
                "# T\n\nSee [it](https://github.com/nabbisen/fjell-os/blob/main/docs/release/v1-non-goals.md).\n"
            )]),
            ExitCode::SUCCESS
        );
        // a link to a sibling chapter is not a stub
        assert_eq!(
            run_stub_check(&[(inside, "# T\n\nSee [sibling](./other.md).\n")]),
            ExitCode::SUCCESS
        );
        // a long page that happens to link outside is not a stub
        let long = format!(
            "# T\n\n{}\n\n[out](../../release/x.md)\n",
            "body. ".repeat(400)
        );
        assert_eq!(run_stub_check(&[(inside, &long)]), ExitCode::SUCCESS);
    }

    #[test]
    fn unique_check_reports_both_kinds_of_collision() {
        let dirs = vec![PathBuf::from("docs/src/perf"), PathBuf::from("docs/perf")];
        assert_ne!(run_unique_check(&dirs, &[]), ExitCode::SUCCESS);

        let one = vec![PathBuf::from("docs/src/verification")];
        assert_ne!(
            run_unique_check(&one, &["verification".to_string()]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_unique_check(&one, &["crates".to_string()]),
            ExitCode::SUCCESS
        );
    }
}
