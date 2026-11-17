//! Slice 1 (RFC-v0.22-001): the syscall-surface subcheck.
//!
//! Compares three things and requires all to agree:
//!   1. Declared `SyscallNumber` variants in `crates/fjell-abi/src/syscall.rs`.
//!   2. Dispatched variants in `crates/fjell-kernel/src/trap/syscall.rs`
//!      (i.e. every name that appears in a `Some(SyscallNumber::Name)` arm —
//!      the catch-all `Some(_) | None` arm never matches that pattern, so
//!      undispatched names are excluded naturally, not by special-casing).
//!   3. The committed expectations in `tests/syscall/expected.toml`.
//!
//! `undispatched` is checked as an explicit **set of names**, not a bare
//! count — a count alone would let one undispatched syscall silently
//! replace another and still pass (the same reasoning as the ABI-snapshot
//! gate: changing the surface must force a deliberate edit to a committed
//! file).

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

    run_check(&abi_src, &dispatch_src, &expected)
}

/// Core comparison. Pure function of its inputs so it can be exercised with
/// synthetic fixtures in tests, independent of the real repository files.
pub fn run_check(abi_src: &str, dispatch_src: &str, expected: &Expected) -> ExitCode {
    let declared = parse_declared(abi_src);
    let dispatched = parse_dispatched(dispatch_src);

    let mut undispatched: Vec<String> = declared
        .iter()
        .map(|(name, _)| name.clone())
        .filter(|n| !dispatched.contains(n))
        .collect();
    undispatched.sort();

    let mut expected_undispatched = expected.undispatched.clone();
    expected_undispatched.sort();

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
    if undispatched != expected_undispatched {
        let extra: Vec<&String> = undispatched
            .iter()
            .filter(|n| !expected_undispatched.contains(n))
            .collect();
        let stale: Vec<&String> = expected_undispatched
            .iter()
            .filter(|n| !undispatched.contains(n))
            .collect();
        problems.push(format!(
            "undispatched set mismatch: in source but not expected.toml: {extra:?}; \
             in expected.toml but not source: {stale:?}"
        ));
    }

    if problems.is_empty() {
        println!(
            "syscall-surface: PASS ({} declared, {} dispatched, {} undispatched)",
            declared.len(),
            dispatched.len(),
            undispatched.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("syscall-surface: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        eprintln!("  If this change is intended, update {EXPECTED_PATH} deliberately.");
        ExitCode::FAILURE
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
    Some(_) | None => {
        return SysError::UnknownSyscall;
    }
}
"#;

    fn matching_expected() -> Expected {
        Expected {
            declared_count: 4,
            dispatched_count: 3,
            undispatched: vec!["CapInstall".to_string()],
        }
    }

    #[test]
    fn matching_source_and_expected_passes() {
        let result = run_check(ABI_FIXTURE, DISPATCH_FIXTURE, &matching_expected());
        assert_eq!(result, ExitCode::SUCCESS);
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
        assert_eq!(dispatched.len(), 3);
        assert!(!dispatched.contains("CapInstall"));
    }

    /// Required failure demonstration, direction 1 (RFC §Testing item 1):
    /// a syscall added to the enum without updating expectations. The new
    /// variant is undispatched, so it should appear in the source's
    /// undispatched set but not in the (stale) expectations — a mismatch.
    #[test]
    fn new_declared_syscall_not_in_expected_fails() {
        let abi_with_new_variant = r#"
pub enum SyscallNumber {
    Yield = 0,
    Exit = 1,
    CapCopy = 10,
    CapInstall = 17,
    NewSyscall = 200,
}
"#;
        // expected.toml still describes the old surface (4 declared, one
        // undispatched name) — it was not updated for the new variant.
        let result = run_check(abi_with_new_variant, DISPATCH_FIXTURE, &matching_expected());
        assert_eq!(
            result,
            ExitCode::FAILURE,
            "adding a declared syscall without updating expected.toml must fail the check"
        );
    }

    /// Required failure demonstration, direction 2 (RFC §Testing item 1):
    /// an expectation names a syscall that no longer exists in source.
    #[test]
    fn stale_expected_entry_no_longer_in_source_fails() {
        let stale_expected = Expected {
            declared_count: 4,
            dispatched_count: 3,
            // "RetiredSyscall" is not in ABI_FIXTURE at all.
            undispatched: vec!["RetiredSyscall".to_string()],
        };
        let result = run_check(ABI_FIXTURE, DISPATCH_FIXTURE, &stale_expected);
        assert_eq!(
            result,
            ExitCode::FAILURE,
            "an expectation naming a nonexistent syscall must fail the check"
        );
    }

    #[test]
    fn parse_expected_reads_inline_array() {
        let src = r#"
declared_count = 35
dispatched_count = 26
undispatched = ["CapInstall", "PlatformReboot"]
"#;
        let e = parse_expected(src).unwrap();
        assert_eq!(e.declared_count, 35);
        assert_eq!(e.dispatched_count, 26);
        assert_eq!(e.undispatched, vec!["CapInstall", "PlatformReboot"]);
    }

    #[test]
    fn parse_expected_reads_multiline_array() {
        let src = "declared_count = 35\ndispatched_count = 26\nundispatched = [\n    \"CapInstall\",\n    \"PlatformReboot\",\n]\n";
        let e = parse_expected(src).unwrap();
        assert_eq!(e.undispatched, vec!["CapInstall", "PlatformReboot"]);
    }
}
