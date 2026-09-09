//! Library half of `fjell-consistency-check` (RFC-0.29-002 §7).
//!
//! Exists for exactly one reason: `errata::parse_summary_rows` is the one
//! shared parser for `docs/rfcs/ERRATA.md`'s `## Summary` table, reused by
//! this crate's own `errata-tracking`/`errata-limitations` subchecks *and*
//! by `crates/fjell-tools`'s `release_rehearsal` (Gate 7) — a different
//! crate, which is why this needs a `[lib]` target at all rather than a
//! `pub(crate)` module.

pub mod errata;
