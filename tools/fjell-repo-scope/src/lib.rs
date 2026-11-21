//! Which files count as this repository -- answered once, from git.
//!
//! E-025 found a hand-maintained skip list (`target`, `.git`, `.git-exclude`) deciding what an audit
//! scanned; a scratch checkout under `.git-exclude/tmp/` doubled a count, and the list was fixed for
//! its two instances. `fjell-mmio-audit` then carried the same defect for roughly seventeen
//! milestones inside a release gate (E-125), reporting a count that was ~91% other people's trees,
//! because the fix had been applied to the tools and not to the *question*. This crate is the
//! question: every tool that needs "the repository's files" asks here, and the `repo-scope`
//! subcheck of `fjell-consistency-check` fails on a tool that walks the tree by itself.
//!
//! * [`Scope::Working`] -- tracked files plus untracked files `.gitignore` does not exclude
//!   (`git ls-files --cached --others --exclude-standard`). For auditing *code*: uncommitted work
//!   is still audited, scratch checkouts and `target/` (both ignored) are not.
//! * [`Scope::Tracked`] -- tracked files only. For a claim about what is *shipped*.
//!
//! There is deliberately **no name list in this crate**. Anything a caller excludes beyond the scope
//! (the mmio audit skips its own directory, because its pattern constants would otherwise be read as
//! access sites) it excludes itself, by name, in view, and says why.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Tracked files plus untracked files that are not ignored.
    Working,
    /// Tracked files only.
    Tracked,
}

impl Scope {
    fn args(self) -> &'static [&'static str] {
        match self {
            Scope::Working => &["--cached", "--others", "--exclude-standard"],
            Scope::Tracked => &[],
        }
    }
}

/// Every file in `scope` under `root`, as paths joined onto `root`, sorted. Fails (rather than
/// returning an empty or partial list) if `root` is not a git repository: a scope it could not
/// derive is not a scope to fall back from.
pub fn files(root: &Path, scope: Scope) -> io::Result<Vec<PathBuf>> {
    let root_str = root.to_string_lossy().into_owned();
    let out = Command::new("git")
        .args(["-C", &root_str, "ls-files", "-z"])
        .args(scope.args())
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "git ls-files failed under {} (not a git repository?): {}",
            root.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(parse_nul_separated(&out.stdout, root))
}

/// `files`, keeping only those with extension `ext` (no dot).
pub fn files_with_extension(root: &Path, scope: Scope, ext: &str) -> io::Result<Vec<PathBuf>> {
    Ok(files(root, scope)?
        .into_iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ext))
        .collect())
}

fn parse_nul_separated(stdout: &[u8], root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|rel| root.join(String::from_utf8_lossy(rel).as_ref()))
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git(dir: &Path, args: &[&str]) {
        let st = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("git runs");
        assert!(st.success(), "git {args:?}");
    }

    /// A throwaway repository: one tracked file, one untracked-but-not-ignored, one ignored
    /// directory (a scratch checkout), one ignored file.
    fn repo(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fjell-repo-scope-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("src")).unwrap();
        fs::create_dir_all(d.join("scratch/deep")).unwrap();
        git(&d, &["init", "-q"]);
        fs::write(d.join(".gitignore"), "/scratch/\n*.log\n").unwrap();
        fs::write(d.join("src/tracked.rs"), "").unwrap();
        fs::write(d.join("src/untracked.rs"), "").unwrap();
        fs::write(d.join("scratch/deep/other.rs"), "").unwrap();
        fs::write(d.join("src/noise.log"), "").unwrap();
        git(&d, &["add", ".gitignore", "src/tracked.rs"]);
        d
    }

    fn rel(root: &Path, v: Vec<PathBuf>) -> Vec<String> {
        v.iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn working_scope_sees_untracked_work_and_not_an_ignored_scratch_tree() {
        let d = repo("working");
        let got = rel(&d, files(&d, Scope::Working).unwrap());
        assert!(got.contains(&"src/tracked.rs".to_string()), "{got:?}");
        assert!(
            got.contains(&"src/untracked.rs".to_string()),
            "uncommitted work is audited: {got:?}"
        );
        assert!(
            !got.iter().any(|p| p.starts_with("scratch/")),
            "an ignored scratch tree is not the repository: {got:?}"
        );
        assert!(!got.contains(&"src/noise.log".to_string()), "{got:?}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn tracked_scope_sees_only_what_is_shipped() {
        let d = repo("tracked");
        let got = rel(&d, files(&d, Scope::Tracked).unwrap());
        assert_eq!(got, [".gitignore", "src/tracked.rs"]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn extension_filter_keeps_the_scope() {
        let d = repo("ext");
        let got = rel(&d, files_with_extension(&d, Scope::Working, "rs").unwrap());
        assert_eq!(
            got,
            ["src/tracked.rs", "src/untracked.rs"],
            "scratch/deep/other.rs is out of scope"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_directory_that_is_not_a_repository_is_an_error_not_an_empty_scope() {
        let d =
            std::env::temp_dir().join(format!("fjell-repo-scope-norepo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        // Control: ask git the same question, from the same directory, the same way `files` does.
        let direct = Command::new("git")
            .arg("-C")
            .arg(&d)
            .args(["ls-files", "-z"])
            .output()
            .unwrap();
        if direct.status.success() {
            // The temp directory sits inside a git repository on this machine: there is no
            // "not a repository" to show. Say so rather than pass silently.
            eprintln!("skipped: {} is inside a git repository", d.display());
        } else {
            let e =
                files(&d, Scope::Working).expect_err("git refused, so the scope must be an error");
            assert!(e.to_string().contains("not a git repository"), "{e}");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn nul_separated_output_is_parsed_without_empties() {
        let v = parse_nul_separated(b"b.rs\0a.rs\0\0", Path::new("/r"));
        assert_eq!(v, [PathBuf::from("/r/a.rs"), PathBuf::from("/r/b.rs")]);
    }
}
