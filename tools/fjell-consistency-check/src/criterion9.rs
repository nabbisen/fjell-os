//! 0.50 small item C: **a release record names the two CI runs criterion 9 reads.**
//!
//! Criterion 9 reads **two** runs of `ci.yml` -- the release commit's *push* run and a *dispatched* run -- because neither holds the
//! whole job set: `fuzz-run` and `miri` are skipped on push, and `ci-pages` is guarded by `github.event_name != 'workflow_dispatch'`
//! (`v0-release-cycle.md`). The record is what the project has of that reading (*"read from the run, never from anything else"*), and
//! `releases/0.49.0.md` names **one**: the push run, *"not a dispatch"*, with `fuzz-run` and `miri` listed as *skipped by design*.
//! Nothing in the tree could say so, because `release-records` checked structure and not this.
//!
//! **The rule is read from the record's own wording, scoped to the criterion-9 section, and label-based.** Three measured reasons,
//! each a way a simpler rule would have passed the one record it exists to catch:
//!
//! * **Document-scoped counting passes `0.49.0`.** The record holds three long numbers: `45843432462` (the push run, the only one
//!   in the criterion-9 section), `45843464701` (the **`release` workflow's** run, in *The GitHub Release*) and a Pages *artifact*
//!   id. Counting ids over the document gets three. So the section is extracted first.
//! * **"Count GitHub run ids" is not enough either:** one of those decoys *is* a GitHub run id, of the wrong workflow. So an id
//!   counts only when it is **labelled** `push` or `dispatch` in the section's own text -- how `0.32.0`–`0.48.0` already write it.
//! * **The shape changed.** `0.31.0`–`0.48.0` carry criterion 9 as an exit-criteria table row (`| 9 | …`); `0.49.0` as a subsection
//!   (`### … criterion 9`). The section is **either** anchor, and **neither found is a failure at or above the floor** (a check that
//!   passes over no section is the E-014 shape).
//!
//! **The floor is derived, not written**: the lowest-version record whose section carries both labels. Below it -- 11 records today:
//! the ten from `0.21.3` to `0.30.0`, which predate criterion 9 (first exercised at `0.31.0`), and `0.31.0` itself, which has it
//! without the dispatch step that RFC-0.32-001 D6 added -- a record cannot name two runs and is not asked to. At or above the floor
//! a record that does not is red, unless it is a **named exemption with a reason**, and an exemption that is stale (the record now
//! names both, or does not exist, or sits below the floor) is red too -- the `unsafe-audit` exemption shape.
//!
//! **The version being cut is not asked.** Its two runs are the CI runs for the tagged commit, which do not exist at the
//! release commit; the record is filled by the follow-up commit and judged at the next cut. A ceiling of exactly one, and the
//! reason it is safe is that the next cut does ask.
//!
//! **What this does not do:** it does not read GitHub, so it cannot tell a real run id from a typed one, nor a `ci.yml` run from
//! another workflow's that happens to sit after the word `dispatch`; it judges that the record *says* it read both, in the words
//! the cycle document requires. Whether the runs were green is the record's table, and the cut's.

use crate::release_records::version_key;

/// One exemption: a record at or above the floor that does not name both runs, with the reason.
pub struct Exemption<'a> {
    pub version: &'a str,
    pub reason: &'a str,
}

/// What the check found in one record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The criterion-9 section was located (a table row `| 9 |` and/or a heading naming "criterion 9").
    pub section: bool,
    pub push: Vec<String>,
    pub dispatch: Vec<String>,
}

impl Found {
    pub fn names_both(&self) -> bool {
        !self.push.is_empty() && !self.dispatch.is_empty()
    }
}

/// The criterion-9 section of a record: the exit-criteria row that begins `| 9 |`, and every subsection whose heading names
/// "criterion 9" (to the next heading of the same or a higher level). `None` when neither exists.
pub fn section_of(record: &str) -> Option<String> {
    let lines: Vec<&str> = record.lines().collect();
    let mut out = String::new();
    let mut found = false;
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim_start().starts_with("| 9 |") {
            out.push_str(l);
            out.push('\n');
            found = true;
        }
        if let Some(level) = heading_level(l)
            && l.to_ascii_lowercase().contains("criterion 9")
        {
            found = true;
            out.push_str(l);
            out.push('\n');
            let mut j = i + 1;
            while j < lines.len() {
                if let Some(lv) = heading_level(lines[j])
                    && lv <= level
                {
                    break;
                }
                out.push_str(lines[j]);
                out.push('\n');
                j += 1;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    found.then_some(out)
}

fn heading_level(line: &str) -> Option<usize> {
    let n = line.chars().take_while(|c| *c == '#').count();
    (n > 0 && line[n..].starts_with(' ')).then_some(n)
}

/// Every `push` / `dispatch` label in `section`, with the first run id (>= 10 digits) after it on the same line and within
/// 40 characters of non-digit text (markdown, `run`, backticks, an em dash).
pub fn labelled_runs(section: &str) -> (Vec<String>, Vec<String>) {
    let mut push = Vec::new();
    let mut dispatch = Vec::new();
    for line in section.lines() {
        let lower = line.to_ascii_lowercase();
        let bytes = lower.as_bytes();
        for (label, bucket) in [("push", &mut push), ("dispatch", &mut dispatch)] {
            let mut from = 0;
            while let Some(p) = lower[from..].find(label) {
                let start = from + p;
                let end = start + label.len();
                from = end;
                // whole word only
                let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
                let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
                if !(before_ok && after_ok) {
                    continue;
                }
                if let Some(id) = id_after(&line[end..])
                    && !bucket.contains(&id)
                {
                    bucket.push(id);
                }
            }
        }
    }
    (push, dispatch)
}

/// The first run of >= 10 digits in `s`, provided at most 40 non-digit characters precede it.
fn id_after(s: &str) -> Option<String> {
    let mut skipped = 0usize;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c.is_ascii_digit() {
            let run: String = s[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
            return (run.len() >= 10).then_some(run);
        }
        skipped += 1;
        if skipped > 40 {
            return None;
        }
    }
    None
}

pub fn inspect(record: &str) -> Found {
    match section_of(record) {
        None => Found {
            section: false,
            push: vec![],
            dispatch: vec![],
        },
        Some(sec) => {
            let (push, dispatch) = labelled_runs(&sec);
            Found {
                section: true,
                push,
                dispatch,
            }
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub records: usize,
    /// The derived floor (lowest record that names both), `None` only when no record does (an error before this is built).
    pub floor: String,
    pub below_floor: Vec<String>,
    pub naming_both: usize,
    pub exempt: Vec<(String, String)>,
    /// The record for the version **currently being cut**, which is not asked to name its runs: at the
    /// release commit they do not exist yet. `None` when no record matches the workspace version.
    pub in_flight: Option<String>,
}

/// Judge every record. `records` is `(version, text)`.
///
/// `in_flight` is the **workspace version** — the release being cut. Its own record is **not asked to
/// name its runs**, and that is structural rather than a concession: criterion 9's two runs are the CI
/// runs *for the tagged commit*, so they cannot exist until after the tag, which is after the release
/// commit this check runs against. Asking would make `release-rehearsal` unpassable at every cut.
///
/// **The window is exactly one milestone.** The record is filled by the follow-up commit and judged at
/// the *next* cut — which is precisely how 0.49.0's gap was found, by reading 0.50's rehearsal of it.
/// A record below the workspace version is asked in full, so this is a ceiling of one, not a skip.
///
/// *Added at the 0.50.0 cut, by the architect, when this check — which the architect specified and the
/// implementer built and demonstrated red against eleven historical records — failed on the record
/// being written. Neither of us had run it against a record that did not yet have its runs.*
pub fn evaluate(
    records: &[(String, String)],
    exemptions: &[Exemption<'_>],
    in_flight: Option<&str>,
) -> Result<Report, Vec<String>> {
    let mut keyed: Vec<(&str, (u64, u64, u64), Found)> = Vec::new();
    for (v, text) in records {
        if let Some(k) = version_key(v) {
            keyed.push((v.as_str(), k, inspect(text)));
        }
    }
    keyed.sort_by_key(|(_, k, _)| *k);
    let mut problems = Vec::new();
    let Some(floor) = keyed
        .iter()
        .find(|(_, _, f)| f.names_both())
        .map(|(v, k, _)| (*v, *k))
    else {
        return Err(vec![
            "no release record names both a push and a dispatch run in its criterion-9 section, so no floor can be derived; either the records changed shape or this check's anchors no longer match them".into(),
        ]);
    };
    let mut below = Vec::new();
    let mut naming_both = 0usize;
    let mut exempt_used: Vec<(String, String)> = Vec::new();
    let mut in_flight_seen: Option<String> = None;
    for (v, k, f) in &keyed {
        if *k < floor.1 {
            below.push((*v).to_string());
            continue;
        }
        // The release being cut: its runs are measured after the tag, so its record cannot name them yet.
        if in_flight == Some(*v) {
            in_flight_seen = Some((*v).to_string());
            if let Some(e) = exemptions.iter().find(|e| e.version == *v) {
                problems.push(format!(
                    "exemption for {v} is unnecessary: it is the version being cut, which this check does not ask (\"{}\")",
                    e.reason
                ));
            }
            continue;
        }
        let ex = exemptions.iter().find(|e| e.version == *v);
        if f.names_both() {
            naming_both += 1;
            if let Some(e) = ex {
                problems.push(format!(
                    "stale exemption for {v}: its criterion-9 section names a push run and a dispatch run now, so the exemption (\"{}\") is retired",
                    e.reason
                ));
            }
            continue;
        }
        let why = match (f.section, f.push.is_empty(), f.dispatch.is_empty()) {
            (false, _, _) => "has no criterion-9 section (no exit-criteria row `| 9 |` and no heading naming \"criterion 9\")".to_string(),
            (true, false, true) => format!(
                "names a push run ({}) and **no dispatch run**: criterion 9 reads two runs, because `fuzz-run` and `miri` are skipped on push and `ci-pages` is skipped on dispatch",
                f.push.join(", ")
            ),
            (true, true, false) => format!(
                "names a dispatch run ({}) and **no push run**",
                f.dispatch.join(", ")
            ),
            (true, true, true) => "names **neither** a push nor a dispatch run (an id counts only when labelled `push` or `dispatch` in the section's own text)".to_string(),
            (true, false, false) => unreachable!("names_both"),
        };
        match ex {
            Some(e) => exempt_used.push((v.to_string(), e.reason.to_string())),
            None => problems.push(format!("release record {v} {why}")),
        }
    }
    for e in exemptions {
        match keyed.iter().find(|(v, _, _)| *v == e.version) {
            None => problems.push(format!(
                "stale exemption for {}: there is no such release record",
                e.version
            )),
            Some((_, k, _)) if *k < floor.1 => problems.push(format!(
                "stale exemption for {}: it is below the derived floor {}, which is not asked to name two runs",
                e.version, floor.0
            )),
            Some(_) => {}
        }
        if e.reason.trim().is_empty() {
            problems.push(format!(
                "exemption for {} has no reason: an exemption without one is a record, not an exception",
                e.version
            ));
        }
    }
    if problems.is_empty() {
        Ok(Report {
            records: keyed.len(),
            floor: floor.0.to_string(),
            in_flight: in_flight_seen,
            below_floor: below,
            naming_both,
            exempt: exempt_used,
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The three shapes the real records use, reduced.
    const ROW_FORM: &str = "## Exit criteria\n\n| 8 | x | y |\n| 9 | The release commit's CI runs read | push run **`35089262713`** (attempt 2) — **37 success**; dispatch run **`35089305545`** — **45 success** |\n| 10 | z | w |\n";
    const ROW_FORM_SPLIT_WORDING: &str = "| 9 | The release commit's CI runs read | push **`43245929970`** — 44 success; dispatch **`43251039061`** — 51 success. **A first dispatch, `43245930851`, went red.** |\n";
    const SECTION_FORM_ONE_RUN: &str = "## Exit criteria\n\n* **Criterion 9 (CI)** — see *Tag and publish*.\n\n### CI for the tagged commit — criterion 9\n\n| | |\n|---|---|\n| Run | push **`45843432462`**, head `093ccdf5` — the tagged commit's own run, not a dispatch |\n| Result | 48 success · 2 skipped |\n\n### The GitHub Release\n\n| Workflow | **`45843464701`**, `release`, **success** |\n\n### Small item A\n\nartifact `151289025`\n";
    const SECTION_FORM_TWO_RUNS: &str = "### CI for the tagged commit — criterion 9\n\n| Run | push **`45843432462`** |\n| Run | dispatch **`45843999999`** on the same commit |\n\n### The GitHub Release\n\nworkflow `45843464701`\n";
    const ONE_RUN_ROW: &str = "| 9 | Release commit's CI run read | run **`34703606384`** — **34 success · 0 failure · 1 skipped** |\n";
    const NO_CRITERION_9: &str = "## Exit criteria\n\n| 8 | x | y |\n";

    fn rec(v: &str, t: &str) -> (String, String) {
        (v.to_string(), t.to_string())
    }

    #[test]
    fn the_row_form_and_the_section_form_are_both_found_and_labelled() {
        let f = inspect(ROW_FORM);
        assert!(f.section && f.names_both(), "{f:?}");
        assert_eq!(f.push, ["35089262713"]);
        assert_eq!(f.dispatch, ["35089305545"]);
        let f = inspect(ROW_FORM_SPLIT_WORDING);
        assert!(f.names_both());
        assert_eq!(
            f.dispatch.len(),
            2,
            "both dispatches named, including the first, red one: {f:?}"
        );
        let f = inspect(SECTION_FORM_TWO_RUNS);
        assert!(f.names_both(), "{f:?}");
    }

    /// **The hole in the first design**: counted over the whole document, `0.49.0` has three long numbers and passes. Scoped to the
    /// section and labelled, it names one push run and no dispatch.
    #[test]
    fn a_document_scoped_count_would_pass_the_record_this_exists_for_and_the_section_scoped_rule_does_not()
     {
        let doc_ids = SECTION_FORM_ONE_RUN
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| s.len() >= 9)
            .count();
        assert_eq!(
            doc_ids, 3,
            "the three decoys: the push run, the release workflow's run, a Pages artifact id"
        );
        let f = inspect(SECTION_FORM_ONE_RUN);
        assert!(f.section);
        assert_eq!(f.push, ["45843432462"]);
        assert!(
            f.dispatch.is_empty(),
            "the release workflow's id is outside the section and unlabelled: {f:?}"
        );
        assert!(!f.names_both());
    }

    #[test]
    fn a_record_with_no_criterion_9_section_has_none() {
        let f = inspect(NO_CRITERION_9);
        assert!(!f.section && !f.names_both());
    }

    fn set() -> Vec<(String, String)> {
        vec![
            rec("0.30.0", NO_CRITERION_9),
            rec("0.31.0", ONE_RUN_ROW),
            rec("0.32.0", ROW_FORM),
            rec("0.48.0", ROW_FORM_SPLIT_WORDING),
        ]
    }

    #[test]
    fn the_floor_is_derived_and_the_records_below_it_are_not_asked() {
        let r = evaluate(&set(), &[], None).unwrap();
        assert_eq!(r.floor, "0.32.0");
        assert_eq!(r.below_floor, ["0.30.0", "0.31.0"]);
        assert_eq!(r.naming_both, 2);
    }

    #[test]
    fn a_record_above_the_floor_with_one_run_is_red_and_says_which_is_missing() {
        let mut s = set();
        s.push(rec("0.49.0", SECTION_FORM_ONE_RUN));
        let err = evaluate(&s, &[], None).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("0.49.0")
                && e.contains("no dispatch run")
                && e.contains("45843432462")),
            "{err:?}"
        );
    }

    #[test]
    fn a_record_above_the_floor_with_no_section_is_red() {
        let mut s = set();
        s.push(rec("0.50.0", NO_CRITERION_9));
        let err = evaluate(&s, &[], None).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("0.50.0") && e.contains("no criterion-9 section")),
            "{err:?}"
        );
    }

    #[test]
    fn a_named_exemption_passes_and_is_reported_and_goes_stale_when_the_record_is_fixed() {
        let mut s = set();
        s.push(rec("0.49.0", SECTION_FORM_ONE_RUN));
        let ex = [Exemption {
            version: "0.49.0",
            reason: "the gap itself",
        }];
        let r = evaluate(&s, &ex, None).unwrap();
        assert_eq!(
            r.exempt,
            [("0.49.0".to_string(), "the gap itself".to_string())]
        );
        // the record gains its dispatch run: the exemption must be retired
        let mut s2 = set();
        s2.push(rec("0.49.0", SECTION_FORM_TWO_RUNS));
        let err = evaluate(&s2, &ex, None).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("stale exemption for 0.49.0")),
            "{err:?}"
        );
    }

    #[test]
    fn exemptions_for_nothing_or_below_the_floor_or_without_a_reason_are_red() {
        let s = set();
        let err = evaluate(
            &s,
            &[Exemption {
                version: "9.9.9",
                reason: "x",
            }],
            None,
        )
        .unwrap_err();
        assert!(err.iter().any(|e| e.contains("no such release record")));
        let err = evaluate(
            &s,
            &[Exemption {
                version: "0.31.0",
                reason: "x",
            }],
            None,
        )
        .unwrap_err();
        assert!(err.iter().any(|e| e.contains("below the derived floor")));
        let mut s2 = set();
        s2.push(rec("0.49.0", SECTION_FORM_ONE_RUN));
        let err = evaluate(
            &s2,
            &[Exemption {
                version: "0.49.0",
                reason: " ",
            }],
            None,
        )
        .unwrap_err();
        assert!(err.iter().any(|e| e.contains("has no reason")));
    }

    #[test]
    fn if_no_record_names_both_there_is_no_floor_and_that_is_an_error_not_a_pass() {
        let s = vec![
            rec("0.31.0", ONE_RUN_ROW),
            rec("0.49.0", SECTION_FORM_ONE_RUN),
        ];
        assert!(evaluate(&s, &[], None).is_err());
    }

    #[test]
    fn a_label_must_be_a_whole_word_and_the_id_close_behind_it() {
        // "dispatched" is not the label; an id far from the label is not its id.
        let (p, d) = labelled_runs("the dispatched workflow ran: `45843999999`\n");
        assert!(p.is_empty() && d.is_empty(), "{p:?} {d:?}");
        let (p, _) = labelled_runs("push run **`35089262713`**\n");
        assert_eq!(p, ["35089262713"]);
        let (p, _) = labelled_runs(
            "push — and then a very long stretch of prose with no digits at all before `35089262713`\n",
        );
        assert!(p.is_empty());
    }

    // ── the real records ────────────────────────────────────────────────────────
    fn real_records() -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../releases");
        let mut v = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if let Some(ver) = n.strip_suffix(".md")
                && crate::release_records::is_bare_tag(ver)
            {
                v.push((ver.to_string(), std::fs::read_to_string(e.path()).unwrap()));
            }
        }
        v
    }

    /// **The ceiling of one, and that it is a ceiling and not a skip.** The record for the version being
    /// cut has no runs yet, so it is not asked — but a record *below* it with one run is still red. A
    /// blanket skip would pass the very shape E-137 records.
    #[test]
    fn the_version_being_cut_is_not_asked_and_the_one_below_it_still_is() {
        let s = vec![
            rec("0.32.0", SECTION_FORM_TWO_RUNS),
            rec("0.49.0", SECTION_FORM_ONE_RUN),
            rec(
                "0.50.0",
                "## Tag and publish\n\n### CI for the tagged commit — criterion 9\n\nmeasured in the follow-up commit\n",
            ),
        ];
        // Cutting 0.50.0: its own record is not asked, and 0.49.0's single run is still a failure.
        let err = evaluate(&s, &[], Some("0.50.0")).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("0.49.0") && e.contains("no dispatch run")),
            "the record below the one being cut must still be asked: {err:?}"
        );
        assert!(
            !err.iter().any(|e| e.contains("0.50.0")),
            "the version being cut must not be asked: {err:?}"
        );
        // With 0.49.0 exempted, cutting 0.50.0 passes and says which record was held back.
        let r = evaluate(
            &s,
            &[Exemption {
                version: "0.49.0",
                reason: "E-137",
            }],
            Some("0.50.0"),
        )
        .unwrap();
        assert_eq!(r.in_flight.as_deref(), Some("0.50.0"));
        // And with no version in flight, the same tree is red on 0.50.0 too -- so the ceiling is doing the work.
        let err2 = evaluate(
            &s,
            &[Exemption {
                version: "0.49.0",
                reason: "E-137",
            }],
            None,
        )
        .unwrap_err();
        assert!(err2.iter().any(|e| e.contains("0.50.0")), "{err2:?}");
    }

    /// An exemption for the version being cut is **unnecessary**, and saying so stops one being carried
    /// forward after the follow-up commit fills the record.
    #[test]
    fn an_exemption_for_the_version_being_cut_is_reported_as_unnecessary() {
        let s = vec![
            rec("0.32.0", SECTION_FORM_TWO_RUNS),
            rec("0.50.0", "### criterion 9\n\npending\n"),
        ];
        let err = evaluate(
            &s,
            &[Exemption {
                version: "0.50.0",
                reason: "not measured yet",
            }],
            Some("0.50.0"),
        )
        .unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("0.50.0") && e.contains("unnecessary")),
            "{err:?}"
        );
    }

    /// **The red run the item's evidence rests on:** against the real records, with no exemption, `0.49.0` is the one red, and
    /// the floor is `0.32.0` with 11 records below it.
    ///
    /// **It reads the real workspace version rather than `None`**, which is what the gate does — and it has to:
    /// from 0.50.0 on, the tree carries a record for the version being cut whose runs are measured only after
    /// the tag, so a test passing `None` here goes red at every cut on a record that is correctly unfinished.
    /// That is how the ceiling was found: this assertion failed at the 0.50.0 cut.
    #[test]
    fn the_real_records_are_red_on_0_49_0_alone_and_the_floor_is_0_32_0() {
        let recs = real_records();
        let ws = crate::release_records::workspace_version(include_str!("../../../Cargo.toml"))
            .expect("the workspace version is readable");
        let err = evaluate(&recs, &[], Some(ws.as_str())).unwrap_err();
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(
            err[0].contains("release record 0.49.0") && err[0].contains("no dispatch run"),
            "{err:?}"
        );
        let r = evaluate(
            &recs,
            &[Exemption {
                version: "0.49.0",
                reason: "x",
            }],
            Some(ws.as_str()),
        )
        .unwrap();
        assert_eq!(r.floor, "0.32.0");
        assert_eq!(
            r.below_floor.len(),
            11,
            "the ten that predate criterion 9 and 0.31.0: {:?}",
            r.below_floor
        );
        assert_eq!(r.naming_both, 17, "0.32.0 .. 0.48.0");
    }
}
