//! RFC-0.31-002 D3: the `toolchain-declarations` subcheck, inverted for CI.
//!
//! **What changed and why.** RFC-0.30-003 built this subcheck to compare
//! every versioned `rustc-<v>`/`cargo-<v>` mention in `ci.yml` against
//! `rust-toolchain.toml`'s channel, because CI installed its toolchain from
//! seventeen hand-copied apt blocks and *"structurally cannot read
//! `rust-toolchain.toml` via rustup"*. That premise was wrong in a way
//! nobody could see from the workflow file: apt's `rust-src` ships the
//! standard library's source without `library/Cargo.lock`, so
//! `-Z build-std` — which `crates/fjell-tools/src/qemu.rs:63,107` passes on
//! every kernel build — could never work under it. The seventeen blocks
//! were installing a toolchain that structurally could not compile this
//! product, and had been since the first QEMU job (E-041).
//!
//! CI now installs through rustup, from `rust-toolchain.toml`, via
//! `.github/actions/toolchain`. So the thing this subcheck used to require
//! — at least one versioned mention in `ci.yml` — is now precisely the
//! thing it must forbid. After RFC-0.31-002, `ci.yml` carries **zero**
//! versioned toolchain mentions, because its declaration *is*
//! `rust-toolchain.toml`. The apt-era scan survives as the regression guard
//! against the blocks coming back.
//!
//! **What this checks:**
//!
//! 1. **`ci.yml` names no `rustc-<v>`/`cargo-<v>` version at all.** Any such
//!    mention fails, naming the line. This is the guard, not a comparison:
//!    a reintroduced apt block is wrong whichever version it names, because
//!    the toolchain it installs cannot build the product.
//! 2. **Every toolchain-dependent job references the composite action** —
//!    and the set of such jobs is *derived from the jobs' own `run:` lines*,
//!    never from a list of job names. A hand list is E-014's family; this
//!    crate has removed three of them this month, and a job added without
//!    its toolchain step is exactly what such a list fails to notice.
//! 3. **The derivation is not allowed to go blind** (the positive control).
//!    If the job parser finds no jobs, or finds no toolchain-dependent job
//!    at all, that is a failure and not a pass. Every defect this milestone
//!    has closed shares one shape — an instrument reporting success over an
//!    empty set — and a derived predicate that silently derives nothing is
//!    the purest form of it.
//! 4. **The composite action exists.** A job referencing an action that is
//!    not in the tree fails at `Set up job`, before the D2 check it was
//!    supposed to run; checking it here means the local gate says so first.
//! 5. **The four documentation sites still agree with the channel** —
//!    `local-development.md`'s prerequisite row *and* its `rustup toolchain
//!    install` line, `quick-start.md`'s apt line, and
//!    `release-checklist.md`'s check. These are unchanged by this line.
//!    They are live declarations for a human, who will still install a
//!    toolchain by hand, and `rustc-1.91` is still what a human types at
//!    Ubuntu (E-037's second survivor is about the pin, not the audience).
//!
//! `Cargo.toml`'s `rust-version` is still not compared here: it is a floor,
//! not a mirror. Tracked in `docs/rfcs/ERRATA.md`'s E-037 entry.
//!
//! The Verus (`1.95.0-x86_64-unknown-linux-gnu`) and `nightly` toolchains
//! remain out of scope (RFC-0.31-002 Non-goals). A job that explicitly
//! selects another toolchain with `cargo +<name>` is declaring its own and
//! is not required to use the action.

use crate::read_file;
use std::process::ExitCode;

const NAME: &str = "toolchain-declarations";
const RUST_TOOLCHAIN_PATH: &str = "rust-toolchain.toml";
const CI_PATH: &str = ".github/workflows/ci.yml";
const ACTION_PATH: &str = ".github/actions/toolchain/action.yml";
const ACTION_REF: &str = "uses: ./.github/actions/toolchain";
const LOCAL_DEV_PATH: &str = "docs/src/internals/local-development.md";
const QUICK_START_PATH: &str = "docs/src/tutorials/quick-start.md";
const RELEASE_CHECKLIST_PATH: &str = "docs/release/release-checklist.md";

/// What makes a job toolchain-dependent, and the words this subcheck uses
/// to say so. Matched against the job's own `run:` lines — never against
/// its name.
///
/// The first five are RFC-0.31-002 D3's named set. The last is broader on
/// purpose: any bare `cargo`/`rustc` invocation needs *a* toolchain, and
/// the only declared one is `rust-toolchain.toml`. Widening the rule beyond
/// the five costs nothing here (every job that matches the five also
/// matches the sixth) and means a new job that merely runs `cargo test` is
/// covered on the day it is added rather than on the day someone remembers
/// to widen this list.
const BUILD_TRIGGERS: &[(&str, &str)] = &[
    ("cargo xtask build", "builds the product"),
    ("cargo xtask qemu-test", "boots the kernel under QEMU"),
    ("cargo xtask qemu-negative", "boots the kernel under QEMU"),
    ("cargo xtask two-build-check", "builds the product twice"),
    (
        "riscv64gc-unknown-none-elf",
        "targets the RISC-V bare-metal triple",
    ),
    ("-Z build-std", "needs the standard library's source"),
];

pub fn check() -> ExitCode {
    let Some(rust_toolchain_src) = read_file(NAME, RUST_TOOLCHAIN_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(ci_src) = read_file(NAME, CI_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(action_src) = read_file(NAME, ACTION_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(local_dev_src) = read_file(NAME, LOCAL_DEV_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(quick_start_src) = read_file(NAME, QUICK_START_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(checklist_src) = read_file(NAME, RELEASE_CHECKLIST_PATH) else {
        return ExitCode::FAILURE;
    };
    run_check(
        &rust_toolchain_src,
        &ci_src,
        Some(&action_src),
        &local_dev_src,
        &quick_start_src,
        &checklist_src,
    )
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
/// Each parameter is one live site's full text; `action_src` is `None` when
/// `.github/actions/toolchain/action.yml` is absent from the tree.
pub fn run_check(
    rust_toolchain_src: &str,
    ci_src: &str,
    action_src: Option<&str>,
    local_dev_src: &str,
    quick_start_src: &str,
    checklist_src: &str,
) -> ExitCode {
    let Some(anchor) = extract_channel(rust_toolchain_src) else {
        eprintln!("{NAME}: FAIL — {RUST_TOOLCHAIN_PATH} has no `channel = \"...\"` line");
        return ExitCode::FAILURE;
    };

    let mut problems: Vec<String> = Vec::new();
    let mut checked = 0usize;

    // (1) The regression guard: the apt blocks must not come back.
    for (line_no, line, version) in find_versioned_mentions(ci_src) {
        problems.push(format!(
            "{CI_PATH}:{line_no}: names toolchain version {version:?} — CI installs from \
             {RUST_TOOLCHAIN_PATH} through {ACTION_PATH} and must name no version of its own \
             (an apt `rustc-<v>` toolchain cannot `-Z build-std`; RFC-0.31-002/E-041). \
             The line: {}",
            line.trim()
        ));
    }

    // (4) The action a job is required to reference has to be in the tree.
    if action_src.is_none() {
        problems.push(format!(
            "{ACTION_PATH}: missing — every building job references it, so CI would fail at \
             `Set up job` before the toolchain check it exists to run"
        ));
    }

    // (2) Every toolchain-dependent job references it, derived from run lines.
    let jobs = parse_jobs(ci_src);
    let mut dependent = 0usize;
    for job in &jobs {
        let Some(reason) = job.toolchain_reason() else {
            continue;
        };
        dependent += 1;
        checked += 1;
        if !job.text.contains(ACTION_REF) {
            problems.push(format!(
                "{CI_PATH}:{}: job `{}` {reason} but does not `{ACTION_REF}` — \
                 it would build against whatever toolchain the runner happens to have",
                job.line, job.name
            ));
        }
    }

    // (3) The positive control. A derived set that derives nothing is not a
    // pass; it is the instrument having gone blind, which is the defect
    // family this whole milestone is about.
    if jobs.is_empty() {
        problems.push(format!(
            "{CI_PATH}: parsed zero jobs — the job parser has gone blind, and a subcheck that \
             checks nothing must not report PASS"
        ));
    } else if dependent == 0 {
        problems.push(format!(
            "{CI_PATH}: parsed {} job(s) but none matched any build trigger — either every \
             building job was deleted, or the triggers no longer match how this workflow \
             spells its build commands. Either way this subcheck is now asserting nothing.",
            jobs.len()
        ));
    }

    // (5) The four documentation sites, unchanged by RFC-0.31-002.
    match extract_local_dev_table_version(local_dev_src) {
        Some(v) => {
            checked += 1;
            if v != anchor {
                problems.push(format!(
                    "{LOCAL_DEV_PATH}: prerequisite table says {v:?}, but {RUST_TOOLCHAIN_PATH} says {anchor:?}"
                ));
            }
        }
        None => problems.push(format!(
            "{LOCAL_DEV_PATH}: no `| Rust | <version>` prerequisite row found"
        )),
    }
    match extract_local_dev_command_version(local_dev_src) {
        Some(v) => {
            checked += 1;
            if v != anchor {
                problems.push(format!(
                    "{LOCAL_DEV_PATH}: `rustup toolchain install {v}` disagrees with {RUST_TOOLCHAIN_PATH}'s {anchor:?}"
                ));
            }
        }
        None => problems.push(format!(
            "{LOCAL_DEV_PATH}: no `rustup toolchain install <version>` line found"
        )),
    }

    match extract_quick_start_version(quick_start_src) {
        Some(v) => {
            checked += 1;
            if v != anchor {
                problems.push(format!(
                    "{QUICK_START_PATH}: installs rustc-{v}, but {RUST_TOOLCHAIN_PATH} says {anchor:?}"
                ));
            }
        }
        None => problems.push(format!(
            "{QUICK_START_PATH}: no `apt install rustc-<version>` line found"
        )),
    }

    match extract_checklist_version(checklist_src) {
        Some(v) => {
            checked += 1;
            if v != anchor {
                problems.push(format!(
                    "{RELEASE_CHECKLIST_PATH}: checks for {v:?}, but {RUST_TOOLCHAIN_PATH} says {anchor:?}"
                ));
            }
        }
        None => problems.push(format!(
            "{RELEASE_CHECKLIST_PATH}: no `rustc --version | grep \"...\"` line found"
        )),
    }

    if problems.is_empty() {
        println!(
            "{NAME}: PASS ({dependent} toolchain-dependent CI job(s) on {ACTION_PATH}, \
             0 versioned mentions in {CI_PATH}, {checked} site(s) checked against \
             {RUST_TOOLCHAIN_PATH}'s channel {anchor:?})"
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("{NAME}: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

/// One `jobs:` entry: its key, the 1-based line it starts on, and its full
/// text down to the next entry.
struct Job {
    name: String,
    line: usize,
    text: String,
}

impl Job {
    /// Why this job needs the declared toolchain, or `None` if it does not.
    ///
    /// Read from the job's own command lines, with full-line comments
    /// removed first — a job's prose often names another job's build
    /// command (`ci-check`'s comment names `ci-cross-check`), and a comment
    /// is not a thing that runs.
    fn toolchain_reason(&self) -> Option<&'static str> {
        let body = strip_comment_lines(&self.text);
        for (trigger, reason) in BUILD_TRIGGERS {
            if body.contains(trigger) {
                return Some(reason);
            }
        }
        // A job that pins its own toolchain with `cargo +<name>` has
        // declared one and is out of scope (nightly fuzzing, Verus).
        if body.contains("cargo +") || body.contains("rustc +") {
            return None;
        }
        if body.contains("cargo ") || body.contains("rustc ") {
            return Some("runs cargo");
        }
        None
    }
}

/// Split `ci.yml` into its `jobs:` entries. A job key is the only thing in
/// this file indented by exactly two spaces and ending in `:` *after* the
/// top-level `jobs:` line — `on:`'s own `push:`/`schedule:` keys sit above
/// it and are skipped by starting there.
fn parse_jobs(src: &str) -> Vec<Job> {
    let lines: Vec<&str> = src.lines().collect();
    let Some(jobs_at) = lines.iter().position(|l| l.trim_end() == "jobs:") else {
        return Vec::new();
    };

    let mut starts: Vec<(usize, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(jobs_at + 1) {
        let t = line.trim_end();
        if !t.starts_with("  ") || t.starts_with("   ") || !t.ends_with(':') {
            continue;
        }
        let key = &t[2..t.len() - 1];
        if !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            starts.push((i, key.to_string()));
        }
    }

    let mut jobs = Vec::new();
    for (n, (i, name)) in starts.iter().enumerate() {
        let end = starts.get(n + 1).map(|(j, _)| *j).unwrap_or(lines.len());
        jobs.push(Job {
            name: name.clone(),
            line: i + 1,
            text: lines[*i..end].join("\n"),
        });
    }
    jobs
}

/// Drop whole-line comments. Inside a `run: |` block a leading `#` is a
/// shell comment and outside one it is a YAML comment; neither runs.
fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `rust-toolchain.toml`'s `channel = "1.91"` — the anchor the four
/// documentation sites are compared against.
fn extract_channel(src: &str) -> Option<String> {
    let line = src
        .lines()
        .find(|l| l.trim_start().starts_with("channel"))?;
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Every versioned `rustc-<version>` / `cargo-<version>` mention in
/// `ci.yml`, with the line it sits on. Zero of these is the correct state
/// after RFC-0.31-002; each one found is reported by line.
///
/// **Deliberately not anchored to any install line's exact spelling** — the
/// property RFC-0.30-003's review found and fixed, and it matters more now
/// that the finding is "any mention at all". The first version of this
/// function matched `strip_prefix("sudo apt-get install -y rustc-")`, and a
/// block reworded to `... --no-install-recommends rustc-1.90 ...` became
/// invisible to it. There is no spelling for a reintroduced apt block to
/// hide behind here.
///
/// The `ln -sf /usr/bin/rustc-<version>` lines are found too, on purpose:
/// installing one compiler and symlinking another is a real way for a job
/// to run something other than what it declares.
fn find_versioned_mentions(src: &str) -> Vec<(usize, &str, String)> {
    let mut found = Vec::new();
    for (i, line) in src.lines().enumerate() {
        for prefix in ["rustc-", "cargo-"] {
            let mut rest = line;
            while let Some(at) = rest.find(prefix) {
                rest = &rest[at + prefix.len()..];
                let token: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                // `cargo-fuzz` and friends are not versioned mentions.
                if !token.is_empty() {
                    found.push((i + 1, line, token));
                }
            }
        }
    }
    found
}

/// `| Rust | 1.91 (stable) | ... |` — the second markdown-table cell.
fn extract_local_dev_table_version(src: &str) -> Option<String> {
    let line = src.lines().find(|l| l.contains("| Rust |"))?;
    let cell = line.split('|').nth(2)?.trim();
    cell.split_whitespace().next().map(str::to_string)
}

/// `rustup toolchain install 1.91` — the *first* such line, which is the
/// Fjell one; the Verus section's own `rustup toolchain install
/// 1.95.0-x86_64-unknown-linux-gnu` line is textually later and out of
/// scope regardless.
fn extract_local_dev_command_version(src: &str) -> Option<String> {
    let line = src
        .lines()
        .find(|l| l.trim_start().starts_with("rustup toolchain install "))?;
    line.trim_start()
        .strip_prefix("rustup toolchain install ")?
        .split_whitespace()
        .next()
        .map(str::to_string)
}

/// `sudo apt install rustc-1.91 cargo-1.91 rust-1.91-src lld llvm` —
/// matched on `rustc-` specifically (not `rust-`, which also appears in
/// `rust-1.91-src` on the same line and would otherwise be found first).
fn extract_quick_start_version(src: &str) -> Option<String> {
    let line = src
        .lines()
        .find(|l| l.contains("apt install") && l.contains("rustc-"))?;
    let rest = line.split("rustc-").nth(1)?;
    rest.split_whitespace().next().map(str::to_string)
}

/// `rustc --version | grep "1.91"` — the one live *check*, not just prose.
fn extract_checklist_version(src: &str) -> Option<String> {
    let line = src
        .lines()
        .find(|l| l.contains("rustc --version") && l.contains("grep"))?;
    let start = line.find("grep \"")? + "grep \"".len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_TOOLCHAIN: &str = "[toolchain]\nchannel = \"1.91\"\nprofile = \"minimal\"\n";
    const ACTION: &str = "name: Install the declared toolchain\nruns:\n  using: composite\n";
    const LOCAL_DEV: &str = "| Tool | Version | Purpose |\n|---|---|---|\n| Rust | 1.91 (stable) | Kernel and all service crates |\n\n```sh\nrustup toolchain install 1.91\n```\n";
    const QUICK_START: &str =
        "```bash\nsudo apt install rustc-1.91 cargo-1.91 rust-src lld llvm\n```\n";
    const CHECKLIST: &str = "```bash run-verified\nrustc --version | grep \"1.91\"\n```\n";

    /// A correct post-RFC-0.31-002 workflow: no versioned mention, and the
    /// one building job on the composite action.
    const CI: &str = "\
jobs:
  ci-check:
    steps:
      - uses: actions/checkout@v7
      - name: Install toolchain
        uses: ./.github/actions/toolchain
      - name: cargo check
        run: cargo check -p fjell-tools
  ci-qemu-smoke:
    steps:
      - uses: actions/checkout@v7
      - name: Install toolchain
        uses: ./.github/actions/toolchain
      - name: smoke
        run: cargo xtask qemu-test m8
";

    fn run(ci: &str) -> ExitCode {
        run_check(
            RUST_TOOLCHAIN,
            ci,
            Some(ACTION),
            LOCAL_DEV,
            QUICK_START,
            CHECKLIST,
        )
    }

    #[test]
    fn extract_channel_reads_the_quoted_value() {
        assert_eq!(extract_channel(RUST_TOOLCHAIN), Some("1.91".to_string()));
    }

    #[test]
    fn a_correct_workflow_passes() {
        assert_eq!(run(CI), ExitCode::SUCCESS);
    }

    /// D8's second demonstration, as a test: an apt block reintroduced.
    #[test]
    fn a_reintroduced_apt_block_fails() {
        let with_apt = CI.replace(
            "      - name: Install toolchain\n        uses: ./.github/actions/toolchain\n",
            "      - name: Install Rust 1.91\n        run: |\n          sudo apt-get install -y rustc-1.91 cargo-1.91 rust-src\n",
        );
        assert_eq!(run(&with_apt), ExitCode::FAILURE);
    }

    /// The version named does not matter. A block naming the *current*
    /// channel is still wrong, because an apt toolchain cannot
    /// `-Z build-std` at any version — the premise RFC-0.30-003 could not
    /// see and E-041 recorded.
    #[test]
    fn an_apt_block_naming_the_current_channel_is_still_a_failure() {
        let sneaky = format!("{CI}          sudo apt-get install -y rustc-1.91 cargo-1.91\n");
        assert_eq!(run(&sneaky), ExitCode::FAILURE);
    }

    /// The scope-blindness property RFC-0.30-003's review installed,
    /// carried over: no rewording hides a mention.
    #[test]
    fn a_reworded_install_line_cannot_hide_a_mention() {
        let reworded = format!(
            "{CI}          sudo apt-get install -y --no-install-recommends rustc-1.90 cargo-1.90\n"
        );
        assert_eq!(run(&reworded), ExitCode::FAILURE);
        let found = find_versioned_mentions(&reworded);
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|(_, _, v)| v == "1.90"));
    }

    /// A symlink to another compiler is a mention too.
    #[test]
    fn an_ln_sf_shim_is_a_mention() {
        let shim = format!("{CI}          ln -sf /usr/bin/rustc-1.90 $HOME/.local/bin/rustc\n");
        assert_eq!(run(&shim), ExitCode::FAILURE);
    }

    #[test]
    fn cargo_fuzz_is_not_read_as_a_version() {
        assert!(find_versioned_mentions("cargo install cargo-fuzz\n").is_empty());
    }

    /// The point of deriving from `run:` lines rather than a job-name list:
    /// a job added without its toolchain step is caught on the day it is
    /// added, with no list to remember to update.
    #[test]
    fn a_new_building_job_without_the_action_fails_naming_it() {
        let added = format!(
            "{CI}{}",
            concat!(
                "  ci-new-thing:\n",
                "    steps:\n",
                "      - uses: actions/checkout@v7\n",
                "      - name: build\n",
                "        run: cargo xtask qemu-test m7\n",
            )
        );
        assert_eq!(run(&added), ExitCode::FAILURE);
    }

    #[test]
    fn a_job_that_only_builds_docs_needs_no_toolchain() {
        let docs_only = "\
jobs:
  ci-docs:
    steps:
      - uses: actions/checkout@v7
      - name: mdbook build
        run: cd docs && mdbook build
  ci-check:
    steps:
      - name: Install toolchain
        uses: ./.github/actions/toolchain
      - run: cargo check
";
        assert_eq!(run(docs_only), ExitCode::SUCCESS);
    }

    /// A job that names another toolchain explicitly declares its own.
    #[test]
    fn a_nightly_job_is_out_of_scope() {
        let nightly = "\
jobs:
  ci-check:
    steps:
      - name: Install toolchain
        uses: ./.github/actions/toolchain
      - run: cargo check
  ci-fuzz-nightly:
    steps:
      - run: rustup toolchain install nightly
      - run: cargo +nightly fuzz run parse
";
        assert_eq!(run(nightly), ExitCode::SUCCESS);
    }

    /// A comment naming another job's build command is not a build.
    #[test]
    fn a_comment_does_not_make_a_job_toolchain_dependent() {
        let commented = "\
jobs:
  ci-check:
    steps:
      - name: Install toolchain
        uses: ./.github/actions/toolchain
      - run: cargo check
  ci-docs:
    steps:
      # The RISC-V kernel is covered by ci-cross-check, which runs
      # cargo xtask qemu-test m8 against riscv64gc-unknown-none-elf.
      - run: cd docs && mdbook build
";
        assert_eq!(run(commented), ExitCode::SUCCESS);
    }

    /// The positive control. A derivation that derives nothing must not
    /// report PASS — "invisible is indistinguishable from absent" is the
    /// defect this milestone exists to remove, and a derived predicate is
    /// the easiest place to reintroduce it.
    #[test]
    fn a_workflow_with_no_building_job_fails_rather_than_passing_vacuously() {
        let no_builders = "\
jobs:
  ci-docs:
    steps:
      - run: cd docs && mdbook build
";
        assert_eq!(run(no_builders), ExitCode::FAILURE);
    }

    #[test]
    fn an_unparseable_workflow_fails_rather_than_passing_vacuously() {
        assert_eq!(run("# no jobs: key at all\n"), ExitCode::FAILURE);
    }

    #[test]
    fn parse_jobs_skips_the_on_block_and_finds_every_job() {
        let src = "\
on:
  push:
    branches: [main]
  schedule:
    - cron: '0 3 * * *'
jobs:
  ci-format:
    runs-on: ubuntu-24.04
  ci-check:
    runs-on: ubuntu-24.04
";
        let jobs = parse_jobs(src);
        assert_eq!(
            jobs.iter().map(|j| j.name.as_str()).collect::<Vec<_>>(),
            vec!["ci-format", "ci-check"]
        );
    }

    /// A missing composite action fails locally rather than on the runner.
    #[test]
    fn a_missing_composite_action_fails() {
        assert_eq!(
            run_check(RUST_TOOLCHAIN, CI, None, LOCAL_DEV, QUICK_START, CHECKLIST),
            ExitCode::FAILURE
        );
    }

    /// E-037's second survivor, re-stated. Under the apt blocks an exact
    /// patch pin failed this subcheck on an otherwise correct tree, because
    /// Ubuntu carries `rustc-1.91`, not `rustc-1.91.1` — recorded in
    /// RFC-0.30-003's review as a real collision between rustup channels
    /// and apt package names. `ci.yml` names no apt package now, and rustup
    /// accepts `channel = "1.91.1"`, so the collision is gone: pinning is
    /// now a decision about the pin alone.
    #[test]
    fn an_exact_patch_pin_no_longer_collides_with_ci() {
        let pinned = "[toolchain]\nchannel = \"1.91.1\"\n";
        let local_dev = "| Rust | 1.91.1 (stable) | x |\n\nrustup toolchain install 1.91.1\n";
        let quick = "sudo apt install rustc-1.91.1 cargo-1.91.1\n";
        let checklist = "rustc --version | grep \"1.91.1\"\n";
        assert_eq!(
            run_check(pinned, CI, Some(ACTION), local_dev, quick, checklist),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn extract_quick_start_is_not_fooled_by_rust_dash_src() {
        assert_eq!(
            extract_quick_start_version(
                "sudo apt install rustc-1.91 cargo-1.91 rust-1.91-src lld llvm"
            ),
            Some("1.91".to_string())
        );
    }

    #[test]
    fn extract_local_dev_command_finds_the_fjell_line_not_verus() {
        let src = "rustup toolchain install 1.91\n...\nrustup toolchain install 1.95.0-x86_64-unknown-linux-gnu --profile minimal\n";
        assert_eq!(
            extract_local_dev_command_version(src),
            Some("1.91".to_string())
        );
    }

    /// A documentation site left behind at a bump still fails, naming it —
    /// the property RFC-0.30-003 built this subcheck for, narrowed to the
    /// four sites a human still reads.
    #[test]
    fn a_doc_site_left_behind_at_a_bump_fails_naming_it() {
        let bumped = "[toolchain]\nchannel = \"1.92\"\nprofile = \"minimal\"\n";
        assert_eq!(
            run_check(bumped, CI, Some(ACTION), LOCAL_DEV, QUICK_START, CHECKLIST),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn missing_channel_fails_naming_the_file() {
        assert_eq!(
            run_check(
                "[toolchain]\nprofile = \"minimal\"\n",
                CI,
                Some(ACTION),
                LOCAL_DEV,
                QUICK_START,
                CHECKLIST
            ),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn checklist_disagreement_alone_fails() {
        let stale_checklist = "rustc --version | grep \"1.90\"\n";
        assert_eq!(
            run_check(
                RUST_TOOLCHAIN,
                CI,
                Some(ACTION),
                LOCAL_DEV,
                QUICK_START,
                stale_checklist
            ),
            ExitCode::FAILURE
        );
    }
}
