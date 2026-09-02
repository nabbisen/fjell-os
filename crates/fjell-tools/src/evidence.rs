//! RFC-0.27-004 R2: `cargo xtask evidence promote`.
//!
//! Copies one QEMU serial log into `tests/evidence/<rfc-id>/<name>.log` and
//! writes its D2/D3 provenance to a sidecar
//! `tests/evidence/<rfc-id>/<name>.provenance.txt`. Promotion is a
//! deliberate act with required arguments (D1) — there is no default path,
//! no default `--instrumented` answer, and no code path that reaches this
//! from `test-all` or `release-rehearsal`.
//!
//! Two source shapes:
//!
//!   - `--run-dir <dir>` — a directory `qemu_run::run_profile` wrote (R3):
//!     `run_id`, `profile`, `commit_sha` and `command` are read from its
//!     `run-info.txt` rather than re-typed, because that file recorded them
//!     at the one moment they were true. `--instrumented` is still always
//!     required from the human promoting it (D3: the dirty-tree bit alone
//!     is a proxy, not an answer).
//!   - `--source <log-file>` with `--run-id`, `--profile`, `--commit`, and
//!     `--command` all supplied explicitly — for a log produced before this
//!     tooling existed (R6's historical reconciliation), which has no
//!     `run-info.txt` to read. Nothing is inferred or defaulted here either;
//!     a missing field is a refusal to promote, not a guess.
//!
//! `--instrumented <text>` is mandatory in both shapes. Write the literal
//! `none` for a clean build, or a description of what existed and that it
//! is gone. There is no flag that promotes without answering this.

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

struct PromoteArgs {
    run_dir: Option<String>,
    source: Option<String>,
    run_id: Option<String>,
    profile: Option<String>,
    commit: Option<String>,
    command: Option<String>,
    rfc: Option<String>,
    name: Option<String>,
    instrumented: Option<String>,
}

fn parse_args(args: &[String]) -> PromoteArgs {
    let mut a = PromoteArgs {
        run_dir: None,
        source: None,
        run_id: None,
        profile: None,
        commit: None,
        command: None,
        rfc: None,
        name: None,
        instrumented: None,
    };
    let mut i = 0;
    while i < args.len() {
        let val = |i: usize| args.get(i + 1).cloned();
        match args[i].as_str() {
            "--run-dir" => a.run_dir = val(i),
            "--source" => a.source = val(i),
            "--run-id" => a.run_id = val(i),
            "--profile" => a.profile = val(i),
            "--commit" => a.commit = val(i),
            "--command" => a.command = val(i),
            "--rfc" => a.rfc = val(i),
            "--name" => a.name = val(i),
            "--instrumented" => a.instrumented = val(i),
            _ => {}
        }
        i += 1;
    }
    a
}

pub fn cmd_evidence(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("promote") => cmd_promote(&args[1..]),
        _ => {
            eprintln!(
                "Usage: cargo xtask evidence promote --rfc <id> --name <name> \
                 --instrumented <none|description> \
                 (--run-dir <tests/qemu/artifacts/<profile>/runs/<run-id>> \
                 | --source <log> --run-id <id> --profile <name> --commit <sha> --command <cmd>)"
            );
            ExitCode::FAILURE
        }
    }
}

/// Fields read from a `run-info.txt` written by `qemu_run::run_profile`.
struct RunInfo {
    run_id: String,
    profile: String,
    commit_sha: String,
    command: String,
}

fn parse_run_info(src: &str) -> Option<RunInfo> {
    let mut run_id = None;
    let mut profile = None;
    let mut commit_sha = None;
    let mut command = None;
    for line in src.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim().to_string());
        match k {
            "run_id" => run_id = Some(v),
            "profile" => profile = Some(v),
            "commit_sha" => commit_sha = Some(v),
            "command" => command = Some(v),
            _ => {}
        }
    }
    Some(RunInfo {
        run_id: run_id?,
        profile: profile?,
        commit_sha: commit_sha?,
        command: command?,
    })
}

fn cmd_promote(args: &[String]) -> ExitCode {
    let a = parse_args(args);

    let Some(rfc) = a.rfc else {
        eprintln!("[xtask] evidence promote: --rfc <id> is required");
        return ExitCode::FAILURE;
    };
    let Some(name) = a.name else {
        eprintln!("[xtask] evidence promote: --name <name> is required");
        return ExitCode::FAILURE;
    };
    let Some(instrumented) = a.instrumented else {
        eprintln!(
            "[xtask] evidence promote: --instrumented <none|description> is required — \
             D3: an instrumented build must say so, and there is no default answer"
        );
        return ExitCode::FAILURE;
    };
    if instrumented.trim().is_empty() {
        eprintln!("[xtask] evidence promote: --instrumented must not be empty");
        return ExitCode::FAILURE;
    }

    let (source, run_id, profile, commit_sha, command) = if let Some(run_dir) = &a.run_dir {
        let run_dir = Path::new(run_dir);
        let Ok(info_src) = fs::read_to_string(run_dir.join("run-info.txt")) else {
            eprintln!(
                "[xtask] evidence promote: cannot read {}/run-info.txt",
                run_dir.display()
            );
            return ExitCode::FAILURE;
        };
        let Some(info) = parse_run_info(&info_src) else {
            eprintln!(
                "[xtask] evidence promote: {}/run-info.txt is missing a required field",
                run_dir.display()
            );
            return ExitCode::FAILURE;
        };
        (
            run_dir.join("serial.log"),
            info.run_id,
            info.profile,
            info.commit_sha,
            info.command,
        )
    } else {
        let (Some(source), Some(run_id), Some(profile), Some(commit), Some(command)) = (
            a.source.clone(),
            a.run_id.clone(),
            a.profile.clone(),
            a.commit.clone(),
            a.command.clone(),
        ) else {
            eprintln!(
                "[xtask] evidence promote: --source requires --run-id, --profile, --commit, \
                 and --command all supplied explicitly — nothing is inferred for a historical log"
            );
            return ExitCode::FAILURE;
        };
        (
            std::path::PathBuf::from(source),
            run_id,
            profile,
            commit,
            command,
        )
    };

    if !source.exists() {
        eprintln!(
            "[xtask] evidence promote: source {} does not exist",
            source.display()
        );
        return ExitCode::FAILURE;
    }

    match check_ancestor(&commit_sha) {
        AncestorCheck::Ancestor => {}
        AncestorCheck::NotAncestor => {
            eprintln!(
                "[xtask] evidence promote: commit {commit_sha:?} is not an ancestor of HEAD — refusing to promote"
            );
            return ExitCode::FAILURE;
        }
        AncestorCheck::Inconclusive(reason) => {
            eprintln!(
                "[xtask] evidence promote: WARNING — could not verify commit {commit_sha:?} is an ancestor of HEAD ({reason}). Promoting anyway; the `evidence` subcheck will re-verify this from a full clone."
            );
        }
    }

    let dest_dir = Path::new("tests/evidence").join(&rfc);
    if let Err(e) = fs::create_dir_all(&dest_dir) {
        eprintln!(
            "[xtask] evidence promote: cannot create {}: {e}",
            dest_dir.display()
        );
        return ExitCode::FAILURE;
    }
    let dest_log = dest_dir.join(format!("{name}.log"));
    let dest_prov = dest_dir.join(format!("{name}.provenance.txt"));

    if let Err(e) = fs::copy(&source, &dest_log) {
        eprintln!(
            "[xtask] evidence promote: cannot copy {} to {}: {e}",
            source.display(),
            dest_log.display()
        );
        return ExitCode::FAILURE;
    }

    let provenance = format!(
        "run_id = {run_id}\nprofile = {profile}\ncommit_sha = {commit_sha}\ncommand = {command}\ninstrumented = {instrumented}\n"
    );
    if let Err(e) = fs::write(&dest_prov, provenance.as_bytes()) {
        eprintln!(
            "[xtask] evidence promote: cannot write {}: {e}",
            dest_prov.display()
        );
        return ExitCode::FAILURE;
    }

    println!(
        "[xtask] promoted {} -> {} (provenance: {})",
        source.display(),
        dest_log.display(),
        dest_prov.display()
    );
    ExitCode::SUCCESS
}

enum AncestorCheck {
    Ancestor,
    NotAncestor,
    Inconclusive(String),
}

/// Is `sha` an ancestor of `HEAD`? This is the one place in this project's
/// tooling that shells out to `git` for a repository-graph question rather
/// than reading committed files — a deliberate, narrow exception to the
/// convention `errata_tracking`'s design note states (no subcheck shells
/// out to git, so behaviour is identical in a full clone, a shallow clone,
/// or an exported tarball). "Is this exact commit object reachable from
/// HEAD" has no git-free proxy: there is no committed file recording every
/// commit that was ever HEAD, unlike "has this version shipped" (which
/// `errata_tracking` answers from `docs/release/records/*.md` instead).
/// On a **shallow clone**, this can report `Inconclusive` for a genuinely
/// valid historical sha whose object was never fetched — a real limitation,
/// disclosed rather than silently producing a false FAIL.
fn check_ancestor(sha: &str) -> AncestorCheck {
    if sha == "unknown" || sha.is_empty() {
        return AncestorCheck::Inconclusive("no commit sha recorded".into());
    }
    let exists = Command::new("git")
        .args(["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .output();
    match exists {
        Ok(o) if o.status.success() => {}
        Ok(_) => {
            return AncestorCheck::Inconclusive(
                "commit object not present locally — possibly a shallow clone".into(),
            );
        }
        Err(e) => return AncestorCheck::Inconclusive(format!("git unavailable: {e}")),
    }
    match Command::new("git")
        .args(["merge-base", "--is-ancestor", sha, "HEAD"])
        .status()
    {
        Ok(s) if s.success() => AncestorCheck::Ancestor,
        Ok(_) => AncestorCheck::NotAncestor,
        Err(e) => AncestorCheck::Inconclusive(format!("git unavailable: {e}")),
    }
}
