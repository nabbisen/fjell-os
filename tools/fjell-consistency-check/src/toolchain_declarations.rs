//! RFC-0.30-003 §7 (shape 3): the `toolchain-declarations` subcheck.
//!
//! Twenty-two places declare the Fjell build toolchain's version
//! (E-037/Finding 1). CI structurally cannot read `rust-toolchain.toml`
//! via rustup (Finding 6: `apt` on `ubuntu-24.04` has no current rustc, so
//! bumping CI means changing the install method, not editing a number) —
//! so this does not consolidate the 22 into one file (§7 shape 1, named as
//! this line's successor). It makes leaving one behind at a bump
//! impossible to miss instead.
//!
//! **What this checks, and what it deliberately does not:**
//!
//! - The 17 `.github/workflows/ci.yml` install blocks, `rust-toolchain.toml`
//!   itself (the anchor), `docs/src/internals/local-development.md`'s
//!   table row *and* its `rustup toolchain install` line, `docs/src/
//!   tutorials/quick-start.md`'s apt line, and `docs/release/
//!   release-checklist.md`'s check — 21 of the 22 sites — must all name the
//!   same version as `rust-toolchain.toml`'s `channel`, the one file whose
//!   only job is declaring it (D3: a live declaration, not a historical
//!   record).
//! - `Cargo.toml`'s `rust-version` is the 22nd site and is **not** compared
//!   here. It is a floor, not a mirror — this project's own Non-goals say
//!   it "generally should not" move on every toolchain bump, so requiring
//!   it to always equal the channel would be wrong the first time they
//!   legitimately diverge. Tracked, not silently dropped: see
//!   `docs/rfcs/ERRATA.md`'s E-037 entry.
//! - The Verus (`1.95.0-x86_64-unknown-linux-gnu`) and `nightly` toolchains
//!   are a different, explicitly out-of-scope question (RFC-0.30-003
//!   Non-goals, Finding 4) — not read here at all.
//! - Every historical mention this same `1.91` grep also finds — release
//!   notes, old handoffs, Verus review records — is correct as written and
//!   is not a "declaration" this subcheck has any opinion about (D3).

use crate::read_file;
use std::process::ExitCode;

const NAME: &str = "toolchain-declarations";
const RUST_TOOLCHAIN_PATH: &str = "rust-toolchain.toml";
const CI_PATH: &str = ".github/workflows/ci.yml";
const LOCAL_DEV_PATH: &str = "docs/src/internals/local-development.md";
const QUICK_START_PATH: &str = "docs/src/tutorials/quick-start.md";
const RELEASE_CHECKLIST_PATH: &str = "docs/release/release-checklist.md";

pub fn check() -> ExitCode {
    let Some(rust_toolchain_src) = read_file(NAME, RUST_TOOLCHAIN_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(ci_src) = read_file(NAME, CI_PATH) else {
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
        &local_dev_src,
        &quick_start_src,
        &checklist_src,
    )
}

/// Core comparison, pure in its inputs for testing with synthetic
/// fixtures. Each parameter is one live site's full text.
pub fn run_check(
    rust_toolchain_src: &str,
    ci_src: &str,
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

    let ci_versions = extract_ci_versions(ci_src);
    if ci_versions.is_empty() {
        problems.push(format!(
            "{CI_PATH}: no `apt-get install -y rustc-<version>` blocks found — expected at least one"
        ));
    }
    for v in &ci_versions {
        checked += 1;
        if v != &anchor {
            problems.push(format!(
                "{CI_PATH}: a job installs rustc-{v}, but {RUST_TOOLCHAIN_PATH} says {anchor:?}"
            ));
        }
    }

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
            "{NAME}: PASS ({checked} live sites agree with {RUST_TOOLCHAIN_PATH}'s channel {anchor:?}, \
             plus {RUST_TOOLCHAIN_PATH} itself = {} sites)",
            checked + 1
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

/// `rust-toolchain.toml`'s `channel = "1.91"` — the anchor every other
/// live site is compared against.
fn extract_channel(src: &str) -> Option<String> {
    let line = src
        .lines()
        .find(|l| l.trim_start().starts_with("channel"))?;
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Every `sudo apt-get install -y rustc-<version> ...` line in `ci.yml` —
/// 17 of them today, one per job that builds Fjell.
fn extract_ci_versions(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| {
            l.trim_start()
                .strip_prefix("sudo apt-get install -y rustc-")
        })
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// `| Rust | 1.91 (stable) | ... |` — the second markdown-table cell.
fn extract_local_dev_table_version(src: &str) -> Option<String> {
    let line = src.lines().find(|l| l.contains("| Rust |"))?;
    let cell = line.split('|').nth(2)?.trim();
    cell.split_whitespace().next().map(str::to_string)
}

/// `rustup toolchain install 1.91` — the *first* such line, which is the
/// Fjell one; the Verus section's own `rustup toolchain install
/// 1.95.0-x86_64-unknown-linux-gnu` line is textually later and irrelevant
/// here regardless (Finding 4, out of scope).
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

/// `rustc --version | grep "1.91"` — the one live *check*, not just prose
/// (Finding 2).
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
    const CI: &str = "      - name: x\n        run: |\n          sudo apt-get update\n          sudo apt-get install -y rustc-1.91 cargo-1.91 rust-src\n";
    const LOCAL_DEV: &str = "| Tool | Version | Purpose |\n|---|---|---|\n| Rust | 1.91 (stable) | Kernel and all service crates |\n\n```sh\nrustup toolchain install 1.91\n```\n";
    const QUICK_START: &str =
        "```bash\nsudo apt install rustc-1.91 cargo-1.91 rust-src lld llvm\n```\n";
    const CHECKLIST: &str = "```bash run-verified\nrustc --version | grep \"1.91\"\n```\n";

    #[test]
    fn extract_channel_reads_the_quoted_value() {
        assert_eq!(extract_channel(RUST_TOOLCHAIN), Some("1.91".to_string()));
    }

    #[test]
    fn extract_ci_versions_finds_every_install_block() {
        let two_jobs = format!("{CI}\n{CI}");
        assert_eq!(
            extract_ci_versions(&two_jobs),
            vec!["1.91".to_string(), "1.91".to_string()]
        );
    }

    #[test]
    fn extract_quick_start_is_not_fooled_by_rust_dash_src() {
        // The real bug this line found: `rust-1.91-src` contains "1.91"
        // too, and comes right after `rustc-1.91` on the same line.
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

    #[test]
    fn all_sites_agreeing_passes() {
        assert_eq!(
            run_check(RUST_TOOLCHAIN, CI, LOCAL_DEV, QUICK_START, CHECKLIST),
            ExitCode::SUCCESS
        );
    }

    /// Required demonstration (D6): a declaration site left behind at a
    /// bump. `rust-toolchain.toml` moves to 1.92; `ci.yml` is not updated —
    /// exactly the shape of every version bump this project has done.
    #[test]
    fn a_site_left_behind_at_a_bump_fails_naming_it() {
        let bumped = "[toolchain]\nchannel = \"1.92\"\nprofile = \"minimal\"\n";
        let result = run_check(bumped, CI, LOCAL_DEV, QUICK_START, CHECKLIST);
        assert_eq!(result, ExitCode::FAILURE);
    }

    #[test]
    fn missing_channel_fails_naming_the_file() {
        assert_eq!(
            run_check(
                "[toolchain]\nprofile = \"minimal\"\n",
                CI,
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
            run_check(RUST_TOOLCHAIN, CI, LOCAL_DEV, QUICK_START, stale_checklist),
            ExitCode::FAILURE
        );
    }
}
