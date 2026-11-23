//! 0.48 small item A (E-130): **the crates.io landing page's errata figures are derived from the register,
//! not maintained by hand.**
//!
//! `crates/fjell-os/README.md` is the page an outsider sees first, and it states one figure about the
//! register: *"That register currently carries **N accepted limitations and M open**"*. It was
//! hand-maintained, and it was wrong at the 0.47.0 cut (it said 7 where the register said 6, because E-012
//! closed in that milestone) and then wrong the other way within the hour -- **7 -> 6 -> 7** -- when E-130
//! was filed. 0.47 small item C had just made the same page's *tier* count derived and gated; this is the
//! next figure of the same family (E-015 / E-025 / E-125 / E-130).
//!
//! **What is judged, and against what.** The register's `## Summary` table is the one place a disposition
//! lives (E-114), so the figures are counted from it through the shared parser (`errata::parse_summary_rows`,
//! `is_accepted`, `is_open`). Nothing here lists an erratum id.
//!
//! * **`accepted`** must equal the register's `ACCEPTED` count, **always**.
//! * **`open`** must equal the register's `OPEN` count **when that count is zero** -- the only state in which
//!   the page may be published, because Gate 7 refuses a release with an `OPEN` erratum. While the register
//!   *has* `OPEN` entries (a milestone in progress files them `OPEN` on purpose) the sentence "0 open" is
//!   false of the working tree, and this check **says so in its output without failing**: failing here would
//!   keep Gate 12 red for most of every milestone, which teaches people to ignore Gate 12, and the release is
//!   already blocked by Gate 7. A page that states a *nonzero* open figure must state the register's.
//!
//! **Fails closed.** A page with no such sentence, or one whose figures cannot be read, is a failure: a check
//! that finds nothing to compare has not compared anything.
//!
//! **What this does not do:** it does not judge the sentence's *prose* (that the limitations are "found by
//! auditing the verification tooling", the adjacent claim about hardware), only its two figures. The page's
//! other hand-maintained claims -- *"eleven mechanical gates, one manual check"* -- are not gated here (see
//! the 0.48 review request).

use crate::errata::{is_accepted, is_open, parse_summary_rows};

pub const LANDING_PAGE: &str = "crates/fjell-os/README.md";
pub const ERRATA_PATH: &str = "rfcs/ERRATA.md";

/// What the page says, parsed.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Stated {
    pub accepted: usize,
    pub open: usize,
}

/// What the register holds.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Held {
    pub accepted: usize,
    pub open: usize,
}

/// The two figures in `carries **N accepted limitations and M open**` (emphasis optional, wording otherwise
/// exact). `None` when the sentence is absent or a figure is not a number.
pub fn parse_stated(page: &str) -> Option<Stated> {
    let flat: String = page.chars().filter(|c| *c != '*').collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    let i = flat.find(" accepted limitation")?;
    let accepted = number_before(&flat[..i])?;
    let rest = &flat[i..];
    let j = rest.find(" and ")?;
    let after = &rest[j + " and ".len()..];
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    let tail = &after[digits.len()..];
    if digits.is_empty() || !tail.trim_start().starts_with("open") {
        return None;
    }
    Some(Stated {
        accepted,
        open: digits.parse().ok()?,
    })
}

fn number_before(s: &str) -> Option<usize> {
    let digits: String = s
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

/// Count the register's `ACCEPTED` and `OPEN` rows from its Summary table.
pub fn held(errata: &str) -> Held {
    let rows = parse_summary_rows(errata);
    Held {
        accepted: rows.iter().filter(|r| is_accepted(&r.status)).count(),
        open: rows.iter().filter(|r| is_open(&r.status)).count(),
    }
}

/// The verdict: `Ok` carries a line to print (it may carry a note); `Err` carries every problem.
pub fn judge(page: &str, errata: &str) -> Result<String, Vec<String>> {
    let Some(s) = parse_stated(page) else {
        return Err(vec![format!(
            "{LANDING_PAGE} states no `N accepted limitations and M open` sentence the check can read -- the \
             page's one errata figure is ungated, and an absent claim is not a passing one"
        )]);
    };
    let h = held(errata);
    if h.accepted == 0 && h.open == 0 {
        return Err(vec![format!(
            "{ERRATA_PATH} has no ACCEPTED or OPEN rows in its Summary table -- the register could not be read"
        )]);
    }
    let mut problems = Vec::new();
    if s.accepted != h.accepted {
        problems.push(format!(
            "{LANDING_PAGE} says {} accepted limitations, but the register's Summary table has {} ACCEPTED \
             rows -- correct the page (it is what crates.io shows, and `cargo publish` packages the working tree)",
            s.accepted, h.accepted
        ));
    }
    let mut note = String::new();
    if h.open == 0 {
        if s.open != 0 {
            problems.push(format!(
                "{LANDING_PAGE} says {} open, but the register has no OPEN rows",
                s.open
            ));
        }
    } else if s.open != 0 && s.open != h.open {
        problems.push(format!(
            "{LANDING_PAGE} says {} open, but the register has {} OPEN rows (a page that states a nonzero \
             figure must state the register's)",
            s.open, h.open
        ));
    } else if s.open == 0 {
        note = format!(
            "; NOTE the register has {} OPEN row(s) so \"0 open\" is not yet true of this tree -- Gate 7 \
             blocks the release until it is",
            h.open
        );
    }
    if problems.is_empty() {
        Ok(format!(
            "landing-page: PASS (the page says {} accepted limitations and {} open; the register's Summary \
             table has {} ACCEPTED and {} OPEN{note}. This judges those two figures, not the sentence's prose)",
            s.accepted, s.open, h.accepted, h.open
        ))
    } else {
        Err(problems)
    }
}

// ── the extension (0.48 stage-1 review): the root README's figures, and no gate count on the page ───────────

pub const ROOT_README: &str = "README.md";
pub const READINESS_PATH: &str = "docs/src/releasing/v1-readiness.md";

/// The four counts after the readiness link: `... v1-readiness.md](...) — N DONE, M IN PROGRESS, K DEFERRED, J OPEN`.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Readiness {
    pub done: usize,
    pub in_progress: usize,
    pub deferred: usize,
    pub open: usize,
}

fn leading_number(s: &str) -> Option<(usize, &str)> {
    let t = s.trim_start();
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    Some((digits.parse().ok()?, &t[digits.len()..]))
}

/// The readiness figures the root README states, from the line that links the matrix.
pub fn parse_root_readiness(readme: &str) -> Option<Readiness> {
    let line = readme.lines().find(|l| l.contains("v1-readiness.md"))?;
    let after = &line[line.find("v1-readiness.md")?..];
    let after = &after[after.find(" — ")? + " — ".len()..];
    let mut rest = after;
    let mut out = [0usize; 4];
    for (i, label) in ["DONE", "IN PROGRESS", "DEFERRED", "OPEN"]
        .iter()
        .enumerate()
    {
        let (n, r) = leading_number(rest)?;
        let r = r.trim_start();
        let r = r.strip_prefix(label)?;
        out[i] = n;
        rest = r.trim_start_matches([',', ' ']);
    }
    Some(Readiness {
        done: out[0],
        in_progress: out[1],
        deferred: out[2],
        open: out[3],
    })
}

/// The matrix's own `## Summary at <v>` table (`readiness-summary` holds it equal to the rows).
pub fn parse_matrix_summary(matrix: &str) -> Option<Readiness> {
    let mut in_summary = false;
    let mut got = [None::<usize>; 4];
    for line in matrix.lines() {
        let t = line.trim();
        if t.starts_with("## Summary at") {
            in_summary = true;
            continue;
        }
        if in_summary && t.starts_with("## ") {
            break;
        }
        if !in_summary || !t.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = t.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() < 2 {
            continue;
        }
        let idx = match cells[0] {
            "DONE" => 0,
            "IN PROGRESS" => 1,
            "DEFERRED" => 2,
            "OPEN" => 3,
            _ => continue,
        };
        got[idx] = cells[1].parse().ok();
    }
    Some(Readiness {
        done: got[0]?,
        in_progress: got[1]?,
        deferred: got[2]?,
        open: got[3]?,
    })
}

/// `N files in \`done/\`` as the root README states it.
pub fn parse_root_done_files(readme: &str) -> Option<usize> {
    let i = readme.find(" files in `done/`")?;
    let digits: String = readme[..i]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

const NUMBER_WORDS: &[&str] = &[
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

/// A count of gates on a published page (*"eleven mechanical gates"*): a third hand-maintained figure that went stale
/// with the others. The page states **no** such number; the gates are in the rehearsal table.
pub fn gate_count_claims(page: &str) -> Vec<String> {
    let flat = page.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = flat.to_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find(" mechanical gates") {
        let at = from + i;
        let word = lower[..at].rsplit(' ').next().unwrap_or("");
        if word.chars().all(|c| c.is_ascii_digit()) && !word.is_empty()
            || NUMBER_WORDS.contains(&word)
        {
            out.push(format!("`{word} mechanical gates`"));
        }
        from = at + 1;
    }
    out
}

/// The root README's figures against their sources. `done_files` is the folder's real count.
pub fn judge_root(
    root_readme: &str,
    matrix: &str,
    done_files: usize,
) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    let stated = parse_root_readiness(root_readme);
    let held = parse_matrix_summary(matrix);
    match (stated, held) {
        (None, _) => problems.push(format!(
            "{ROOT_README} states no `N DONE, M IN PROGRESS, K DEFERRED, J OPEN` after its readiness-matrix link the check can read"
        )),
        (_, None) => problems.push(format!("{READINESS_PATH} has no readable `## Summary at` table")),
        (Some(a), Some(b)) if a != b => problems.push(format!(
            "{ROOT_README} says {} DONE / {} IN PROGRESS / {} DEFERRED / {} OPEN, but {READINESS_PATH}'s summary table says \
             {} / {} / {} / {}",
            a.done, a.in_progress, a.deferred, a.open, b.done, b.in_progress, b.deferred, b.open
        )),
        _ => {}
    }
    match parse_root_done_files(root_readme) {
        None => problems.push(format!(
            "{ROOT_README} states no `N files in `done/`` the check can read"
        )),
        Some(n) if n != done_files => problems.push(format!(
            "{ROOT_README} says {n} files in `done/`, but the folder holds {done_files}"
        )),
        _ => {}
    }
    if problems.is_empty() {
        Ok(format!(
            "{ROOT_README}: readiness {:?} equals the matrix's summary; {done_files} files in done/ equals the folder",
            stated.unwrap()
        ))
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTER: &str = "# R\n\n## Summary\n\n| Errata | Tracking RFC | Status |\n|---|---|---|\n\
        | E-9001 a | RFC-1 | CLOSED |\n| E-9002 b | RFC-2 | ACCEPTED (v1.0 limitation) |\n\
        | E-9003 c | RFC-3 | ACCEPTED |\n| E-9004 d | RFC-4 | CLOSED |\n";

    fn page(a: usize, o: usize) -> String {
        format!(
            "x\n\nThat register currently carries **{a} accepted limitations and {o} open** — including\nmore.\n"
        )
    }

    fn with_open(n: usize) -> String {
        let mut r = REGISTER.to_string();
        for i in 0..n {
            r.push_str(&format!("| E-909{i} z | RFC-9 | OPEN |\n"));
        }
        r
    }

    #[test]
    fn parses_the_sentence_with_or_without_emphasis_and_across_a_line_break() {
        assert_eq!(
            parse_stated(&page(7, 0)),
            Some(Stated {
                accepted: 7,
                open: 0
            })
        );
        assert_eq!(
            parse_stated("carries 12 accepted limitations and 3 open — x"),
            Some(Stated {
                accepted: 12,
                open: 3
            })
        );
        assert_eq!(
            parse_stated("carries **1 accepted\nlimitation and 0 open**"),
            Some(Stated {
                accepted: 1,
                open: 0
            })
        );
    }

    #[test]
    fn a_page_with_no_readable_sentence_is_an_error_not_a_pass() {
        for bad in [
            "nothing here",
            "carries many accepted limitations and none open",
            "carries 7 accepted limitations",
        ] {
            let e = judge(bad, REGISTER).unwrap_err();
            assert!(e[0].contains("states no"), "{bad}: {e:?}");
        }
    }

    #[test]
    fn the_control_is_green() {
        assert!(judge(&page(2, 0), REGISTER).is_ok());
    }

    #[test]
    fn a_stale_accepted_count_is_red_in_both_directions_and_names_both_figures() {
        for stated in [1usize, 3, 7] {
            let e = judge(&page(stated, 0), REGISTER).unwrap_err();
            assert!(
                e.iter()
                    .any(|p| p.contains(&format!("says {stated} accepted"))
                        && p.contains("has 2 ACCEPTED")),
                "{stated}: {e:?}"
            );
        }
    }

    /// The 0.47.0 sequence: the register went 7 -> 6 -> 7 inside an hour and the page was right for none of it.
    #[test]
    fn it_follows_the_register_as_the_register_moves() {
        let seven = REGISTER.replace(
            "| E-9004 d | RFC-4 | CLOSED |",
            "| E-9004 d | RFC-4 | ACCEPTED |",
        );
        assert!(judge(&page(3, 0), &seven).is_ok());
        assert!(judge(&page(2, 0), &seven).is_err());
        let six = REGISTER.replace(
            "| E-9002 b | RFC-2 | ACCEPTED (v1.0 limitation) |",
            "| E-9002 b | RFC-2 | CLOSED |",
        );
        assert!(judge(&page(1, 0), &six).is_ok());
        assert!(judge(&page(2, 0), &six).is_err());
    }

    #[test]
    fn open_is_exact_when_the_register_has_none() {
        let e = judge(&page(2, 2), REGISTER).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("says 2 open") && p.contains("no OPEN rows")),
            "{e:?}"
        );
    }

    #[test]
    fn a_milestone_in_progress_is_a_note_not_a_failure_and_a_wrong_nonzero_figure_is_red() {
        let r = with_open(2);
        let ok = judge(&page(2, 0), &r).unwrap();
        assert!(ok.contains("NOTE") && ok.contains("Gate 7"), "{ok}");
        assert!(judge(&page(2, 2), &r).is_ok());
        let e = judge(&page(2, 5), &r).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("says 5 open") && p.contains("has 2 OPEN")),
            "{e:?}"
        );
    }

    #[test]
    fn an_unreadable_register_is_an_error() {
        assert!(judge(&page(0, 0), "# R\n\nno table\n").is_err());
    }

    // ── the extension ─────────────────────────────────────────────────────────────────────────────
    const ROOT: &str = "- [v1.0 Readiness Matrix](docs/src/releasing/v1-readiness.md) — 56 DONE, 2 IN PROGRESS, 3 DEFERRED, 0 OPEN\n- [RFC Process](rfcs/README.md) — 238 files in `done/`\n";
    const MATRIX: &str = "## Summary at 0.48.0\n\n| Status | Count |\n|--------|-------|\n| DONE | 56 |\n| IN PROGRESS | 2 |\n| DEFERRED | 3 |\n| OPEN | 0 |\n\n## Next\n";

    #[test]
    fn the_root_readmes_figures_parse_and_the_control_is_green() {
        assert_eq!(
            parse_root_readiness(ROOT),
            Some(Readiness {
                done: 56,
                in_progress: 2,
                deferred: 3,
                open: 0
            })
        );
        assert_eq!(parse_matrix_summary(MATRIX), parse_root_readiness(ROOT));
        assert_eq!(parse_root_done_files(ROOT), Some(238));
        assert!(judge_root(ROOT, MATRIX, 238).is_ok());
    }

    #[test]
    fn each_root_figure_is_red_when_it_drifts_and_names_both_sides() {
        let e = judge_root(&ROOT.replace("56 DONE", "55 DONE"), MATRIX, 238).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("says 55 DONE") && p.contains("table says 56")),
            "{e:?}"
        );
        let e =
            judge_root(&ROOT.replace("2 IN PROGRESS", "0 IN PROGRESS"), MATRIX, 238).unwrap_err();
        assert!(e.iter().any(|p| p.contains("IN PROGRESS")), "{e:?}");
        let e = judge_root(ROOT, MATRIX, 241).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("says 238 files in `done/`") && p.contains("holds 241")),
            "{e:?}"
        );
    }

    #[test]
    fn an_absent_root_claim_is_a_failure_not_a_pass() {
        assert!(judge_root("nothing", MATRIX, 238).unwrap_err().len() >= 2);
        assert!(judge_root(ROOT, "no table", 238).is_err());
    }

    #[test]
    fn a_gate_count_on_a_published_page_is_found_and_prose_without_one_is_not() {
        assert_eq!(
            gate_count_claims("Releases are gated on eleven mechanical gates, and more").len(),
            1
        );
        assert_eq!(gate_count_claims("gated on 11 mechanical gates").len(), 1);
        assert!(
            gate_count_claims("Releases are gated on mechanical gates and a manual review")
                .is_empty()
        );
        assert!(gate_count_claims("the mechanical gates are listed in the rehearsal").is_empty());
    }

    #[test]
    fn the_real_root_readme_and_the_crates_page_state_what_the_register_and_matrix_hold() {
        let counts = crate::readme_counts::count_folders(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
        )
        .expect("folders");
        let r = judge_root(&real(ROOT_README), &real(READINESS_PATH), counts.done_total);
        assert!(r.is_ok(), "{r:?}");
        let g = gate_count_claims(&real(LANDING_PAGE));
        assert!(
            g.is_empty(),
            "the crates.io page must state no gate count: {g:?}"
        );
    }

    fn real(path: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path),
        )
        .expect(path)
    }

    /// On the real tree: the page states the sentence (so the gate is not vacuous) and the figures hold.
    #[test]
    fn the_real_landing_page_states_the_figures_and_they_match_the_register() {
        let page = real(LANDING_PAGE);
        assert!(
            parse_stated(&page).is_some(),
            "the real landing page must carry the sentence the gate reads"
        );
        let r = judge(&page, &real(ERRATA_PATH));
        assert!(r.is_ok(), "{r:?}");
    }
}
