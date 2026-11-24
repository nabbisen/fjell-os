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
//! | 5 | the tag is an **annotated** object and `git verify-tag` passes against the **committed** key, whose fingerprint equals the pinned one | the key and fingerprint from **`main`**, not the tag |
//! | 6 | no Release already exists for the tag | `gh release view` |
//! | 7 | CI on the tagged commit concluded `success` (bounded wait; a timeout **fails**) | `gh run list` |
//! | 8 | no `v`-prefixed tag exists | `git tag -l 'v*'` |
//!
//! **Why the key is read from `main` and not from the tag.** A tag whose own tree carried a key *and* the fingerprint pinning
//! it would verify against itself. The trust anchor is whatever the repository's default branch says, which changing takes a
//! reviewed commit -- the same shape as `TOOLCHAIN.lock` and `baseline-digests.txt`. **The owner commits the public key; this
//! code never generates, stores or touches key material**, and with no key committed precondition 5 *fails*, naming that.
//!
//! **What an unobtainable fact means.** In a real run, a fact that could not be read (no network, no `gh`) is a **failure**,
//! never a pass. `--dry-run` marks the network facts (6, 7) *not checked*, says so, and still fails on everything it can read.

use fjell_consistency_check::release_records::{
    extract_changelog_section, is_bare_tag, workspace_version,
};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

pub const KEY_PATH: &str = "docs/src/releasing/release-signing-key.asc";
pub const FINGERPRINT_PATH: &str = "docs/src/releasing/release-signing-key.fingerprint";
/// How long the CI precondition waits for a run that has not finished, in minutes. A tunable the owner may change (also the
/// workflow's `RELEASE_CI_WAIT_MINUTES`); **a timeout fails, it never publishes**.
pub const DEFAULT_CI_WAIT_MINUTES: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sig {
    Verified,
    /// No public key is committed on `main` (or no fingerprint): the owner's act, and the check refuses until it is done.
    NoKeyCommitted,
    /// The committed key's fingerprint is not the pinned one.
    FingerprintMismatch {
        pinned: String,
        found: String,
    },
    /// The tag carries no signature.
    NoSignature,
    /// A signature that does not verify (tampered, or the wrong key).
    Bad(String),
}

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
    pub object_kind: Option<String>,
    pub workspace_version_at_tag: Option<String>,
    pub changelog_at_tag: Option<String>,
    pub record_at_tag: Option<String>,
    pub ancestor_of_main: Option<bool>,
    pub sig: Sig,
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
    match f.object_kind.as_deref() {
        Some("tag") => {}
        Some(other) => out.push(format!("5: `{}` is a {other}, not an annotated tag object -- a lightweight tag carries no signature", f.tag)),
        None => out.push(format!("5: tag `{}` does not exist", f.tag)),
    }
    match &f.sig {
        Sig::Verified => {}
        Sig::NoKeyCommitted => out.push(format!(
            "5: no release signing key is committed on main ({KEY_PATH} and {FINGERPRINT_PATH}); the owner commits the *public* key and pins its fingerprint -- until then no tag can be verified"
        )),
        Sig::FingerprintMismatch { pinned, found } => out.push(format!("5: the committed key's fingerprint is {found}, but {FINGERPRINT_PATH} pins {pinned}")),
        Sig::NoSignature => out.push("5: the tag carries no signature".into()),
        Sig::Bad(why) => out.push(format!("5: the tag's signature does not verify against the committed key: {why}")),
    }
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

// ── gathering (the part that touches git, gpg and gh) ──────────────────────────────────────────────────────────────────

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

fn normalise_fpr(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase()
}

/// Verify `tag` against the key committed on `main_ref`. Uses a **throwaway keyring** that holds only that key.
pub fn verify_signature(cwd: &Path, tag: &str, main_ref: &str) -> Sig {
    let (Some(key), Some(pinned)) = (
        git_show(cwd, main_ref, KEY_PATH),
        git_show(cwd, main_ref, FINGERPRINT_PATH),
    ) else {
        return Sig::NoKeyCommitted;
    };
    let pinned = normalise_fpr(&pinned);
    // Unique per call, not per process and tag: two verifications of the same tag name in one process (the tests do) would
    // otherwise share, and delete, one another's keyring.
    static CALL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = CALL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let home = std::env::temp_dir().join(format!(
        "fjell-preflight-gnupg-{}-{n}-{}",
        std::process::id(),
        tag.replace('/', "_")
    ));
    let _ = std::fs::remove_dir_all(&home);
    if std::fs::create_dir_all(&home).is_err() {
        return Sig::Bad("cannot create a throwaway keyring".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700));
    }
    let keyfile = home.join("key.asc");
    let _ = std::fs::write(&keyfile, key);
    let h = home.to_string_lossy().to_string();
    let envs = [("GNUPGHOME", h.as_str())];
    let result = (|| {
        let imp = run(
            cwd,
            "gpg",
            &["--batch", "--import", &keyfile.to_string_lossy()],
            &envs,
        );
        if !imp.ok {
            return Sig::Bad(format!(
                "the committed key does not import: {}",
                imp.stderr.lines().last().unwrap_or("")
            ));
        }
        let list = run(
            cwd,
            "gpg",
            &["--batch", "--with-colons", "--fingerprint"],
            &envs,
        );
        // the primary key's fingerprint is the first `fpr` record after `pub`
        let mut primary = None;
        let mut seen_pub = false;
        for l in list.stdout.lines() {
            if l.starts_with("pub:") {
                seen_pub = true;
            } else if seen_pub && l.starts_with("fpr:") {
                primary = l.split(':').nth(9).map(normalise_fpr);
                break;
            }
        }
        let found = primary.unwrap_or_default();
        if found != pinned {
            return Sig::FingerprintMismatch {
                pinned: pinned.clone(),
                found,
            };
        }
        let v = run(
            cwd,
            "git",
            &["verify-tag", "--raw", &format!("refs/tags/{tag}")],
            &envs,
        );
        let status = v.stderr.clone() + &v.stdout;
        if v.ok
            && status.lines().any(|l| {
                l.starts_with("[GNUPG:] VALIDSIG")
                    && l.split_whitespace().nth(11).map(normalise_fpr).as_deref()
                        == Some(pinned.as_str())
            })
        {
            Sig::Verified
        } else if status.contains("no signature found") || (status.is_empty() && !v.ok) {
            Sig::NoSignature
        } else if status.contains("BADSIG") {
            Sig::Bad("BAD signature".into())
        } else if status.contains("NO_PUBKEY") || status.contains("ERRSIG") {
            Sig::Bad("signed by a key other than the committed one".into())
        } else {
            Sig::Bad(
                status
                    .lines()
                    .last()
                    .unwrap_or("verification failed")
                    .to_string(),
            )
        }
    })();
    let _ = std::fs::remove_dir_all(&home);
    result
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
    let kind = run(cwd, "git", &["cat-file", "-t", &tag_ref], &[]);
    let object_kind = kind.ok.then(|| kind.stdout.trim().to_string());
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
    let sig = if object_kind.as_deref() == Some("tag") {
        verify_signature(cwd, tag, main_ref)
    } else {
        Sig::NoSignature
    };
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
        object_kind,
        workspace_version_at_tag,
        changelog_at_tag: at("CHANGELOG.md"),
        record_at_tag: at(&format!("releases/{tag}.md")),
        ancestor_of_main,
        sig,
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
            "release-preflight: PASS for {tag} ({}): bare tag, workspace version, ancestor of {main_ref}, CHANGELOG section, release record, annotated tag verifying against the committed key{}",
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
            object_kind: Some("tag".into()),
            workspace_version_at_tag: Some("0.48.0".into()),
            changelog_at_tag: Some("## [0.48.0] — d — s\n\nBody.\n".into()),
            record_at_tag: Some("record".into()),
            ancestor_of_main: Some(true),
            sig: Sig::Verified,
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
            ("5", Box::new(|f| f.object_kind = Some("commit".into()))),
            ("5", Box::new(|f| f.sig = Sig::NoSignature)),
            ("5", Box::new(|f| f.sig = Sig::NoKeyCommitted)),
            ("5", Box::new(|f| f.sig = Sig::Bad("BAD signature".into()))),
            (
                "5",
                Box::new(|f| {
                    f.sig = Sig::FingerprintMismatch {
                        pinned: "AA".into(),
                        found: "BB".into(),
                    }
                }),
            ),
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
                e.iter().all(|l| l.starts_with(&format!("{n}:"))) || n == "0" || n == "5",
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
            |f| f.object_kind = None,
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

    // ── gathering, against a scratch repository and throwaway keys ─────────────────────────────────────────────────────
    //
    // These need `git` and `gpg`. **A missing `gpg` fails the test**: a skipped signature test would read as a pass.

    struct Scratch {
        dir: PathBuf,
        home: PathBuf,
    }

    fn sh(cwd: &Path, home: &Path, prog: &str, args: &[&str]) -> Out {
        run(
            cwd,
            prog,
            args,
            &[
                ("GNUPGHOME", &home.to_string_lossy()),
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_CONFIG_SYSTEM", "/dev/null"),
            ],
        )
    }

    fn genkey(home: &Path, uid: &str) -> String {
        let o = run(
            home,
            "gpg",
            &[
                "--batch",
                "--passphrase",
                "",
                "--quick-gen-key",
                uid,
                "ed25519",
                "sign",
                "never",
            ],
            &[("GNUPGHOME", &home.to_string_lossy())],
        );
        assert!(o.ok, "gpg is required for these tests: {}", o.stderr);
        let l = run(
            home,
            "gpg",
            &["--batch", "--with-colons", "--fingerprint", uid],
            &[("GNUPGHOME", &home.to_string_lossy())],
        );
        l.stdout
            .lines()
            .find(|l| l.starts_with("fpr:"))
            .unwrap()
            .split(':')
            .nth(9)
            .unwrap()
            .to_string()
    }

    fn scratch(name: &str) -> Scratch {
        let base = std::env::temp_dir().join(format!(
            "fjell-preflight-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("repo");
        let home = base.join("gnupg");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Scratch { dir, home }
    }

    impl Scratch {
        fn git(&self, args: &[&str]) -> Out {
            sh(&self.dir, &self.home, "git", args)
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
        /// A repository on `main` carrying the project's release shape for version `0.1.0`, plus the key and fingerprint.
        fn release_repo(&self, fpr: &str) {
            assert!(self.git(&["init", "-q", "-b", "main"]).ok);
            self.write("Cargo.toml", "[workspace.package]\nversion = \"0.1.0\"\n");
            self.write("CHANGELOG.md", "## [0.1.0] — d — first\n\nShipped.\n");
            self.write("releases/0.1.0.md", "record\n");
            let pubkey = run(
                &self.dir,
                "gpg",
                &["--batch", "--armor", "--export", fpr],
                &[("GNUPGHOME", &self.home.to_string_lossy())],
            );
            self.write(KEY_PATH, &pubkey.stdout);
            self.write(FINGERPRINT_PATH, &format!("{fpr}\n"));
            self.commit("release");
        }
        fn facts(&self, tag: &str) -> Facts {
            // `gather` uses the process environment for gpg; the temp keyring is made by `verify_signature` itself.
            gather(&self.dir, tag, "main", false, 0)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.dir.parent().unwrap());
        }
    }

    fn sign_tag(s: &Scratch, tag: &str, key: &str) {
        let o = s.git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            &format!("user.signingkey={key}"),
            "tag",
            "-s",
            tag,
            "-m",
            "m",
        ]);
        assert!(o.ok, "{}", o.stderr);
    }

    #[test]
    fn a_signed_annotated_tag_on_main_with_every_fact_in_place_passes() {
        let s = scratch("good");
        let fpr = genkey(&s.home, "owner <owner@example.invalid>");
        s.release_repo(&fpr);
        sign_tag(&s, "0.1.0", &fpr);
        let f = s.facts("0.1.0");
        assert_eq!(f.sig, Sig::Verified, "{f:?}");
        assert!(evaluate(&f).is_empty(), "{:?}", evaluate(&f));
    }

    #[test]
    fn each_tag_fact_fails_in_a_real_repository() {
        let s = scratch("bad");
        let fpr = genkey(&s.home, "owner <owner@example.invalid>");
        let other = genkey(&s.home, "intruder <intruder@example.invalid>");
        s.release_repo(&fpr);
        // an annotated but UNSIGNED tag
        assert!(
            s.git(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "tag",
                "-a",
                "0.1.0",
                "-m",
                "m"
            ])
            .ok
        );
        let f = s.facts("0.1.0");
        assert_eq!(f.sig, Sig::NoSignature, "{f:?}");
        assert!(
            evaluate(&f)
                .iter()
                .any(|l| l.starts_with("5:") && l.contains("no signature"))
        );
        // a LIGHTWEIGHT tag
        assert!(s.git(&["tag", "0.1.1"]).ok);
        let f = s.facts("0.1.1");
        assert_eq!(f.object_kind.as_deref(), Some("commit"));
        assert!(
            evaluate(&f)
                .iter()
                .any(|l| l.starts_with("5:") && l.contains("lightweight"))
        );
        // signed by the WRONG key
        sign_tag(&s, "0.1.2", &other);
        let f = s.facts("0.1.2");
        assert!(matches!(f.sig, Sig::Bad(_)), "{:?}", f.sig);
        // a TAMPERED tag: change one byte of a good signed tag's message and re-point the ref at the edited object
        sign_tag(&s, "0.1.3", &fpr);
        let raw = s.git(&["cat-file", "tag", "0.1.3"]).stdout;
        let tampered = raw.replacen("\n\nm\n", "\n\nM\n", 1);
        assert_ne!(raw, tampered);
        let tmp = s.dir.join("t.txt");
        std::fs::write(&tmp, tampered).unwrap();
        let h = s
            .git(&["hash-object", "-t", "tag", "-w", "t.txt"])
            .stdout
            .trim()
            .to_string();
        assert!(s.git(&["update-ref", "refs/tags/0.1.3", &h]).ok);
        let f = s.facts("0.1.3");
        assert!(
            matches!(f.sig, Sig::Bad(ref w) if w.contains("BAD")),
            "{:?}",
            f.sig
        );
        // tag name != version
        sign_tag(&s, "0.2.0", &fpr);
        assert!(
            evaluate(&s.facts("0.2.0"))
                .iter()
                .any(|l| l.starts_with("1:"))
        );
        // a v-prefixed tag exists
        assert!(s.git(&["tag", "v0.1.0"]).ok);
        assert!(
            evaluate(&s.facts("0.1.2"))
                .iter()
                .any(|l| l.starts_with("8:"))
        );
    }

    #[test]
    fn a_tag_off_main_a_missing_key_and_a_wrong_pin_each_fail_in_a_real_repository() {
        let s = scratch("main");
        let fpr = genkey(&s.home, "owner <owner@example.invalid>");
        s.release_repo(&fpr);
        // a commit on a side branch, tagged: not an ancestor of main
        assert!(s.git(&["checkout", "-q", "-b", "side"]).ok);
        s.write("side.txt", "x");
        s.commit("side");
        sign_tag(&s, "0.1.0", &fpr);
        assert!(s.git(&["checkout", "-q", "main"]).ok);
        let f = s.facts("0.1.0");
        assert_eq!(f.ancestor_of_main, Some(false));
        assert!(evaluate(&f).iter().any(|l| l.starts_with("2:")));
        // the pinned fingerprint is not the committed key's
        s.write(
            FINGERPRINT_PATH,
            "0000000000000000000000000000000000000000\n",
        );
        s.commit("repin");
        sign_tag(&s, "0.1.1", &fpr);
        assert!(matches!(
            s.facts("0.1.1").sig,
            Sig::FingerprintMismatch { .. }
        ));
        // no key committed on main at all
        assert!(s.git(&["rm", "-q", KEY_PATH]).ok);
        s.commit("remove key");
        sign_tag(&s, "0.1.2", &fpr);
        assert_eq!(s.facts("0.1.2").sig, Sig::NoKeyCommitted);
    }

    /// A tag whose own tree carries a key and the fingerprint pinning it must not verify against itself: the anchor is `main`.
    #[test]
    fn the_trust_anchor_is_main_not_the_tags_own_tree() {
        let s = scratch("anchor");
        let owner = genkey(&s.home, "owner <owner@example.invalid>");
        let intruder = genkey(&s.home, "intruder <intruder@example.invalid>");
        s.release_repo(&owner);
        // a branch that replaces the key and the pin with the intruder's, tags it, signed by the intruder
        assert!(s.git(&["checkout", "-q", "-b", "evil"]).ok);
        let pubkey = run(
            &s.dir,
            "gpg",
            &["--batch", "--armor", "--export", &intruder],
            &[("GNUPGHOME", &s.home.to_string_lossy())],
        );
        s.write(KEY_PATH, &pubkey.stdout);
        s.write(FINGERPRINT_PATH, &format!("{intruder}\n"));
        s.commit("evil key");
        sign_tag(&s, "0.1.0", &intruder);
        assert!(s.git(&["checkout", "-q", "main"]).ok);
        let f = s.facts("0.1.0");
        assert!(
            matches!(f.sig, Sig::Bad(_)),
            "the tag verified against its own tree's key: {:?}",
            f.sig
        );
        assert!(evaluate(&f).iter().any(|l| l.starts_with("5:")));
    }
}
