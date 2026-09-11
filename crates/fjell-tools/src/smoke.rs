//! QEMU smoke test runner for `cargo xtask qemu-test [milestone]`.
//!
//! Thin wrapper around `qemu_run::Profile::smoke` + `run_profile`; only the
//! execution path is shared with negative tests (RFC 025).
//!
//! ## "Preserved verbatim from the v0.1.0 runner" — the sentence that was
//! ## this file's defect (E-040, RFC-0.31-001 R6)
//!
//! This comment used to say the milestone → marker mapping was *"preserved
//! verbatim from the v0.1.0 runner"*, and it was — that is precisely what
//! went wrong. The mapping accumulated one arm per milestone while the code
//! that **emits** the markers only ever kept the current one, so the runner
//! went on accepting `m1`–`m6` after nothing could print their markers. Each
//! of those names built the tree, booted QEMU, waited out the full 60-second
//! timeout and reported `FAIL` — indistinguishable from a real regression,
//! and the reason three stray artefact directories under
//! `tests/qemu/artifacts/` led to E-040 being filed at all.
//!
//! **`TEST:M7:PASS` is the cumulative pass for everything M1–M6 ever
//! checked** (`trap/dispatch.rs`: *"init orchestrates M1-M7 and exits after
//! those complete"*). The six were deleted rather than reconnected
//! (RFC-0.31-001 D4): re-adding user-space markers would recreate the
//! concurrent-UART garbling that `9363b91` moved emission into the kernel to
//! fix, and would test nothing `m7` does not.
//!
//! What replaces "verbatim": `MILESTONES` below is the **one** list, and a
//! test in this module reads `crates/fjell-kernel/src/trap/dispatch.rs` and
//! asserts both directions — every marker accepted here is emitted there,
//! and every `…:PASS` emitted there is accepted here. RFC-0.29-002 R5
//! deleted one dead arm from this same `match` by asking what emitted *that*
//! marker; it did not ask which of the names this file offers can pass at
//! all. The check is so that nobody has to ask a third time.

use std::process::ExitCode;

use crate::qemu::build_all;
use crate::qemu_run::{Profile, run_profile};

/// Every milestone `qemu-test` accepts, paired with the marker its PASS is
/// keyed on.
///
/// **The one list** (RFC-0.31-001 D3). The dispatch below, the `known:` line
/// printed on an unknown name, and the kernel-agreement check in this
/// module's tests all read it — there is no second copy to go stale, which
/// is the defect RFC-0.30-002's review removed from `consistency-check`'s
/// dispatcher one crate over.
///
/// `test_all.rs`'s `SMOKE_PROFILES` is deliberately *not* this list: it is
/// the subset `test-all` gates on, a different question, and it stays four.
const MILESTONES: &[(&str, &str)] = &[
    ("m7", "TEST:M7:PASS"),
    ("m8", "TEST:M8:PASS"),
    ("v0.4-net", "TEST:V0.4-NET:PASS"),
    ("v0.5-platform", "TEST:V0.5-PLATFORM:PASS"),
    ("v0.7-sync", "TEST:V0.7-SYNC:PASS"),
];

/// `qemu-test` with no argument runs the current milestone.
///
/// Stated as a name, never as "the last entry in `MILESTONES`"
/// (RFC-0.31-001 Risks): a default that follows the end of a list changes
/// meaning whenever the list grows, silently. A test asserts this name is
/// one `MILESTONES` actually accepts.
const DEFAULT_MILESTONE: &str = "m8";

pub fn cmd_qemu_test(milestone: Option<&str>) -> ExitCode {
    let requested = milestone.unwrap_or(DEFAULT_MILESTONE);

    // RFC-0.24-002 Slice 2: an unrecognised *name* is a hard error, never
    // another route to the default (RFC-0.24-001 Pass 2: `qemu-test
    // totally-bogus-xyz` once ran `m8` and reported PASS for it). Since
    // RFC-0.31-001 this path also catches `m1`-`m6`, which cost a full QEMU
    // boot and a 60-second timeout to reach the same answer.
    let Some((mid, marker)) = lookup(requested) else {
        eprintln!("[xtask] qemu-test: unknown milestone `{requested}`");
        eprintln!("[xtask] known: {}", known_names());
        return ExitCode::FAILURE;
    };

    // Smoke always rebuilds before running so the test reflects the
    // current source tree.
    let _ = build_all();

    let profile = Profile::smoke(mid, marker);
    run_profile(&profile)
}

fn lookup(name: &str) -> Option<(&'static str, &'static str)> {
    MILESTONES.iter().copied().find(|(n, _)| *n == name)
}

fn known_names() -> String {
    MILESTONES
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// RFC-0.31-001 D2: the general question — *which of the milestones this
/// file offers can any of them pass?* — asked once, by a check, against the
/// kernel source that actually emits the markers.
#[cfg(test)]
mod kernel_marker_agreement {
    use super::*;
    use crate::callsite_audit::strip_comments_only;
    use std::path::PathBuf;

    const DISPATCH_REL: &str = "crates/fjell-kernel/src/trap/dispatch.rs";

    /// `cargo test` runs test binaries with the crate's own directory as the
    /// working directory, not the workspace root — the same CWD problem
    /// `callsite_audit`'s tests solve this way.
    fn workspace_root() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The dispatcher's source with comments blanked out. Comments are
    /// removed before anything is checked because **a comment cannot emit a
    /// marker** — `dispatch.rs` has a doc comment naming
    /// `TEST:V0.7-SYNC:PASS` while explaining an index, and it is not an
    /// emission. Blanking rather than deleting keeps byte offsets aligned.
    fn dispatch_src_without_comments() -> String {
        let path = workspace_root().join(DISPATCH_REL);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        strip_comments_only(&src)
    }

    /// Every `kprintln!("TEST:…")` literal, as `(start, end, contents)` byte
    /// spans into the comment-stripped source.
    fn emitted_marker_spans(src: &str) -> Vec<(usize, usize, String)> {
        const OPEN: &str = "kprintln!(\"";
        let mut spans = Vec::new();
        let mut i = 0;
        while let Some(rel) = src[i..].find(OPEN) {
            let start = i + rel + OPEN.len();
            let Some(close_rel) = src[start..].find('"') else {
                break;
            };
            let end = start + close_rel;
            if src[start..end].starts_with("TEST:") {
                spans.push((start, end, src[start..end].to_string()));
            }
            i = end + 1;
        }
        spans
    }

    /// Direction A: every marker this runner accepts is one the kernel
    /// actually emits. This is the assertion E-040 would have failed on for
    /// six milestones.
    #[test]
    fn every_accepted_milestone_marker_is_emitted_by_the_kernel() {
        let src = dispatch_src_without_comments();
        let emitted: Vec<String> = emitted_marker_spans(&src)
            .into_iter()
            .map(|(_, _, lit)| lit)
            .collect();
        for (name, marker) in MILESTONES {
            assert!(
                emitted.contains(&(*marker).to_string()),
                "qemu-test accepts `{name}`, keyed on {marker:?}, which no kprintln! in \
                 {DISPATCH_REL} emits. That is E-040's shape: the name builds, boots QEMU, \
                 waits out the full timeout and reports FAIL indistinguishably from a \
                 regression. Emitted markers found: {emitted:?}"
            );
        }
    }

    /// Direction B: every marker the kernel can print a PASS for is a
    /// milestone this runner can be asked to run.
    ///
    /// `TEST:M7:FAIL (init did not exit cleanly)` is emitted and is not a
    /// `:PASS`; it is not a milestone anyone can request, and is covered by
    /// `no_marker_can_hide_from_this_parser` instead.
    #[test]
    fn every_emitted_pass_marker_is_an_accepted_milestone() {
        let src = dispatch_src_without_comments();
        for (_, _, lit) in emitted_marker_spans(&src) {
            if !lit.ends_with(":PASS") {
                continue;
            }
            assert!(
                MILESTONES.iter().any(|(_, m)| *m == lit),
                "{DISPATCH_REL} emits {lit:?}, which no entry in MILESTONES accepts — \
                 the kernel can pass a milestone qemu-test cannot be asked to run"
            );
        }
    }

    /// The check that keeps the other two honest (RFC-0.31-001 Risks, mode 3
    /// of the defect class: invisible reads as absent, absent reads as
    /// nothing to check).
    ///
    /// Both directions above parse plain string literals. A marker built by
    /// `concat!`, or held in a `const` and printed through `{}`, would be
    /// invisible to that parser — and a marker the parser cannot see looks
    /// exactly like a marker that is not there. So every surviving `TEST:`
    /// in the file (comments already blanked) must lie inside a literal the
    /// parser recognised. Change how a marker is constructed and this fails
    /// on the construction, loudly, instead of the other two passing with
    /// one fewer marker.
    #[test]
    fn no_marker_can_hide_from_this_parser() {
        let src = dispatch_src_without_comments();
        let spans = emitted_marker_spans(&src);
        let mut i = 0;
        while let Some(rel) = src[i..].find("TEST:") {
            let at = i + rel;
            assert!(
                spans
                    .iter()
                    .any(|(start, end, _)| at >= *start && at < *end),
                "{DISPATCH_REL} contains `TEST:` at byte {at} that is not inside a \
                 kprintln! string literal this check can read. A marker built by concat!, \
                 or printed from a const, is invisible to the two agreement checks above — \
                 and invisible is indistinguishable from absent. Context: {:?}",
                &src[at.saturating_sub(60)..(at + 40).min(src.len())]
            );
            i = at + "TEST:".len();
        }
    }

    /// `qemu-test` with no argument must run something it accepts — a
    /// default naming a milestone `lookup` rejects would turn the
    /// no-argument invocation into `unknown milestone`.
    #[test]
    fn the_default_milestone_is_one_this_runner_accepts() {
        assert!(
            lookup(DEFAULT_MILESTONE).is_some(),
            "DEFAULT_MILESTONE is {DEFAULT_MILESTONE:?}, which MILESTONES does not accept"
        );
    }

    /// The `known:` line and the dispatch read the same list (D3) — a
    /// regression here would mean advertising a name that cannot be run, or
    /// running one that is never advertised.
    #[test]
    fn known_names_lists_exactly_what_lookup_accepts() {
        for name in known_names().split(", ") {
            assert!(
                lookup(name).is_some(),
                "`known:` advertises {name:?}, which lookup rejects"
            );
        }
        assert_eq!(known_names().split(", ").count(), MILESTONES.len());
    }

    /// The six E-040 names reach the fail-closed path, and cost no QEMU boot
    /// to get there (D1).
    #[test]
    fn the_six_e040_milestones_are_no_longer_accepted() {
        for dead in ["m1", "m2", "m3", "m4", "m5", "m6"] {
            assert!(
                lookup(dead).is_none(),
                "`{dead}` is still accepted; nothing emits its marker, so it can only \
                 boot QEMU and time out (E-040)"
            );
        }
    }
}
