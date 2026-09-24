//! Library half of `fjell-consistency-check` (RFC-0.29-002 §7).
//!
//! `errata::parse_summary_rows` is the one shared parser for
//! `rfcs/ERRATA.md`'s `## Summary` table, reused by this crate's own
//! `errata-tracking`/`errata-limitations` subchecks *and* by
//! `crates/fjell-tools`'s `release_rehearsal` (Gate 7) — a different
//! crate, which is why this needs a `[lib]` target at all rather than a
//! `pub(crate)` module.
//!
//! `toolchain::observe` (RFC-0.30-003 D2) is the one shared "what actually
//! built this" function, reused by `tools/fjell-repro-check` and
//! `crates/fjell-tools` — three artefact-producing paths, one observation.

/// The one package a `cargo test` in CI may name with `-p`: the package the
/// workspace-derived test runs **exclude by name** (`fjell-proptest`: its own tier and
/// job, `--release`, its tests in `tests/`). One fact, one place: the `ci-test-jobs`
/// subcheck allows it and `crates/fjell-tools`' shared test argv excludes it, both
/// reading this, so a change moves both or a test fails (RFC-0.33-004 D11).
pub const UNDERIVED_TEST_PACKAGE: &str = "fjell-proptest";

pub mod errata;
pub mod toolchain;

/// The names of every subcheck, in the order Gate 12 reports them.
///
/// Added at RFC-0.30-003's review. `crates/fjell-tools`'s Gate 12 line
/// carried its own hand-typed copy of this list, and adding
/// `toolchain-declarations` made that copy stale the moment it landed —
/// the same duplicate-list defect the dispatcher in this crate's `main.rs`
/// had until RFC-0.30-002's review, one crate over. Gate 12 now renders
/// this constant, and `main.rs` asserts its own `ALL_SUBCHECKS` agrees
/// with it, so a twelfth subcheck cannot be added to one and not the
/// other.
pub const SUBCHECK_NAMES: &[&str] = &[
    "syscall-surface",
    "errata-limitations",
    "rfc-status-folder",
    "handoff-status",
    "errata-tracking",
    "version-currency",
    "doc-links",
    "doc-counts",
    "standards-mapping",
    "evidence",
    "toolchain-declarations",
    "summary-completeness",
    "prose-in-the-book",
    "no-stub-pages",
    "unique-doc-directory-names",
    "historical-status-lines",
    "security-advisories",
    "ci-test-jobs",
];
