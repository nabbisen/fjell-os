//! RFC-0.48-003: **`release-preflight`** and **`release-notes`** -- what judges a release tag before a GitHub Release is
//! created from it, and what the Release says.
//!
//! The owner's request (2026-11-23) was that a Release be created automatically when a release tag is pushed, **with the
//! trigger designed and managed carefully**. The trigger *is* the deliverable, and **every precondition refuses**: a Release
//! is outward-facing, and deleting one leaves the tag behind. The preconditions live **here, as pure functions over a
//! [`Facts`] struct**, not as shell in a workflow, so each is shown red locally and the owner can run the whole thing on a
//! tag *before* pushing it (`cargo xtask release-preflight 0.48.0 --dry-run`); a precondition written as inline `bash` could be
//! shown red only by pushing a bad tag.
//!
//! | # | Precondition | Read from |
//! |---|---|---|
//! | 0 | the tag is a bare `MAJOR.MINOR.PATCH` | the name |
//! | 1 | tag name == workspace version **at the tagged commit** | the **tag's** tree |
//! | 2 | the tagged commit is an ancestor of `main` | `git merge-base --is-ancestor` |
//! | 3 | `CHANGELOG.md` has a non-empty `## [<v>] — ` section | the **tag's** tree |
//! | 4 | `releases/<v>.md` exists and is non-empty | the **tag's** tree |
//! | 5 | **withdrawn** (see below) -- the number is left so the others keep the numbers the cycle document and the workflow's output use | -- |
//! | 6 | no Release already exists for the tag | `gh release view` |
//! | 7 | CI on the tagged commit concluded `success` (bounded wait; a timeout **fails**) | `gh run list` |
//! | 8 | no `v`-prefixed tag exists | `git tag -l 'v*'` |
//!
//! **Precondition 5 is withdrawn, and the reason is the architect's.** It asked that the tag be annotated, signed and verifying
//! against a committed key. The owner asked for a GitHub Release with notes and **never asked for signature verification**; the
//! check was the architect's invention in the 0.48 scope, and the committed key existed only to anchor it. It caught an
//! accidentally unsigned or lightweight tag and nothing else (whoever can push a tag can push to `main` and change the key), and
//! it duplicated a step every cut already performs: `git tag -v`, with *"Good signature"* recorded in the release record
//! **before the tag is pushed**. **This code no longer verifies a tag's signature.** That assurance lives at the cut, in the
//! release record. (`gh release create --verify-tag` stays in the workflow; it is *not* a signature check -- it aborts if the tag
//! does not exist in the remote.) The number is left as a gap that explains itself rather than renumbering the others.
//!
//! **What an unobtainable fact means.** In a real run, a fact that could not be read (no network, no `gh`) is a **failure**,
//! never a pass. `--dry-run` marks the network facts (6, 7) *not checked*, says so, and still fails on everything it can read.

use fjell_consistency_check::release_records::{
    extract_changelog_section, is_bare_tag, workspace_version,
};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// How long the CI precondition waits for a run that has not finished, in minutes. A tunable the owner may change (also the
/// workflow's `RELEASE_CI_WAIT_MINUTES`); **a timeout fails, it never publishes**.
pub const DEFAULT_CI_WAIT_MINUTES: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ci {
    Success,
    /// A run finished and did not succeed (and none succeeded).
    Failed(String),
    /// Still running when the wait ended, or no run existed: **a timeout is a failure**.
    TimedOut(String),
    /// Could not be read.
    Unknown(String),
    NotChecked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exists {
    No,
    Yes,
    Unknown(String),
    NotChecked,
}

/// Everything the preconditions judge. `None` = could not be read (a failure in a real run).
#[derive(Debug, Clone)]
pub struct Facts {
    pub tag: String,
    pub workspace_version_at_tag: Option<String>,
    pub changelog_at_tag: Option<String>,
    pub record_at_tag: Option<String>,
    pub ancestor_of_main: Option<bool>,
    pub release_exists: Exists,
    pub ci: Ci,
    pub v_prefixed_tags: Vec<String>,
}

/// Judge `f`. Returns every failed precondition, each naming itself; empty = every precondition holds.
pub fn evaluate(f: &Facts) -> Vec<String> {
    let mut out = Vec::new();
    if !is_bare_tag(&f.tag) {
        out.push(format!(
            "0: `{}` is not a bare MAJOR.MINOR.PATCH tag (no `v`, no pre-release suffix)",
            f.tag
        ));
    }
    match &f.workspace_version_at_tag {
        None => {
            out.push("1: could not read Cargo.toml's workspace version at the tagged commit".into())
        }
        Some(v) if *v != f.tag => out.push(format!(
            "1: tag `{}` but the workspace version at that commit is `{v}`",
            f.tag
        )),
        _ => {}
    }
    match f.ancestor_of_main {
        None => out
            .push("2: could not determine whether the tagged commit is an ancestor of main".into()),
        Some(false) => out.push("2: the tagged commit is not an ancestor of main".into()),
        _ => {}
    }
    match &f.changelog_at_tag {
        None => out.push("3: could not read CHANGELOG.md at the tagged commit".into()),
        Some(c) => {
            if let Err(e) = extract_changelog_section(c, &f.tag) {
                out.push(format!("3: {e}"));
            }
        }
    }
    match &f.record_at_tag {
        None => out.push(format!(
            "4: releases/{}.md does not exist at the tagged commit",
            f.tag
        )),
        Some(r) if r.trim().is_empty() => out.push(format!("4: releases/{}.md is empty", f.tag)),
        _ => {}
    }
    // 5: withdrawn (see the module comment). The numbers on either side are kept.
    match &f.release_exists {
        Exists::No | Exists::NotChecked => {}
        Exists::Yes => out.push(format!("6: a Release already exists for `{}` -- a re-run must not duplicate or overwrite it", f.tag)),
        Exists::Unknown(e) => out.push(format!("6: could not tell whether a Release exists ({e}); refusing rather than risking a duplicate")),
    }
    match &f.ci {
        Ci::Success | Ci::NotChecked => {}
        Ci::Failed(e) => out.push(format!("7: CI on the tagged commit did not succeed: {e}")),
        Ci::TimedOut(e) => out.push(format!("7: CI on the tagged commit did not finish in the allowed wait: {e} (a timeout fails; it never publishes)")),
        Ci::Unknown(e) => out.push(format!("7: could not read CI for the tagged commit: {e}")),
    }
    if !f.v_prefixed_tags.is_empty() {
        out.push(format!(
            "8: `v`-prefixed tag(s) exist: {:?} -- the convention is bare tags, and a `v*` tag would silently never publish",
            f.v_prefixed_tags
        ));
    }
    out
}

/// What CI runs say: `lines` are `status conclusion` pairs from `gh run list`. Success if any run completed `success`; failure
/// if none did and some completed otherwise; pending if some are still running; **no runs at all is not success**.
pub fn decide_ci(lines: &[(String, String)]) -> Ci {
    if lines
        .iter()
        .any(|(s, c)| s == "completed" && c == "success")
    {
        return Ci::Success;
    }
    let running = lines.iter().any(|(s, _)| s != "completed");
    if running {
        return Ci::TimedOut("a run is still in progress".into());
    }
    if lines.is_empty() {
        return Ci::TimedOut("no `ci` run exists for that commit yet".into());
    }
    Ci::Failed(format!(
        "the run(s) concluded {:?}",
        lines.iter().map(|(_, c)| c.as_str()).collect::<Vec<_>>()
    ))
}

/// The Release body: the CHANGELOG section for exactly `version`, then one line linking the release record **at the tag**.
/// The record is linked, not copied: two drifting copies of one fact is the E-114 family.
pub fn release_notes(changelog: &str, version: &str, repo: &str) -> Result<String, String> {
    let section = extract_changelog_section(changelog, version)?;
    Ok(format!(
        "{section}\n\n---\n\nRelease record: https://github.com/{repo}/blob/{version}/releases/{version}.md\n"
    ))
}

// ── gathering (the part that touches git and gh) ──────────────────────────────────────────────────────────────────

struct Out {
    ok: bool,
    stdout: String,
    stderr: String,
}

fn run(cwd: &Path, prog: &str, args: &[&str], envs: &[(&str, &str)]) -> Out {
    let mut c = Command::new(prog);
    c.current_dir(cwd).args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    match c.output() {
        Ok(o) => Out {
            ok: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        },
        Err(e) => Out {
            ok: false,
            stdout: String::new(),
            stderr: format!("cannot run {prog}: {e}"),
        },
    }
}

fn git_show(cwd: &Path, rev: &str, path: &str) -> Option<String> {
    let o = run(cwd, "git", &["show", &format!("{rev}:{path}")], &[]);
    o.ok.then_some(o.stdout)
}

fn gh_ci(cwd: &Path, sha: &str, wait_minutes: u64) -> Ci {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_minutes * 60);
    loop {
        let o = run(
            cwd,
            "gh",
            &[
                "run",
                "list",
                "--workflow",
                "ci.yml",
                "--commit",
                sha,
                "--event",
                "push",
                "--json",
                "status,conclusion",
                "--limit",
                "20",
                "--jq",
                ".[] | \"\\(.status) \\(.conclusion)\"",
            ],
            &[],
        );
        if !o.ok {
            return Ci::Unknown(
                o.stderr
                    .lines()
                    .last()
                    .unwrap_or("gh run list failed")
                    .to_string(),
            );
        }
        let rows: Vec<(String, String)> = o
            .stdout
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.to_string(), it.next().unwrap_or("").to_string()))
            })
            .collect();
        match decide_ci(&rows) {
            Ci::TimedOut(why) => {
                if std::time::Instant::now() >= deadline {
                    return Ci::TimedOut(format!("{why}, after {wait_minutes} minute(s)"));
                }
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            other => return other,
        }
    }
}

/// Read every fact. `online` = ask GitHub (preconditions 6 and 7); offline they are `NotChecked`.
pub fn gather(cwd: &Path, tag: &str, main_ref: &str, online: bool, wait_minutes: u64) -> Facts {
    let tag_ref = format!("refs/tags/{tag}");
    let commit = {
        let o = run(
            cwd,
            "git",
            &[
                "rev-parse",
                "--verify",
                "-q",
                &format!("{tag_ref}^{{commit}}"),
            ],
            &[],
        );
        o.ok.then(|| o.stdout.trim().to_string())
    };
    let at = |p: &str| commit.as_deref().and_then(|c| git_show(cwd, c, p));
    let workspace_version_at_tag = at("Cargo.toml").and_then(|t| workspace_version(&t));
    let ancestor_of_main = commit.as_deref().and_then(|c| {
        let o = run(
            cwd,
            "git",
            &["merge-base", "--is-ancestor", c, main_ref],
            &[],
        );
        match o.stderr.is_empty() {
            true => Some(o.ok),
            false => None,
        }
    });
    let v_tags = run(cwd, "git", &["tag", "-l", "v*"], &[]);
    let (release_exists, ci) = if online {
        let r = run(
            cwd,
            "gh",
            &["release", "view", tag, "--json", "tagName"],
            &[],
        );
        let exists = if r.ok {
            Exists::Yes
        } else if r.stderr.to_lowercase().contains("release not found") {
            Exists::No
        } else {
            Exists::Unknown(
                r.stderr
                    .lines()
                    .last()
                    .unwrap_or("gh release view failed")
                    .to_string(),
            )
        };
        let ci = match commit.as_deref() {
            Some(c) => gh_ci(cwd, c, wait_minutes),
            None => Ci::Unknown("no tagged commit".into()),
        };
        (exists, ci)
    } else {
        (Exists::NotChecked, Ci::NotChecked)
    };
    Facts {
        tag: tag.to_string(),
        workspace_version_at_tag,
        changelog_at_tag: at("CHANGELOG.md"),
        record_at_tag: at(&format!("releases/{tag}.md")),
        ancestor_of_main,
        release_exists,
        ci,
        v_prefixed_tags: v_tags.stdout.lines().map(str::to_string).collect(),
    }
}

fn repo_slug() -> Option<String> {
    if let Ok(r) = std::env::var("GITHUB_REPOSITORY") {
        return Some(r);
    }
    let o = run(
        Path::new("."),
        "gh",
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ],
        &[],
    );
    o.ok.then(|| o.stdout.trim().to_string())
}

/// `release-preflight <tag> [--dry-run] [--main <ref>]`
pub fn cmd_release_preflight(args: &[String]) -> ExitCode {
    let Some(tag) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("usage: cargo xtask release-preflight <tag> [--dry-run] [--main <ref>]");
        return ExitCode::FAILURE;
    };
    let dry = args.iter().any(|a| a == "--dry-run");
    let main_ref = args
        .windows(2)
        .find(|w| w[0] == "--main")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "origin/main".to_string());
    let wait = std::env::var("RELEASE_CI_WAIT_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CI_WAIT_MINUTES);
    let facts = gather(&PathBuf::from("."), tag, &main_ref, !dry, wait);
    let problems = evaluate(&facts);
    if problems.is_empty() {
        println!(
            "release-preflight: PASS for {tag} ({}): bare tag, workspace version, ancestor of {main_ref}, CHANGELOG section, release record{}",
            if dry { "dry run: no network" } else { "online" },
            if dry {
                "; NOT checked in a dry run: that no Release exists, and that CI succeeded"
            } else {
                ", no existing Release, CI success, no v-prefixed tag"
            }
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("release-preflight: FAIL for {tag} -- nothing will be published");
        for p in &problems {
            eprintln!("  precondition {p}");
        }
        ExitCode::FAILURE
    }
}

/// `release-notes <version>` -- the Release body on stdout, from the working tree's CHANGELOG (the workflow checks out the tag).
pub fn cmd_release_notes(args: &[String]) -> ExitCode {
    let Some(version) = args.first() else {
        eprintln!("usage: cargo xtask release-notes <version>");
        return ExitCode::FAILURE;
    };
    let Some(repo) = repo_slug() else {
        eprintln!("release-notes: cannot determine the repository (set GITHUB_REPOSITORY)");
        return ExitCode::FAILURE;
    };
    let changelog = match std::fs::read_to_string("CHANGELOG.md") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("release-notes: cannot read CHANGELOG.md: {e}");
            return ExitCode::FAILURE;
        }
    };
    match release_notes(&changelog, version, &repo) {
        Ok(n) => {
            print!("{n}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("release-notes: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Facts {
        Facts {
            tag: "0.48.0".into(),
            workspace_version_at_tag: Some("0.48.0".into()),
            changelog_at_tag: Some("## [0.48.0] — d — s\n\nBody.\n".into()),
            record_at_tag: Some("record".into()),
            ancestor_of_main: Some(true),
            release_exists: Exists::No,
            ci: Ci::Success,
            v_prefixed_tags: vec![],
        }
    }
    fn fails(f: &Facts) -> Vec<String> {
        evaluate(f)
    }

    #[test]
    fn the_control_passes_every_precondition() {
        assert!(fails(&good()).is_empty(), "{:?}", fails(&good()));
    }

    /// One mutation per precondition: each must produce *its own* numbered failure and no other.
    #[test]
    fn each_precondition_fails_alone_and_by_its_own_number() {
        let cases: Vec<(&str, Box<dyn Fn(&mut Facts)>)> = vec![
            ("0", Box::new(|f| f.tag = "v0.48.0".into())),
            (
                "1",
                Box::new(|f| f.workspace_version_at_tag = Some("0.47.0".into())),
            ),
            ("2", Box::new(|f| f.ancestor_of_main = Some(false))),
            (
                "3",
                Box::new(|f| f.changelog_at_tag = Some("## [0.47.0] — d — s\n\nx\n".into())),
            ),
            (
                "3",
                Box::new(|f| f.changelog_at_tag = Some("## [0.48.0] — d — s\n\n".into())),
            ),
            ("4", Box::new(|f| f.record_at_tag = None)),
            ("4", Box::new(|f| f.record_at_tag = Some("  \n".into()))),
            ("6", Box::new(|f| f.release_exists = Exists::Yes)),
            (
                "6",
                Box::new(|f| f.release_exists = Exists::Unknown("no network".into())),
            ),
            ("7", Box::new(|f| f.ci = Ci::Failed("failure".into()))),
            ("7", Box::new(|f| f.ci = Ci::TimedOut("in progress".into()))),
            ("7", Box::new(|f| f.ci = Ci::Unknown("no gh".into()))),
            (
                "8",
                Box::new(|f| f.v_prefixed_tags = vec!["v0.47.0".into()]),
            ),
        ];
        for (n, m) in cases {
            let mut f = good();
            m(&mut f);
            let e = fails(&f);
            assert!(
                !e.is_empty(),
                "precondition {n}: a mutation did not fail anything"
            );
            assert!(
                e.iter().all(|l| l.starts_with(&format!("{n}:"))) || n == "0",
                "{n}: {e:?}"
            );
            assert!(
                e.iter().any(|l| l.starts_with(&format!("{n}:"))),
                "{n} not named: {e:?}"
            );
        }
    }

    #[test]
    fn an_unreadable_fact_is_a_failure_not_a_pass() {
        for m in [
            (|f: &mut Facts| f.workspace_version_at_tag = None) as fn(&mut Facts),
            |f| f.changelog_at_tag = None,
            |f| f.ancestor_of_main = None,
        ] {
            let mut f = good();
            m(&mut f);
            assert!(!fails(&f).is_empty());
        }
    }

    #[test]
    fn a_dry_run_marks_the_network_facts_not_checked_and_still_fails_on_what_it_can_read() {
        let mut f = good();
        f.release_exists = Exists::NotChecked;
        f.ci = Ci::NotChecked;
        assert!(fails(&f).is_empty());
        f.record_at_tag = None;
        assert_eq!(fails(&f).len(), 1);
    }

    #[test]
    fn ci_is_success_only_on_a_completed_success_and_a_wait_that_ends_fails() {
        let r = |s: &str, c: &str| (s.to_string(), c.to_string());
        assert_eq!(decide_ci(&[r("completed", "success")]), Ci::Success);
        assert_eq!(
            decide_ci(&[r("completed", "failure"), r("completed", "success")]),
            Ci::Success,
            "a re-run that succeeded counts"
        );
        assert!(matches!(
            decide_ci(&[r("in_progress", "")]),
            Ci::TimedOut(_)
        ));
        assert!(matches!(
            decide_ci(&[r("completed", "failure")]),
            Ci::Failed(_)
        ));
        assert!(matches!(
            decide_ci(&[r("completed", "cancelled")]),
            Ci::Failed(_)
        ));
        assert!(
            matches!(decide_ci(&[]), Ci::TimedOut(_)),
            "no run at all is not success"
        );
    }

    #[test]
    fn the_notes_are_the_sections_text_and_a_link_to_the_record_at_the_tag() {
        let cl = "## [0.48.0] — d — s\n\nWhat shipped.\n\n## [0.47.0] — d — s\n\nOld.\n";
        let n = release_notes(cl, "0.48.0", "o/r").unwrap();
        assert!(n.starts_with("What shipped.\n"), "{n}");
        assert!(n.contains("https://github.com/o/r/blob/0.48.0/releases/0.48.0.md"));
        assert!(!n.contains("Old."), "only that version's section");
        assert!(release_notes(cl, "0.49.0", "o/r").is_err());
    }

    #[test]
    fn the_real_changelog_yields_notes_for_every_record_from_the_floor() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cl = std::fs::read_to_string(root.join("CHANGELOG.md")).unwrap();
        let mut n = 0;
        for e in std::fs::read_dir(root.join("releases")).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(v) = name.strip_suffix(".md") {
                if is_bare_tag(v) {
                    let notes = release_notes(&cl, v, "o/r");
                    assert!(notes.is_ok(), "{v}: {notes:?}");
                    n += 1;
                }
            }
        }
        assert!(n >= 20, "expected the real records, found {n}");
    }

    // ── gathering, against a scratch repository ────────────────────────────────────────────────────────────────────────
    //
    // These need `git`. **A missing `git` fails the test**: a skipped gathering test would read as a pass.

    struct Scratch {
        dir: PathBuf,
    }

    fn sh(cwd: &Path, args: &[&str]) -> Out {
        run(
            cwd,
            "git",
            args,
            &[
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_CONFIG_SYSTEM", "/dev/null"),
            ],
        )
    }

    fn scratch(name: &str) -> Scratch {
        let base = std::env::temp_dir().join(format!(
            "fjell-preflight-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("repo");
        std::fs::create_dir_all(&dir).unwrap();
        Scratch { dir }
    }

    impl Scratch {
        fn git(&self, args: &[&str]) -> Out {
            sh(&self.dir, args)
        }
        fn write(&self, path: &str, text: &str) {
            let p = self.dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        fn commit(&self, msg: &str) -> String {
            assert!(self.git(&["add", "-A"]).ok);
            let o = self.git(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "commit",
                "-q",
                "-m",
                msg,
            ]);
            assert!(o.ok, "{}", o.stderr);
            self.git(&["rev-parse", "HEAD"]).stdout.trim().to_string()
        }
        /// A repository on `main` carrying the project's release shape for version `0.1.0`.
        fn release_repo(&self) {
            assert!(self.git(&["init", "-q", "-b", "main"]).ok);
            self.write("Cargo.toml", "[workspace.package]\nversion = \"0.1.0\"\n");
            self.write("CHANGELOG.md", "## [0.1.0] — d — first\n\nShipped.\n");
            self.write("releases/0.1.0.md", "record\n");
            self.commit("release");
        }
        fn tag(&self, tag: &str) {
            let o = self.git(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "tag",
                "-a",
                tag,
                "-m",
                "m",
            ]);
            assert!(o.ok, "{}", o.stderr);
        }
        fn facts(&self, tag: &str) -> Facts {
            gather(&self.dir, tag, "main", false, 0)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.dir.parent().unwrap());
        }
    }

    #[test]
    fn a_tag_on_main_with_every_fact_in_place_passes() {
        let s = scratch("good");
        s.release_repo();
        s.tag("0.1.0");
        let f = s.facts("0.1.0");
        assert!(evaluate(&f).is_empty(), "{:?}", evaluate(&f));
    }

    #[test]
    fn each_tag_fact_fails_in_a_real_repository() {
        let s = scratch("bad");
        s.release_repo();
        // tag name != version
        s.tag("0.2.0");
        assert!(
            evaluate(&s.facts("0.2.0"))
                .iter()
                .any(|l| l.starts_with("1:"))
        );
        // a v-prefixed tag exists
        s.tag("0.1.0");
        assert!(s.git(&["tag", "v0.1.0"]).ok);
        assert!(
            evaluate(&s.facts("0.1.0"))
                .iter()
                .any(|l| l.starts_with("8:"))
        );
    }

    #[test]
    fn a_tag_off_main_fails_precondition_2_in_a_real_repository() {
        let s = scratch("main");
        s.release_repo();
        // a commit on a side branch, tagged: not an ancestor of main
        assert!(s.git(&["checkout", "-q", "-b", "side"]).ok);
        s.write("side.txt", "x");
        s.commit("side");
        s.tag("0.1.0");
        assert!(s.git(&["checkout", "-q", "main"]).ok);
        let f = s.facts("0.1.0");
        assert_eq!(f.ancestor_of_main, Some(false));
        assert!(evaluate(&f).iter().any(|l| l.starts_with("2:")));
    }

    /// The dry run the owner runs **before** pushing: the tag does not exist yet. It must fail on exactly the preconditions that
    /// need the tag's tree or commit (1 2 3 4), by number, and on nothing else.
    #[test]
    fn a_tag_that_does_not_exist_fails_only_the_preconditions_that_need_it() {
        let s = scratch("absent");
        s.release_repo();
        let f = s.facts("0.1.0");
        let mut nums: Vec<String> = evaluate(&f)
            .iter()
            .map(|l| l.split(':').next().unwrap().to_string())
            .collect();
        nums.sort();
        assert_eq!(nums, ["1", "2", "3", "4"], "{:?}", evaluate(&f));
    }
}
