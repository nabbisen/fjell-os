//! QEMU negative test runner for
//! `cargo xtask qemu-negative <category>`.
//!
//! Per RFC 026 (negative-test harness) and RFC 042 (v0.2 expansion),
//! every category corresponds to a profile under
//! `tests/qemu/profiles/<category>.toml`. A category with no profile is
//! an error (RFC-0.24-002 Slice 4) — the RFC 025 placeholder path that
//! silently passed against an empty expectation set (`lease`, `evidence`
//! reachable with no profile ever written for either) was removed, not
//! bypassed, since a category a hand-written list names but never wires
//! up a profile for is itself the defect, not a state to run cleanly
//! through.
//!
//! RFC-0.29-001 D1/§6: the two `KNOWN_*` lists this module used to carry
//! (`KNOWN_V01X_CATEGORIES`, `KNOWN_V02_CATEGORIES` — one of the five
//! lists the RFC found disagreeing, thirteen entries including a `cap`
//! alias nothing else used) are gone. `tests/qemu/profiles/*.toml` is the
//! authority now, same as the real run path already was — a category
//! with no profile is simply not derived, and the error message below
//! lists what *is*, from `qemu_run::discover_negative_categories`.

use std::process::ExitCode;

/// Entry point: `cargo xtask qemu-negative <category>`.
pub fn cmd_qemu_negative(category: Option<&str>) -> ExitCode {
    let category = match category {
        Some(c) => c,
        None => {
            eprintln!("Usage: cargo xtask qemu-negative <category>");
            print_known_categories();
            return ExitCode::FAILURE;
        }
    };

    let profile_path = format!("tests/qemu/profiles/{category}.toml");
    if std::path::Path::new(&profile_path).exists() {
        // Delegate to the explicit loader via qemu_run::cmd_qemu_run.
        return crate::qemu_run::cmd_qemu_run(Some(category));
    }

    eprintln!(
        "[xtask] qemu-negative: unknown category `{category}` — no profile at {profile_path}"
    );
    print_known_categories();
    ExitCode::FAILURE
}

fn print_known_categories() {
    match crate::qemu_run::discover_negative_categories() {
        Ok(cats) => {
            let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
            eprintln!("Known categories: {}", names.join(", "));
        }
        Err(e) => eprintln!("(could not derive the known-category list: {e})"),
    }
}

/// `cargo xtask list-negative-categories` — RFC-0.29-001 R3/R4: prints the
/// derived category list as a JSON array, one object per category, for
/// `ci.yml` to consume via `fromJson(...)` in a matrix job rather than
/// carrying its own hand-typed copy (the fourth of the RFC's five
/// disagreeing lists). Shape:
/// `[{"category":"audit","release_gated":true}, ...]` — deliberately not
/// the richer `CategoryInfo` (no `not_gated_reason`): CI's own job only
/// needs to know whether to set `continue-on-error`, and the reason
/// belongs in the one place a human reads it, the profile file itself.
pub fn cmd_list_negative_categories() -> ExitCode {
    match crate::qemu_run::discover_negative_categories() {
        Ok(cats) => {
            let entries: Vec<String> = cats
                .iter()
                .map(|c| {
                    format!(
                        "{{\"category\":{:?},\"release_gated\":{}}}",
                        c.name, c.release_gated
                    )
                })
                .collect();
            println!("[{}]", entries.join(","));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("[xtask] list-negative-categories: {e}");
            ExitCode::FAILURE
        }
    }
}
