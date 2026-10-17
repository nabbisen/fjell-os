//! Crate-level `#![allow(...)]` census — RFC-0.37-002 R5.
//!
//! Exit criterion 2 ("0 warning lines") can be bought by a crate-level
//! `#![allow(...)]`: a gate that only counts warning lines cannot tell a
//! crate with nothing to warn about from one that silenced the warning
//! (E-082). This reports the blind spot as a figure in the release record,
//! never as a gate that fails — the ruling (R5) is that a legitimate,
//! reasoned suppression should not block a cut; an instrument should
//! disclose what it cannot see, not pretend it can see past it.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Walks `<root>/crates/` for crate-root files (`src/main.rs`, `src/lib.rs`)
/// whose first attribute block opens with a crate-level `#![allow(...)]`,
/// and returns the sorted crate names.
///
/// Only `src/main.rs`/`src/lib.rs` directly under a crate's own `src/`
/// count — a module file deeper in the tree can carry its own inner
/// `#![allow(...)]` for a nested `mod { ... }` block, which is a different,
/// already-narrow thing R3 asked for, not the blanket R5 counts.
pub fn crate_level_allow_crates(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    walk(&root.join("crates"), &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, found);
            continue;
        }
        if !is_crate_root(&path) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        if has_crate_level_allow(&contents) {
            if let Some(name) = crate_name_from_src(&path) {
                found.push(name);
            }
        }
    }
}

fn is_crate_root(path: &Path) -> bool {
    let is_root_filename = matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("main.rs") | Some("lib.rs")
    );
    is_root_filename
        && path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some("src")
}

/// A crate-level inner attribute (`#![...]`) can only precede the first
/// item in the file; doc comments and other crate attributes before it are
/// fine. Checked line-by-line rather than parsed, matching the rest of this
/// tool's instruments (RFC-0.35-003's own precedent: a working check beats
/// a hand-rolled parser for a one-line pattern).
fn has_crate_level_allow(contents: &str) -> bool {
    contents
        .lines()
        .map(str::trim)
        .any(|line| line.starts_with("#![allow("))
}

fn crate_name_from_src(path: &Path) -> Option<String> {
    path.parent()? // src/
        .parent()? // <crate>/
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

/// Prints the census as a non-blocking record figure. Called from
/// `cargo xtask build` (exit criterion 2's own command) so the release
/// record can quote it alongside the warning count, and available
/// standalone as `cargo xtask lint-allow-report` for the same figure
/// without a full build.
pub fn print_report(root: &Path) {
    let crates = crate_level_allow_crates(root);
    if crates.is_empty() {
        println!("[xtask] crate-level #![allow(...)]: 0 crates");
    } else {
        println!(
            "[xtask] crate-level #![allow(...)]: {} crate(s): {} — each allow is expected to \
             state its reason inline (RFC-0.37-002 R3); this count does not fail the build.",
            crates.len(),
            crates.join(", ")
        );
    }
}

pub fn cmd_lint_allow_report() -> ExitCode {
    print_report(&PathBuf::from("."));
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC-0.37-002 R6: a counter first seen reporting zero has tested
    /// nothing. This exercises both states against the same scratch crate —
    /// absent, then present, then removed — rather than trusting the
    /// present-day tree to hold a fixture forever.
    #[test]
    fn detects_a_scratch_crate_level_allow_appearing_and_disappearing() {
        let root = std::env::temp_dir().join(format!(
            "fjell-lint-allow-report-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let src_dir = root.join("crates").join("scratch-crate").join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let lib_rs = src_dir.join("lib.rs");

        std::fs::write(
            &lib_rs,
            "//! A scratch crate with no allow.\n\npub fn f() {}\n",
        )
        .unwrap();
        assert!(
            !crate_level_allow_crates(&root).contains(&"scratch-crate".to_string()),
            "a crate with no crate-level allow must not be counted"
        );

        std::fs::write(
            &lib_rs,
            "//! A scratch crate.\n#![allow(dead_code)]\n\npub fn f() {}\n",
        )
        .unwrap();
        assert!(
            crate_level_allow_crates(&root).contains(&"scratch-crate".to_string()),
            "a crate-level allow must move the count — a counter that always reports \
             zero has tested nothing"
        );

        std::fs::write(
            &lib_rs,
            "//! A scratch crate with no allow.\n\npub fn f() {}\n",
        )
        .unwrap();
        assert!(
            !crate_level_allow_crates(&root).contains(&"scratch-crate".to_string()),
            "removing the allow must move the count back"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_module_level_allow_deeper_in_the_tree_is_not_counted() {
        let root = std::env::temp_dir().join(format!(
            "fjell-lint-allow-report-test-nested-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let src_dir = root.join("crates").join("scratch-crate").join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "pub mod sub;\n").unwrap();
        std::fs::write(
            src_dir.join("sub.rs"),
            "#![allow(dead_code)]\npub fn f() {}\n",
        )
        .unwrap();

        assert!(
            crate_level_allow_crates(&root).is_empty(),
            "a nested module's own inner attribute is not the crate-level blanket R5 counts"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
