//! `cargo xtask rfc promote <version> [--dry-run]` — RFC-0.43-004 (E-116).
//!
//! Step 7 of the cut: the milestone's RFCs move from `rfcs/accepted/` to `rfcs/done/`. That is
//! **five tasks** that used to be performed by hand, and a partial hand-performance is what
//! put three renames in history without their `Status:` edits (`a130ace`, fixed by `fe30ad6`):
//!
//!   1. each `rfcs/accepted/RFC-<milestone>-NNN-*.md` moves to `rfcs/done/`;
//!   2. that file's **line 3**, `**Status:** Accepted — accepted <date>`, becomes
//!      `**Status:** Implemented (<version>) — accepted <date>`, keeping its own date;
//!   3. its handoff's **line 3** governing-RFC link, `../../accepted/` -> `../../done/`;
//!   4. its handoff's **line 5**, `... (Accepted, <x>)`, becomes `... (Implemented, <version>)`;
//!   5. `rfcs/README.md`: the row moves from the Accepted table to the last Implemented table
//!      with `<version>` in its `Shipped` column, the Accepted heading becomes `— none` (with
//!      its fixed one-line text) when the folder empties, and the counts move.
//!
//! **The design is the refusals.** This runs once per milestone, so it is cold every time and
//! its author will not be the person running it.
//!
//! * **Compute, then apply** ([`plan_promotion`] is pure over an in-memory [`Tree`]). If any
//!   task cannot be derived for any RFC, *nothing* is written and *every* reason is reported.
//! * **It does not touch the git index** (no `git mv`, no staging): a plain filesystem rename,
//!   then the full list of changed paths, for the caller to stage explicitly. With nothing
//!   staged early there is no state in which the index shows a rename whose edits are missing.
//! * **It does not repair a half-moved tree.** A `Status:` already `Implemented`, a handoff link
//!   already `done/`, a row already in the Implemented table: refuse, and say what is
//!   inconsistent. Guessing intent is worse than a one-commit hand fix.
//! * Applying keeps an undo log: if a write fails after a rename, everything is put back.
//!
//! **What it does not write: prose.** An Implemented row's hand-written summary is the
//! architect's. The tool moves the row it finds and carries the Accepted row's `Tracks` cell
//! forward as a plainly mechanical `— tracks …` stub, which the output flags for replacement.
//! When the Accepted folder empties, the milestone narrative paragraph in that section is
//! replaced with the fixed "nothing accepted" text — as the 0.42.0 cut did by hand — and the
//! output says how many lines that removed.
//!
//! The five counts are validated before anything is planned, with the same parsing rules
//! `consistency-check`'s `doc-counts` uses (that subcheck lives in a binary and cannot be called
//! from here, so the rules are restated and a test pins them to the real README).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ACCEPTED_DIR: &str = "rfcs/accepted";
const DONE_DIR: &str = "rfcs/done";
const PROPOSED_DIR: &str = "rfcs/proposed";
const ARCHIVE_DIR: &str = "rfcs/archive";
const HANDOFFS_DIR: &str = "rfcs/handoffs";
const README_PATH: &str = "rfcs/README.md";
/// Folder-local index pages: in the folder, not RFCs (as `doc-counts` treats them).
const NON_RFC_FILES: &[&str] = &["README.md", "v0.7.x-index.md"];

/// The text that replaces the Accepted section's body when the folder empties.
const EMPTY_ACCEPTED_BODY: &[&str] = &[
    "",
    "Nothing is accepted and awaiting a cut. `accepted/README.md` keeps the folder in",
    "a fresh clone.",
    "",
];

// ── The in-memory tree the pure planner reads ────────────────────────────────

/// File counts as `doc-counts` derives them: `.md` files per folder, less the index pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counts {
    /// All `.md` files in `done/` (including `v0.7.x-index.md`).
    pub done_total: usize,
    pub accepted: usize,
    pub proposed: usize,
    pub archive: usize,
}

/// Everything the planner reads. Nothing here touches the disk.
#[derive(Debug, Clone)]
pub struct Tree {
    /// `(file name, text)` for every `.md` in `rfcs/accepted/` except the folder index.
    pub accepted: Vec<(String, String)>,
    /// Handoff directory name (the RFC file's stem) -> the text of its
    /// `implementation-handoff.md`, for those that exist.
    pub handoffs: BTreeMap<String, String>,
    /// File names already in `rfcs/done/`.
    pub done_names: BTreeSet<String>,
    pub readme: String,
    pub counts: Counts,
}

/// One content edit, at its final path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub path: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

/// A complete, validated promotion: derived entirely before any write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub version: String,
    pub renames: Vec<Rename>,
    /// Edits to apply **after** the renames, at the post-rename paths.
    pub edits: Vec<Edit>,
    /// Every path the command changes (old and new, so the caller can stage deletions too).
    pub changed_paths: Vec<String>,
    /// The report body — identical for `--dry-run` and the real run.
    pub report: Vec<String>,
}

// ── Argument handling ────────────────────────────────────────────────────────

/// `X.Y.Z` -> the milestone `X.Y` that RFC ids carry. Strict: three numeric components.
fn milestone_of(version: &str) -> Result<String, String> {
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if parts.len() != 3 || !parts.iter().all(numeric) {
        return Err(format!(
            "`{version}` is not a release version of the form X.Y.Z (e.g. 0.43.0); the \
            RFCs' `Implemented (<version>)` status and the README's `Shipped` column carry \
            the full release version"
        ));
    }
    Ok(format!("{}.{}", parts[0], parts[1]))
}

// ── Small text helpers (line-preserving) ─────────────────────────────────────

fn lines_keep(src: &str) -> Vec<String> {
    src.split_inclusive('\n').map(str::to_string).collect()
}

/// A line's content without its terminator, and the terminator.
fn split_eol(line: &str) -> (&str, &str) {
    match line.strip_suffix('\n') {
        Some(rest) => match rest.strip_suffix('\r') {
            Some(r) => (r, "\r\n"),
            None => (rest, "\n"),
        },
        None => (line, ""),
    }
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Parse a table row into exactly `n` trimmed cells.
fn row_cells(line: &str, n: usize) -> Option<Vec<String>> {
    let t = line.trim();
    let inner = t.strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<String> = inner.split('|').map(|c| c.trim().to_string()).collect();
    (cells.len() == n).then_some(cells)
}

fn leading_digits(s: &str) -> Option<usize> {
    let d: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    d.parse().ok()
}

// ── The five counts, validated before anything is planned ────────────────────

/// What the README claims. `None` means the line could not be found or parsed.
#[derive(Debug, PartialEq, Eq)]
struct Claims {
    done_header: Option<usize>,
    rfcs_plus: Option<usize>,
    accepted: Option<usize>,
    proposed: Option<usize>,
    archive: Option<usize>,
}

fn folder_header_count(src: &str, label: &str, folder: &str) -> Option<usize> {
    let prefix = format!("## {label} ({folder})");
    let line = src.lines().find(|l| l.starts_with(&prefix))?;
    let rest = line[line.rfind('—')? + '—'.len_utf8()..].trim();
    if rest.eq_ignore_ascii_case("none") {
        return Some(0);
    }
    leading_digits(rest)
}

fn read_claims(readme: &str) -> Claims {
    let done_header = readme
        .lines()
        .find(|l| l.starts_with("## Implemented (done/)"))
        .and_then(|l| {
            let rest = l[l.rfind('—')? + '—'.len_utf8()..].trim();
            leading_digits(rest)
        });
    let rfcs_plus = readme
        .lines()
        .find(|l| l.contains("RFCs plus"))
        .and_then(leading_digits);
    Claims {
        done_header,
        rfcs_plus,
        accepted: folder_header_count(readme, "Accepted", "accepted/"),
        proposed: folder_header_count(readme, "Proposed", "proposed/"),
        archive: folder_header_count(readme, "Archive", "archive/"),
    }
}

/// Every way the README's five counts disagree with the folders (empty = they agree).
fn count_disagreements(readme: &str, c: &Counts) -> Vec<String> {
    let claims = read_claims(readme);
    let mut out = Vec::new();
    let mut cmp = |name: &str, claimed: Option<usize>, actual: usize| match claimed {
        Some(n) if n == actual => {}
        Some(n) => out.push(format!("{name} claims {n}, the folder holds {actual}")),
        None => out.push(format!("{name} could not be found in rfcs/README.md")),
    };
    cmp(
        "`## Implemented (done/) — N files`",
        claims.done_header,
        c.done_total,
    );
    cmp(
        "\"N RFCs plus `v0.7.x-index.md`\"",
        claims.rfcs_plus,
        c.done_total.saturating_sub(1),
    );
    cmp("`## Accepted (accepted/)`", claims.accepted, c.accepted);
    cmp("`## Proposed (proposed/)`", claims.proposed, c.proposed);
    cmp("`## Archive (archive/)`", claims.archive, c.archive);
    out
}

// ── The README, located and edited ───────────────────────────────────────────

/// Index of the first line starting with `prefix`.
fn find_line(lines: &[String], prefix: &str) -> Option<usize> {
    lines.iter().position(|l| l.starts_with(prefix))
}

/// The closing rule of the section whose heading is at `head`: the **last** `---` line before
/// the next `## ` heading (or the end of the file). Not the first `---` after the heading: the
/// Implemented section holds milestone sub-sections separated by interior rules, and ending it
/// at the first one would put new rows in the wrong table.
fn section_rule(lines: &[String], head: usize) -> Option<usize> {
    let next_heading = (head + 1..lines.len())
        .find(|&i| lines[i].starts_with("## "))
        .unwrap_or(lines.len());
    (head + 1..next_heading)
        .rev()
        .find(|&i| split_eol(&lines[i]).0 == "---")
}

struct ReadmeEdit {
    text: String,
    /// Report lines describing what changed in the README.
    report: Vec<String>,
}

/// The README half of the promotion. `moved` is `(id, file name)` in ascending id order.
fn edit_readme(
    readme: &str,
    version: &str,
    moved: &[(String, String)],
    new_done_total: usize,
    accepted_remaining: usize,
) -> Result<ReadmeEdit, Vec<String>> {
    let mut why: Vec<String> = Vec::new();
    let mut lines = lines_keep(readme);
    let mut report: Vec<String> = Vec::new();

    // ── locate the two sections ──
    let acc_head = find_line(&lines, "## Accepted (accepted/)");
    let impl_head = find_line(&lines, "## Implemented (done/)");
    let (Some(acc_head), Some(impl_head)) = (acc_head, impl_head) else {
        if acc_head.is_none() {
            why.push("rfcs/README.md has no `## Accepted (accepted/)` heading".into());
        }
        if impl_head.is_none() {
            why.push("rfcs/README.md has no `## Implemented (done/)` heading".into());
        }
        return Err(why);
    };
    let Some(acc_end) = section_rule(&lines, acc_head) else {
        return Err(vec![
            "the Accepted section of rfcs/README.md is not closed by a `---` line".into(),
        ]);
    };
    let Some(impl_end) = section_rule(&lines, impl_head) else {
        return Err(vec![
            "the Implemented section of rfcs/README.md is not closed by a `---` line".into(),
        ]);
    };
    if impl_end >= acc_head {
        return Err(vec![
            "rfcs/README.md's Implemented section does not end before the Accepted section; \
            the layout this tool reads (Implemented, then Accepted) has changed"
                .into(),
        ]);
    }

    // ── find and take each moved row out of the Accepted table ──
    let mut taken: Vec<(String, Vec<String>)> = Vec::new(); // (id, [title cell, tracks cell])
    let mut drop_lines: BTreeSet<usize> = BTreeSet::new();
    for (id, file) in moved {
        let needle = format!("(./accepted/{file})");
        let hit = (acc_head..acc_end).find(|&i| {
            let t = lines[i].trim_start();
            t.starts_with('|') && t.contains(&needle)
        });
        let Some(i) = hit else {
            why.push(format!(
                "rfcs/README.md's Accepted table has no row for {id} (looking for a row linking \
                `./accepted/{file}`)"
            ));
            continue;
        };
        let Some(cells) = row_cells(split_eol(&lines[i]).0, 3) else {
            why.push(format!(
                "the Accepted-table row for {id} is not `| id | title | tracks |` (three cells): {:?}",
                split_eol(&lines[i]).0
            ));
            continue;
        };
        if cells[0] != *id {
            why.push(format!(
                "the Accepted-table row linking `{file}` is labelled `{}`, not `{id}`",
                cells[0]
            ));
            continue;
        }
        drop_lines.insert(i);
        taken.push((id.clone(), vec![cells[1].clone(), cells[2].clone()]));
    }

    // ── already listed as implemented? (a half-moved tree) ──
    for (id, file) in moved {
        let needle = format!("(./done/{file})");
        if (impl_head..impl_end).any(|i| lines[i].contains(&needle)) {
            why.push(format!(
                "{id} already has a row in the Implemented section (`./done/{file}`): the \
                tree is partly moved already; this tool does not repair that"
            ));
        }
    }

    // ── the Implemented table the rows go to: the LAST table in the section ──
    let mut last_run: Option<(usize, usize)> = None;
    let mut i = impl_head;
    while i < impl_end {
        if lines[i].trim_start().starts_with('|') {
            let start = i;
            while i < impl_end && lines[i].trim_start().starts_with('|') {
                i += 1;
            }
            last_run = Some((start, i)); // [start, i)
        } else {
            i += 1;
        }
    }
    let impl_table_end = match last_run {
        None => {
            why.push("rfcs/README.md's Implemented section holds no table".into());
            None
        }
        Some((start, end)) => {
            let header_ok = row_cells(split_eol(&lines[start]).0, 3)
                .map(|c| c[2] == "Shipped")
                .unwrap_or(false)
                && end - start >= 2
                && split_eol(&lines[start + 1])
                    .0
                    .trim_start()
                    .starts_with("|--");
            if header_ok {
                Some(end)
            } else {
                why.push(format!(
                    "the last table in the Implemented section does not have a `| … | … | \
                    Shipped |` header (found {:?}); the layout this tool reads has changed",
                    split_eol(&lines[start]).0
                ));
                None
            }
        }
    };
    if !why.is_empty() {
        return Err(why);
    }
    let impl_table_end = impl_table_end.unwrap();

    // ── build the new rows ──
    let mut new_rows: Vec<String> = Vec::new();
    for (id, _file) in moved {
        let (_, cells) = taken.iter().find(|(i, _)| i == id).unwrap();
        let title = cells[0].replace("(./accepted/", "(./done/");
        let tracks = &cells[1];
        let body = if tracks.is_empty() {
            title
        } else {
            format!("{title} — tracks {tracks}")
        };
        new_rows.push(format!("| {id} | {body} | {version} |\n"));
        report.push(format!(
            "  README: row {id} moved Accepted -> Implemented (Shipped = {version})"
        ));
    }
    report.push(
        "  NOTE: the moved Implemented rows are mechanical stubs (`— tracks <erratum>`); the \
        hand-written summary — `closed **E-nnn**; …` — is the architect's to write"
            .into(),
    );

    // ── apply to the line vector, bottom-up so indices stay valid ──
    // 1. the Accepted section (below the Implemented one, so edited first)
    let empties = accepted_remaining == 0;
    if empties {
        let removed = acc_end - (acc_head + 1);
        let repl: Vec<String> = EMPTY_ACCEPTED_BODY
            .iter()
            .map(|l| format!("{l}\n"))
            .collect();
        lines.splice(acc_head + 1..acc_end, repl);
        lines[acc_head] = "## Accepted (accepted/) — none\n".to_string();
        report.push(format!(
            "  README: Accepted heading -> `— none`; the section's {removed} body line(s) (the \
            milestone narrative, the table, the trailer) replaced by the fixed two-line text — \
            copy the narrative elsewhere first if it should be kept"
        ));
    } else {
        for &i in drop_lines.iter().rev() {
            lines.remove(i);
        }
        let n = accepted_remaining;
        lines[acc_head] = format!(
            "## Accepted (accepted/) — {n} RFC{}\n",
            if n == 1 { "" } else { "s" }
        );
        report.push(format!(
            "  README: Accepted heading -> `— {n} RFC{}`; the moved rows removed, the rest untouched",
            if n == 1 { "" } else { "s" }
        ));
    }
    // 2. the Implemented table: append after its last row
    for (k, row) in new_rows.iter().enumerate() {
        lines.insert(impl_table_end + k, row.clone());
    }
    // 3. the Implemented counts
    let head_old = split_eol(&lines[impl_head]).0.to_string();
    if !(head_old.starts_with("## Implemented (done/) — ") && head_old.ends_with(" files")) {
        return Err(vec![format!(
            "the Implemented heading is not `## Implemented (done/) — N files` (found {head_old:?})"
        )]);
    }
    lines[impl_head] = format!("## Implemented (done/) — {new_done_total} files\n");
    let plus = (impl_head..impl_end).find(|&i| lines[i].contains("RFCs plus"));
    let Some(plus) = plus else {
        return Err(vec![
            "the Implemented section has no \"N RFCs plus `v0.7.x-index.md`\" line".into(),
        ]);
    };
    let old = lines[plus].clone();
    let digits: String = old.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Err(vec![format!(
            "the \"RFCs plus\" line does not start with the count: {old:?}"
        )]);
    }
    lines[plus] = format!("{}{}", new_done_total - 1, &old[digits.len()..]);
    report.push(format!(
        "  README: counts -> `{new_done_total} files` and `{} RFCs plus …`",
        new_done_total - 1
    ));

    Ok(ReadmeEdit {
        text: lines.concat(),
        report,
    })
}

// ── The pure planner ─────────────────────────────────────────────────────────

/// Derive the whole promotion, or every reason it cannot be done. Pure: reads only `tree`.
pub fn plan_promotion(version: &str, tree: &Tree) -> Result<Plan, Vec<String>> {
    let milestone = milestone_of(version).map_err(|e| vec![e])?;
    let prefix = format!("RFC-{milestone}-");
    let mut why: Vec<String> = Vec::new();

    // The tree must already agree with itself: building on a tree `doc-counts` rejects would
    // bury that disagreement under this tool's own edit.
    for d in count_disagreements(&tree.readme, &tree.counts) {
        why.push(format!(
            "rfcs/README.md already disagrees with the folders before anything is moved — {d}. \
            Fix that first (`consistency-check doc-counts` names it); this tool will not build \
            on it"
        ));
    }

    // R3: which RFCs?
    let mut matching: Vec<&(String, String)> = tree
        .accepted
        .iter()
        .filter(|(name, _)| {
            name.starts_with(&prefix)
                && name.ends_with(".md")
                && name[prefix.len()..]
                    .split('-')
                    .next()
                    .is_some_and(|n| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    matching.sort_by(|a, b| a.0.cmp(&b.0));
    if matching.is_empty() {
        why.push(format!(
            "rfcs/accepted/ holds no RFC for milestone {milestone} (no file named \
            `{prefix}NNN-*.md`); nothing to promote"
        ));
        return Err(why);
    }

    let mut renames = Vec::new();
    let mut edits: Vec<Edit> = Vec::new();
    let mut report: Vec<String> = Vec::new();
    let mut moved: Vec<(String, String)> = Vec::new();

    for (name, text) in matching {
        let stem = name.trim_end_matches(".md");
        // "RFC-0.43-001-slug" -> "0.43-001" (the README's id column)
        let id = format!("{milestone}-{}", &name[prefix.len()..prefix.len() + 3]);
        let from = format!("{ACCEPTED_DIR}/{name}");
        let to = format!("{DONE_DIR}/{name}");

        if tree.done_names.contains(name) {
            why.push(format!(
                "{to} already exists: the move would overwrite it (a half-moved tree; not repaired)"
            ));
            continue;
        }

        // Task 2: line 3 of the RFC.
        let mut rfc_lines = lines_keep(text);
        let mut rfc_ok = true;
        match rfc_lines.get(2).map(|l| split_eol(l).0.to_string()) {
            None => {
                why.push(format!(
                    "{from} has fewer than 3 lines: no `Status:` line to read"
                ));
                rfc_ok = false;
            }
            Some(l3) => {
                let date = l3
                    .strip_prefix("**Status:** Accepted — accepted ")
                    .filter(|d| is_date(d));
                match date {
                    Some(d) => {
                        let new = format!("**Status:** Implemented ({version}) — accepted {d}");
                        let eol = split_eol(&rfc_lines[2]).1.to_string();
                        report.push(format!("  {name}\n    line 3: {l3}\n         -> {new}"));
                        rfc_lines[2] = format!("{new}{eol}");
                    }
                    None => {
                        why.push(format!(
                            "{from} line 3 is not `**Status:** Accepted — accepted YYYY-MM-DD` \
                            (found {l3:?}); only an Accepted RFC can be promoted. If it already \
                            reads Implemented, the tree is partly moved, and this tool does not \
                            repair that or guess"
                        ));
                        rfc_ok = false;
                    }
                }
            }
        }

        // Tasks 3 and 4: the handoff.
        let hand_path = format!("{HANDOFFS_DIR}/{stem}/implementation-handoff.md");
        let mut hand_edit: Option<(String, String)> = None;
        match tree.handoffs.get(stem) {
            None => why.push(format!(
                "no handoff for {name}: expected {hand_path} (a missing handoff directory or \
                file); every promoted RFC's handoff inherits its status"
            )),
            Some(htext) => {
                let mut hl = lines_keep(htext);
                let mut hand_ok = true;
                // line 3: the governing-RFC link
                let want_old = format!("(../../accepted/{name})");
                match hl.get(2).map(|l| split_eol(l).0.to_string()) {
                    Some(l3) if l3.starts_with("**Governing RFC:**") && l3.contains(&want_old) => {
                        let new = l3.replace(&want_old, &format!("(../../done/{name})"));
                        let eol = split_eol(&hl[2]).1.to_string();
                        report.push(format!("  {stem}/implementation-handoff.md\n    line 3: {l3}\n         -> {new}"));
                        hl[2] = format!("{new}{eol}");
                    }
                    other => {
                        why.push(format!(
                            "{hand_path} line 3 is not a `**Governing RFC:**` link to \
                            `../../accepted/{name}` (found {:?})",
                            other.unwrap_or_default()
                        ));
                        hand_ok = false;
                    }
                }
                // line 5: the inherited status
                match hl.get(4).map(|l| split_eol(l).0.to_string()) {
                    Some(l5)
                        if l5.starts_with(
                            "**Status:** inherited from the governing RFC (Accepted, ",
                        ) && l5.ends_with(')') =>
                    {
                        let head = "**Status:** inherited from the governing RFC (";
                        let new = format!("{head}Implemented, {version})");
                        let eol = split_eol(&hl[4]).1.to_string();
                        report.push(format!("    line 5: {l5}\n         -> {new}"));
                        hl[4] = format!("{new}{eol}");
                    }
                    other => {
                        why.push(format!(
                            "{hand_path} line 5 is not `**Status:** inherited from the governing \
                            RFC (Accepted, …)` (found {:?})",
                            other.unwrap_or_default()
                        ));
                        hand_ok = false;
                    }
                }
                if hand_ok {
                    hand_edit = Some((hand_path.clone(), hl.concat()));
                }
            }
        }

        if rfc_ok {
            renames.push(Rename {
                from: from.clone(),
                to: to.clone(),
            });
            edits.push(Edit {
                path: to.clone(),
                before: text.clone(),
                after: rfc_lines.concat(),
            });
            moved.push((id.clone(), name.clone()));
        }
        if let Some((path, after)) = hand_edit {
            let before = tree.handoffs.get(stem).cloned().unwrap_or_default();
            edits.push(Edit {
                path,
                before,
                after,
            });
        }
    }

    // Task 5: the README.
    let new_done_total = tree.counts.done_total + moved.len();
    let accepted_remaining = tree.counts.accepted.saturating_sub(moved.len());
    let readme_edit = if why.is_empty() {
        match edit_readme(
            &tree.readme,
            version,
            &moved,
            new_done_total,
            accepted_remaining,
        ) {
            Ok(e) => Some(e),
            Err(mut w) => {
                why.append(&mut w);
                None
            }
        }
    } else {
        // Still try, so a refusal lists the README's problems alongside the others.
        match edit_readme(
            &tree.readme,
            version,
            &moved,
            new_done_total,
            accepted_remaining,
        ) {
            Ok(_) => None,
            Err(mut w) => {
                why.append(&mut w);
                None
            }
        }
    };

    if !why.is_empty() {
        return Err(why);
    }
    let readme_edit = readme_edit.expect("no refusals, so the README edit succeeded");
    report.extend(readme_edit.report);
    edits.push(Edit {
        path: README_PATH.to_string(),
        before: tree.readme.clone(),
        after: readme_edit.text,
    });

    let mut changed_paths: Vec<String> = Vec::new();
    for r in &renames {
        changed_paths.push(r.from.clone());
        changed_paths.push(r.to.clone());
    }
    for e in &edits {
        if !changed_paths.contains(&e.path) {
            changed_paths.push(e.path.clone());
        }
    }
    changed_paths.sort();

    Ok(Plan {
        version: version.to_string(),
        renames,
        edits,
        changed_paths,
        report,
    })
}

// ── Applying, with an undo log ───────────────────────────────────────────────

enum Undo {
    Rename { from: String, to: String },
    Write { path: String, before: String },
}

/// Apply `plan` to the filesystem rooted at the current directory. On any error everything
/// done so far is put back and the error is returned: all-or-nothing on disk too.
pub fn apply(plan: &Plan) -> Result<(), String> {
    apply_in(Path::new("."), plan)
}

/// [`apply`] with the plan's relative paths resolved under `root` (so tests can use a temp
/// directory without touching the process-wide working directory).
pub fn apply_in(root: &Path, plan: &Plan) -> Result<(), String> {
    let at = |p: &str| root.join(p);
    let mut undo: Vec<Undo> = Vec::new();
    let result = (|| -> Result<(), String> {
        for r in &plan.renames {
            fs::rename(at(&r.from), at(&r.to))
                .map_err(|e| format!("cannot rename {} -> {}: {e}", r.from, r.to))?;
            undo.push(Undo::Rename {
                from: r.to.clone(),
                to: r.from.clone(),
            });
        }
        for e in &plan.edits {
            fs::write(at(&e.path), e.after.as_bytes())
                .map_err(|err| format!("cannot write {}: {err}", e.path))?;
            undo.push(Undo::Write {
                path: e.path.clone(),
                before: e.before.clone(),
            });
        }
        Ok(())
    })();
    if let Err(first) = result {
        let mut failures = Vec::new();
        for u in undo.into_iter().rev() {
            let r = match u {
                Undo::Rename { from, to } => fs::rename(at(&from), at(&to))
                    .map_err(|e| format!("cannot restore {from} -> {to}: {e}")),
                Undo::Write { path, before } => fs::write(at(&path), before.as_bytes())
                    .map_err(|e| format!("cannot restore {path}: {e}")),
            };
            if let Err(f) = r {
                failures.push(f);
            }
        }
        return Err(if failures.is_empty() {
            format!("{first} — everything already done was put back; the tree is unchanged")
        } else {
            format!(
                "{first} — and rolling back FAILED ({}): inspect `git status` before doing \
                anything else",
                failures.join("; ")
            )
        });
    }
    Ok(())
}

// ── Loading the real tree ────────────────────────────────────────────────────

fn md_files(dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let rd = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if e.path().extension().is_some_and(|x| x == "md") && e.path().is_file() {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

fn rfc_count(dir: &Path) -> Result<usize, String> {
    let files = md_files(dir)?;
    let non = NON_RFC_FILES
        .iter()
        .filter(|f| dir.join(f).is_file())
        .count();
    Ok(files.len().saturating_sub(non))
}

/// Read everything the planner needs for `milestone` from the current directory.
pub fn load_tree(milestone: &str) -> Result<Tree, String> {
    load_tree_in(Path::new("."), milestone)
}

/// [`load_tree`] rooted at `root`.
pub fn load_tree_in(root: &Path, milestone: &str) -> Result<Tree, String> {
    let readme = fs::read_to_string(root.join(README_PATH))
        .map_err(|e| format!("cannot read {README_PATH}: {e}"))?;
    let mut accepted = Vec::new();
    for name in md_files(&root.join(ACCEPTED_DIR))? {
        if NON_RFC_FILES.contains(&name.as_str()) {
            continue;
        }
        let text = fs::read_to_string(root.join(ACCEPTED_DIR).join(&name))
            .map_err(|e| format!("cannot read {ACCEPTED_DIR}/{name}: {e}"))?;
        accepted.push((name, text));
    }
    let prefix = format!("RFC-{milestone}-");
    let mut handoffs = BTreeMap::new();
    for (name, _) in accepted.iter().filter(|(n, _)| n.starts_with(&prefix)) {
        let stem = name.trim_end_matches(".md").to_string();
        let p = root
            .join(HANDOFFS_DIR)
            .join(&stem)
            .join("implementation-handoff.md");
        if let Ok(t) = fs::read_to_string(&p) {
            handoffs.insert(stem, t);
        }
    }
    let done_files = md_files(&root.join(DONE_DIR))?;
    let counts = Counts {
        done_total: done_files.len(),
        accepted: rfc_count(&root.join(ACCEPTED_DIR))?,
        proposed: rfc_count(&root.join(PROPOSED_DIR))?,
        archive: rfc_count(&root.join(ARCHIVE_DIR))?,
    };
    Ok(Tree {
        accepted,
        handoffs,
        done_names: done_files.into_iter().collect(),
        readme,
        counts,
    })
}

// ── Command ──────────────────────────────────────────────────────────────────

fn usage() -> ExitCode {
    eprintln!(
        "usage: cargo xtask rfc promote <version> [--dry-run]\n\
         \n\
         <version> is the release version, e.g. 0.43.0: it matches RFC-0.43-NNN in rfcs/accepted/\n\
         and is written into each promoted RFC's `Implemented (<version>)` status and the README's\n\
         `Shipped` column. --dry-run prints exactly what would change and changes nothing."
    );
    ExitCode::FAILURE
}

/// `rfc <subcommand> …`
pub fn cmd_rfc(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("promote") => cmd_promote(&args[1..]),
        _ => usage(),
    }
}

/// The report, shared by `--dry-run` and the real run so a reader comparing them has nothing
/// to guess: only the first and last lines differ.
fn render(plan: &Plan, dry_run: bool) -> String {
    let mut out = String::new();
    if dry_run {
        out.push_str(&format!(
            "rfc promote {} — DRY RUN: this is what would change; nothing has been written\n",
            plan.version
        ));
    } else {
        out.push_str(&format!("rfc promote {} — changed:\n", plan.version));
    }
    for r in &plan.renames {
        out.push_str(&format!("  move  {} -> {}\n", r.from, r.to));
    }
    for l in &plan.report {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(
        "Changed paths (stage these, explicitly — this command does not touch the index):\n",
    );
    for p in &plan.changed_paths {
        out.push_str(&format!("  {p}\n"));
    }
    out.push_str(&format!("  git add -- {}\n", plan.changed_paths.join(" ")));
    if dry_run {
        out.push_str("DRY RUN: no file was written.\n");
    } else {
        out.push_str(
            "Next: stage the paths above, then run `cargo xtask consistency-check --all`.\n",
        );
    }
    out
}

fn cmd_promote(args: &[String]) -> ExitCode {
    let mut dry_run = false;
    let mut version: Option<&str> = None;
    for a in args {
        if a == "--dry-run" {
            dry_run = true;
        } else if a.starts_with("--") {
            eprintln!("rfc promote: unknown flag `{a}`");
            return usage();
        } else if version.replace(a).is_some() {
            eprintln!("rfc promote: more than one version given");
            return usage();
        }
    }
    let Some(version) = version else {
        return usage();
    };
    let milestone = match milestone_of(version) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("rfc promote: REFUSED — {e}");
            return ExitCode::FAILURE;
        }
    };
    let tree = match load_tree(&milestone) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("rfc promote: REFUSED — {e}");
            return ExitCode::FAILURE;
        }
    };
    let plan = match plan_promotion(version, &tree) {
        Ok(p) => p,
        Err(reasons) => {
            eprintln!(
                "rfc promote: REFUSED — nothing was changed. {} reason(s):",
                reasons.len()
            );
            for r in &reasons {
                eprintln!("  - {r}");
            }
            return ExitCode::FAILURE;
        }
    };
    if dry_run {
        print!("{}", render(&plan, true));
        return ExitCode::SUCCESS;
    }
    match apply(&plan) {
        Ok(()) => {
            print!("{}", render(&plan, false));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rfc promote: FAILED — {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = "0.43.0";

    fn rfc(id: &str, date: &str) -> (String, String) {
        let name = format!("RFC-0.43-{id}-thing.md");
        let text = format!(
            "# RFC-0.43-{id}: A thing\n\n**Status:** Accepted — accepted {date}\n**Milestone:** 0.43\nbody\n"
        );
        (name, text)
    }

    fn handoff(id: &str) -> (String, String) {
        let stem = format!("RFC-0.43-{id}-thing");
        let text = format!(
            "# Developer Handoff — RFC-0.43-{id}\n\n**Governing RFC:** [RFC-0.43-{id}](../../accepted/{stem}.md)\n**Milestone:** 0.43\n**Status:** inherited from the governing RFC (Accepted, 0.43)\n**Audience:** x\n"
        );
        (stem, text)
    }

    /// A README shaped like the real one: an Implemented section with interior `---` rules and
    /// two tables, an Accepted section, then the others.
    fn readme(accepted_rows: &[(&str, &str)], done: usize, accepted_n: Option<usize>) -> String {
        let mut s = String::new();
        s.push_str("# Index\n\n---\n\n");
        s.push_str(&format!("## Implemented (done/) — {done} files\n\n"));
        s.push_str(&format!(
            "{} RFCs plus `v0.7.x-index.md`, an overview page.\n\n",
            done - 1
        ));
        s.push_str("### v0.1.0 — old\n\n| ID | Title | Shipped |\n|----|-------|---------|\n| 001 | [Old](./done/001-old.md) | v0.1.0 |\n\n---\n\n");
        s.push_str("### 0.42 — last\n\n| RFC | Title | Shipped |\n|-----|-------|---------|\n| 0.42-001 | [Prev](./done/RFC-0.42-001-prev.md) — closed **E-1**; prose | 0.42.0 |\n\n---\n\n");
        match accepted_n {
            Some(n) => s.push_str(&format!("## Accepted (accepted/) — {n} RFCs\n\n0.43's line opens. Narrative paragraph.\n\n| RFC | Title | Tracks |\n|-----|-------|--------|\n")),
            None => s.push_str("## Accepted (accepted/) — none\n\n"),
        }
        for (id, tracks) in accepted_rows {
            s.push_str(&format!(
                "| 0.43-{id} | [A thing](./accepted/RFC-0.43-{id}-thing.md) | {tracks} |\n"
            ));
        }
        s.push_str("\n`accepted/README.md` keeps the folder in a fresh clone.\n\n---\n\n## Proposed (proposed/) — none\n\n---\n\n## Archive (archive/) — 2 RFCs\n");
        s
    }

    fn tree(ids: &[&str]) -> Tree {
        let rows: Vec<(&str, &str)> = ids.iter().map(|i| (*i, "E-9")).collect();
        let accepted: Vec<(String, String)> = ids.iter().map(|i| rfc(i, "2026-11-12")).collect();
        let handoffs: BTreeMap<String, String> = ids.iter().map(|i| handoff(i)).collect();
        Tree {
            accepted,
            handoffs,
            done_names: BTreeSet::new(),
            readme: readme(&rows, 10, Some(ids.len())),
            counts: Counts {
                done_total: 10,
                accepted: ids.len(),
                proposed: 0,
                archive: 2,
            },
        }
    }

    fn refusals(t: &Tree) -> Vec<String> {
        plan_promotion(V, t).expect_err("must refuse")
    }

    #[test]
    fn milestone_comes_from_a_three_part_version() {
        assert_eq!(milestone_of("0.43.0").unwrap(), "0.43");
        assert_eq!(milestone_of("1.2.10").unwrap(), "1.2");
        for bad in ["0.43", "0.43.0.1", "v0.43.0", "0.43.x", "", "0..0"] {
            assert!(milestone_of(bad).is_err(), "{bad}");
        }
    }

    /// The five tasks, exactly as the procedure lists them.
    #[test]
    fn happy_path_performs_all_five_tasks() {
        let t = tree(&["001", "002"]);
        let p = plan_promotion(V, &t).expect("plans");
        assert_eq!(p.renames.len(), 2);
        assert_eq!(p.renames[0].from, "rfcs/accepted/RFC-0.43-001-thing.md");
        assert_eq!(p.renames[0].to, "rfcs/done/RFC-0.43-001-thing.md");
        let rfc_edit = p
            .edits
            .iter()
            .find(|e| e.path == "rfcs/done/RFC-0.43-001-thing.md")
            .unwrap();
        // task 2: line 3 only, keeping its own date
        assert!(
            rfc_edit
                .after
                .contains("**Status:** Implemented (0.43.0) — accepted 2026-11-12\n")
        );
        assert_eq!(
            rfc_edit.before.lines().count(),
            rfc_edit.after.lines().count()
        );
        let h = p
            .edits
            .iter()
            .find(|e| {
                e.path
                    .ends_with("RFC-0.43-001-thing/implementation-handoff.md")
            })
            .unwrap();
        // tasks 3 and 4
        assert!(h.after.contains("(../../done/RFC-0.43-001-thing.md)"));
        assert!(!h.after.contains("../../accepted/"));
        assert!(h.after.contains("(Implemented, 0.43.0)"));
        let r = p.edits.iter().find(|e| e.path == "rfcs/README.md").unwrap();
        // task 5: counts, heading, rows
        assert!(r.after.contains("## Implemented (done/) — 12 files"));
        assert!(r.after.contains("\n11 RFCs plus `v0.7.x-index.md`"));
        assert!(r.after.contains("## Accepted (accepted/) — none"));
        assert!(r.after.contains("Nothing is accepted and awaiting a cut."));
        assert!(
            !r.after.contains("Narrative paragraph"),
            "emptied section's body is replaced"
        );
        assert!(!r.after.contains("./accepted/RFC-0.43"));
        assert!(r.after.contains(
            "| 0.43-001 | [A thing](./done/RFC-0.43-001-thing.md) — tracks E-9 | 0.43.0 |"
        ));
        // changed paths: old AND new, so the deletion can be staged
        assert!(
            p.changed_paths
                .contains(&"rfcs/accepted/RFC-0.43-001-thing.md".to_string())
        );
        assert!(
            p.changed_paths
                .contains(&"rfcs/done/RFC-0.43-001-thing.md".to_string())
        );
        assert!(p.changed_paths.contains(&"rfcs/README.md".to_string()));
    }

    /// Rows land in the LAST Implemented table, even though the section has interior `---`
    /// rules (the first version of this tool ended the section at the first one).
    #[test]
    fn rows_go_to_the_last_table_past_interior_rules() {
        let t = tree(&["001"]);
        let p = plan_promotion(V, &t).unwrap();
        let after = &p
            .edits
            .iter()
            .find(|e| e.path == "rfcs/README.md")
            .unwrap()
            .after;
        let new_row = after.find("| 0.43-001 | [A thing](./done/").unwrap();
        let prev_row = after.find("| 0.42-001 |").unwrap();
        let old_table = after.find("| 001 | [Old]").unwrap();
        assert!(old_table < prev_row && prev_row < new_row);
        assert!(
            after[prev_row..new_row].lines().count() <= 3,
            "right after the 0.42 table's last row"
        );
    }

    fn rfc_other_milestone() -> (String, String) {
        (
            "RFC-0.44-001-thing.md".to_string(),
            "# x\n\n**Status:** Accepted — accepted 2026-11-12\n".to_string(),
        )
    }

    #[test]
    fn a_partial_promotion_keeps_the_section_and_its_other_rows() {
        let mut t = tree(&["001", "002"]);
        t.accepted.push(rfc_other_milestone());
        t.counts.accepted = 3;
        t.readme = readme(&[("001", "E-1"), ("002", "E-2")], 10, Some(3));
        // a 0.44 row stays in the Accepted table
        t.readme = t.readme.replace(
            "\n`accepted/README.md` keeps",
            "| 0.44-001 | [Other](./accepted/RFC-0.44-001-thing.md) | E-7 |\n\n`accepted/README.md` keeps",
        );
        let p = plan_promotion(V, &t).unwrap();
        let after = &p
            .edits
            .iter()
            .find(|e| e.path == "rfcs/README.md")
            .unwrap()
            .after;
        assert!(after.contains("## Accepted (accepted/) — 1 RFC\n"));
        assert!(
            after.contains("Narrative paragraph"),
            "prose untouched while the folder is not empty"
        );
        assert!(after.contains("| 0.44-001 | [Other](./accepted/RFC-0.44-001-thing.md)"));
    }

    // ── R3: each refusal ────────────────────────────────────────────────────────

    #[test]
    fn refuses_when_no_rfc_matches() {
        let t = tree(&["001"]);
        let e = plan_promotion("0.99.0", &t).unwrap_err();
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("no RFC for milestone 0.99"));
    }

    #[test]
    fn refuses_a_malformed_status_line_and_names_it() {
        for bad in [
            "**Status:** Accepted - accepted 2026-11-12", // hyphen
            "**Status:** Accepted — accepted 2026-1-12",  // not a date
            "**Status:** Accepted — accepted 2026-11-12 (note)",
            "Status: Accepted — accepted 2026-11-12",
            "**Status:** Proposed — accepted 2026-11-12",
            "**Status:** Implemented (0.43.0) — accepted 2026-11-12",
        ] {
            let mut t = tree(&["001"]);
            t.accepted[0].1 = t.accepted[0]
                .1
                .replace("**Status:** Accepted — accepted 2026-11-12", bad);
            let e = refusals(&t);
            assert!(
                e.iter().any(|r| r.contains("line 3 is not")),
                "{bad}: {e:?}"
            );
        }
    }

    #[test]
    fn refuses_a_missing_handoff() {
        let mut t = tree(&["001", "002"]);
        t.handoffs.remove("RFC-0.43-002-thing");
        let e = refusals(&t);
        assert!(
            e.iter()
                .any(|r| r.contains("no handoff for RFC-0.43-002-thing.md")),
            "{e:?}"
        );
    }

    #[test]
    fn refuses_a_handoff_whose_lines_are_not_the_expected_shape() {
        let mut t = tree(&["001"]);
        let h = t.handoffs.get_mut("RFC-0.43-001-thing").unwrap();
        *h = h.replace("(Accepted, 0.43)", "(Proposed, 0.43)");
        assert!(refusals(&t).iter().any(|r| r.contains("line 5 is not")));
        let mut t = tree(&["001"]);
        let h = t.handoffs.get_mut("RFC-0.43-001-thing").unwrap();
        *h = h.replace("../../accepted/", "../../done/"); // already repointed: half-moved
        assert!(
            refusals(&t)
                .iter()
                .any(|r| r.contains("line 3 is not a `**Governing RFC:**` link"))
        );
    }

    #[test]
    fn refuses_a_missing_accepted_row() {
        let mut t = tree(&["001", "002"]);
        t.readme = t
            .readme
            .lines()
            .filter(|l| !l.starts_with("| 0.43-002 |"))
            .map(|l| format!("{l}\n"))
            .collect();
        let e = refusals(&t);
        assert!(e.iter().any(|r| r.contains("no row for 0.43-002")), "{e:?}");
    }

    /// A row found by its link but labelled with a different id is refused too.
    #[test]
    fn refuses_a_mislabelled_accepted_row() {
        let mut t = tree(&["001", "002"]);
        t.readme = t.readme.replace("| 0.43-002 |", "| 0.43-999 |");
        let e = refusals(&t);
        assert!(
            e.iter()
                .any(|r| r.contains("is labelled `0.43-999`, not `0.43-002`")),
            "{e:?}"
        );
    }

    #[test]
    fn refuses_when_the_implemented_table_cannot_be_located() {
        let mut t = tree(&["001"]);
        t.readme = t.readme.replace("| Shipped |", "| When |");
        assert!(
            refusals(&t)
                .iter()
                .any(|r| r.contains("does not have a `| … | … | Shipped |` header"))
        );
        let mut t = tree(&["001"]);
        t.readme = t
            .readme
            .replace("## Implemented (done/)", "## Done (done/)");
        assert!(
            refusals(&t)
                .iter()
                .any(|r| r.contains("no `## Implemented (done/)` heading"))
        );
    }

    #[test]
    fn refuses_when_the_counts_already_disagree() {
        let mut t = tree(&["001"]);
        t.counts.done_total = 11; // README says 10
        let e = refusals(&t);
        assert!(
            e.iter()
                .any(|r| r.contains("already disagrees") && r.contains("claims 10")),
            "{e:?}"
        );
        let mut t = tree(&["001"]);
        t.counts.archive = 5; // README says 2
        assert!(
            refusals(&t)
                .iter()
                .any(|r| r.contains("`## Archive (archive/)`"))
        );
        let mut t = tree(&["001"]);
        t.readme = t.readme.replace("RFCs plus", "RFCs and");
        assert!(
            refusals(&t)
                .iter()
                .any(|r| r.contains("could not be found"))
        );
    }

    #[test]
    fn refuses_to_overwrite_and_refuses_a_row_already_implemented() {
        let mut t = tree(&["001"]);
        t.done_names.insert("RFC-0.43-001-thing.md".to_string());
        assert!(refusals(&t).iter().any(|r| r.contains("already exists")));
        let mut t = tree(&["001"]);
        t.readme = t.readme.replace(
            "| 0.42-001 | [Prev]",
            "| 0.43-001 | [dup](./done/RFC-0.43-001-thing.md) | 0.43.0 |\n| 0.42-001 | [Prev]",
        );
        assert!(
            refusals(&t)
                .iter()
                .any(|r| r.contains("already has a row in the Implemented section"))
        );
    }

    /// R2: one bad RFC of three refuses the whole set, and the plan carries nothing to apply.
    #[test]
    fn one_bad_rfc_refuses_all_of_them() {
        let mut t = tree(&["001", "002", "003"]);
        t.accepted[1].1 = t.accepted[1].1.replace("Accepted —", "Proposed —");
        let e = refusals(&t);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("RFC-0.43-002-thing.md"));
    }

    #[test]
    fn every_reason_is_reported_not_only_the_first() {
        let mut t = tree(&["001", "002", "003"]);
        t.accepted[0].1 = t.accepted[0].1.replace("Accepted —", "Proposed —");
        t.handoffs.remove("RFC-0.43-002-thing");
        t.readme = t.readme.replace("| 0.43-003 |", "| 0.43-777 |");
        assert_eq!(refusals(&t).len(), 3);
    }

    /// `--dry-run` and the real run print the same body: only the first and last lines differ.
    #[test]
    fn dry_run_and_real_output_share_a_body() {
        let p = plan_promotion(V, &tree(&["001", "002"])).unwrap();
        let dry = render(&p, true);
        let real = render(&p, false);
        let body = |s: &str| {
            let l: Vec<&str> = s.lines().collect();
            l[1..l.len() - 1].join("\n")
        };
        assert_eq!(body(&dry), body(&real));
        assert!(dry.contains("DRY RUN") && !real.contains("DRY RUN"));
        // and it prints the full path list a caller stages
        for path in &p.changed_paths {
            assert!(real.contains(path), "{path}");
        }
    }

    // ── applying, on a real temp directory ──────────────────────────────────────

    fn write_tree(root: &Path, t: &Tree) {
        for d in [
            ACCEPTED_DIR,
            DONE_DIR,
            PROPOSED_DIR,
            ARCHIVE_DIR,
            HANDOFFS_DIR,
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::write(root.join(README_PATH), &t.readme).unwrap();
        for (n, text) in &t.accepted {
            fs::write(root.join(ACCEPTED_DIR).join(n), text).unwrap();
        }
        for (stem, text) in &t.handoffs {
            let d = root.join(HANDOFFS_DIR).join(stem);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("implementation-handoff.md"), text).unwrap();
        }
        // fill done/ so its count matches the README's 10
        for i in 0..10 {
            fs::write(root.join(DONE_DIR).join(format!("d{i}.md")), "x").unwrap();
        }
        for i in 0..2 {
            fs::write(root.join(ARCHIVE_DIR).join(format!("a{i}.md")), "x").unwrap();
        }
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("fjell-rfcpromote-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn dump(root: &Path) -> BTreeMap<String, String> {
        fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, String>) {
            for e in fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.insert(
                        p.strip_prefix(root).unwrap().display().to_string(),
                        fs::read_to_string(&p).unwrap(),
                    );
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn load_plan_apply_round_trip_on_disk() {
        let root = scratch("round");
        write_tree(&root, &tree(&["001", "002"]));
        let loaded = load_tree_in(&root, "0.43").unwrap();
        assert!(count_disagreements(&loaded.readme, &loaded.counts).is_empty());
        let plan = plan_promotion(V, &loaded).unwrap();
        apply_in(&root, &plan).unwrap();
        let after = dump(&root);
        assert!(after.contains_key("rfcs/done/RFC-0.43-001-thing.md"));
        assert!(!after.contains_key("rfcs/accepted/RFC-0.43-001-thing.md"));
        assert!(after["rfcs/done/RFC-0.43-002-thing.md"].contains("Implemented (0.43.0)"));
        // and the result is itself consistent: the counts agree with the moved folders
        let again = load_tree_in(&root, "0.43").unwrap();
        assert!(
            count_disagreements(&again.readme, &again.counts).is_empty(),
            "{:?}",
            count_disagreements(&again.readme, &again.counts)
        );
        // promoting twice finds nothing to promote
        assert!(plan_promotion(V, &again).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    /// A write that fails after the renames puts everything back. (Read-only target file.)
    #[test]
    fn a_failed_write_rolls_back_every_rename_and_edit() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("rollback");
        write_tree(&root, &tree(&["001", "002"]));
        let before = dump(&root);
        let plan = plan_promotion(V, &load_tree_in(&root, "0.43").unwrap()).unwrap();
        let ro = root.join("rfcs/handoffs/RFC-0.43-002-thing/implementation-handoff.md");
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o444)).unwrap();
        // running as root would bypass the permission bit; then this test proves nothing
        if fs::OpenOptions::new().write(true).open(&ro).is_ok() {
            let _ = fs::remove_dir_all(&root);
            return;
        }
        let err = apply_in(&root, &plan).unwrap_err();
        assert!(err.contains("put back"), "{err}");
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(dump(&root), before, "the tree must be exactly as it was");
        let _ = fs::remove_dir_all(&root);
    }

    // ── pinned to the real README ───────────────────────────────────────────────

    /// The count rules restated here must read the real README the way `doc-counts` does:
    /// the real tree's five counts agree with its folders under THIS code too.
    #[test]
    fn the_real_readme_agrees_with_the_real_folders() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap();
        let t = load_tree_in(root, "0.43").expect("real tree loads");
        assert!(
            count_disagreements(&t.readme, &t.counts).is_empty(),
            "{:?}",
            count_disagreements(&t.readme, &t.counts)
        );
    }

    #[test]
    fn section_rule_is_the_last_rule_before_the_next_heading() {
        let lines = lines_keep("## A\n\ntext\n\n---\n\nmore\n\n---\n\n## B\n");
        assert_eq!(section_rule(&lines, 0), Some(8));
        assert_eq!(section_rule(&lines_keep("## A\ntext\n## B\n"), 0), None);
    }
}
