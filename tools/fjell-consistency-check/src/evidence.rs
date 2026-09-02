//! RFC-0.27-004: the `evidence` subcheck (Gate 12's 10th subcheck).
//!
//! `tests/evidence/` is the one place this project deliberately commits a
//! QEMU serial log (R1) — everything else under `tests/qemu/artifacts/` and
//! `tests/runs/` stays gitignored and overwritable exactly as before.
//! Direction A and B, same shape as `errata-tracking`'s two directions:
//!
//!   - **A** — every citation of a `tests/evidence/...` path, in any
//!     tracked `.md` file, resolves to a file that exists and carries
//!     well-formed provenance (a `.provenance.txt` sidecar with `run_id`,
//!     `profile`, `commit_sha`, `command`, `instrumented` all present),
//!     whose `commit_sha` is a real ancestor of `HEAD`.
//!   - **B** — every `.log` file under `tests/evidence/` is cited by at
//!     least one tracked `.md` file. An uncited file is either a document
//!     deleted without its evidence, or a promotion made for no reason —
//!     both are drift, and B is what stops this directory becoming the
//!     landfill `tests/runs/` already is (handoff §3).
//!
//! ## The one place this tool shells out to git, and why
//!
//! Every other subcheck in this crate reads only committed files — see
//! `errata_tracking`'s design note on the same question — so behaviour is
//! identical in a full clone, a shallow clone, or an exported tarball.
//! "Is this exact commit object an ancestor of `HEAD`" has no such
//! git-free answer: unlike "has this version shipped" (which
//! `errata_tracking` answers from `docs/release/records/*.md` instead),
//! there is no committed file recording every commit that was ever `HEAD`.
//! This check shells out to `git cat-file` and `git merge-base
//! --is-ancestor` — a deliberate, narrow exception, disclosed here rather
//! than silently breaking the stated convention.
//!
//! **Known limitation, disclosed rather than silently wrong:** on a
//! **shallow clone**, this can report a genuinely valid historical sha as
//! unverifiable, because the commit object itself was never fetched. That
//! is a false FAIL, not a false PASS — this check never reports a
//! fabricated sha as valid, but it can refuse to vouch for a real one on
//! an incomplete clone. See the handoff's own instruction (§R5.4) to say
//! so in writing rather than drop the check.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const EVIDENCE_DIR: &str = "tests/evidence";
const EXCLUDE_DIRS: &[&str] = &["target", ".git", ".git-exclude"];
const REQUIRED_PROVENANCE_FIELDS: &[&str] =
    &["run_id", "profile", "commit_sha", "command", "instrumented"];

pub fn check() -> ExitCode {
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    if let Err(e) = walk_markdown(Path::new("."), &mut files) {
        eprintln!("consistency-check: cannot walk repository tree: {e}");
        return ExitCode::FAILURE;
    }
    let docs: Vec<(&Path, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_path(), c.as_str()))
        .collect();

    let mut evidence_logs = Vec::new();
    if let Err(e) = walk_evidence_logs(Path::new(EVIDENCE_DIR), &mut evidence_logs) {
        eprintln!("consistency-check: cannot walk {EVIDENCE_DIR}: {e}");
        return ExitCode::FAILURE;
    }

    run_check(&docs, &evidence_logs, EVIDENCE_DIR, check_ancestor_real)
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

/// Every `.log` file under `tests/evidence/`, repo-root-relative, forward
/// slashed. Sidecars (`.provenance.txt`) and `README.md` are not evidence
/// files themselves and are not subject to direction B.
fn walk_evidence_logs(dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_evidence_logs(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "log") {
            out.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Same bracket-scan `[label](url)` extractor as `doc_links::extract_links`
/// — duplicated per this crate's own established convention (see
/// `standards_mapping::extract_evidence_paths`'s doc-comment for the same
/// call made there).
fn extract_links(src: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut i = 0;
    while i < src.len() {
        if src.as_bytes()[i] == b'[' {
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

#[derive(Debug, PartialEq, Eq)]
enum AncestorResult {
    Ancestor,
    NotAncestor,
    Inconclusive(String),
}

/// The real, git-shelling implementation — kept behind a function pointer
/// so `run_check` can be tested with a synthetic answer, the same
/// separation `syscall_surface` and friends use to keep `run_check` pure.
fn check_ancestor_real(sha: &str) -> AncestorResult {
    if sha.is_empty() {
        return AncestorResult::Inconclusive("empty commit_sha".into());
    }
    match Command::new("git")
        .args(["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .output()
    {
        Ok(o) if o.status.success() => {}
        Ok(_) => {
            return AncestorResult::Inconclusive(
                "commit object not present locally — possibly a shallow clone".into(),
            );
        }
        Err(e) => return AncestorResult::Inconclusive(format!("git unavailable: {e}")),
    }
    match Command::new("git")
        .args(["merge-base", "--is-ancestor", sha, "HEAD"])
        .status()
    {
        Ok(s) if s.success() => AncestorResult::Ancestor,
        Ok(_) => AncestorResult::NotAncestor,
        Err(e) => AncestorResult::Inconclusive(format!("git unavailable: {e}")),
    }
}

fn parse_provenance(src: &str) -> std::collections::BTreeMap<String, String> {
    src.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// Core comparison, pure in its inputs (`ancestor_check` is injected so
/// tests never shell out to git for a synthetic sha; `evidence_prefix` lets
/// tests sandbox against a temp directory instead of the real
/// `tests/evidence`). `docs` is every tracked markdown file; `evidence_logs`
/// is every real `.log` path found under `evidence_prefix`.
fn run_check(
    docs: &[(&Path, &str)],
    evidence_logs: &[String],
    evidence_prefix: &str,
    ancestor_check: fn(&str) -> AncestorResult,
) -> ExitCode {
    let mut problems: Vec<String> = Vec::new();
    let mut cited: BTreeSet<String> = BTreeSet::new();
    let mut citations_checked = 0usize;
    let prefix_slash = format!("{evidence_prefix}/");

    // Direction A.
    for (path, content) in docs {
        let dir = path.parent().unwrap_or(Path::new("."));
        for link in extract_links(content) {
            let target = link.split('#').next().unwrap_or(&link);
            let resolved = normalise(&dir.join(target));
            let resolved_str = resolved.to_string_lossy().replace('\\', "/");
            if !resolved_str.starts_with(&prefix_slash) {
                continue;
            }
            let file_label = path.to_string_lossy().replace('\\', "/");
            let file_label = file_label.strip_prefix("./").unwrap_or(&file_label);
            cited.insert(resolved_str.clone());
            citations_checked += 1;

            if !resolved.exists() {
                problems.push(format!(
                    "{file_label}: cites {resolved_str:?}, which does not exist"
                ));
                continue;
            }
            if !resolved_str.ends_with(".log") {
                continue; // a citation of the directory itself, or a provenance file directly
            }
            let prov_path = PathBuf::from(
                resolved_str.trim_end_matches(".log").to_string() + ".provenance.txt",
            );
            let Ok(prov_src) = fs::read_to_string(&prov_path) else {
                problems.push(format!(
                    "{file_label}: cites {resolved_str:?}, which has no provenance sidecar ({})",
                    prov_path.display()
                ));
                continue;
            };
            let fields = parse_provenance(&prov_src);
            let missing: Vec<&str> = REQUIRED_PROVENANCE_FIELDS
                .iter()
                .filter(|f| !fields.contains_key(**f))
                .copied()
                .collect();
            if !missing.is_empty() {
                problems.push(format!(
                    "{file_label}: {resolved_str:?}'s provenance is missing field(s) {missing:?}"
                ));
                continue;
            }
            let sha = &fields["commit_sha"];
            match ancestor_check(sha) {
                AncestorResult::Ancestor => {}
                AncestorResult::NotAncestor => {
                    problems.push(format!(
                        "{file_label}: {resolved_str:?}'s provenance names commit {sha:?}, which is NOT an ancestor of HEAD"
                    ));
                }
                AncestorResult::Inconclusive(reason) => {
                    problems.push(format!(
                        "{file_label}: {resolved_str:?}'s provenance commit {sha:?} could not be verified as an ancestor of HEAD ({reason})"
                    ));
                }
            }
        }
    }

    // Direction B.
    for log in evidence_logs {
        if !cited.contains(log) {
            problems.push(format!(
                "{log}: orphaned — not cited by any tracked document"
            ));
        }
    }

    if problems.is_empty() {
        println!(
            "evidence: PASS ({citations_checked} citation(s) checked, {} evidence file(s), 0 orphans)",
            evidence_logs.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("evidence: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_ancestor(_: &str) -> AncestorResult {
        AncestorResult::Ancestor
    }
    fn always_not_ancestor(_: &str) -> AncestorResult {
        AncestorResult::NotAncestor
    }

    fn write_evidence(dir: &Path, rfc: &str, name: &str, log: &str, provenance: Option<&str>) {
        let d = dir.join(rfc);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(format!("{name}.log")), log).unwrap();
        if let Some(p) = provenance {
            fs::write(d.join(format!("{name}.provenance.txt")), p).unwrap();
        }
    }

    fn full_provenance() -> String {
        "run_id = 20260101-000000\nprofile = smoke-m8\ncommit_sha = deadbeef\ncommand = cargo xtask qemu-test m8\ninstrumented = none\n".to_string()
    }

    /// Isolated temp dir per test so `tests/evidence/` on the real
    /// checkout is never touched by the test suite itself.
    fn tmp_evidence_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fjell-evidence-subcheck-test-{tag}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn well_formed_citation_with_valid_provenance_passes() {
        let dir = tmp_evidence_dir("wellformed");
        write_evidence(&dir, "RFC-1", "x", "log body", Some(&full_provenance()));
        let evidence_dir_str = dir.to_string_lossy().replace('\\', "/");
        let doc_content = format!("[x]({evidence_dir_str}/RFC-1/x.log)");
        let docs = [(Path::new("some/doc.md"), doc_content.as_str())];
        let logs = vec![format!("{evidence_dir_str}/RFC-1/x.log")];
        assert_eq!(
            run_check(&docs, &logs, &evidence_dir_str, always_ancestor),
            ExitCode::SUCCESS
        );
    }

    /// Required demonstration 1: a citation to a missing evidence file.
    #[test]
    fn citation_to_missing_file_fails_naming_both() {
        let dir = tmp_evidence_dir("missingfile");
        let evidence_dir_str = dir.to_string_lossy().replace('\\', "/");
        let doc_content = format!("[x]({evidence_dir_str}/RFC-1/gone.log)");
        let docs = [(Path::new("some/doc.md"), doc_content.as_str())];
        let result = run_check(&docs, &[], &evidence_dir_str, always_ancestor);
        assert_eq!(result, ExitCode::FAILURE);
    }

    /// Required demonstration 2: an evidence file with no provenance.
    #[test]
    fn evidence_file_with_no_provenance_fails() {
        let dir = tmp_evidence_dir("noprov");
        write_evidence(&dir, "RFC-1", "x", "log body", None);
        let evidence_dir_str = dir.to_string_lossy().replace('\\', "/");
        let doc_content = format!("[x]({evidence_dir_str}/RFC-1/x.log)");
        let docs = [(Path::new("some/doc.md"), doc_content.as_str())];
        let logs = vec![format!("{evidence_dir_str}/RFC-1/x.log")];
        assert_eq!(
            run_check(&docs, &logs, &evidence_dir_str, always_ancestor),
            ExitCode::FAILURE
        );
    }

    /// Required demonstration 3: an orphan evidence file.
    #[test]
    fn orphan_evidence_file_fails() {
        let dir = tmp_evidence_dir("orphan");
        write_evidence(&dir, "RFC-1", "x", "log body", Some(&full_provenance()));
        let evidence_dir_str = dir.to_string_lossy().replace('\\', "/");
        let logs = vec![format!("{evidence_dir_str}/RFC-1/x.log")];
        // No document cites it.
        let docs: [(&Path, &str); 0] = [];
        assert_eq!(
            run_check(&docs, &logs, &evidence_dir_str, always_ancestor),
            ExitCode::FAILURE
        );
    }

    /// Required demonstration 4 — the one that matters: provenance naming
    /// a commit sha that is not an ancestor of HEAD.
    #[test]
    fn provenance_naming_a_non_ancestor_commit_fails() {
        let dir = tmp_evidence_dir("nonancestor");
        write_evidence(&dir, "RFC-1", "x", "log body", Some(&full_provenance()));
        let evidence_dir_str = dir.to_string_lossy().replace('\\', "/");
        let doc_content = format!("[x]({evidence_dir_str}/RFC-1/x.log)");
        let docs = [(Path::new("some/doc.md"), doc_content.as_str())];
        let logs = vec![format!("{evidence_dir_str}/RFC-1/x.log")];
        assert_eq!(
            run_check(&docs, &logs, &evidence_dir_str, always_not_ancestor),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn missing_provenance_field_fails_naming_it() {
        let dir = tmp_evidence_dir("missingfield2");
        write_evidence(
            &dir,
            "RFC-1",
            "x",
            "log body",
            Some("run_id = 1\nprofile = m8\n"),
        );
        let evidence_dir_str = dir.to_string_lossy().replace('\\', "/");
        let doc_content = format!("[x]({evidence_dir_str}/RFC-1/x.log)");
        let docs = [(Path::new("some/doc.md"), doc_content.as_str())];
        let logs = vec![format!("{evidence_dir_str}/RFC-1/x.log")];
        assert_eq!(
            run_check(&docs, &logs, &evidence_dir_str, always_ancestor),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn citation_outside_tests_evidence_is_ignored() {
        let docs = [(
            Path::new("some/doc.md"),
            "[x](../docs/security/threat-model-v1.md)",
        )];
        assert_eq!(
            run_check(&docs, &[], "tests/evidence", always_ancestor),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn extracts_links() {
        assert_eq!(
            extract_links("see [a](b/c.log) and [d](e.md)"),
            vec!["b/c.log".to_string(), "e.md".to_string()]
        );
    }
}
