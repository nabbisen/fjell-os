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
use std::collections::BTreeSet;
use std::process::ExitCode;

const ABI_SYSCALL_PATH: &str = "crates/fjell-abi/src/syscall.rs";
const DISPATCH_PATH: &str = "crates/fjell-kernel/src/trap/syscall.rs";
const EXPECTED_PATH: &str = "tests/syscall/expected.toml";

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Expected {
    pub declared_count: usize,
    pub dispatched_count: usize,
    pub undispatched: Vec<String>,
}

pub fn check() -> ExitCode {
    let Some(abi_src) = read_file(ABI_SYSCALL_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(dispatch_src) = read_file(DISPATCH_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(expected_src) = read_file(EXPECTED_PATH) else {
        return ExitCode::FAILURE;
    };

    let expected = match parse_expected(&expected_src) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("consistency-check: cannot parse {EXPECTED_PATH}: {e}");
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

/// Parse `Name = number,` variant lines inside `pub enum SyscallNumber { ... }`.
fn parse_declared(src: &str) -> Vec<(String, u32)> {
    let mut in_enum = false;
    let mut variants = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if !in_enum {
            if trimmed.starts_with("pub enum SyscallNumber") {
                in_enum = true;
            }
            continue;
        }
        if trimmed == "}" {
            break;
        }
        if let Some(eq_pos) = trimmed.find('=') {
            let name_part = trimmed[..eq_pos].trim();
            let looks_like_variant = !name_part.is_empty()
                && name_part
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
                && name_part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_');
            if looks_like_variant {
                let rest = trimmed[eq_pos + 1..].trim().trim_end_matches(',');
                if let Ok(num) = rest.parse::<u32>() {
                    variants.push((name_part.to_string(), num));
                }
            }
        }
    }
    variants
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

/// Minimal parser for the small TOML subset `expected.toml` actually uses
/// (two integer scalars, one string array — inline or multi-line). No
/// general TOML/serde dependency: this project's tools avoid parser
/// dependencies by convention (RFC-v0.22-001 states this explicitly for
/// Gate 11; the same reasoning applies to a format this small).
fn parse_expected(src: &str) -> Result<Expected, String> {
    let mut declared_count = None;
    let mut dispatched_count = None;
    let mut undispatched = Vec::new();
    let mut in_array = false;

    for raw_line in src.lines() {
        let line = match raw_line.find('#') {
            Some(i) => raw_line[..i].trim(),
            None => raw_line.trim(),
        };
        if line.is_empty() {
            continue;
        }
        if in_array {
            if line.starts_with(']') {
                in_array = false;
                continue;
            }
            let item = line.trim_end_matches(',').trim().trim_matches('"');
            if !item.is_empty() {
                undispatched.push(item.to_string());
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "declared_count" => {
                declared_count = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("declared_count: {e}"))?,
                );
            }
            "dispatched_count" => {
                dispatched_count = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("dispatched_count: {e}"))?,
                );
            }
            "undispatched" => {
                if let Some(after) = value.strip_prefix('[') {
                    if let Some(end) = after.find(']') {
                        for item in after[..end].split(',') {
                            let item = item.trim().trim_matches('"');
                            if !item.is_empty() {
                                undispatched.push(item.to_string());
                            }
                        }
                    } else {
                        in_array = true;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Expected {
        declared_count: declared_count.ok_or("missing declared_count")?,
        dispatched_count: dispatched_count.ok_or("missing dispatched_count")?,
        undispatched,
    })
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
