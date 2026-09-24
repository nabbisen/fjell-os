//! `ci-test-jobs` — a CI job that runs `cargo test` must not name packages
//! (RFC-0.33-004 D2, E-049).
//!
//! `.github/workflows/ci.yml` used to test crates from three hand-written `-p`
//! lists. Ten crates were in none of them (fourteen by the time this was
//! re-measured, two added by the line before), so their `--lib` tests ran nowhere
//! in CI, and some `-p` entries tested nothing at all (a service crate with no lib
//! target, beside crates that have one, is skipped silently). The lists grew by
//! hand — 85 entries became 101 — because nothing refused a hand-written list.
//! This does.
//!
//! **What it forbids:** a `-p` / `--package` on a `cargo test` command in any job.
//! Test jobs run the workspace-derived invocations (`cargo xtask host-lib-tests`,
//! `host-bin-tests`), whose set is computed from `cargo metadata`, so a new crate
//! is tested the day it is added. **What it does not:** `cargo check` and friends
//! legitimately name packages (a cross-check of a subset is the point), and
//! `cargo miri test` runs a named subset by design; only `cargo test` is a test
//! job's command. **One allowance:** `-p fjell-proptest`, the single package the
//! derived runs exclude *by name*, because it runs in its own job with its own
//! flags. A comment quoting `cargo test -p …` is not a command.

use std::process::ExitCode;

const NAME: &str = "ci-test-jobs";
const CI_PATH: &str = ".github/workflows/ci.yml";

/// The one package a `cargo test` line may name.
const ALLOWED_PACKAGE: &str = "fjell-proptest";

/// The rule, stated in the failure itself so a reader who trips it learns why from
/// the message and not from a handoff.
const RULE: &str = "a CI job that runs `cargo test` must not name packages with `-p`: a \
    hand-written list is what left crates untested and silently tested nothing for others \
    (E-049). Use `cargo xtask host-lib-tests` / `host-bin-tests`, which derive the set from \
    the workspace, so a new crate is tested the day it is added. (`cargo check` may name \
    packages; the one allowed `cargo test -p` is `-p fjell-proptest`, which the derived runs \
    exclude by name.)";

/// One violation: the job, the 1-based line the command starts on, the command.
#[derive(Debug, PartialEq, Eq)]
pub struct Violation {
    pub job: String,
    pub line: usize,
    pub command: String,
}

/// Every `cargo test` command in the workflow that names a package it may not.
pub fn violations(ci: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut job = String::from("(before any job)");
    let mut in_jobs = false;
    let lines: Vec<&str> = ci.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();
        if raw == "jobs:" {
            in_jobs = true;
        } else if in_jobs
            && raw.starts_with("  ")
            && !raw.starts_with("   ")
            && raw.trim_end().ends_with(':')
        {
            job = trimmed.trim_end_matches(':').to_string();
        }
        if trimmed.starts_with('#') || trimmed.is_empty() {
            i += 1;
            continue;
        }
        // Join backslash continuations into one logical command.
        let start = i;
        let mut cmd = trimmed.trim_end_matches('\\').trim().to_string();
        while lines[i].trim_end().ends_with('\\') && i + 1 < lines.len() {
            i += 1;
            let next = lines[i].trim();
            if next.starts_with('#') {
                continue;
            }
            cmd.push(' ');
            cmd.push_str(next.trim_end_matches('\\').trim());
        }
        i += 1;
        if let Some(rest) = cmd.find("cargo test").map(|p| &cmd[p..]) {
            if names_a_forbidden_package(rest) {
                out.push(Violation {
                    job: job.clone(),
                    line: start + 1,
                    command: cmd.clone(),
                });
            }
        }
    }
    out
}

/// Does a `cargo test …` command name a package other than the allowed one?
fn names_a_forbidden_package(cmd: &str) -> bool {
    let mut toks = cmd.split_whitespace().peekable();
    while let Some(t) = toks.next() {
        // A shell operator ends this command; what follows is another one.
        if matches!(t, "&&" | "||" | ";" | "|") {
            break;
        }
        let pkg = if t == "-p" || t == "--package" {
            toks.next()
        } else if let Some(v) = t.strip_prefix("--package=") {
            Some(v)
        } else if let Some(v) = t
            .strip_prefix("-p")
            .filter(|v| !v.is_empty() && !v.starts_with('-'))
        {
            Some(v)
        } else {
            None
        };
        if let Some(p) = pkg {
            if p.trim_matches(['"', '\'']) != ALLOWED_PACKAGE {
                return true;
            }
        }
    }
    false
}

pub fn run_check(ci: &str) -> ExitCode {
    let v = violations(ci);
    if v.is_empty() {
        let n = ci
            .lines()
            .filter(|l| !l.trim().starts_with('#') && l.contains("cargo test"))
            .count();
        println!("{NAME}: PASS ({n} `cargo test` command(s) in {CI_PATH}, none names a package)");
        ExitCode::SUCCESS
    } else {
        eprintln!("{NAME}: FAIL");
        for x in &v {
            eprintln!(
                "  {CI_PATH}:{} (job {}): `{}`",
                x.line,
                x.job,
                x.command.chars().take(120).collect::<String>()
            );
        }
        eprintln!("  {RULE}");
        ExitCode::FAILURE
    }
}

pub fn check() -> ExitCode {
    let Some(ci) = crate::read_file(NAME, CI_PATH) else {
        return ExitCode::FAILURE;
    };
    run_check(&ci)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WELL: &str = "jobs:\n  ci-a:\n    steps:\n      - run: cargo xtask host-lib-tests\n";

    #[test]
    fn a_workspace_derived_test_job_passes() {
        assert!(violations(WELL).is_empty());
        assert_eq!(run_check(WELL), ExitCode::SUCCESS);
    }

    /// The demonstration D7 asks for: a hand list reintroduced is refused, naming
    /// the job and the line.
    #[test]
    fn a_hand_written_package_list_on_cargo_test_is_refused() {
        let ci = "jobs:\n  ci-test-x:\n    steps:\n      - run: |\n          cargo test \\\n            -p fjell-cap \\\n            -p fjell-ipc \\\n            --lib\n";
        let v = violations(ci);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].job, "ci-test-x");
        assert_eq!(v[0].line, 5);
        assert_eq!(run_check(ci), ExitCode::FAILURE);
    }

    #[test]
    fn every_spelling_of_a_package_flag_is_caught() {
        for c in [
            "cargo test -p fjell-cap",
            "cargo test --package fjell-cap",
            "cargo test --package=fjell-cap",
            "cargo test -pfjell-cap",
            "cargo test --lib -p fjell-cap --features x",
        ] {
            let ci = format!("jobs:\n  ci-t:\n    steps:\n      - run: {c}\n");
            assert_eq!(violations(&ci).len(), 1, "{c}");
        }
    }

    #[test]
    fn the_one_allowance_is_fjell_proptest() {
        let ok = "jobs:\n  ci-p:\n    steps:\n      - run: cargo test -p fjell-proptest\n";
        assert!(violations(ok).is_empty());
        let bad =
            "jobs:\n  ci-p:\n    steps:\n      - run: cargo test -p fjell-proptest -p fjell-cap\n";
        assert_eq!(violations(bad).len(), 1);
    }

    /// §B: only `cargo test` is a test job. `cargo check` names packages by design.
    #[test]
    fn cargo_check_and_miri_may_name_packages() {
        let ci = "jobs:\n  ci-c:\n    steps:\n      - run: cargo check -p fjell-cap -p fjell-ipc\n      - run: cargo miri test -p fjell-cap\n";
        assert!(violations(ci).is_empty());
    }

    /// A comment quoting the old command is not the command; nor is `mkdir -p`,
    /// the false positive that made `fjell-ci-coverage` count `"fuzz/corpus/$t"` as
    /// a package.
    #[test]
    fn a_comment_and_mkdir_dash_p_are_not_a_package_list() {
        let ci = "jobs:\n  ci-c:\n    steps:\n      # cargo test -p fjell-cap used to be here\n      - run: |\n          mkdir -p \"fuzz/corpus/$t\"\n          cargo xtask host-lib-tests\n";
        assert!(violations(ci).is_empty());
    }

    #[test]
    fn a_second_command_after_a_shell_operator_is_its_own() {
        // `-p` after `&&` belongs to the next command (here a cargo check), not to
        // the `cargo test` before it.
        let ci = "jobs:\n  ci-c:\n    steps:\n      - run: cargo test --workspace --lib && cargo check -p fjell-cap\n";
        assert!(violations(ci).is_empty());
    }

    /// The real workflow passes — after this line replaced its lists — and a test
    /// reads it, not a fixture.
    #[test]
    fn the_committed_workflow_names_no_test_package() {
        let ci = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/ci.yml"),
        )
        .unwrap();
        let v = violations(&ci);
        assert!(v.is_empty(), "{v:#?}");
        // and it does contain `cargo test` commands, so passing is not vacuous
        assert!(ci.contains("cargo test -p fjell-proptest"));
    }
}
