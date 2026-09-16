//! RFC-0.32-004 D3 — the `security-advisories` subcheck.
//!
//! RFC-v0.15-003 specified an advisory register in v0.15 and marked itself
//! Implemented; the directory never existed, and nothing would have noticed
//! the first malformed record written into it (E-051). This checks the
//! register the way `errata-tracking` checks the errata register, and it has
//! one design constraint above the rest:
//!
//! **It passes on an empty register and fails on a malformed one.** Today the
//! register is empty, which is the true state. When the first advisory is
//! written — under time pressure, by one maintainer — it will be validated by
//! an instrument that has already been running for months, not by one written
//! that afternoon.
//!
//! It fails when:
//!   - a record is missing a required field, repeats one, or has one this
//!     check does not recognise;
//!   - a record's id disagrees with its file name, or two records share an
//!     id, or the sequence within a year skips a number;
//!   - the index lists a record that does not exist, or a record is missing
//!     from the index;
//!   - a record's `Fixed in` names a version that was never tagged — the one
//!     field a reader acts on;
//!   - `.github/SECURITY.md` and the process document state different
//!     acknowledgement windows. That disagreement is how E-051 read on the
//!     day it was filed, and D2 is one commitment.

use crate::read_file;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::process::ExitCode;

const NAME: &str = "security-advisories";

pub const REGISTER_DIR: &str = "docs/src/security/advisories";
const INDEX_FILE: &str = "README.md";
const PROCESS_PATH: &str = "docs/src/security/advisory-process.md";
const SECURITY_MD_PATH: &str = ".github/SECURITY.md";

/// Required fields, in the order RFC-v0.15-003 §3.4 lists them. The release
/// checklist's copy of this template had lost `Mitigation` and `Reproducer`
/// by the time this check was written.
const REQUIRED_FIELDS: &[&str] = &[
    "ID",
    "Severity",
    "Reported",
    "Disclosed",
    "Affected",
    "Fixed in",
    "Reporter",
    "Description",
    "Threat ref",
    "Mitigation",
    "References",
    "Reproducer",
];

/// Recognised but optional. RFC-v0.15-003 §3.5 says a record "carries the CVE
/// id once assigned", and its own template has no field for it — so a check
/// that rejected unknown fields without this would reject the first advisory
/// that ever received a CVE.
const OPTIONAL_FIELDS: &[&str] = &["CVE"];

const SEVERITIES: &[&str] = &["Critical", "High", "Medium", "Low"];

pub fn check() -> ExitCode {
    let index_path = format!("{REGISTER_DIR}/{INDEX_FILE}");
    let Some(index_src) = read_file(NAME, &index_path) else {
        return ExitCode::FAILURE;
    };
    let Some(entries) = crate::read_dir_named(NAME, REGISTER_DIR) else {
        return ExitCode::FAILURE;
    };
    let mut records: Vec<(String, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == INDEX_FILE || !entry.path().is_file() {
            continue;
        }
        match fs::read_to_string(entry.path()) {
            Ok(content) => records.push((name, content)),
            Err(e) => {
                eprintln!("{NAME}: FAIL — cannot read {REGISTER_DIR}/{name}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    records.sort();

    let Some(security_md) = read_file(NAME, SECURITY_MD_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(process_md) = read_file(NAME, PROCESS_PATH) else {
        return ExitCode::FAILURE;
    };

    run_check(
        &index_src,
        &records,
        &repository_tags(),
        &security_md,
        &process_md,
    )
}

/// Every tag in the repository, or `None` if they cannot be listed.
///
/// `None` and "no tags" are reported differently on purpose: a shallow CI
/// checkout has no tags, and a check that treated that as "this version was
/// never tagged" would be wrong in one direction, while one that skipped the
/// comparison would be wrong in the other.
fn repository_tags() -> Option<BTreeSet<String>> {
    let out = std::process::Command::new("git")
        .args(["tag", "--list"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

/// `**Field:** value` lines, in order, as written.
fn parse_fields(src: &str) -> Vec<(String, String)> {
    src.lines()
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix("**")?;
            let (field, value) = rest.split_once(":**")?;
            Some((field.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// The window a document states for one step of the process: found at `label`
/// (the process document's form, e.g. `Acknowledgement:`) or, failing that,
/// `phrase` (SECURITY.md's form, e.g. `An acknowledgement`), then the text
/// after "within" to the end of **that sentence** — case-insensitive, with
/// Markdown emphasis stripped and a trailing "of the report" removed.
///
/// Bounded to one sentence on purpose: the process document's opening
/// paragraph mentions "a severity decision" in a sentence with no window, and
/// an unbounded search would read the next "within" anywhere after it as that
/// step's promise.
fn stated_window(src: &str, label: &str, phrase: &str) -> Option<String> {
    let flat = src.replace('\n', " ").replace("**", "");
    let lower = flat.to_lowercase();
    let at = lower
        .find(label)
        .map(|i| i + label.len())
        .or_else(|| lower.find(phrase).map(|i| i + phrase.len()))?;
    let end = flat[at..].find('.').map_or(flat.len(), |e| at + e);
    let sentence = &flat[at..end];
    let within = sentence.to_lowercase().find("within")? + "within".len();
    let window = sentence[within..]
        .trim()
        .trim_end_matches(" of the report")
        .trim()
        .to_string();
    (!window.is_empty()).then_some(window)
}

/// When the reporter hears the report was received.
fn acknowledgement_window(src: &str) -> Option<String> {
    stated_window(src, "acknowledgement:", "an acknowledgement")
}

/// When the reporter is told the severity — the decision that starts the fix
/// clock. Added at RFC-0.32-004's review with the owner's 14-day commitment:
/// a second promise stated in two documents is a second place for E-051's
/// drift, so it is compared the same way.
fn triage_window(src: &str) -> Option<String> {
    stated_window(src, "severity decision:", "a severity decision")
}

/// `FSAD-2026-001.md` -> `(2026, 1)`.
fn parse_record_name(name: &str) -> Option<(u32, u32)> {
    let stem = name.strip_suffix(".md")?;
    let rest = stem.strip_prefix("FSAD-")?;
    let (year, seq) = rest.split_once('-')?;
    if year.len() != 4 || seq.len() != 3 {
        return None;
    }
    Some((year.parse().ok()?, seq.parse().ok()?))
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Core check, pure in its inputs.
pub fn run_check(
    index_src: &str,
    records: &[(String, String)],
    tags: &Option<BTreeSet<String>>,
    security_md: &str,
    process_md: &str,
) -> ExitCode {
    let mut problems: Vec<String> = Vec::new();

    // ── D2: one commitment, stated identically in both places ────────────
    match (
        acknowledgement_window(security_md),
        acknowledgement_window(process_md),
    ) {
        (Some(a), Some(b)) if a == b => {}
        (Some(a), Some(b)) => problems.push(format!(
            "{SECURITY_MD_PATH} promises acknowledgement within \"{a}\" but {PROCESS_PATH} says \
             \"{b}\" — D2 is one commitment, and two was how E-051 read"
        )),
        (None, _) => problems.push(format!(
            "{SECURITY_MD_PATH} states no acknowledgement window this check can find"
        )),
        (_, None) => problems.push(format!(
            "{PROCESS_PATH} states no acknowledgement window this check can find"
        )),
    }
    match (triage_window(security_md), triage_window(process_md)) {
        (Some(a), Some(b)) if a == b => {}
        (Some(a), Some(b)) => problems.push(format!(
            "{SECURITY_MD_PATH} promises a severity decision within \"{a}\" but {PROCESS_PATH} \
             says \"{b}\" — the triage window is one commitment, stated in two places"
        )),
        (None, _) => problems.push(format!(
            "{SECURITY_MD_PATH} states no severity-decision window this check can find"
        )),
        (_, None) => problems.push(format!(
            "{PROCESS_PATH} states no severity-decision window this check can find"
        )),
    }

    // ── each record ──────────────────────────────────────────────────────
    let mut ids: BTreeMap<(u32, u32), String> = BTreeMap::new();
    let mut id_fields: BTreeMap<String, String> = BTreeMap::new();
    for (name, content) in records {
        let Some((year, seq)) = parse_record_name(name) else {
            problems.push(format!(
                "{REGISTER_DIR}/{name}: not a record name — records are FSAD-<year>-<seq>.md, \
                 and nothing else belongs in the register"
            ));
            continue;
        };
        // The sequence check below is keyed on file names; two files in one
        // directory can never share one, so a duplicate id can only exist in
        // the ID *fields*. That is checked separately, by value, further down.
        ids.insert((year, seq), name.clone());

        let fields = parse_fields(content);
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for (field, value) in &fields {
            let known = REQUIRED_FIELDS
                .iter()
                .chain(OPTIONAL_FIELDS)
                .find(|f| **f == field.as_str());
            match known {
                None => problems.push(format!(
                    "{REGISTER_DIR}/{name}: unrecognised field `{field}` — the fields are \
                     {REQUIRED_FIELDS:?}, and {OPTIONAL_FIELDS:?} optionally"
                )),
                Some(f) => {
                    if seen.insert(f, value.as_str()).is_some() {
                        problems.push(format!(
                            "{REGISTER_DIR}/{name}: field `{field}` appears more than once"
                        ));
                    }
                }
            }
        }
        for required in REQUIRED_FIELDS {
            if !seen.contains_key(required) {
                problems.push(format!(
                    "{REGISTER_DIR}/{name}: missing required field `{required}`"
                ));
            }
        }

        let stem = name.trim_end_matches(".md");
        if let Some(id) = seen.get("ID") {
            if let Some(prev) = id_fields.insert(id.to_string(), name.clone()) {
                problems.push(format!(
                    "{REGISTER_DIR}/{name}: id `{id}` is also claimed by {REGISTER_DIR}/{prev} — \
                     two records cannot share an id"
                ));
            }
            if *id != stem {
                problems.push(format!(
                    "{REGISTER_DIR}/{name}: its ID field says `{id}`, but the file is named `{stem}`"
                ));
            }
        }
        if let Some(sev) = seen.get("Severity") {
            if !SEVERITIES.contains(sev) {
                problems.push(format!(
                    "{REGISTER_DIR}/{name}: severity `{sev}` is not one of {SEVERITIES:?}"
                ));
            }
        }
        for date_field in ["Reported", "Disclosed"] {
            if let Some(d) = seen.get(date_field) {
                if !is_iso_date(d) {
                    problems.push(format!(
                        "{REGISTER_DIR}/{name}: `{date_field}` is `{d}`, not YYYY-MM-DD"
                    ));
                }
            }
        }

        // The one field a reader acts on: it must name a release that exists.
        if let Some(fixed) = seen.get("Fixed in") {
            match tags {
                None => problems.push(format!(
                    "{REGISTER_DIR}/{name}: `Fixed in: {fixed}` cannot be verified — this checkout \
                     lists no tags (a shallow clone?), and a version nobody can confirm was \
                     released is not one to send a reader to"
                )),
                Some(tags) if tags.contains(*fixed) => {}
                Some(tags) => {
                    let hint = fixed
                        .strip_prefix('v')
                        .filter(|bare| tags.contains(*bare))
                        .map(|bare| format!(" (the tag is spelled `{bare}`, without the `v`)"))
                        .unwrap_or_default();
                    problems.push(format!(
                        "{REGISTER_DIR}/{name}: `Fixed in: {fixed}` names a version that was never \
                         tagged{hint}"
                    ));
                }
            }
        }
    }

    // ── ids consecutive within a year ────────────────────────────────────
    let mut by_year: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (year, seq) in ids.keys() {
        by_year.entry(*year).or_default().push(*seq);
    }
    for (year, seqs) in &by_year {
        for (expected, actual) in (1u32..).zip(seqs) {
            if *actual != expected {
                problems.push(format!(
                    "FSAD-{year}: the sequence skips from {:03} to {actual:03} — ids are assigned \
                     in order, so a gap is a record that is missing",
                    expected - 1
                ));
                break;
            }
        }
    }

    // ── the index and the directory agree ────────────────────────────────
    let listed: BTreeSet<String> = index_src
        .lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| {
            let i = l.find("FSAD-")?;
            let id: String = l[i..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            Some(id)
        })
        .collect();
    let on_disk: BTreeSet<String> = records
        .iter()
        .filter(|(n, _)| parse_record_name(n).is_some())
        .map(|(n, _)| n.trim_end_matches(".md").to_string())
        .collect();
    for id in listed.difference(&on_disk) {
        problems.push(format!(
            "{REGISTER_DIR}/{INDEX_FILE} lists {id}, but {REGISTER_DIR}/{id}.md does not exist"
        ));
    }
    for id in on_disk.difference(&listed) {
        problems.push(format!(
            "{REGISTER_DIR}/{id}.md exists but is missing from {REGISTER_DIR}/{INDEX_FILE}"
        ));
    }

    if problems.is_empty() {
        let tag_note = match tags {
            Some(t) => format!("{} tags available to check `Fixed in` against", t.len()),
            None => "tags unavailable, and no record needed them".to_string(),
        };
        println!(
            "{NAME}: PASS ({} advisory record(s) — {}; index and directory agree; \
             SECURITY.md and the process state one acknowledgement window; {tag_note})",
            records.len(),
            if records.is_empty() {
                "the register is empty, which is its true state"
            } else {
                "every required field present, ids unique and consecutive"
            }
        );
        return ExitCode::SUCCESS;
    }

    eprintln!("{NAME}: FAIL");
    for p in &problems {
        eprintln!("  {p}");
    }
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_INDEX: &str = "# Security Advisories\n\n| ID | Severity |\n|---|---|\n";
    const SEC: &str = "- An acknowledgement within 7 days. More.\n\
                       - A severity decision, with the reason, within 14 days of the report.";
    const PROC: &str = "**Acknowledgement:** within **7 days** of the report.\n\
                        **Severity decision:** within **14 days** of the report.";

    fn tags() -> Option<BTreeSet<String>> {
        Some(["0.31.0", "0.32.0"].iter().map(|s| s.to_string()).collect())
    }

    fn record(id: &str, fixed: &str) -> String {
        format!(
            "# {id}\n\n**ID:** {id}\n**Severity:** High\n**Reported:** 2026-01-02\n\
             **Disclosed:** 2026-02-03\n**Affected:** 0.20.0..0.31.0\n**Fixed in:** {fixed}\n\
             **Reporter:** anonymous\n**Description:** x\n**Threat ref:** T13\n\
             **Mitigation:** none\n**References:** E-999\n**Reproducer:** withheld\n"
        )
    }

    fn index_listing(ids: &[&str]) -> String {
        let mut s = EMPTY_INDEX.to_string();
        for id in ids {
            s.push_str(&format!("| [{id}](./{id}.md) | High |\n"));
        }
        s
    }

    /// Demonstration 2's control, as a test: the register the tree ships.
    #[test]
    fn an_empty_register_passes() {
        assert_eq!(
            run_check(EMPTY_INDEX, &[], &tags(), SEC, PROC),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn a_well_formed_record_passes() {
        let r = vec![(
            "FSAD-2026-001.md".to_string(),
            record("FSAD-2026-001", "0.32.0"),
        )];
        assert_eq!(
            run_check(&index_listing(&["FSAD-2026-001"]), &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn a_missing_required_field_fails() {
        let body = record("FSAD-2026-001", "0.32.0").replace("**Mitigation:** none\n", "");
        let r = vec![("FSAD-2026-001.md".to_string(), body)];
        assert_ne!(
            run_check(&index_listing(&["FSAD-2026-001"]), &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn an_unrecognised_field_fails_but_cve_is_recognised() {
        let with_cve = record("FSAD-2026-001", "0.32.0") + "**CVE:** CVE-2026-0001\n";
        let r = vec![("FSAD-2026-001.md".to_string(), with_cve)];
        assert_eq!(
            run_check(&index_listing(&["FSAD-2026-001"]), &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS,
            "CVE is promised by RFC-v0.15-003 §3.5 and must be accepted"
        );
        let with_junk = record("FSAD-2026-001", "0.32.0") + "**Severty:** High\n";
        let r = vec![("FSAD-2026-001.md".to_string(), with_junk)];
        assert_ne!(
            run_check(&index_listing(&["FSAD-2026-001"]), &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS
        );
    }

    /// Two files cannot share a name, so a duplicate can only be two records
    /// whose ID fields agree. The first version of this check keyed on file
    /// names and could never fire; demonstration 1b refused the pair for a
    /// different reason, which is how it was found.
    #[test]
    fn a_duplicate_id_is_named_as_a_duplicate() {
        let a = record("FSAD-2026-001", "0.32.0");
        let dup = [
            ("FSAD-2026-001.md".to_string(), a.clone()),
            ("FSAD-2026-002.md".to_string(), a),
        ];
        let fields: Vec<_> = dup.iter().map(|(_, c)| parse_fields(c)).collect();
        let ids: Vec<_> = fields
            .iter()
            .map(|f| f.iter().find(|(k, _)| k == "ID").unwrap().1.clone())
            .collect();
        assert_eq!(ids[0], ids[1], "the fixture must actually share an id");
    }

    #[test]
    fn a_duplicate_id_fails() {
        let r = vec![
            (
                "FSAD-2026-001.md".to_string(),
                record("FSAD-2026-001", "0.32.0"),
            ),
            (
                "FSAD-2026-002.md".to_string(),
                record("FSAD-2026-001", "0.32.0"),
            ),
        ];
        assert_ne!(
            run_check(
                &index_listing(&["FSAD-2026-001", "FSAD-2026-002"]),
                &r,
                &tags(),
                SEC,
                PROC
            ),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn a_gap_in_the_sequence_fails() {
        let r = vec![
            (
                "FSAD-2026-001.md".to_string(),
                record("FSAD-2026-001", "0.32.0"),
            ),
            (
                "FSAD-2026-003.md".to_string(),
                record("FSAD-2026-003", "0.32.0"),
            ),
        ];
        assert_ne!(
            run_check(
                &index_listing(&["FSAD-2026-001", "FSAD-2026-003"]),
                &r,
                &tags(),
                SEC,
                PROC
            ),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn the_index_and_directory_must_agree_both_ways() {
        let r = vec![(
            "FSAD-2026-001.md".to_string(),
            record("FSAD-2026-001", "0.32.0"),
        )];
        assert_ne!(
            run_check(EMPTY_INDEX, &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS,
            "a record missing from the index"
        );
        assert_ne!(
            run_check(&index_listing(&["FSAD-2026-001"]), &[], &tags(), SEC, PROC),
            ExitCode::SUCCESS,
            "an index row with no record"
        );
    }

    /// The one field a reader acts on.
    #[test]
    fn fixed_in_must_name_a_tag_that_exists() {
        let r = vec![(
            "FSAD-2026-001.md".to_string(),
            record("FSAD-2026-001", "0.99.0"),
        )];
        assert_ne!(
            run_check(&index_listing(&["FSAD-2026-001"]), &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS
        );
        // The RFC's template writes `vA.B.C`; this repository's tags have no `v`.
        let r = vec![(
            "FSAD-2026-001.md".to_string(),
            record("FSAD-2026-001", "v0.32.0"),
        )];
        assert_ne!(
            run_check(&index_listing(&["FSAD-2026-001"]), &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS
        );
    }

    /// Fail closed: no tags to check against is not permission to skip.
    #[test]
    fn fixed_in_fails_closed_when_tags_are_unavailable() {
        let r = vec![(
            "FSAD-2026-001.md".to_string(),
            record("FSAD-2026-001", "0.32.0"),
        )];
        assert_ne!(
            run_check(&index_listing(&["FSAD-2026-001"]), &r, &None, SEC, PROC),
            ExitCode::SUCCESS
        );
        // ...but an empty register needs no tags at all.
        assert_eq!(
            run_check(EMPTY_INDEX, &[], &None, SEC, PROC),
            ExitCode::SUCCESS
        );
    }

    /// D2 held by an instrument: the two documents cannot drift apart again.
    #[test]
    fn security_md_and_the_process_must_state_one_window() {
        assert_ne!(
            run_check(
                EMPTY_INDEX,
                &[],
                &tags(),
                "- An acknowledgement within a small number of days.",
                PROC
            ),
            ExitCode::SUCCESS
        );
    }

    /// The owner's second commitment, held the same way as the first.
    #[test]
    fn security_md_and_the_process_must_state_one_triage_window() {
        let sec = SEC.replace("within 14 days", "within 21 days");
        assert_ne!(
            run_check(EMPTY_INDEX, &[], &tags(), &sec, PROC),
            ExitCode::SUCCESS
        );
        let silent = "- An acknowledgement within 7 days.";
        assert_ne!(
            run_check(EMPTY_INDEX, &[], &tags(), silent, PROC),
            ExitCode::SUCCESS
        );
    }

    /// A step named in a sentence without a window must not borrow the next
    /// sentence's "within": the process document opens with exactly that.
    #[test]
    fn a_window_is_read_from_its_own_sentence_only() {
        let src = "we promise a severity decision you are told about. \
                   Something else happens within 3 days.";
        assert_eq!(triage_window(src), None);
        assert_eq!(
            triage_window("A severity decision, with the reason, within 14 days of the report."),
            Some("14 days".to_string())
        );
    }

    #[test]
    fn a_stray_file_in_the_register_fails() {
        let r = vec![("notes.md".to_string(), "# scratch\n".to_string())];
        assert_ne!(
            run_check(EMPTY_INDEX, &r, &tags(), SEC, PROC),
            ExitCode::SUCCESS
        );
    }
}
