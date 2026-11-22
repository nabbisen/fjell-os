//! RFC-0.47-002 R5: **`rfc-policy`** -- the lifecycle policy's kept rules are written down, and a check can
//! find them missing.
//!
//! RFC-0.25-002 (D1) kept five rules from the in-repo policy: `Accepted`, `Implemented-with-Errata`,
//! `Closed`, the drift/errata rule and the required-sections list. They landed at `be8cf6f`, verified by
//! name, and an untitled rewrite of RFC 000 (`4d8044a`) deleted all five two months later. Nothing
//! noticed -- **the instrument that would have caught it was the rule that went missing** (E-128). This
//! subcheck is that instrument. It judges four things and **derives its population** (it names no RFC):
//!
//! 1. **Vocabulary.** The states are the `## Lifecycle states` table in RFC 000 (`status::vocabulary`),
//!    which is also what `rfc-status-folder`, `handoff-status` and `rfc-evidence` read, so the words cannot
//!    drift from the policy. Here: every RFC in `proposed/`, `accepted/` and `done/` whose Status field
//!    names *no* defined state fails, naming the file and the line.
//! 2. **The rule is present, by name, at both ends** -- RFC-0.25-002's own acceptance criterion
//!    (*"present and checked by name"*), which nothing ever checked. RFC 000's `## Lifecycle states`
//!    section must contain the drift rule's first clause ([`RULE_CLAUSE`]), and `rfcs/ERRATA.md`'s header
//!    must link to RFC 000. Whitespace and markdown emphasis are ignored; the **clause is held here on
//!    purpose**, so an edit that removes the rule has to edit this file too, in the open.
//! 3. **`Implemented-with-Errata` => named in the register.** Every RFC carrying that status has its id in
//!    `ERRATA.md`. (The rule says an entry is added; this is the half of it a program can hold.)
//! 4. **Forward only: a live erratum's subject RFC is not plain `Implemented`.** For register entries that
//!    carry an `- **RFC:** <id>` field. The field is required of entries numbered
//!    [`FIELD_REQUIRED_FROM`] and above; earlier entries are not retro-applied (17 of 127 headings named
//!    their RFC, so a heading parse would have been a check of 13% of the register calling itself a check).
//!
//! **What it cannot do, so a green is not over-read.** It cannot judge whether an RFC's normative text is
//! satisfied by the code -- that is a reviewer's question. A green `rfc-policy` means *the rule is written
//! down and its bookkeeping holds*; it does not mean no RFC over-claims.

use crate::errata::{is_accepted, is_open, parse_summary_rows};
use crate::status::{Vocabulary, extract_status_keyword_in, parse_policy, status_field_line};

/// The drift rule's first clause. Normalised (no `*`, backticks or `>`, whitespace collapsed) before
/// comparison, so markup around it can change and the words cannot.
pub const RULE_CLAUSE: &str = "An RFC may not be marked Implemented if its normative text makes a claim the merged code does not satisfy";

/// Entries from this number on must carry the `- **RFC:**` field. E-128 and E-129 were filed before the
/// field's shape existed and are backfilled by the architect (RFC-0.47-002 D4).
pub const FIELD_REQUIRED_FROM: u32 = 130;

/// What the check read, as text: policy, register, and every RFC with the folder it lives in.
pub struct Inputs<'a> {
    pub policy: &'a str,
    pub errata: &'a str,
    /// `(path label, folder, content)` for every RFC in `proposed/`, `accepted/`, `done/`.
    pub rfcs: &'a [(String, String, String)],
}

/// What a green run covered, for the one-line account.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub states: usize,
    pub rfcs: usize,
    pub with_errata: usize,
    pub fielded_entries: usize,
}

fn normalise(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '*' | '`' | '>'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The section of `src` that starts at the `## ` heading `heading` and ends at the next `## ` heading.
fn section<'a>(src: &'a str, heading: &str) -> Option<&'a str> {
    let start = src
        .find(&format!("\n{heading}\n"))
        .map(|i| i + 1)
        .or_else(|| src.starts_with(&format!("{heading}\n")).then_some(0))?;
    let rest = &src[start + heading.len()..];
    let end = rest.find("\n## ").map_or(rest.len(), |i| i);
    Some(&rest[..end])
}

/// `RFC-0.47-001` from `RFC-0.47-001-some-title.md`; `None` for files that are not numbered RFCs.
pub fn rfc_id(path: &str) -> Option<String> {
    let file = path.rsplit('/').next()?.strip_suffix(".md")?;
    let parts: Vec<&str> = file.split('-').collect();
    if parts.len() >= 3
        && parts[0] == "RFC"
        && parts[2].len() >= 3
        && parts[2].chars().all(|c| c.is_ascii_digit())
    {
        Some(format!("RFC-{}-{}", parts[1], parts[2]))
    } else {
        None
    }
}

/// Is `id` named in `text` as a whole identifier (not as the prefix of a longer number)?
fn names(text: &str, id: &str) -> bool {
    let mut from = 0;
    while let Some(i) = text[from..].find(id) {
        let end = from + i + id.len();
        if !text[end..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        {
            return true;
        }
        from = end;
    }
    false
}

struct Entry<'a> {
    num: u32,
    field: Option<&'a str>,
}

/// Each `## E-NNN` entry's number and its `- **RFC:**` field value, if any.
fn entries(errata: &str) -> Vec<Entry<'_>> {
    let mut out: Vec<Entry<'_>> = Vec::new();
    let mut in_summary = false;
    for line in errata.lines() {
        if line.trim() == "## Summary" {
            in_summary = true;
        }
        if in_summary {
            continue;
        }
        if let Some(rest) = line.strip_prefix("## E-") {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(num) = digits.parse() {
                out.push(Entry { num, field: None });
                continue;
            }
        }
        if let Some(rest) = line.trim_start().strip_prefix("- **RFC:**") {
            if let Some(e) = out.last_mut() {
                if e.field.is_none() {
                    e.field = Some(rest.trim());
                }
            }
        }
    }
    out
}

/// Pure core. `Ok` carries what was covered; `Err` carries every problem found, each naming its subject.
pub fn evaluate(i: &Inputs<'_>) -> Result<Report, Vec<String>> {
    let vocab: Vocabulary = parse_policy(i.policy).map_err(|e| vec![e])?;
    let mut problems: Vec<String> = Vec::new();

    // 0. every lifecycle folder has at least one state defined for it: with `Accepted` deleted from the
    //    table, `accepted/` is empty today and nothing else would notice.
    for folder in ["proposed", "accepted", "done"] {
        if vocab.names_in(folder).is_empty() {
            problems.push(format!(
                "RFC 000's state table defines no state for the `{folder}/` folder -- an RFC placed there could not carry a legal status"
            ));
        }
    }

    // 1. every RFC's Status field names a defined state
    let mut status_of: Vec<(String, Option<String>)> = Vec::new(); // (rfc id, keyword)
    for (path, folder, content) in i.rfcs {
        let Some(line) = status_field_line(content) else {
            continue; // the documented no-Status-field exceptions
        };
        match extract_status_keyword_in(content, &vocab) {
            None => problems.push(format!(
                "{path}: Status field `{}` names no state RFC 000 defines",
                line.trim()
            )),
            Some(k) => {
                if let Some(id) = rfc_id(path) {
                    status_of.push((id, Some(k.to_string())));
                }
                let _ = folder;
            }
        }
    }

    // 2. the rule is present at both ends
    match section(i.policy, "## Lifecycle states") {
        Some(sec) if normalise(sec).contains(&normalise(RULE_CLAUSE)) => {}
        Some(_) => problems.push(format!(
            "RFC 000's `## Lifecycle states` section does not contain the drift/errata rule \
             (\"{RULE_CLAUSE}\") -- the rule that every errata disposition applies is not written down"
        )),
        None => problems.push("RFC 000 has no `## Lifecycle states` section".to_string()),
    }
    let header_end = i.errata.find("\n## E-").unwrap_or(i.errata.len());
    if !i.errata[..header_end].contains("000-rfc-lifecycle-policy.md") {
        problems.push(
            "rfcs/ERRATA.md's header does not link to rfcs/done/000-rfc-lifecycle-policy.md -- the register \
             must point at the rule it applies"
                .to_string(),
        );
    }

    // 3. Implemented-with-Errata => named in the register
    let mut with_errata = 0usize;
    for (path, _folder, content) in i.rfcs {
        let (Some(id), Some("Implemented-with-Errata")) =
            (rfc_id(path), extract_status_keyword_in(content, &vocab))
        else {
            continue;
        };
        with_errata += 1;
        if !names(i.errata, &id) {
            problems.push(format!(
                "{path}: Status is Implemented-with-Errata but {id} is named in no entry of rfcs/ERRATA.md \
                 -- the drift rule says an entry is added"
            ));
        }
    }

    // 4. forward field
    let rows = parse_summary_rows(i.errata);
    let mut fielded = 0usize;
    for e in entries(i.errata) {
        let id = format!("E-{}", e.num);
        let Some(field) = e.field else {
            if e.num >= FIELD_REQUIRED_FROM {
                problems.push(format!(
                    "{id}: no `- **RFC:**` field (required of entries from E-{FIELD_REQUIRED_FROM}): name the RFC whose text over-claims, or `none`"
                ));
            }
            continue;
        };
        fielded += 1;
        let value = field.trim_matches(|c: char| c == '`' || c == '*' || c.is_whitespace());
        if value == "none" {
            continue;
        }
        let token: String = value
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
            .collect();
        let status = status_of
            .iter()
            .find(|(rid, _)| *rid == token)
            .and_then(|(_, k)| k.as_deref());
        let Some(status) = status else {
            problems.push(format!(
                "{id}: `- **RFC:** {value}` names no RFC in proposed/, accepted/ or done/ (use `none` if the drift is not an RFC's text)"
            ));
            continue;
        };
        let live = rows
            .iter()
            .find(|r| r.id == id)
            .is_some_and(|r| is_open(&r.status) || is_accepted(&r.status));
        if live && status == "Implemented" {
            problems.push(format!(
                "{id} is live (OPEN or ACCEPTED) against {token}, which is marked plain Implemented: by the \
                 drift rule it is re-marked Implemented-with-Errata in the same change that files the entry"
            ));
        }
    }

    if problems.is_empty() {
        Ok(Report {
            states: vocab.states.len(),
            rfcs: i.rfcs.len(),
            with_errata,
            fielded_entries: fielded,
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real(path: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path),
        )
        .expect(path)
    }

    const POLICY: &str = "# P\n\n## Lifecycle states\n\n| State | Folder | Meaning |\n|---|---|---|\n\
        | **Proposed** | `proposed/` | x |\n| **Accepted** | `accepted/` | x |\n| **Implemented** | `done/` | x |\n\
        | **Implemented-with-Errata** | `done/` | x |\n\n### The drift and errata rule\n\n\
        > **An RFC may not be marked Implemented if its normative text makes a claim\n\
        > the merged code does not satisfy.** More words may follow.\n\n## Folder layout\n";
    const ERRATA: &str = "# R\n\nSee [the rule](./done/000-rfc-lifecycle-policy.md#x).\n\n## E-001 — a thing\n\n- **Status:** x\n\n## Summary\n\n| Errata | Tracking RFC | Status |\n|---|---|---|\n| E-001 a | RFC-0.1-001 | CLOSED |\n";

    fn rfc(path: &str, folder: &str, status: &str) -> (String, String, String) {
        (
            path.to_string(),
            folder.to_string(),
            format!("# T\n\n**Status:** {status}\n"),
        )
    }

    fn run(
        policy: &str,
        errata: &str,
        rfcs: &[(String, String, String)],
    ) -> Result<Report, Vec<String>> {
        evaluate(&Inputs {
            policy,
            errata,
            rfcs,
        })
    }

    #[test]
    fn the_control_is_green() {
        let rfcs = [rfc(
            "rfcs/done/RFC-0.1-001-a.md",
            "done",
            "Implemented (0.1.0)",
        )];
        assert!(run(POLICY, ERRATA, &rfcs).is_ok());
    }

    // 1. vocabulary ----------------------------------------------------------------------------
    #[test]
    fn a_status_word_the_policy_does_not_define_is_red_and_named() {
        let rfcs = [rfc(
            "rfcs/done/RFC-0.1-001-a.md",
            "done",
            "Closed (all done)",
        )];
        let e = run(POLICY, ERRATA, &rfcs).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("RFC-0.1-001-a.md") && p.contains("names no state")),
            "{e:?}"
        );
    }

    #[test]
    fn deleting_a_defined_and_used_state_from_the_table_is_red() {
        let policy = POLICY.replace("| **Accepted** | `accepted/` | x |\n", "");
        let rfcs = [rfc(
            "rfcs/accepted/RFC-0.1-002-b.md",
            "accepted",
            "Accepted (owner)",
        )];
        let e = run(&policy, ERRATA, &rfcs).unwrap_err();
        assert!(e.iter().any(|p| p.contains("RFC-0.1-002-b.md")), "{e:?}");
    }

    #[test]
    fn deleting_accepted_is_red_even_when_no_rfc_is_in_that_folder() {
        let policy = POLICY.replace("| **Accepted** | `accepted/` | x |\n", "");
        let e = run(&policy, ERRATA, &[]).unwrap_err();
        assert!(e.iter().any(|p| p.contains("`accepted/` folder")), "{e:?}");
    }

    #[test]
    fn deleting_implemented_with_errata_turns_its_rfcs_red_not_into_implemented() {
        let policy = POLICY.replace("| **Implemented-with-Errata** | `done/` | x |\n", "");
        let rfcs = [rfc(
            "rfcs/done/RFC-0.9-001-z.md",
            "done",
            "Implemented-with-Errata (0.9.0)",
        )];
        let e = run(&policy, ERRATA, &rfcs).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("RFC-0.9-001-z.md") && p.contains("names no state")),
            "{e:?}"
        );
    }

    #[test]
    fn a_policy_with_its_table_gone_is_an_error() {
        let policy = POLICY.replace("| State | Folder | Meaning |", "| nothing |");
        assert!(run(&policy, ERRATA, &[]).is_err());
    }

    // 2. the rule's presence -------------------------------------------------------------------
    #[test]
    fn removing_the_rule_from_rfc_000_is_red() {
        let policy = POLICY.replace(
            "An RFC may not be marked Implemented if its normative text makes a claim",
            "Some other sentence about a claim",
        );
        let e = run(&policy, ERRATA, &[]).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("does not contain the drift/errata rule")),
            "{e:?}"
        );
    }

    #[test]
    fn the_rule_in_the_wrong_section_is_red() {
        // present in the file but after `## Folder layout`: not in the definition of the states.
        let policy = POLICY.replace(
            "### The drift and errata rule",
            "## Folder layout\n\n### The drift and errata rule",
        );
        assert!(run(&policy, ERRATA, &[]).is_err());
    }

    #[test]
    fn rewording_after_the_first_clause_and_changing_the_markup_stays_green() {
        let policy = POLICY
            .replace("**An RFC may not be marked Implemented if its normative text makes a claim\n> the merged code does not satisfy.** More words may follow.", "An RFC `may not be marked` *Implemented* if its normative text makes a claim the   merged code does not satisfy. Entirely different words after.");
        assert!(run(&policy, ERRATA, &[]).is_ok());
    }

    #[test]
    fn removing_the_register_headers_link_is_red() {
        let errata = ERRATA.replace("./done/000-rfc-lifecycle-policy.md#x", "./nowhere.md");
        let e = run(POLICY, &errata, &[]).unwrap_err();
        assert!(
            e.iter().any(|p| p.contains("header does not link")),
            "{e:?}"
        );
    }

    // 3. Implemented-with-Errata => named ------------------------------------------------------
    #[test]
    fn an_implemented_with_errata_rfc_no_entry_names_is_red_and_one_that_is_named_is_green() {
        let unnamed = [rfc(
            "rfcs/done/RFC-0.9-001-z.md",
            "done",
            "Implemented-with-Errata (0.9.0)",
        )];
        let e = run(POLICY, ERRATA, &unnamed).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("RFC-0.9-001") && p.contains("named in no entry")),
            "{e:?}"
        );
        let errata = ERRATA.replace("## E-001 — a thing", "## E-001 — RFC-0.9-001: a thing");
        assert!(run(POLICY, &errata, &unnamed).is_ok());
        // a prefix of a longer number is not a name
        let errata = ERRATA.replace("## E-001 — a thing", "## E-001 — RFC-0.9-0011: a thing");
        assert!(run(POLICY, &errata, &unnamed).is_err());
    }

    // 4. the forward field ----------------------------------------------------------------------
    fn with_entry(field: &str, status: &str) -> String {
        format!(
            "# R\n\nSee [the rule](./done/000-rfc-lifecycle-policy.md).\n\n## E-130 — a thing\n\n- **RFC:** {field}\n\n## Summary\n\n| Errata | Tracking RFC | Status |\n|---|---|---|\n| E-130 a | RFC-0.1-001 | {status} |\n"
        )
    }

    #[test]
    fn a_live_entry_whose_rfc_is_plain_implemented_is_red_and_the_errata_status_or_none_is_green() {
        let plain = [rfc(
            "rfcs/done/RFC-0.1-001-a.md",
            "done",
            "Implemented (0.1.0)",
        )];
        let e = run(POLICY, &with_entry("RFC-0.1-001", "OPEN"), &plain).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("E-130") && p.contains("plain Implemented")),
            "{e:?}"
        );
        let marked = [rfc(
            "rfcs/done/RFC-0.1-001-a.md",
            "done",
            "Implemented-with-Errata (0.1.0)",
        )];
        let errata = with_entry("RFC-0.1-001", "OPEN")
            .replace("## E-130 — a thing", "## E-130 — RFC-0.1-001: a thing");
        assert!(run(POLICY, &errata, &marked).is_ok());
        assert!(run(POLICY, &with_entry("none", "OPEN"), &plain).is_ok());
        // closed entries are not live
        assert!(run(POLICY, &with_entry("RFC-0.1-001", "CLOSED"), &plain).is_ok());
    }

    #[test]
    fn a_field_naming_no_rfc_and_a_missing_required_field_are_red() {
        let e = run(POLICY, &with_entry("RFC-9.9-999", "OPEN"), &[]).unwrap_err();
        assert!(e.iter().any(|p| p.contains("names no RFC")), "{e:?}");
        let missing = with_entry("none", "OPEN").replace("- **RFC:** none\n", "");
        let e = run(POLICY, &missing, &[]).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("E-130") && p.contains("no `- **RFC:**` field")),
            "{e:?}"
        );
    }

    #[test]
    fn entries_before_the_cutoff_are_not_retro_applied() {
        // E-001 in ERRATA has no field and passes.
        assert!(run(POLICY, ERRATA, &[]).is_ok());
    }

    // the real tree -----------------------------------------------------------------------------
    #[test]
    fn the_real_policy_defines_the_rule_and_the_register_links_it() {
        let policy = real(crate::status::POLICY_PATH);
        let errata = real("rfcs/ERRATA.md");
        let e = run(&policy, &errata, &[]);
        assert!(e.is_ok(), "{e:?}");
    }
}
