//! `closed-erratum-prose` -- prose that still asserts a defect the register says is
//! closed (RFC-0.41-005).
//!
//! **Why this exists.** 0.40.0 shipped two documents asserting a defect 0.40.0 itself had
//! fixed: `v1-readiness.md`'s attestation row said the record "is constants" and named the
//! erratum beside the word `ACCEPTED`, and `fjell-diagnosticsd`'s module comment said the
//! signed record "is independent constants" with the erratum as the clause's tail. (The
//! exact texts are in commit 8ba6762; they are not quoted here because this file is itself
//! in the scope it checks.) `v1-limitations.md` had been swept correctly at the cut, so
//! this was not carelessness; it was a sweep with no instrument behind it.
//! `errata-limitations` only checks that every ACCEPTED erratum is *referenced*; a CLOSED
//! one has no obligation, so nothing looked at prose about it.
//!
//! **What this checks.** For every erratum whose Summary status is `CLOSED`, every
//! reference to its identifier in the *live* prose of the tree (scope below), judged in
//! its **segment** -- the smallest enclosing parenthesis if the id sits in one, else the
//! sentence or table cell -- against the two shapes the instances take:
//!
//! - **A. A live status is attached to a closed erratum.** The segment contains a live
//!   status word (`ACCEPTED`, `OPEN`, `unscheduled`, `IN PROGRESS`, `still open`,
//!   `remain(s) open`, `stay(s) open`) and, of the status words in the segment (a live one,
//!   or `closed`/`CLOSED`), the **nearest to this id is a live one**; words struck through
//!   with `~~` do not count. This is the readiness row: an id, then `ACCEPTED`, in one
//!   parenthesis.
//! - **B. A present-tense claim whose grounds are the closed erratum.** The id closes the
//!   segment as an appositive -- a comma before it, then `;` `)` `.` or the end -- after a
//!   present-tense assertion verb (`is`, `are`, `stays`, `remains`, `holds`, `has`,
//!   `have`, `carries`, `supplies`, `asserts`), and the segment has no closure word, no
//!   RFC/decision reference and no past-tense marker (`was`, `were`, `had`, `found`,
//!   `filed`, `named`, `recorded`). This is the comment: `(<a claim in the present tense>,
//!   E-NNN; ...)`.
//!
//! **Forms that are accepted -- the legitimate set, and why.** The tree mentions closed
//! errata constantly and almost always correctly, so the rule is a net for two known
//! shapes, not a requirement that every mention wear a closure form. Measured on the tree
//! this check was written against: 683 references to closed errata in 515 files, none
//! flagged. They take these forms, all of which pass:
//! - a closure word in the segment (`closes E-NNN`, `closed 2026-10-29 by RFC-... (E-NNN)`,
//!   `fixed`, `retired`, `removed`, `no longer`, `used to`, `since`);
//! - an RFC or decision reference in the segment -- `(RFC-0.33-003, E-NNN)`, `RFC-0.38-001
//!   D2 / E-NNN:` -- the commonest form in source comments: provenance, crediting the line
//!   that fixed or filed it;
//! - a bare provenance citation, `(E-NNN)`, `(E-NNN, E-MMM)`, `E-NNN's shape`, `the E-NNN
//!   shape`, which names an erratum without asserting its defect is present;
//! - a past-tense account (`E-NNN was a reservation defect ...`);
//! - a table row or list in which the id is followed by its own `(CLOSED)`; or the
//!   strikethrough form `~~ACCEPTED~~ **CLOSED**`;
//! - a *different* erratum's live status: `E-NNN (CLOSED); E-MMM (ACCEPTED)` -- a status
//!   word attaches to the *nearest* id, not to every id in the segment.
//!
//! **Scope, and why.** Markdown under `docs/src/` (except `history/`, `audit/` and
//! `assurance/`, which are dated records of what was true then); `README.md` and
//! `crates/*/README.md`; and the `.rs` files under `crates/` and `tools/` (doc comments and
//! code comments assert facts about the code). **Not in scope:** `rfcs/` (each RFC, handoff
//! and answer is a dated document), `releases/`, `CHANGELOG.md`, `tests/evidence/`,
//! `ROADMAP.md` (a chronological narrative whose sections are written at their milestone),
//! `.github/` and `.toml`, and `rfcs/ERRATA.md` itself, which is **read, never edited**:
//! the register is the source of truth and the check consumes it. A stale claim in a dated
//! document is a record of its time; a stale claim in a live one is the defect.
//!
//! **What it cannot decide, stated so a reader does not assume otherwise.** It matches
//! *forms*, not meaning. A live document can assert a closed defect in words that fit
//! neither shape -- the defect described without naming its erratum, an id placed mid-clause
//! rather than as an appositive tail, or a claim hedged with a past-tense word -- and this
//! check will not see it. B in particular is tuned to the shape the known instance took,
//! and was *chosen narrow on purpose*: a broad rule that fired on correct prose would be
//! worked around by rephrasing until it stopped complaining, which destroys its value. The
//! alternatives measured and rejected: requiring a closure form of every reference (on the
//! tree, 248 of the 683 references carry neither a closure word nor an RFC reference --
//! bare provenance citations, history, analogies -- and would all need exemption);
//! flagging every such reference that sits beside a present-tense verb (44 hits, most of
//! them correct analogies and history). **The exception list is empty.**
//!
//! Pure over its inputs (`judge_sources`), so its rules are unit-tested on the host; the
//! subcheck only gathers the files.

use crate::{read_dir_named, read_file};
use fjell_consistency_check::errata::parse_summary_rows;
use std::fs;
use std::process::ExitCode;

const NAME: &str = "closed-erratum-prose";
const ERRATA_PATH: &str = "rfcs/ERRATA.md";

/// `docs/src/` subtrees that are dated records, not live statements of current state.
const EXCLUDED_DOC_DIRS: &[&str] = &["history", "audit", "assurance"];

/// A finding: the rule that fired, the file and line, the erratum, and its segment.
#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub rule: char,
    pub file: String,
    pub line: usize,
    pub id: String,
    pub segment: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    pub references: usize,
    pub closed_errata: usize,
    pub findings: Vec<Finding>,
}

// ── text machinery (no regex dependency: a tool that checks the tree should not pull one in) ──

/// `E-NNN` (exactly three digits) not embedded in a longer identifier such as
/// `LEASE-CALLSITE-001`. Returns `(start, end)` byte offsets.
fn find_ids(text: &str) -> Vec<(usize, usize)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 5 <= b.len() {
        if b[i] == b'E'
            && b[i + 1] == b'-'
            && b[i + 2].is_ascii_digit()
            && b[i + 3].is_ascii_digit()
            && b[i + 4].is_ascii_digit()
        {
            let before_ok = i == 0
                || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'-');
            let after_ok = i + 5 >= b.len() || !b[i + 5].is_ascii_digit();
            if before_ok && after_ok {
                out.push((i, i + 5));
                i += 5;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn is_word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Does `text` contain `word` as a whole word? `ci` ignores ASCII case.
fn has_word(text: &str, word: &str, ci: bool) -> bool {
    let (t, w) = if ci {
        (text.to_ascii_lowercase(), word.to_ascii_lowercase())
    } else {
        (text.to_string(), word.to_string())
    };
    let tb = t.as_bytes();
    let mut from = 0;
    while let Some(rel) = t[from..].find(&w) {
        let s = from + rel;
        let e = s + w.len();
        let before = s == 0 || !is_word_char(tb[s - 1]);
        let after = e >= tb.len() || !is_word_char(tb[e]);
        if before && after {
            return true;
        }
        from = s + 1;
    }
    false
}

fn has_any(text: &str, words: &[&str], ci: bool) -> bool {
    words.iter().any(|w| has_word(text, w, ci))
}

/// Words that say an erratum's defect is no longer live, or that credit the fix.
const CLOSURE: &[&str] = &[
    "close", "closes", "closed", "closing", "closure", "fix", "fixed", "fixes", "retire",
    "retired", "retires", "resolved", "repaired", "removed",
];
const CLOSURE_PHRASES: &[&str] = &["no longer", "used to", "since"];
const PAST: &[&str] = &["was", "were", "had", "found", "filed", "named", "recorded"];
const PRESENT: &[&str] = &[
    "is", "are", "stay", "stays", "remain", "remains", "hold", "holds", "has", "have", "carries",
    "supplies", "asserts",
];
/// Live-status words that are case-sensitive status vocabulary.
const STALE_UPPER: &[&str] = &["ACCEPTED", "OPEN"];

fn closure_in(seg: &str) -> bool {
    has_any(seg, CLOSURE, true) || has_any(seg, CLOSURE_PHRASES, true)
}

/// An RFC reference (`RFC-0.33-003`, `RFC 053`, `RFC-v0.7.4-003`) or a decision token
/// (`D2`) in the segment.
fn rfc_in(seg: &str) -> bool {
    let b = seg.as_bytes();
    let mut i = 0;
    while i + 3 <= b.len() {
        if &b[i..i + 3] == b"RFC" && (i == 0 || !is_word_char(b[i - 1])) {
            let mut j = i + 3;
            while j < b.len() && (b[j] == b'-' || b[j] == b' ') {
                j += 1;
            }
            if j < b.len() && b[j] == b'v' {
                j += 1;
            }
            if j < b.len() && b[j].is_ascii_digit() {
                return true;
            }
        }
        // a decision token: D then digits, as a whole word
        if b[i] == b'D' && (i == 0 || !is_word_char(b[i - 1])) {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && (j >= b.len() || !is_word_char(b[j])) {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Live-status word positions in `seg` (byte offsets).
fn stale_positions(seg: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let b = seg.as_bytes();
    for w in STALE_UPPER
        .iter()
        .chain(["unscheduled", "IN PROGRESS"].iter())
    {
        let mut from = 0;
        while let Some(rel) = seg[from..].find(w) {
            let s = from + rel;
            let e = s + w.len();
            if (s == 0 || !is_word_char(b[s - 1])) && (e >= b.len() || !is_word_char(b[e])) {
                out.push(s);
            }
            from = s + 1;
        }
    }
    for phrase in [
        "still open",
        "remain open",
        "remains open",
        "stay open",
        "stays open",
    ] {
        let low = seg.to_ascii_lowercase();
        let mut from = 0;
        while let Some(rel) = low[from..].find(phrase) {
            out.push(from + rel);
            from += rel + 1;
        }
    }
    out
}

/// Byte ranges enclosed by pairs of `~~` (struck-through text is history, not a claim).
fn struck_ranges(seg: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(a) = seg[from..].find("~~") {
        let a = from + a;
        match seg[a + 2..].find("~~") {
            Some(z) => {
                out.push((a, a + 2 + z + 2));
                from = a + 2 + z + 2;
            }
            None => break,
        }
    }
    out
}

/// Positions of `closed` / `CLOSED` (whole word, any case) in `seg`.
fn closed_positions(seg: &str) -> Vec<usize> {
    let low = seg.to_ascii_lowercase();
    let b = low.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = low[from..].find("closed") {
        let s = from + rel;
        let e = s + 6;
        if (s == 0 || !is_word_char(b[s - 1])) && (e >= b.len() || !is_word_char(b[e])) {
            out.push(s);
        }
        from = s + 1;
    }
    out
}

/// Strip a leading comment/quote marker so a wrapped `//` or `///` comment reads as prose.
fn strip_marker(line: &str) -> &str {
    let l = line.trim();
    for p in ["//!", "///", "//", "#", ">", "*"] {
        if let Some(rest) = l.strip_prefix(p) {
            return rest.trim();
        }
    }
    l
}

/// Paragraphs: consecutive non-blank lines joined by a space, each with the 1-based line of
/// its first line and, per source line, the byte offset where that line starts in the joined
/// text (so a position maps back to a line number).
struct Para {
    text: String,
    first_line: usize,
    line_starts: Vec<usize>,
}

fn paragraphs(src: &str) -> Vec<Para> {
    let mut out: Vec<Para> = Vec::new();
    let mut cur: Option<Para> = None;
    for (n, line) in src.lines().enumerate() {
        let s = strip_marker(line);
        if s.is_empty() {
            if let Some(p) = cur.take() {
                out.push(p);
            }
            continue;
        }
        match cur.as_mut() {
            None => {
                cur = Some(Para {
                    text: s.to_string(),
                    first_line: n + 1,
                    line_starts: vec![0],
                })
            }
            Some(p) => {
                p.text.push(' ');
                p.line_starts.push(p.text.len());
                p.text.push_str(s);
            }
        }
    }
    if let Some(p) = cur {
        out.push(p);
    }
    out
}

fn line_of(p: &Para, pos: usize) -> usize {
    let idx = p.line_starts.iter().rposition(|&s| s <= pos).unwrap_or(0);
    p.first_line + idx
}

/// The segment containing byte `pos` of `par`: the smallest enclosing parenthesis (at most
/// 400 bytes), else the sentence or table cell.
fn segment_of(par: &str, pos: usize) -> (usize, usize) {
    let b = par.as_bytes();
    // enclosing parentheses
    let mut depth = 0i32;
    let mut open = None;
    let mut k = pos as isize;
    while k >= 0 {
        match b[k as usize] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    open = Some(k as usize);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
        k -= 1;
    }
    if let Some(s) = open {
        let mut d = 0i32;
        for (j, &c) in b.iter().enumerate().skip(s) {
            match c {
                b'(' => d += 1,
                b')' => {
                    d -= 1;
                    if d == 0 {
                        if j + 1 - s <= 400 {
                            return (s, j + 1);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    // sentence / cell
    let start = [". ", "; ", " | "]
        .iter()
        .filter_map(|d| par[..pos].rfind(d).map(|i| i + d.len() - 1))
        .max()
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = [". ", " | "]
        .iter()
        .filter_map(|d| par[pos..].find(d).map(|i| pos + i))
        .min()
        .unwrap_or(par.len());
    (start, end)
}

/// Judge one file's text. Returns `(references_to_closed_errata, findings)`.
fn judge_text(file: &str, text: &str, closed: &[String]) -> (usize, Vec<Finding>) {
    let mut refs = 0;
    let mut out = Vec::new();
    for p in paragraphs(text) {
        for (s, e) in find_ids(&p.text) {
            let id = &p.text[s..e];
            if !closed.iter().any(|c| c == id) {
                continue;
            }
            refs += 1;
            let (a, z) = segment_of(&p.text, s);
            let seg = &p.text[a..z];
            let at = s - a; // the id's offset in the segment
            let finding = |rule: char| Finding {
                rule,
                file: file.to_string(),
                line: line_of(&p, s),
                id: id.to_string(),
                segment: seg.chars().take(200).collect(),
            };

            // A -- a live status attached to this closed erratum: of the status words in the
            // segment, the one nearest this id is a live one (a struck-through word is history).
            let struck = struck_ranges(seg);
            let live: Vec<usize> = stale_positions(seg)
                .into_iter()
                .filter(|p| !struck.iter().any(|(a, z)| p >= a && p < z))
                .collect();
            if !live.is_empty() {
                let closed_pos = closed_positions(seg);
                let nearest = |v: &[usize]| v.iter().map(|p| p.abs_diff(at)).min();
                let d_live = nearest(&live);
                let d_closed = nearest(&closed_pos);
                // The id must also be the nearest id to that live word (a status word
                // belongs to the erratum it sits beside).
                let ids = find_ids(seg);
                let live_attached = live.iter().any(|sp| {
                    ids.iter()
                        .min_by_key(|(is, _)| is.abs_diff(*sp))
                        .map(|(is, _)| *is == at)
                        .unwrap_or(false)
                });
                let live_is_nearer = match (d_live, d_closed) {
                    (Some(l), Some(c)) => l < c,
                    (Some(_), None) => true,
                    _ => false,
                };
                if live_attached && live_is_nearer {
                    out.push(finding('A'));
                }
                continue;
            }

            // B -- a present-tense claim whose grounds are this closed erratum.
            let before = seg[..at].trim_end();
            let after = seg[at + 5..].trim_start();
            let tail = after.is_empty()
                || after.starts_with(';')
                || after.starts_with(')')
                || after.starts_with('.');
            if before.ends_with(',')
                && tail
                && has_any(&seg[..at], PRESENT, true)
                && !closure_in(seg)
                && !rfc_in(seg)
                && !has_any(seg, PAST, true)
            {
                out.push(finding('B'));
            }
        }
    }
    (refs, out)
}

/// Judge a set of `(path, text)` sources against the register's text. Pure.
pub fn judge_sources(errata_src: &str, files: &[(String, String)]) -> Report {
    let closed: Vec<String> = parse_summary_rows(errata_src)
        .into_iter()
        .filter(|r| r.status.starts_with("CLOSED"))
        .map(|r| r.id)
        .collect();
    let mut report = Report {
        closed_errata: closed.len(),
        ..Report::default()
    };
    for (path, text) in files {
        report.files += 1;
        let (refs, mut f) = judge_text(path, text, &closed);
        report.references += refs;
        report.findings.append(&mut f);
    }
    report
}

// ── gathering the live prose ─────────────────────────────────────────────────

fn walk(dir: &str, md: bool, rs: bool, skip_dirs: &[&str], out: &mut Vec<String>) {
    let Some(entries) = read_dir_named(NAME, dir) else {
        return;
    };
    let mut names: Vec<(String, bool)> = Vec::new();
    for e in entries.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        names.push((n, e.path().is_dir()));
    }
    names.sort();
    for (n, is_dir) in names {
        if n.starts_with('.') || n == "target" || n == "prebuilt" {
            continue;
        }
        let path = format!("{dir}/{n}");
        if is_dir {
            if skip_dirs.contains(&n.as_str()) {
                continue;
            }
            walk(&path, md, rs, skip_dirs, out);
        } else if (md && n.ends_with(".md")) || (rs && n.ends_with(".rs")) {
            out.push(path);
        }
    }
}

/// The live prose, by path: see the module doc for what is in scope and why.
fn live_files() -> Vec<String> {
    let mut v = Vec::new();
    walk("docs/src", true, false, EXCLUDED_DOC_DIRS, &mut v);
    v.push("README.md".to_string());
    // crates/*/README.md and every .rs under crates/ and tools/
    let mut crate_files = Vec::new();
    walk("crates", true, true, &[], &mut crate_files);
    v.extend(
        crate_files
            .into_iter()
            .filter(|p| p.ends_with(".rs") || p.ends_with("/README.md")),
    );
    walk("tools", false, true, &[], &mut v);
    v
}

pub fn check() -> ExitCode {
    let Some(errata) = read_file(NAME, ERRATA_PATH) else {
        return ExitCode::FAILURE;
    };
    let mut files = Vec::new();
    for path in live_files() {
        // A file this check was told to read and cannot is a file it cannot vouch for.
        match fs::read(&path) {
            Ok(bytes) => files.push((path, String::from_utf8_lossy(&bytes).into_owned())),
            Err(e) => {
                eprintln!("{NAME}: FAIL -- cannot read {path}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let report = judge_sources(&errata, &files);
    if report.closed_errata == 0 {
        eprintln!(
            "{NAME}: FAIL -- the register's Summary table yielded no CLOSED erratum; the parse is not seeing it"
        );
        return ExitCode::FAILURE;
    }
    if report.findings.is_empty() {
        println!(
            "{NAME}: PASS ({} references to {} closed errata in {} live files; none asserts a closed defect as live)",
            report.references, report.closed_errata, report.files
        );
        return ExitCode::SUCCESS;
    }
    for f in &report.findings {
        let what = match f.rule {
            'A' => "a live status is attached to a closed erratum",
            _ => "a present-tense claim rests on a closed erratum",
        };
        eprintln!(
            "{NAME}: FAIL -- {}:{}: {} ({}): {}",
            f.file, f.line, what, f.id, f.segment
        );
    }
    eprintln!(
        "{NAME}: FAIL ({} finding(s)); the register says these errata are CLOSED",
        report.findings.len()
    );
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A register in which E-094 is CLOSED and E-095 is ACCEPTED.
    fn register() -> String {
        "## Summary\n\n| Errata | Tracking | Status |\n|---|---|---|\n\
         | E-094 the record is constants | RFC-0.40-001 | CLOSED |\n\
         | E-095 three placeholders | unscheduled | ACCEPTED |\n"
            .to_string()
    }

    fn judge_one(text: &str) -> Report {
        judge_sources(&register(), &[("f.md".to_string(), text.to_string())])
    }

    // The fixtures build the erratum id from a variable so this file does not itself
    // contain the stale shapes it tests for (it is in scope for its own check).
    const ID: &str = "E-094";

    // ── the two instances ────────────────────────────────────────────────────

    /// Instance 1, `v1-readiness.md`'s attestation row before 8ba6762: a live status
    /// attached to a closed erratum.
    #[test]
    fn a_live_status_beside_a_closed_erratum_is_refused() {
        let t = format!(
            "the **record it produces is constants** (**{ID}**, ACCEPTED, limitations row 13): it supplies every digest"
        );
        let r = judge_one(&t);
        assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
        assert_eq!(r.findings[0].rule, 'A');
        assert_eq!(r.findings[0].id, ID);
    }

    /// Instance 2, `fjell-diagnosticsd`'s module comment before 8ba6762: a present-tense
    /// claim whose grounds are the closed erratum, inside a sentence whose other half is true.
    #[test]
    fn a_present_tense_claim_resting_on_a_closed_erratum_is_refused() {
        let t = format!(
            "//! make attestation real** (the signed attestation record is independent\n//! constants, {ID}; `last_attestation_available` stays `false`) **and it does not**"
        );
        let r = judge_one(&t);
        assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
        assert_eq!(r.findings[0].rule, 'B');
        // the id is on the second source line of the wrapped comment
        assert_eq!(r.findings[0].line, 2);
    }

    /// And both instances after the fix pass: the corrected wording.
    #[test]
    fn the_corrected_wordings_pass() {
        let a = format!(
            "the record says only what is true since 0.40.0 (RFC-0.40-001, closes **{ID}**, limitations row 13)"
        );
        let b = "make attestation real** (the signed attestation record is produced by `attestd`, not here; `last_attestation_available` stays `false`)";
        assert!(judge_one(&a).findings.is_empty());
        assert!(judge_one(b).findings.is_empty());
    }

    // ── the legitimate set: forms that must keep passing ─────────────────────

    #[test]
    fn the_legitimate_forms_pass() {
        let forms = [
            format!("Closed by RFC-0.40-001: {ID} no longer holds"),
            format!("closed 2026-10-29 by RFC-0.40-001 ({ID})"),
            format!("(that would be {ID} with extra steps)"),
            format!("the same shape as {ID}'s fabrication"),
            format!("(RFC-0.33-003, {ID})"),
            format!("RFC-0.38-001 D2 / {ID}: the chain"),
            format!("({ID})"),
            format!("({ID}, E-095)"),
            format!("the {ID} shape, with a second presentation in it"),
            format!("{ID} was a reservation defect: the allocator reserved half"),
            format!(
                "(Erratum **{ID}**, ~~ACCEPTED 2026-10-19, unscheduled~~ **CLOSED 2026-10-24** by **RFC-0.38-001**)"
            ),
            format!("| Errata **{ID}** (CLOSED); **E-095** (ACCEPTED) |"),
            // an ACCEPTED erratum's live status is not this check's business
            "the placeholders (E-095, ACCEPTED, 0.41) are not computed".to_string(),
        ];
        for f in &forms {
            let r = judge_one(f);
            assert!(
                r.findings.is_empty(),
                "wrongly refused `{f}`: {:?}",
                r.findings
            );
        }
    }

    /// The status word attaches to the NEAREST id, not to every id in the segment.
    #[test]
    fn a_status_word_attaches_to_the_nearest_id() {
        let ok = format!("{ID} (CLOSED); E-095 (ACCEPTED)");
        assert!(judge_one(&ok).findings.is_empty());
        // swap them: now the closed erratum is the one beside ACCEPTED
        let bad = format!("E-095 (CLOSED); {ID} (ACCEPTED)");
        let r = judge_one(&bad);
        assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
        assert_eq!(r.findings[0].rule, 'A');
    }

    #[test]
    fn an_id_inside_a_longer_identifier_is_not_a_reference() {
        let r = judge_one("LEASE-CALLSITE-001 and SITE-094 and XE-094 and E-0941 are not errata");
        assert_eq!(r.references, 0);
    }

    #[test]
    fn a_wrapped_comment_is_read_as_one_sentence() {
        // The id is on the second line; its segment is the whole parenthesis.
        let t = format!("//! (the signed record is independent\n//! constants, {ID}; and so on)");
        assert_eq!(judge_one(&t).findings.len(), 1);
    }

    #[test]
    fn counts_and_lines_are_reported() {
        let t = format!(
            "first line\n\nsecond paragraph with a clean cite ({ID})\nand the record, which is constants, {ID}; more\n"
        );
        let r = judge_one(&t);
        assert_eq!(r.references, 2);
        assert_eq!(r.closed_errata, 1);
        assert_eq!(r.files, 1);
    }

    #[test]
    fn a_register_without_closed_errata_checks_nothing() {
        let r = judge_sources(
            "## Summary\n| E-095 x | unscheduled | ACCEPTED |\n",
            &[("f.md".into(), "E-095, ACCEPTED".into())],
        );
        assert_eq!(r.closed_errata, 0);
        assert_eq!(r.references, 0);
    }
}
