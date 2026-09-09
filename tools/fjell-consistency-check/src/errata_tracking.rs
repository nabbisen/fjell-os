//! RFC-0.27-001: the `errata-tracking` subcheck (S1).
//!
//! `docs/rfcs/ERRATA.md`'s `## Summary` table has a tracking column that is
//! the only thing in this project that could serve as a scheduling source,
//! and it was stale through two releases: E-014/E-015/E-016/E-017 all read
//! `"0.25 candidate"` after 0.25 *and* 0.26 had shipped. Nothing read that
//! column, so nothing noticed.
//!
//! Three properties, checked here:
//!
//! 1. **Every tracking value parses** as an RFC identifier (`RFC-0.26-004`,
//!    `RFC-v0.16-001`), a bare milestone (`0.27`), or the literal
//!    `unscheduled`. Prose — parenthetical commentary, trailing words,
//!    anything else — is rejected: it is exactly what made the column
//!    unreadable by a check in the first place.
//! 2. **No erratum names a milestone that has already shipped**, unless the
//!    erratum is `CLOSED`. A bare milestone is a promise ("this will be
//!    addressed by then"); once shipped without one, that promise is
//!    broken and stale. A `CLOSED` entry naming the milestone it actually
//!    shipped in (for entries that predate this project's practice of
//!    tracking by RFC id — E-003, E-010) is accurate history, not a broken
//!    promise, so `CLOSED` is exempted from this rule rather than forced
//!    into a fabricated RFC id.
//! 3. **RFC ↔ erratum agreement is bidirectional.** An erratum tracked by
//!    an RFC id must be named somewhere in that RFC's file (same shape as
//!    `errata-limitations`'s substring check against
//!    `v1-limitations.md`) — direction A. An RFC that claims to close an
//!    erratum must be the one that erratum's tracking field names —
//!    direction B, the direction `errata-limitations` does not check.
//!
//! ## Design question: how does the check know what has shipped?
//!
//! Three candidates were named in the RFC: `git tag`, `CHANGELOG.md`
//! headings, or `docs/release/records/*.md`. **Chosen: release records.**
//! Every other subcheck in this tool reads only committed files, never
//! shells out to git — a syscall-surface or handoff-status run behaves
//! identically in a full clone, a shallow clone, or an exported tarball,
//! and `git tag` would not (a shallow clone often has no tags at all).
//! `CHANGELOG.md` was rejected too: its headings mix release entries with
//! prose subsections and would need heavier parsing to avoid
//! false-positives, where each release record is a single-purpose file
//! whose `**Version:**` field is exactly the fact this check needs, no more.
//! The trade-off: a shipped release with no record file (none exist before
//! `0.21.3`) is invisible to this check. Every erratum this RFC's data
//! concerns (E-011 onward) is well inside the range `docs/release/records/`
//! covers, so this is accepted rather than backfilled.
//!
//! ## Design note: direction B's "claims to close" detector
//!
//! There is no single committed phrasing for "this RFC closes E-0NN" —
//! observed variants include `"closes **E-016**"`, `"expected to close
//! **E-021**"`, and `"ERRATA **E-019** (this RFC closes it)"`, which puts
//! the erratum id *before* the word "closes". Rather than pattern-match one
//! phrasing (RFCs are prose, not a DSL), direction B looks for an erratum
//! id and the stem `"clos"` (covers close/closes/closing) within the same
//! **paragraph and clause** as the id, inside the file's header block
//! (first 20 lines) — see `header_claims_close`'s own doc comment for the
//! two-scope design and the two false positives against the current tree
//! that made both scopes necessary (a byte window was tried first and
//! bled a "closes" claim into an unrelated erratum's clause in the same
//! paragraph; clause-scoping alone then bled a *different* RFC's claim
//! across a paragraph break into unrelated prose). This is a heuristic,
//! not a parser: a false positive here means a *stricter* check, not a
//! missed one — direction B failing spuriously blocks nothing but its own
//! green tick, whereas direction A (the one that catches a stale or
//! nonexistent RFC reference) never depends on this heuristic at all.
//! `rfcs/archive/` is excluded from direction B entirely, for an unrelated
//! reason — see the exclusion at its call site.

use crate::read_file;
use fjell_consistency_check::errata::{Tracking, classify_tracking, is_closed, parse_summary_rows};
use std::collections::BTreeSet;
use std::fs;
use std::process::ExitCode;

const ERRATA_PATH: &str = "docs/rfcs/ERRATA.md";
const RECORDS_DIR: &str = "docs/release/records";
const RFC_DIRS: &[&str] = &[
    "rfcs/proposed",
    "rfcs/accepted",
    "rfcs/done",
    "rfcs/archive",
];

/// Extract the `**Version:**` field from a release record and reduce it to
/// `major.minor` (the shape errata tracking uses for a bare milestone).
fn parse_record_milestone(src: &str) -> Option<String> {
    let line = src.lines().find(|l| l.contains("**Version:**"))?;
    let start = line.find('`')?;
    let rest = &line[start + 1..];
    let end = rest.find('`')?;
    let version = &rest[..end];
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        Some(format!("{}.{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// Every erratum id (`E-NNN`) appearing in `src`, using the same recognition
/// rule as `extract_erratum_id` but scanning the whole text rather than one
/// table cell.
fn erratum_ids_in(src: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while let Some(pos) = src[i..].find("E-") {
        let start = i + pos;
        let rest = &src[start + 2..];
        let digit_len = rest.chars().take_while(char::is_ascii_digit).count();
        if digit_len > 0 {
            found.push((start, format!("E-{}", &rest[..digit_len])));
        }
        i = start + 2;
        if i >= bytes.len() {
            break;
        }
    }
    found
}

/// Direction B's heuristic: does `header` (conventionally the file's first
/// ~20 lines) contain the stem `"clos"` in the same **clause**, within the
/// same **paragraph**, as an occurrence of `id`?
///
/// Two scopes, because one alone was not enough — each was tried and each
/// produced a live false positive against the current tree before the
/// other was added:
///
/// - **Paragraph** (text between blank lines) first: `RFC-0.24-002`'s
///   `**Relates to:**` paragraph ends `"...ERRATA E-013."`, and the very
///   next paragraph (`## Summary`'s prose) happens to contain "close-out" —
///   a clause-only search with no paragraph boundary walks straight past
///   the sentence-ending period into unrelated prose and wrongly credits
///   this RFC with closing E-013.
/// - **Clause** (text between `,`/`;`) within that paragraph: this
///   project's RFCs write `**Relates to:**` as a comma/semicolon-separated
///   list, each item carrying its own parenthetical, e.g. *"ERRATA
///   **E-019** (this RFC closes it), **E-010** (why IPC negative coverage
///   going dark is not a small thing)"* — paragraph-scoping alone would
///   still credit this one paragraph's "closes it" to **E-010**, which the
///   sentence never claims to close; it is cited only as background for a
///   *different* erratum's clause.
/// `path`'s own RFC id (the `RFC-...` filename prefix, before the slug),
/// so `header_claims_close` can tell "this RFC closes X" from a clause
/// whose real grammatical subject is a *different*, explicitly-cited RFC.
fn own_rfc_id(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let rest = name.strip_prefix("RFC-")?;
    // The id is `RFC-` + version chars (alnum/./-) up to the first `-`
    // that starts a lowercase slug word (a version component is only
    // digits, optionally `v`-prefixed, per `classify_tracking`'s own
    // rule) — simplest robust cut: consume alnum/`.` greedily, plus `-`
    // only when immediately followed by a digit or `v`+digit (another
    // version segment), stopping at the slug's first word.
    let mut end = 0usize;
    let bytes = rest.as_bytes();
    while end < bytes.len() {
        let c = bytes[end] as char;
        if c.is_ascii_alphanumeric() || c == '.' {
            end += 1;
            continue;
        }
        if c == '-' {
            let next = rest[end + 1..].chars().next();
            let starts_version = matches!(next, Some(d) if d.is_ascii_digit())
                || (next == Some('v')
                    && rest[end + 2..]
                        .chars()
                        .next()
                        .is_some_and(|d| d.is_ascii_digit()));
            if starts_version {
                end += 1;
                continue;
            }
        }
        break;
    }
    if end == 0 {
        return None;
    }
    Some(format!("RFC-{}", &rest[..end]))
}

fn header_claims_close(header: &str, id: &str, own_id: Option<&str>) -> bool {
    for (pos, found_id) in erratum_ids_in(header) {
        if found_id != id {
            continue;
        }
        let para_lo = header[..pos].rfind("\n\n").map(|p| p + 2).unwrap_or(0);
        let para_hi = header[pos..]
            .find("\n\n")
            .map(|p| pos + p)
            .unwrap_or(header.len());
        let paragraph = &header[para_lo..para_hi];
        let pos_in_para = pos - para_lo;
        let lo = paragraph[..pos_in_para]
            .rfind([',', ';'])
            .map(|p| p + 1)
            .unwrap_or(0);
        let hi = paragraph[pos_in_para..]
            .find([',', ';'])
            .map(|p| pos_in_para + p)
            .unwrap_or(paragraph.len());
        let clause = &paragraph[lo..hi];
        if !clause.to_ascii_lowercase().contains("clos") {
            continue;
        }
        // The clause's real subject may be a *different*, explicitly
        // cited RFC — e.g. a `**Relates to:**` item reading "RFC-0.28-005
        // (which closed one E-015 instance and established the
        // template)" is background about what RFC-0.28-005 did, not a
        // first-person claim by the file being scanned. Found live:
        // RFC-0.29-001's own header cites RFC-0.28-005 this way in the
        // same clause as E-015.
        if let Some(cited) = find_rfc_id_in(clause) {
            if Some(cited.as_str()) != own_id {
                continue;
            }
        }
        return true;
    }
    false
}

/// The first `RFC-<id>` token in `text`, if any — same character-class
/// rule as `classify_tracking`'s own RFC-id validation.
fn find_rfc_id_in(text: &str) -> Option<String> {
    let pos = text.find("RFC-")?;
    let rest = &text[pos + "RFC-".len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(format!("RFC-{}", &rest[..end]))
}

pub fn check() -> ExitCode {
    let Some(errata_src) = read_file(ERRATA_PATH) else {
        return ExitCode::FAILURE;
    };

    let Ok(record_entries) = fs::read_dir(RECORDS_DIR) else {
        eprintln!("consistency-check: cannot read {RECORDS_DIR}");
        return ExitCode::FAILURE;
    };
    let mut record_srcs = Vec::new();
    for entry in record_entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            if let Ok(content) = fs::read_to_string(&path) {
                record_srcs.push(content);
            }
        }
    }

    let mut rfc_files: Vec<(String, String)> = Vec::new();
    for dir in RFC_DIRS {
        let Ok(entries) = fs::read_dir(dir) else {
            eprintln!("consistency-check: cannot read {dir}");
            return ExitCode::FAILURE;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md")
                && path.file_name().and_then(|n| n.to_str()) != Some("README.md")
            {
                if let Ok(content) = fs::read_to_string(&path) {
                    rfc_files.push((path.to_string_lossy().to_string(), content));
                }
            }
        }
    }

    let shipped: BTreeSet<String> = record_srcs
        .iter()
        .filter_map(|s| parse_record_milestone(s))
        .collect();
    let refs: Vec<(&str, &str)> = rfc_files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();

    run_check(&errata_src, &shipped, &refs)
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
pub fn run_check(
    errata_src: &str,
    shipped: &BTreeSet<String>,
    rfc_files: &[(&str, &str)],
) -> ExitCode {
    let rows = parse_summary_rows(errata_src);
    let mut problems: Vec<String> = Vec::new();

    for row in &rows {
        let tracking = classify_tracking(&row.tracking);
        match &tracking {
            Tracking::Invalid => {
                problems.push(format!(
                    "{}: tracking value {:?} does not parse as an RFC id, a bare milestone, or `unscheduled`",
                    row.id, row.tracking
                ));
                continue;
            }
            Tracking::Milestone(m) => {
                let closed = is_closed(&row.status);
                if !closed && shipped.contains(m) {
                    problems.push(format!(
                        "{}: tracking names milestone {m}, which has already shipped, but status is {:?} (not CLOSED)",
                        row.id, row.status
                    ));
                }
            }
            Tracking::Unscheduled => {}
            Tracking::RfcId(rfc_id) => {
                // Direction A: the named RFC's file must mention this erratum.
                let found = rfc_files
                    .iter()
                    .find(|(path, _)| file_matches_rfc_id(path, rfc_id));
                match found {
                    None => problems.push(format!(
                        "{}: tracking names {rfc_id}, but no file under rfcs/{{proposed,accepted,done,archive}} matches that id",
                        row.id
                    )),
                    Some((path, content)) => {
                        if !content.contains(&row.id) {
                            problems.push(format!(
                                "{}: tracking names {rfc_id} ({path}), but that file never mentions {}",
                                row.id, row.id
                            ));
                        }
                    }
                }
            }
        }
    }

    // Direction B: an RFC claiming to close E-NNN must be the RFC E-NNN's
    // own tracking field names. `rfcs/archive/` is excluded: an archived
    // RFC was superseded precisely because its premise or plan did not
    // hold, and its header's original "closes E-NNN" claim is exactly the
    // part that got superseded — checking it against the live register
    // would demand pointing the erratum back at a withdrawn RFC. Direction
    // A has no such exclusion: if an erratum's tracking field ever does
    // name an archived RFC, that file must still actually mention it.
    for (path, content) in rfc_files {
        if path.contains("/archive/") {
            continue;
        }
        let header: String = content.lines().take(20).collect::<Vec<_>>().join("\n");
        let own_id = own_rfc_id(path);
        let mut claimed: BTreeSet<String> = BTreeSet::new();
        for (_, id) in erratum_ids_in(&header) {
            if header_claims_close(&header, &id, own_id.as_deref()) {
                claimed.insert(id);
            }
        }
        for id in claimed {
            let Some(row) = rows.iter().find(|r| r.id == id) else {
                continue; // erratum id in RFC prose with no register entry — not this check's concern
            };
            let names_this_rfc = matches!(
                classify_tracking(&row.tracking),
                Tracking::RfcId(ref rfc_id) if file_matches_rfc_id(path, rfc_id)
            );
            if !names_this_rfc {
                problems.push(format!(
                    "{path} claims to close {id}, but {id}'s tracking field is {:?}, not this file",
                    row.tracking
                ));
            }
        }
    }

    if problems.is_empty() {
        println!("errata-tracking: PASS ({} entries checked)", rows.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("errata-tracking: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

/// Does `path`'s filename start with `rfc_id` (stripped of its `RFC-`
/// prefix is not needed — filenames carry the same prefix) followed by a
/// `-` or `.`? e.g. `rfc_id = "RFC-v0.16-001"` matches
/// `rfcs/done/RFC-v0.16-001-ed25519-interoperability-closure.md`.
fn file_matches_rfc_id(path: &str, rfc_id: &str) -> bool {
    let Some(name) = path.rsplit('/').next() else {
        return false;
    };
    name.strip_prefix(rfc_id)
        .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped_fixture() -> BTreeSet<String> {
        ["0.24", "0.25", "0.26"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn classifies_rfc_ids() {
        assert_eq!(
            classify_tracking("RFC-0.26-004"),
            Tracking::RfcId("RFC-0.26-004".to_string())
        );
        assert_eq!(
            classify_tracking("RFC-v0.16-001"),
            Tracking::RfcId("RFC-v0.16-001".to_string())
        );
    }

    #[test]
    fn classifies_bare_milestones() {
        assert_eq!(
            classify_tracking("0.27"),
            Tracking::Milestone("0.27".to_string())
        );
    }

    #[test]
    fn classifies_unscheduled() {
        assert_eq!(classify_tracking("unscheduled"), Tracking::Unscheduled);
    }

    #[test]
    fn rejects_prose() {
        assert_eq!(
            classify_tracking("0.25 candidate (recorded, not fixed)"),
            Tracking::Invalid
        );
        assert_eq!(
            classify_tracking("RFC after v0.23.0 (recorded, not fixed)"),
            Tracking::Invalid
        );
        assert_eq!(
            classify_tracking("v0.21.3-001 (v0.22 disposition)"),
            Tracking::Invalid
        );
        assert_eq!(classify_tracking("v0.20.0 fix"), Tracking::Invalid);
    }

    /// Required failure demonstration, rule 1: the exact tracking values
    /// recorded in ERRATA.md before this RFC's normalisation.
    #[test]
    fn unnormalised_tracking_values_fail_rule_1() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-014 x | 0.25 candidate (recorded, not fixed) | ACCEPTED |
"#;
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[]),
            ExitCode::FAILURE
        );
    }

    /// Required failure demonstration, rule 2 — the RFC's own framing: E-014
    /// through E-017 all read `0.25 candidate` after 0.25 (and 0.26) shipped.
    #[test]
    fn accepted_erratum_naming_a_shipped_milestone_fails() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-014 x | 0.25 | ACCEPTED |
"#;
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[]),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn accepted_erratum_naming_an_unshipped_milestone_passes() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-019 x | 0.27 | ACCEPTED |
"#;
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[]),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn closed_erratum_naming_a_shipped_milestone_passes() {
        // E-003/E-010 shape: predates RFC-id tracking, fixed ad hoc within
        // a release rather than by a named RFC.
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-003 x | 0.15 | CLOSED |
"#;
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[]),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn unscheduled_never_fails_rule_2() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-014 x | unscheduled | ACCEPTED |
"#;
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[]),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn direction_a_passes_when_rfc_file_mentions_the_erratum() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-020 x | RFC-0.26-004 | CLOSED |
"#;
        let rfc = "rfcs/done/RFC-0.26-004-readiness-channel.md";
        let content = "closes **E-020**\n";
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[(rfc, content)]),
            ExitCode::SUCCESS
        );
    }

    /// Required failure demonstration, rule 3 direction A: tracking names
    /// an RFC that exists but never mentions the erratum.
    #[test]
    fn direction_a_fails_when_rfc_file_never_mentions_the_erratum() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-020 x | RFC-0.26-004 | CLOSED |
"#;
        let rfc = "rfcs/done/RFC-0.26-004-readiness-channel.md";
        let content = "nothing relevant here\n";
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[(rfc, content)]),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn direction_a_fails_when_the_named_rfc_file_does_not_exist() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-020 x | RFC-0.26-999 | CLOSED |
"#;
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[]),
            ExitCode::FAILURE
        );
    }

    /// Required failure demonstration, rule 3 direction B — the live case
    /// this RFC's own investigation found: RFC-0.26-003 claims (in its own
    /// header) to close E-019, but the register still tracked it as a bare
    /// milestone candidate rather than naming that RFC.
    #[test]
    fn direction_b_fails_when_an_rfc_claims_a_close_the_register_does_not_reflect() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-019 x | 0.27 | ACCEPTED |
"#;
        let rfc = "rfcs/accepted/RFC-0.26-003-ipc-blocked-recv-rendezvous.md";
        let content = "**Relates to:** ERRATA **E-019** (this RFC closes it)\n";
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[(rfc, content)]),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn direction_b_passes_when_the_register_names_the_claiming_rfc() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-019 x | RFC-0.26-003 | ACCEPTED |
"#;
        let rfc = "rfcs/accepted/RFC-0.26-003-ipc-blocked-recv-rendezvous.md";
        let content = "**Relates to:** ERRATA **E-019** (this RFC closes it)\n";
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[(rfc, content)]),
            ExitCode::SUCCESS
        );
    }

    /// The live false-positive that motivated clause-splitting over a byte
    /// window: RFC-0.26-003 cites E-010 as background inside a *different*
    /// erratum's "closes" clause. A window search flags E-010 too; a
    /// clause search must not.
    #[test]
    fn direction_b_does_not_bleed_a_close_claim_into_the_next_clause() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-010 x | 0.20 | CLOSED |
| E-019 x | RFC-0.26-003 | ACCEPTED |
"#;
        let rfc = "rfcs/accepted/RFC-0.26-003-ipc-blocked-recv-rendezvous.md";
        let content = "**Relates to:** ERRATA **E-019** (this RFC closes it), **E-010** (why IPC negative coverage going dark is not a small thing)\n";
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[(rfc, content)]),
            ExitCode::SUCCESS,
            "E-010 must not be treated as claimed-closed just because \"closes\" appears in E-019's adjacent clause"
        );
    }

    /// The live false-positive that motivated paragraph-scoping on top of
    /// clause-splitting: RFC-0.24-002's `**Relates to:**` paragraph ends
    /// "...ERRATA E-013." with no comma after it, so a clause-only search
    /// walks straight through the paragraph break into the next
    /// paragraph's unrelated prose, which happens to contain "close-out".
    #[test]
    fn direction_b_does_not_bleed_a_close_claim_across_a_paragraph_break() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-013 x | unscheduled | ACCEPTED |
"#;
        let rfc = "rfcs/done/RFC-0.24-002-instrument-repairs.md";
        let content = "**Relates to:** RFC-0.24-001, RFC-v0.22-001, ERRATA E-013.\n\n\
             ## Summary\n\n\
             Most are dispositioned to a deferred family or to the audit's close-out.\n";
        assert_eq!(
            run_check(errata, &shipped_fixture(), &[(rfc, content)]),
            ExitCode::SUCCESS,
            "E-013 must not be treated as claimed-closed just because \"close-out\" appears in the next paragraph"
        );
    }

    /// The live false-positive this design found and excluded: an archived
    /// RFC's header still carries its original "closes E-NNN" claim, but
    /// the erratum was actually closed by whatever superseded it.
    #[test]
    fn direction_b_ignores_archived_rfcs() {
        let errata = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-020 x | RFC-0.26-004 | CLOSED |
"#;
        let archived_rfc = "rfcs/archive/RFC-0.26-002-abdd-path-synchronisation.md";
        let archived_content = "**Relates to:** ERRATA **E-020** (this RFC closes it)\n";
        let closing_rfc = "rfcs/done/RFC-0.26-004-readiness-channel.md";
        let closing_content = "closes **E-020**\n";
        assert_eq!(
            run_check(
                errata,
                &shipped_fixture(),
                &[
                    (archived_rfc, archived_content),
                    (closing_rfc, closing_content)
                ]
            ),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn header_claims_close_handles_id_before_the_word_closes() {
        let header = "**Relates to:** ERRATA **E-019** (this RFC closes it)";
        assert!(header_claims_close(header, "E-019", None));
    }

    #[test]
    fn header_claims_close_handles_id_after_the_word_closes() {
        let header = "**Relates to:** closes **E-016** and **E-023**";
        assert!(header_claims_close(header, "E-016", None));
        assert!(header_claims_close(header, "E-023", None));
    }

    /// RFC-0.29-002 R3: `header_claims_close`'s "clos" stem search does
    /// not recognise this project's own `**Tracks.**` field convention —
    /// found live in RFC-0.28-003's committed header (`**Tracks.**
    /// **E-019** — ...`, no "clos" stem anywhere nearby), and confirmed as
    /// a real historical incident, not a hypothetical one: E-019's
    /// tracking field sat stale at `RFC-0.26-003` after RFC-0.28-003 was
    /// accepted, undetected by this check, and was only retracked because
    /// it was manually noticed to be the right thing to do.
    ///
    /// **Deliberately not fixed here.** A `**Tracks.**`-based extension
    /// was built and then reverted: this project tracks some errata
    /// across a *sequence* of RFCs as work continues (E-015: tracked by
    /// RFC-0.29-001, then handed to RFC-0.29-002 once one instance
    /// survived it) — a later RFC's `**Tracks.**` field naming the same
    /// erratum an *earlier*, already-`Implemented` RFC's header also
    /// tracks is not a wrong claim, it is two true statements about two
    /// different points in the erratum's history. Treating every
    /// `**Tracks.**` mention as an exclusive, current claim broke on this
    /// exact case in testing (`RFC-0.29-001`'s own header, once E-015 was
    /// retracked to `RFC-0.29-002`). Recorded as E-014's second surviving
    /// instance rather than shipped as a fix with a demonstrated
    /// false-positive.
    #[test]
    fn header_claims_close_does_not_recognise_the_tracks_field_convention() {
        let header = "**Tracks.** **E-019** — `tests/qemu/profiles/ipc.toml` passes and nothing holds it there.\n**Touches.** something else entirely.";
        assert!(
            !header_claims_close(header, "E-019", None),
            "documents the known, disclosed gap -- not a claim this is fixed"
        );
    }

    /// RFC-0.29-002 R3: a second, distinct `header_claims_close` defect,
    /// found live while retracking E-015 to this RFC. RFC-0.29-001's own
    /// `**Relates to:**` clause reads *"RFC-0.28-005 (which closed one
    /// E-015 instance and established the template)"* — background about
    /// what a *different* RFC did, not a first-person claim by
    /// RFC-0.29-001 itself. Before this fix, the clause-scoped "clos" stem
    /// search could not tell the difference and flagged RFC-0.29-001 as
    /// claiming to close E-015 the moment E-015's tracking field moved to
    /// RFC-0.29-002 — a real regression this exact check hit during this
    /// RFC's own work, not a hypothetical.
    #[test]
    fn header_claims_close_attributes_a_cited_rfcs_own_action_correctly() {
        let header = "**Relates to:** RFC-0.24-001 (which found both); RFC-0.28-005 (which closed one E-015 instance and established the template); RFC-v0.22-001 (a new tier is a new instrument and must be demonstrated failing).";
        assert!(
            !header_claims_close(header, "E-015", Some("RFC-0.29-001")),
            "the clause credits RFC-0.28-005 with closing it, not RFC-0.29-001"
        );
        assert!(
            header_claims_close(header, "E-015", Some("RFC-0.28-005")),
            "RFC-0.28-005's own file should still recognise this as its own claim"
        );
    }

    #[test]
    fn own_rfc_id_extracts_version_stopping_at_the_slug() {
        assert_eq!(
            own_rfc_id("rfcs/accepted/RFC-0.28-003-blocked-recv-rendezvous.md"),
            Some("RFC-0.28-003".to_string())
        );
        assert_eq!(
            own_rfc_id("rfcs/done/RFC-v0.16-001-ed25519-interoperability-closure.md"),
            Some("RFC-v0.16-001".to_string())
        );
    }

    #[test]
    fn parse_record_milestone_reduces_to_major_minor() {
        let src = "**Version:** `0.26.0`\n";
        assert_eq!(parse_record_milestone(src), Some("0.26".to_string()));
    }

    #[test]
    fn file_matches_rfc_id_requires_boundary_after_prefix() {
        assert!(file_matches_rfc_id(
            "rfcs/done/RFC-v0.16-001-ed25519-interoperability-closure.md",
            "RFC-v0.16-001"
        ));
        // RFC-v0.16-0010 (if it existed) must not match RFC-v0.16-001.
        assert!(!file_matches_rfc_id(
            "rfcs/done/RFC-v0.16-0010-something.md",
            "RFC-v0.16-001"
        ));
    }
}
