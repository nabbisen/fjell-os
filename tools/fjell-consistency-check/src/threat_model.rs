//! `threat-model` — the threat-model gate the v0.9–v0.15 handoff claimed and nobody built
//! (0.44 small item A, **E-027**).
//!
//! That handoff said each in-scope threat *"references an existing merged RFC (the
//! threat-model gate fails otherwise)"*. There was no such gate. The property survived three
//! additions by hand, which is the argument for the gate, not against it.
//!
//! **What is checked, all derived from `docs/src/security/threat-model-v1.md`:**
//! 1. every `### T<n>` section is numbered `1..N` with no gap and no duplicate;
//! 2. each cites **at least one RFC that resolves to a file in `rfcs/done/`**, and **every
//!    well-formed citation in it resolves** (a pointer to nothing is E-028's shape);
//! 3. the out-of-scope table's `OS<n>` rows are numbered `1..M` the same way, each with a
//!    threat and a rationale;
//! 4. where `standards-mapping.md` states "`N` in-scope threats", `N` equals the derived count.
//!
//! **No count is written in this file.** 20 became 23 without anyone noticing, so a literal
//! here would be edited at every addition and go stale the same way. `N` and `M` are read.
//!
//! **What "cites an RFC" means, and why it is not a substring match.** A bare `RFC` matches
//! prose *about* RFCs and external standards: T10 says "RFC 8032" (the IETF's Ed25519), which
//! is not a Fjell RFC. A citation here is `RFC` + (`-` or a space) + an id in one of the two
//! shapes this project's RFCs have, and it counts **only if a file for it exists in
//! `rfcs/done/`** -- "an existing merged RFC", the original claim, taken literally:
//!   * legacy, exactly three digits (`RFC 031`) -> `done/031-*.md`. Four digits (`RFC 8032`) is
//!     not an id of this shape and is not a citation;
//!   * versioned (`RFC-0.34-005`, `RFC v0.9-002`) -> `done/RFC-<id>-*.md`.
//!
//! **One section is waived, by name and with a reason that is true** (`WAIVERS`): see there.
//! A waiver applies only while the section's own `Defence` line is exactly `None.`; if it gains
//! a citation the run says the waiver can be retired, and a waiver for a section that no longer
//! exists is a failure -- a waiver is not a place to park a name.

use crate::read_file;
use std::process::ExitCode;

const NAME: &str = "threat-model";
const MODEL_PATH: &str = "docs/src/security/threat-model-v1.md";
const STANDARDS_PATH: &str = "docs/src/compliance/standards-mapping.md";
const DONE_DIR: &str = "rfcs/done";

pub struct Waiver {
    pub section: &'static str,
    pub reason: &'static str,
}

/// Sections that are allowed to cite no RFC. Each is keyed by id so a *new* uncited section
/// is a failure that needs a reviewed code edit, not a freedom the document grants itself.
const WAIVERS: &[Waiver] = &[Waiver {
    section: "T23",
    reason: "the section's own Defence line reads `None.` -- it records a threat with no \
             mitigation, and says in its Residual paragraph why none exists (nothing lets the \
             operator cross-check a presentation against the node's own record). It cites no \
             RFC because there is none to cite. Whether an unmitigated entry belongs under \
             `In-scope threats (v1.0 commits to mitigating)` is the architect's question, \
             not this check's",
}];

/// One `### T<n>` section.
#[derive(Debug, PartialEq, Eq)]
pub struct Section {
    pub number: u32,
    pub heading: String,
    pub body: String,
}

/// The `### T<n> — …` sections, each bounded by the next `##`/`###` heading -- NOT by the next
/// `###` alone: the last T section would otherwise swallow the out-of-scope table and the
/// operator obligations after it and inherit their citations.
pub fn sections(doc: &str) -> Vec<Section> {
    let lines: Vec<&str> = doc.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some(n) = threat_number(lines[i]) {
            let mut j = i + 1;
            while j < lines.len() && !lines[j].starts_with("## ") && !lines[j].starts_with("### ") {
                j += 1;
            }
            out.push(Section {
                number: n,
                heading: lines[i].to_string(),
                body: lines[i + 1..j].join("\n"),
            });
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// `### T12 — …` -> `Some(12)`.
fn threat_number(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("### T")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    // what follows the number must not continue an identifier (`### T1x`)
    match rest[digits.len()..].chars().next() {
        None | Some(' ') | Some('\u{2014}') | Some(':') | Some('.') => digits.parse().ok(),
        _ => None,
    }
}

/// A well-formed RFC citation found in text, and the file prefix it must resolve to.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Citation {
    /// As written, e.g. `RFC v0.9-002`.
    pub text: String,
    /// The name prefix a `rfcs/done/` file must start with.
    pub file_prefix: String,
}

/// Every well-formed citation in `text` (see the module doc for the two shapes).
pub fn citations(text: &str) -> Vec<Citation> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= b.len() {
        // `RFC` at a word start, then `-` or a space.
        let word_start = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if word_start && &b[i..i + 3] == b"RFC" && (b[i + 3] == b'-' || b[i + 3] == b' ') {
            let id_start = i + 4;
            if let Some((id, len)) = parse_id(&text[id_start..]) {
                out.push(Citation {
                    text: text[i..id_start + len].to_string(),
                    file_prefix: id,
                });
                i = id_start + len;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// The id at the start of `s`, as `(file prefix, bytes consumed)`.
///   * `ddd`                 -> `ddd-`            (legacy; exactly three digits, then a non-word byte)
///   * `[v]D.D[.D]-ddd`      -> `RFC-[v]D.D[.D]-ddd-`
fn parse_id(s: &str) -> Option<(String, usize)> {
    parse_versioned(s).or_else(|| parse_legacy(s))
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `[v]D.D[.D]-ddd` -> (`RFC-<id>-`, length). Falls through (`None`) on anything else, so a
/// legacy id followed by a full stop (`RFC 051.`) is still read as legacy.
fn parse_versioned(s: &str) -> Option<(String, usize)> {
    let b = s.as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut p = 0;
    if b.first() == Some(&b'v') {
        p += 1;
    }
    let a = digits(p);
    if a == 0 || b.get(p + a) != Some(&b'.') {
        return None;
    }
    let mut q = p + a + 1;
    let m = digits(q);
    if m == 0 {
        return None;
    }
    q += m;
    if b.get(q) == Some(&b'.') {
        let r = digits(q + 1);
        if r > 0 {
            q += 1 + r;
        }
    }
    if b.get(q) == Some(&b'-') && digits(q + 1) == 3 && !b.get(q + 4).is_some_and(|c| is_word(*c)) {
        return Some((format!("RFC-{}-", &s[..q + 4]), q + 4));
    }
    None
}

/// Exactly three digits, then a non-word byte (so `8032`, `0311` and `031x` are not ids).
fn parse_legacy(s: &str) -> Option<(String, usize)> {
    let b = s.as_bytes();
    let n = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if n == 3 && !b.get(3).is_some_and(|c| is_word(*c)) {
        return Some((format!("{}-", &s[..3]), 3));
    }
    None
}

/// The rows of the out-of-scope table: `| OS<n> | threat | rationale |`.
pub fn out_of_scope_rows(doc: &str) -> Vec<(u32, String, String)> {
    let mut rows = Vec::new();
    for line in doc.lines() {
        let t = line.trim();
        let Some(inner) = t.strip_prefix('|').and_then(|r| r.strip_suffix('|')) else {
            continue;
        };
        let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
        if cells.len() != 3 {
            continue;
        }
        let Some(d) = cells[0].strip_prefix("OS") else {
            continue;
        };
        if d.is_empty() || !d.bytes().all(|c| c.is_ascii_digit()) {
            continue;
        }
        rows.push((
            d.parse().unwrap_or(0),
            cells[1].to_string(),
            cells[2].to_string(),
        ));
    }
    rows
}

/// What the run found, for the one-line account.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub threats: usize,
    pub cited: usize,
    pub waived: Vec<String>,
    pub out_of_scope: usize,
    /// `Some(n)` if `standards-mapping.md` states a count and it agreed.
    pub standards_count_checked: Option<usize>,
}

/// Does this section's `Defence` line read exactly `None.`?
fn declares_no_defence(body: &str) -> bool {
    body.lines().any(|l| {
        let l = l.trim();
        l.strip_prefix("**Defence:**")
            .is_some_and(|r| r.trim().trim_end_matches("  ").trim() == "None.")
    })
}

/// `N` from every "N in-scope threats" in `text`.
pub fn claimed_in_scope_counts(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find(" in-scope threats") {
        let end = from + rel;
        let digits: String = text[..end]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if let Ok(n) = digits.parse() {
            out.push(n); // an empty `digits` does not parse, so no separate emptiness test
        }
        from = end + 1;
    }
    out
}

/// Judge the threat model. Pure: reads only its arguments.
///
/// * `doc` -- the threat model; `done_names` -- file names in `rfcs/done/`;
///   `standards` -- `standards-mapping.md` if readable (its count claim is cross-checked);
///   `waivers` -- production passes [`WAIVERS`]; a test passes its own, so the real waiver
///   list is not a hidden input to every fixture.
pub fn judge(
    doc: &str,
    done_names: &[String],
    standards: Option<&str>,
    waivers: &[Waiver],
) -> Result<Report, Vec<String>> {
    let mut why: Vec<String> = Vec::new();
    let secs = sections(doc);
    if secs.is_empty() {
        return Err(vec![format!(
            "{MODEL_PATH} has no `### T<n>` section: a threat model with no threats is a \
             document this check cannot read, not one that passes"
        )]);
    }

    // 1. numbering. Only the FIRST break is reported: one deleted section shifts every
    // later heading, and eighteen lines saying so would bury the one that matters.
    if let Some((idx, s)) = secs
        .iter()
        .enumerate()
        .find(|(idx, s)| s.number != *idx as u32 + 1)
    {
        why.push(format!(
            "threat sections are not numbered T1..T{}: at position {} the heading is `{}` \
             (expected T{}) -- a gap, a duplicate or an order change; every later heading is \
             out of step from here",
            secs.len(),
            idx + 1,
            s.heading,
            idx + 1
        ));
    }

    // 2. citations
    let mut cited = 0;
    let mut waived = Vec::new();
    for s in &secs {
        let id = format!("T{}", s.number);
        let cites = citations(&s.body);
        let mut resolving = 0;
        for c in &cites {
            if done_names.iter().any(|n| n.starts_with(&c.file_prefix)) {
                resolving += 1;
            } else {
                why.push(format!(
                    "{id} cites `{}`, but rfcs/done/ holds no file starting `{}` -- a citation \
                     to an RFC that is not merged (or not there)",
                    c.text, c.file_prefix
                ));
            }
        }
        let waiver = waivers.iter().find(|w| w.section == id);
        match (resolving > 0, waiver) {
            (true, None) => cited += 1,
            (true, Some(w)) => {
                cited += 1;
                println!(
                    "{NAME}: note — {id} now cites a resolving RFC; its waiver ({}) can be retired",
                    w.reason
                );
            }
            (false, Some(w)) => {
                if declares_no_defence(&s.body) {
                    waived.push(id.clone());
                    println!(
                        "{NAME}: note — {id} cites no RFC and is waived: {}",
                        w.reason
                    );
                } else {
                    why.push(format!(
                        "{id} cites no RFC and has a waiver, but its `Defence` line no longer \
                         reads `None.` -- the waiver's reason is no longer true of it"
                    ));
                }
            }
            (false, None) => why.push(format!(
                "{id} ({}) cites no RFC that resolves to a file in rfcs/done/. Each in-scope \
                 threat names the merged RFC that defends it; a bare `RFC` in prose, or an \
                 external standard (`RFC 8032`), is not one",
                s.heading.trim_start_matches("### ").trim()
            )),
        }
    }
    for w in waivers {
        if !secs.iter().any(|s| format!("T{}", s.number) == w.section) {
            why.push(format!(
                "a waiver names {}, but the threat model has no such section: a waiver is not a \
                 place to park a name",
                w.section
            ));
        }
    }

    // 3. out-of-scope rows
    let rows = out_of_scope_rows(doc);
    if rows.is_empty() {
        why.push(format!("{MODEL_PATH} has no `| OS<n> | … | … |` row"));
    }
    if let Some((idx, (n, _, _))) = rows
        .iter()
        .enumerate()
        .find(|(idx, (n, _, _))| *n != *idx as u32 + 1)
    {
        why.push(format!(
            "out-of-scope rows are not numbered OS1..OS{}: at position {} the row is OS{n} \
             (expected OS{}); every later row is out of step from here",
            rows.len(),
            idx + 1,
            idx + 1
        ));
    }
    for (n, threat, rationale) in &rows {
        if threat.is_empty() || rationale.is_empty() {
            why.push(format!("OS{n} has an empty threat or rationale cell"));
        }
    }

    // 4. the one document that states the count
    let mut standards_count_checked = None;
    if let Some(text) = standards {
        for claimed in claimed_in_scope_counts(text) {
            if claimed != secs.len() {
                why.push(format!(
                    "{STANDARDS_PATH} says \"{claimed} in-scope threats\"; {MODEL_PATH} has {}",
                    secs.len()
                ));
            }
            standards_count_checked = Some(claimed);
        }
    }

    if why.is_empty() {
        Ok(Report {
            threats: secs.len(),
            cited,
            waived,
            out_of_scope: rows.len(),
            standards_count_checked,
        })
    } else {
        Err(why)
    }
}

pub fn check() -> ExitCode {
    let Some(doc) = read_file(NAME, MODEL_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(entries) = crate::read_dir_named(NAME, DONE_DIR) else {
        return ExitCode::FAILURE;
    };
    let done_names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // The standards-mapping cross-check is optional only in that the claim may be absent;
    // an unreadable file is reported, not skipped.
    let standards = match std::fs::read_to_string(STANDARDS_PATH) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot read {STANDARDS_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match judge(&doc, &done_names, standards.as_deref(), WAIVERS) {
        Ok(r) => {
            let waived = if r.waived.is_empty() {
                String::new()
            } else {
                format!(", {} waived ({})", r.waived.len(), r.waived.join(", "))
            };
            let std_part = match r.standards_count_checked {
                Some(n) => format!("; standards-mapping.md's \"{n} in-scope threats\" agrees"),
                None => "; standards-mapping.md states no threat count".to_string(),
            };
            println!(
                "{NAME}: PASS ({} threats T1..T{}: {} cite a resolving RFC{waived}; {} out-of-scope rows OS1..OS{}{std_part})",
                r.threats, r.threats, r.cited, r.out_of_scope, r.out_of_scope
            );
            ExitCode::SUCCESS
        }
        Err(why) => {
            for w in &why {
                eprintln!("{NAME}: FAIL — {w}");
            }
            eprintln!("{NAME}: FAIL");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done() -> Vec<String> {
        [
            "031-unified-capability-enforcement.md",
            "RFC-v0.9-002-lint.md",
            "RFC-0.34-005-presentation.md",
            "RFC-v0.7.3-002-crypto.md",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn doc(t1: &str, t2: &str) -> String {
        format!(
            "# Model\n\n## In-scope threats\n\n### T1 — one\n\n{t1}\n\n---\n\n### T2 — two\n\n{t2}\n\n---\n\n\
             ## Out-of-scope threats\n\n| Code | Threat | Rationale |\n|---|---|---|\n\
             | OS1 | a | because |\n| OS2 | b | because |\n\n## Operator obligations\n\nSee RFC 031.\n"
        )
    }

    #[test]
    fn citation_shapes() {
        let t = "RFC 031, RFC v0.9-002, RFC-0.34-005 and RFC-v0.7.3-002.";
        let got: Vec<String> = citations(t).into_iter().map(|c| c.file_prefix).collect();
        assert_eq!(
            got,
            ["031-", "RFC-v0.9-002-", "RFC-0.34-005-", "RFC-v0.7.3-002-"]
        );
    }

    /// The reason this is not a substring match.
    #[test]
    fn external_and_prose_rfcs_are_not_citations() {
        for t in [
            "Ed25519 per RFC 8032.", // four digits: the IETF's, not ours
            "the RFC process",       // prose
            "RFCs 031 and 040",      // plural
            "RFC-",                  // bare
            "RFC 03",                // too short
            "RFC 0311",              // too long
            "RFC v0.9-02",           // short serial
            "XRFC 031",              // inside a word
            "RFC 031x",              // runs on
        ] {
            assert!(citations(t).is_empty(), "{t:?} -> {:?}", citations(t));
        }
    }

    #[test]
    fn the_last_section_does_not_inherit_the_citations_that_follow_it() {
        // `doc()` puts "RFC 031" in a section AFTER T2 (Operator obligations).
        let d = doc("RFC 031.", "no citation here at all");
        let e = judge(&d, &done(), None, &[]).unwrap_err();
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].starts_with("T2 "), "{e:?}");
    }

    #[test]
    fn passes_when_each_cites_a_resolving_rfc() {
        let r = judge(
            &doc("RFC 031.", "RFC v0.9-002 and RFC 8032."),
            &done(),
            None,
            &[],
        )
        .unwrap();
        assert_eq!((r.threats, r.cited, r.out_of_scope), (2, 2, 2));
    }

    #[test]
    fn an_external_rfc_alone_does_not_satisfy_a_section() {
        let e = judge(&doc("RFC 031.", "Ed25519 (RFC 8032)."), &done(), None, &[]).unwrap_err();
        assert!(e.iter().any(|r| r.starts_with("T2 ")), "{e:?}");
    }

    #[test]
    fn a_dangling_citation_fails_even_beside_a_good_one() {
        let e = judge(&doc("RFC 031 and RFC 999.", "RFC 031."), &done(), None, &[]).unwrap_err();
        assert!(e.iter().any(|r| r.contains("T1 cites `RFC 999`")), "{e:?}");
    }

    #[test]
    fn numbering_gaps_and_duplicates_fail() {
        let d = doc("RFC 031.", "RFC 031.").replace("### T2 —", "### T3 —");
        let e = judge(&d, &done(), None, &[]).unwrap_err();
        assert!(
            e.iter()
                .any(|r| r.contains("at position 2") && r.contains("(expected T2)")),
            "{e:?}"
        );
        let d = doc("RFC 031.", "RFC 031.").replace("### T2 —", "### T1 —");
        assert!(judge(&d, &done(), None, &[]).is_err());
    }

    #[test]
    fn out_of_scope_rows_are_counted_and_checked() {
        let d =
            doc("RFC 031.", "RFC 031.").replace("| OS2 | b | because |", "| OS3 | b | because |");
        let e = judge(&d, &done(), None, &[]).unwrap_err();
        assert!(
            e.iter()
                .any(|r| r.contains("at position 2") && r.contains("(expected OS2)")),
            "{e:?}"
        );
        let d = doc("RFC 031.", "RFC 031.").replace("| OS2 | b | because |", "| OS2 | b |  |");
        assert!(
            judge(&d, &done(), None, &[])
                .unwrap_err()
                .iter()
                .any(|r| r.contains("empty"))
        );
        let d = doc("RFC 031.", "RFC 031.")
            .replace("| OS1 | a | because |\n| OS2 | b | because |\n", "");
        assert!(
            judge(&d, &done(), None, &[])
                .unwrap_err()
                .iter()
                .any(|r| r.contains("no `| OS"))
        );
    }

    /// Nothing is hard-coded: a third section is just counted.
    #[test]
    fn counts_are_derived_not_written_down() {
        let mut d = doc("RFC 031.", "RFC 031.");
        d = d.replace(
            "## Out-of-scope threats",
            "### T3 — three\n\nRFC v0.9-002.\n\n---\n\n## Out-of-scope threats",
        );
        let r = judge(&d, &done(), None, &[]).unwrap();
        assert_eq!(r.threats, 3);
    }

    #[test]
    fn the_standards_mapping_count_is_cross_checked() {
        let d = doc("RFC 031.", "RFC 031.");
        let ok = judge(
            &d,
            &done(),
            Some("... 2 in-scope threats each mapped ..."),
            &[],
        )
        .unwrap();
        assert_eq!(ok.standards_count_checked, Some(2));
        let e = judge(&d, &done(), Some("... 23 in-scope threats ..."), &[]).unwrap_err();
        assert!(
            e.iter().any(|r| r.contains("says \"23 in-scope threats\"")),
            "{e:?}"
        );
        // absent claim: nothing to check, and it says so
        assert_eq!(
            judge(&d, &done(), Some("no count here"), &[])
                .unwrap()
                .standards_count_checked,
            None
        );
    }

    #[test]
    fn no_threat_sections_is_a_failure_not_a_pass() {
        assert!(judge("# nothing\n", &done(), None, &[]).is_err());
    }

    #[test]
    fn a_waiver_only_holds_while_it_is_true() {
        // T23 waived, and its Defence really is `None.`
        let mut d = String::from("## In-scope\n\n");
        for n in 1..=23 {
            let body = if n == 23 {
                "**Defence:** None.  \n**Residual:** why."
            } else {
                "RFC 031."
            };
            d.push_str(&format!("### T{n} — t\n\n{body}\n\n"));
        }
        d.push_str("## Out-of-scope\n\n| OS1 | a | b |\n");
        let t23 = [Waiver {
            section: "T23",
            reason: "test",
        }];
        let r = judge(&d, &done(), None, &t23).unwrap();
        assert_eq!(r.waived, ["T23"]);
        // ...but if T23 stops saying `None.`, the waiver no longer covers it
        let d2 = d.replace(
            "**Defence:** None.",
            "**Defence:** Some prose, no citation.",
        );
        let e = judge(&d2, &done(), None, &t23).unwrap_err();
        assert!(
            e.iter().any(|r| r.contains("no longer reads `None.`")),
            "{e:?}"
        );
        // ...and a waiver for a section that is gone is itself a failure
        let d3 = d.replace("### T23 — t", "### Not a threat heading");
        let e = judge(&d3, &done(), None, &t23).unwrap_err();
        assert!(e.iter().any(|r| r.contains("a waiver names T23")), "{e:?}");
    }

    /// Pinned to the REAL document and the REAL folder: the instrument and the thing it
    /// reads agree today, and it is not vacuous.
    #[test]
    fn the_real_threat_model_passes_and_has_a_real_population() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap();
        let doc = std::fs::read_to_string(root.join(MODEL_PATH)).unwrap();
        let standards = std::fs::read_to_string(root.join(STANDARDS_PATH)).unwrap();
        let names: Vec<String> = std::fs::read_dir(root.join(DONE_DIR))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let r = judge(&doc, &names, Some(&standards), WAIVERS).expect("the real model passes");
        // Not a pinned count: it must simply be non-vacuous and self-consistent.
        assert!(r.threats > 0 && r.out_of_scope > 0);
        assert_eq!(r.cited + r.waived.len(), r.threats);
        assert_eq!(r.standards_count_checked, Some(r.threats));
    }
}
