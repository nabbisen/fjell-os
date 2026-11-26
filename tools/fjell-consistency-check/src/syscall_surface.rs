//! The syscall-surface subcheck: **every declared syscall is dispatched, or it is retired** (RFC-0.49-002).
//!
//! Slice 1 (RFC-v0.22-001) made this a *record*: it compared the declared `SyscallNumber` variants, the dispatched ones and an
//! explicit `undispatched = [..]` list in `tests/syscall/expected.toml`, and passed with three undispatched syscalls because
//! all three were written in the list. **A list the check is satisfied by editing records a state; it does not refuse one** --
//! a new declared-but-undispatched variant passed the moment someone added its name. This is the rule instead:
//!
//!   1. Declared variants: `crates/fjell-abi/src/syscall.rs`.
//!   2. Dispatched variants: every name in a `Some(SyscallNumber::Name)` arm of `crates/fjell-kernel/src/trap/syscall.rs`
//!      (the catch-all arm never matches that pattern, so undispatched names are excluded naturally).
//!   3. **A declared variant with no dispatch arm FAILS**, naming the two ways out -- dispatch it, or retire it (remove the
//!      variant and leave a comment that its number is not reused, as `Reboot = 120` was) -- **unless it is `held`**: named in
//!      `expected.toml`'s `held`, with a `held_reason` and a `held_until` workspace version. A hold is a dated exception, the
//!      shape `unsafe-audit`'s seeded exemptions have: it fails when its syscall is no longer undispatched (stale), when it has
//!      no reason or no expiry, and **when the workspace version has moved past `held_until`**.
//!   4. The declared and dispatched counts stay as a drift guard against the committed file.
//!
//! The old `undispatched` key is **retired** and `parse_expected` refuses it, so the record cannot come back by an edit.

use crate::read_file;
use fjell_consistency_check::syscall_table::{Expected, parse_declared, parse_expected};
use std::collections::BTreeSet;
use std::process::ExitCode;

const ABI_SYSCALL_PATH: &str = "crates/fjell-abi/src/syscall.rs";
const DISPATCH_PATH: &str = "crates/fjell-kernel/src/trap/syscall.rs";
const EXPECTED_PATH: &str = "tests/syscall/expected.toml";

const NAME: &str = "syscall-surface";

pub fn check() -> ExitCode {
    let Some(abi_src) = read_file(NAME, ABI_SYSCALL_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(dispatch_src) = read_file(NAME, DISPATCH_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(expected_src) = read_file(NAME, EXPECTED_PATH) else {
        return ExitCode::FAILURE;
    };

    let expected = match parse_expected(&expected_src) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot parse {EXPECTED_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let version = read_file(NAME, "Cargo.toml")
        .and_then(|t| fjell_consistency_check::release_records::workspace_version(&t));
    run_check(&abi_src, &dispatch_src, &expected, version.as_deref())
}

/// `(major, minor, patch)` of `a.b.c`, or `None`.
fn version_tuple(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.trim().split('.');
    let t = (
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    );
    it.next().is_none().then_some(t)
}

/// What a passing judgement says: declared, dispatched, and the names held (declared, undispatched, with a live hold).
#[derive(Debug, PartialEq, Eq)]
pub struct Surface {
    pub declared: usize,
    pub dispatched: usize,
    pub held: Vec<String>,
}

/// The judgement. Pure over its inputs so it can be exercised with synthetic fixtures. `workspace_version` is the version the
/// hold's expiry is compared with; `None` (unreadable) is a **failure** when anything is held.
pub fn judge(
    abi_src: &str,
    dispatch_src: &str,
    expected: &Expected,
    workspace_version: Option<&str>,
) -> Result<Surface, Vec<String>> {
    let declared = parse_declared(abi_src);
    let dispatched = parse_dispatched(dispatch_src);

    let mut undispatched: Vec<String> = declared
        .iter()
        .map(|(name, _)| name.clone())
        .filter(|n| !dispatched.contains(n))
        .collect();
    undispatched.sort();

    let mut problems: Vec<String> = Vec::new();

    if declared.len() != expected.declared_count {
        problems.push(format!(
            "declared count mismatch: source has {}, expected.toml says {}",
            declared.len(),
            expected.declared_count
        ));
    }
    if dispatched.len() != expected.dispatched_count {
        problems.push(format!(
            "dispatched count mismatch: source has {}, expected.toml says {}",
            dispatched.len(),
            expected.dispatched_count
        ));
    }

    // The rule: a declared syscall with no dispatch arm fails, unless it is held.
    for n in &undispatched {
        if !expected.undispatched.contains(n) {
            problems.push(format!(
                "`{n}` is declared in the ABI and no dispatch arm handles it, so every call answers UnknownSyscall. A declared syscall is \
                 dispatched or it is retired: dispatch it, or retire it (remove the variant, leave a comment that its number is not \
                 reused -- `Reboot = 120` is the precedent). This check does not record an undispatched syscall"
            ));
        }
    }
    // A hold is a dated exception: stale, reasonless, undated and expired holds all fail.
    for h in &expected.undispatched {
        if !undispatched.contains(h) {
            problems.push(format!(
                "stale hold `{h}`: it is not a declared-and-undispatched syscall any more, so it is retired from `held`"
            ));
        }
    }
    if !expected.undispatched.is_empty() {
        if expected
            .held_reason
            .as_deref()
            .is_none_or(|r| r.trim().is_empty())
        {
            problems.push("`held` has no `held_reason`: a hold without a reason is a record, not an exception".into());
        }
        match (expected.held_until.as_deref(), workspace_version) {
            (None, _) => problems.push("`held` has no `held_until`: a hold with no expiry would stand for ever".into()),
            (Some(u), _) if version_tuple(u).is_none() => {
                problems.push(format!("`held_until = {u:?}` is not a MAJOR.MINOR.PATCH version"))
            }
            (Some(_), None) => problems.push("the workspace version is unreadable, so a hold's expiry cannot be judged".into()),
            (Some(u), Some(w)) => match (version_tuple(u), version_tuple(w)) {
                (Some(until), Some(now)) if now > until => problems.push(format!(
                    "the hold on {:?} expired at {u} and the workspace is at {w}: dispatch the syscall or retire it",
                    expected.undispatched
                )),
                (_, None) => problems.push(format!("the workspace version {w:?} is not MAJOR.MINOR.PATCH")),
                _ => {}
            },
        }
    }

    if problems.is_empty() {
        Ok(Surface {
            declared: declared.len(),
            dispatched: dispatched.len(),
            held: undispatched,
        })
    } else {
        Err(problems)
    }
}

/// Run the judgement and print it.
pub fn run_check(
    abi_src: &str,
    dispatch_src: &str,
    expected: &Expected,
    workspace_version: Option<&str>,
) -> ExitCode {
    match judge(abi_src, dispatch_src, expected, workspace_version) {
        Ok(s) => {
            if s.held.is_empty() {
                println!(
                    "syscall-surface: PASS ({} declared, {} dispatched, 0 undispatched: every declared syscall is dispatched)",
                    s.declared, s.dispatched
                );
            } else {
                println!(
                    "syscall-surface: PASS ({} declared, {} dispatched, {} undispatched, **held, not resolved**: {} until {} -- {})",
                    s.declared,
                    s.dispatched,
                    s.held.len(),
                    s.held.join(", "),
                    expected.held_until.as_deref().unwrap_or("?"),
                    expected.held_reason.as_deref().unwrap_or("?")
                );
            }
            ExitCode::SUCCESS
        }
        Err(problems) => {
            eprintln!("syscall-surface: FAIL");
            for p in &problems {
                eprintln!("  {p}");
            }
            eprintln!("  If a count change is intended, update {EXPECTED_PATH} deliberately.");
            ExitCode::FAILURE
        }
    }
}

/// Every name appearing in a `Some(SyscallNumber::Name)` dispatch pattern.
/// The wildcard fallback (`Some(_) | None => ...`) never matches this
/// textual pattern, so undispatched variants are excluded without any
/// special-case for the fallback arm itself.
fn parse_dispatched(src: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let marker = "Some(SyscallNumber::";
    let mut rest = src;
    while let Some(pos) = rest.find(marker) {
        let after = &rest[pos + marker.len()..];
        match after.find(')') {
            Some(end) => {
                let name = after[..end].trim();
                if !name.is_empty() {
                    set.insert(name.to_string());
                }
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABI_FIXTURE: &str = r#"
pub enum SyscallNumber {
    Yield = 0,
    Exit = 1,
    CapCopy = 10,
    CapInstall = 17,
}
"#;

    const DISPATCH_FIXTURE: &str = r#"
match SyscallNumber::from_usize(nr) {
    Some(SyscallNumber::Yield) => sys_yield(tf),
    Some(SyscallNumber::Exit) => sys_exit(tf),
    Some(SyscallNumber::CapCopy) => dispatch_cap(tf),
    Some(SyscallNumber::CapInstall) => sys_cap_install(tf),
    Some(_) | None => {
        return SysError::UnknownSyscall;
    }
}
"#;

    fn ok_expected() -> Expected {
        Expected {
            declared_count: 4,
            dispatched_count: 4,
            undispatched: vec![],
            held_until: None,
            held_reason: None,
        }
    }

    /// The same surface with `CapInstall` declared but not dispatched.
    fn dispatch_without_cap_install() -> String {
        DISPATCH_FIXTURE.replace(
            "    Some(SyscallNumber::CapInstall) => sys_cap_install(tf),\n",
            "",
        )
    }

    fn held_expected() -> Expected {
        Expected {
            declared_count: 4,
            dispatched_count: 3,
            undispatched: vec!["CapInstall".into()],
            held_until: Some("0.49.0".into()),
            held_reason: Some("awaiting the owner's ruling".into()),
        }
    }

    #[test]
    fn every_declared_syscall_dispatched_passes_with_zero_undispatched() {
        let s = judge(
            ABI_FIXTURE,
            DISPATCH_FIXTURE,
            &ok_expected(),
            Some("0.49.0"),
        )
        .unwrap();
        assert_eq!((s.declared, s.dispatched, s.held.len()), (4, 4, 0));
    }

    /// THE RULE: a declared syscall with no dispatch arm fails -- and writing its name where the old check recorded it does not help.
    #[test]
    fn a_declared_but_undispatched_syscall_fails_and_names_the_two_ways_out() {
        let d = dispatch_without_cap_install();
        let mut e = ok_expected();
        e.dispatched_count = 3;
        let err = judge(ABI_FIXTURE, &d, &e, Some("0.49.0")).unwrap_err();
        assert!(
            err.iter().any(|p| p.contains("`CapInstall` is declared")
                && p.contains("dispatch it, or retire it")
                && p.contains("does not record")),
            "{err:?}"
        );
    }

    #[test]
    fn a_new_variant_with_a_hold_but_no_reason_or_no_expiry_still_fails() {
        let d = dispatch_without_cap_install();
        let mut e = held_expected();
        e.held_reason = None;
        let err = judge(ABI_FIXTURE, &d, &e, Some("0.49.0")).unwrap_err();
        assert!(
            err.iter().any(|p| p.contains("no `held_reason`")),
            "{err:?}"
        );
        let mut e = held_expected();
        e.held_until = None;
        let err = judge(ABI_FIXTURE, &d, &e, Some("0.49.0")).unwrap_err();
        assert!(err.iter().any(|p| p.contains("no `held_until`")), "{err:?}");
    }

    #[test]
    fn a_dated_hold_passes_until_its_milestone_and_fails_after_it() {
        let d = dispatch_without_cap_install();
        let s = judge(ABI_FIXTURE, &d, &held_expected(), Some("0.49.0")).unwrap();
        assert_eq!(
            s.held,
            vec!["CapInstall".to_string()],
            "held, and reported as held -- not as resolved"
        );
        let err = judge(ABI_FIXTURE, &d, &held_expected(), Some("0.50.0")).unwrap_err();
        assert!(
            err.iter()
                .any(|p| p.contains("expired at 0.49.0") && p.contains("0.50.0")),
            "{err:?}"
        );
        let err = judge(ABI_FIXTURE, &d, &held_expected(), None).unwrap_err();
        assert!(
            err.iter().any(|p| p.contains("unreadable")),
            "an unreadable version is a failure, not a pass"
        );
    }

    #[test]
    fn a_stale_hold_is_red() {
        // CapInstall is dispatched now, but is still held: retire the hold
        let mut e = held_expected();
        e.dispatched_count = 4;
        let err = judge(ABI_FIXTURE, DISPATCH_FIXTURE, &e, Some("0.49.0")).unwrap_err();
        assert!(
            err.iter().any(|p| p.contains("stale hold `CapInstall`")),
            "{err:?}"
        );
    }

    #[test]
    fn a_hold_naming_a_syscall_that_does_not_exist_is_stale_too() {
        let mut e = ok_expected();
        e.undispatched = vec!["RetiredSyscall".into()];
        e.held_until = Some("0.49.0".into());
        e.held_reason = Some("x".into());
        let err = judge(ABI_FIXTURE, DISPATCH_FIXTURE, &e, Some("0.49.0")).unwrap_err();
        assert!(
            err.iter()
                .any(|p| p.contains("stale hold `RetiredSyscall`")),
            "{err:?}"
        );
    }

    #[test]
    fn the_counts_still_guard_against_drift() {
        let mut e = ok_expected();
        e.declared_count = 5;
        assert!(judge(ABI_FIXTURE, DISPATCH_FIXTURE, &e, Some("0.49.0")).is_err());
        let mut e = ok_expected();
        e.dispatched_count = 3;
        assert!(judge(ABI_FIXTURE, DISPATCH_FIXTURE, &e, Some("0.49.0")).is_err());
    }

    #[test]
    fn parse_declared_finds_all_variants() {
        let declared = parse_declared(ABI_FIXTURE);
        assert_eq!(declared.len(), 4);
        assert!(declared.iter().any(|(n, v)| n == "CapInstall" && *v == 17));
    }

    #[test]
    fn parse_dispatched_excludes_the_wildcard_arm() {
        let dispatched = parse_dispatched(DISPATCH_FIXTURE);
        assert_eq!(dispatched.len(), 4);
        assert!(!dispatched.contains("Some(_)"));
    }

    #[test]
    fn parse_expected_reads_the_hold_inline_and_multiline() {
        let src = "declared_count = 35\ndispatched_count = 34\nheld = [\"CapInstall\"]\nheld_until = \"0.49.0\"\nheld_reason = \"why\"\n";
        let e = parse_expected(src).unwrap();
        assert_eq!(e.undispatched, vec!["CapInstall"]);
        assert_eq!(
            (e.held_until.as_deref(), e.held_reason.as_deref()),
            (Some("0.49.0"), Some("why"))
        );
        let src = "declared_count = 35\ndispatched_count = 26\nheld = [\n    \"CapInstall\",\n    \"PlatformReboot\",\n]\n";
        assert_eq!(
            parse_expected(src).unwrap().undispatched,
            vec!["CapInstall", "PlatformReboot"]
        );
    }

    #[test]
    fn the_retired_undispatched_key_is_refused_so_the_record_cannot_come_back() {
        let src = "declared_count = 35\ndispatched_count = 26\nundispatched = [\"CapInstall\"]\n";
        let err = parse_expected(src).unwrap_err();
        assert!(err.contains("retired") && err.contains("held"), "{err}");
    }

    #[test]
    fn version_tuples_compare_numerically_not_as_text() {
        assert!(version_tuple("0.10.0") > version_tuple("0.9.0"));
        assert_eq!(version_tuple("0.49"), None);
        assert_eq!(version_tuple("0.49.0.1"), None);
    }
}
