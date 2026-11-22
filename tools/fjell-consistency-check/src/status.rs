//! Shared status-keyword parsing for `rfc-status-folder`, `handoff-status`, `rfc-evidence` and
//! `rfc-policy` (Slice 4, RFC-v0.22-001; **RFC-0.47-002: the vocabulary comes from the policy**).
//!
//! RFC 000 (`rfcs/done/000-rfc-lifecycle-policy.md`) defines the lifecycle states in a table whose
//! columns are `State | Folder | Meaning`. **That table is the only source of the vocabulary**: this
//! module parses it, and nothing here lists a status word. Until RFC-0.47-002 the words were a Rust
//! constant (`STATUS_KEYWORDS`, plus a second one in `rfc_status_folder`) while this very doc said RFC 000
//! "names the fixed set" -- which it did not, for four of the seven words -- so a word was a legal status
//! because code said so, which is the dependency pointing the wrong way (E-128).

use std::collections::HashSet;
use std::sync::OnceLock;

/// Where the policy lives, relative to the repository root.
pub const POLICY_PATH: &str = "rfcs/done/000-rfc-lifecycle-policy.md";

/// One defined lifecycle state and the folder an RFC in it lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    /// `proposed`, `accepted` or `done`.
    pub folder: String,
}

/// The states the policy defines, in table order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    pub states: Vec<State>,
}

const FOLDERS: &[&str] = &["proposed", "accepted", "done"];

/// Parse the `## Lifecycle states` table out of RFC 000's text.
///
/// Fails -- never returns an empty vocabulary -- when the section, the table, either required column, or any
/// row is missing or malformed, when a name repeats, or when a folder is not one of the three the layout has:
/// a check that reads "no states" as "nothing to check" would pass on a policy with its table deleted.
pub fn parse_policy(src: &str) -> Result<Vocabulary, String> {
    let mut in_section = false;
    let mut header: Option<Vec<String>> = None;
    let mut states: Vec<State> = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        if t == "## Lifecycle states" {
            in_section = true;
            continue;
        }
        if in_section && t.starts_with("## ") {
            break;
        }
        if !in_section || !t.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = t
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cells
            .iter()
            .all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
        {
            continue; // the |---|---| separator
        }
        match &header {
            None => {
                if cells.len() < 3 || cells[0] != "State" || cells[1] != "Folder" {
                    return Err(format!(
                        "{POLICY_PATH}: the `## Lifecycle states` table must start `| State | Folder | Meaning |`, found `{t}`"
                    ));
                }
                header = Some(cells);
            }
            Some(_) => {
                if cells.len() < 3 {
                    return Err(format!("{POLICY_PATH}: malformed state row `{t}`"));
                }
                let name = cells[0].trim_matches('*').trim().to_string();
                let folder = cells[1]
                    .trim_matches('`')
                    .trim_end_matches('/')
                    .trim()
                    .to_string();
                if name.is_empty() || name.contains(' ') {
                    return Err(format!(
                        "{POLICY_PATH}: state name `{}` is not one word",
                        cells[0]
                    ));
                }
                if !FOLDERS.contains(&folder.as_str()) {
                    return Err(format!(
                        "{POLICY_PATH}: state `{name}` names folder `{folder}`, which is not one of {FOLDERS:?}"
                    ));
                }
                states.push(State { name, folder });
            }
        }
    }
    if header.is_none() || states.is_empty() {
        return Err(format!(
            "{POLICY_PATH}: no `## Lifecycle states` table with a `State | Folder` header and at least one row"
        ));
    }
    let mut seen = HashSet::new();
    for s in &states {
        if !seen.insert(s.name.as_str()) {
            return Err(format!(
                "{POLICY_PATH}: state `{}` is defined twice",
                s.name
            ));
        }
    }
    Ok(Vocabulary { states })
}

impl Vocabulary {
    /// State names an RFC may carry in `folder` (`proposed`, `accepted`, `done`).
    pub fn names_in(&self, folder: &str) -> Vec<&str> {
        self.states
            .iter()
            .filter(|s| s.folder == folder)
            .map(|s| s.name.as_str())
            .collect()
    }

    /// The first state name that appears in a Status line, **leftmost wins, longest wins a tie** -- so
    /// `Implemented-with-Errata` is found rather than its `Implemented` prefix, and a status that merely
    /// *mentions* another state later in the line (`Proposed -- ... Withdrawn ...`) is the state it starts with.
    pub fn keyword_in<'v>(&'v self, line: &str) -> Option<&'v str> {
        let after = match line
            .find("**Status:**")
            .or_else(|| line.find("**Status.**"))
        {
            Some(i) => &line[i + "**Status:**".len()..],
            None => line,
        };
        self.states
            .iter()
            .filter_map(|s| {
                // A state word must stand alone: `Implemented-with-Errata` is not `Implemented` followed by
                // more text, and with that state deleted from the table it must read as *undefined* rather
                // than silently as its prefix.
                after
                    .match_indices(s.name.as_str())
                    .find(|(i, m)| {
                        let before = after[..*i].chars().next_back();
                        let next = after[i + m.len()..].chars().next();
                        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
                            && !next.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
                    })
                    .map(|(i, _)| (i, s.name.as_str()))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(b.1.len().cmp(&a.1.len())))
            .map(|(_, n)| n)
    }
}

/// The first line carrying a bold `Status:`/`Status.` field label, with or without a heading marker.
/// `None` means the file has **no Status field at all** -- the documented exception for the policy document
/// itself and the v0.7.x patch-set index -- which is a different fact from a Status field naming no defined
/// state, and callers must not conflate them (`rfc-status-folder` used to, and skipped both).
pub fn status_field_line(src: &str) -> Option<&str> {
    src.lines()
        .find(|l| l.contains("**Status:**") || l.contains("**Status.**"))
}

/// The status keyword of `src` against an explicit vocabulary. Pure; this is what the tests and `rfc-policy`
/// use.
pub fn extract_status_keyword_in<'v>(src: &str, vocab: &'v Vocabulary) -> Option<&'v str> {
    vocab.keyword_in(status_field_line(src)?)
}

static VOCAB: OnceLock<Result<Vocabulary, String>> = OnceLock::new();

/// Where to read the policy from: the repository-relative path (every real run is from the repository
/// root), else the path relative to this crate's manifest -- which is where `cargo test` runs from, in this
/// crate's library tests and its binary's alike.
fn policy_file() -> std::path::PathBuf {
    let rel = std::path::PathBuf::from(POLICY_PATH);
    if rel.exists() {
        return rel;
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(POLICY_PATH)
}

/// The project's vocabulary, read once from RFC 000. An `Err` is a **failure for the caller to report**
/// (every check that needs the vocabulary starts by asking for it), never an empty set.
pub fn vocabulary() -> Result<&'static Vocabulary, String> {
    VOCAB
        .get_or_init(|| {
            let p = policy_file();
            std::fs::read_to_string(&p)
                .map_err(|e| format!("cannot read {}: {e}", p.display()))
                .and_then(|src| parse_policy(&src))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// The status keyword of `src` against the project's vocabulary. `None` when the file has no Status field, when
/// the Status field names no defined state, **or when the policy itself cannot be read** -- so every caller
/// starts with [`vocabulary`] and reports its error rather than relying on this.
pub fn extract_status_keyword(src: &str) -> Option<&'static str> {
    let v = vocabulary().ok()?;
    extract_status_keyword_in(src, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A policy table small enough to read, used where the test is about the parser and not the project.
    const FIXTURE: &str = "# P\n\n## Lifecycle states\n\n| State | Folder | Meaning |\n|---|---|---|\n\
        | **Proposed** | `proposed/` | x |\n| **Accepted** | `accepted/` | x |\n| **Implemented** | `done/` | x |\n\
        | **Implemented-with-Errata** | `done/` | x |\n\n## Folder layout\n";

    fn v() -> Vocabulary {
        parse_policy(FIXTURE).expect("fixture parses")
    }

    #[test]
    fn the_real_policy_parses_and_defines_the_ruled_set() {
        let real = vocabulary().expect("RFC 000's table parses");
        let names: Vec<&str> = real.states.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Proposed",
                "Accepted",
                "Implemented",
                "Implemented-with-Errata",
                "Superseded",
                "Withdrawn"
            ]
        );
        assert_eq!(real.names_in("proposed"), ["Proposed"]);
        assert_eq!(real.names_in("accepted"), ["Accepted"]);
    }

    #[test]
    fn finds_status_after_colon_label() {
        let src = "# Title\n\n**Status:** Implemented (v0.21.3)\n";
        assert_eq!(extract_status_keyword_in(src, &v()), Some("Implemented"));
    }

    #[test]
    fn finds_status_after_period_label_in_heading() {
        let src = "# Title\n\n## **Status.** Implemented (v0.9.0)\n";
        assert_eq!(extract_status_keyword_in(src, &v()), Some("Implemented"));
    }

    #[test]
    fn prefers_longest_match_implemented_with_errata() {
        let src = "**Status:** Implemented-with-Errata (see ERRATA E-002)\n";
        assert_eq!(
            extract_status_keyword_in(src, &v()),
            Some("Implemented-with-Errata")
        );
    }

    #[test]
    fn returns_none_when_no_status_field_present() {
        let src = "# RFC Lifecycle Policy\n\n## Status model\n\n| Status | Meaning |\n";
        assert_eq!(extract_status_keyword_in(src, &v()), None);
        assert_eq!(status_field_line(src), None);
    }

    #[test]
    fn the_leftmost_state_wins_not_the_first_in_the_table() {
        // `Accepted` precedes `Proposed`? No: here the line *starts* with Proposed and mentions Implemented later.
        let src = "**Status:** Proposed -- will be Implemented once accepted\n";
        assert_eq!(extract_status_keyword_in(src, &v()), Some("Proposed"));
    }

    #[test]
    fn a_status_field_naming_no_defined_state_is_not_no_status_field() {
        let src = "**Status:** Closed (all done)\n";
        assert!(status_field_line(src).is_some());
        assert_eq!(extract_status_keyword_in(src, &v()), None);
    }

    #[test]
    fn a_state_word_must_stand_alone() {
        let without_errata = FIXTURE.replace("| **Implemented-with-Errata** | `done/` | x |\n", "");
        let v = parse_policy(&without_errata).unwrap();
        // with the state deleted, `Implemented-with-Errata` is undefined, not `Implemented`
        let src = "**Status:** Implemented-with-Errata (E-002)\n";
        assert_eq!(extract_status_keyword_in(src, &v), None);
        // but a plain Implemented followed by punctuation or a space still is
        assert_eq!(
            extract_status_keyword_in("**Status:** Implemented.\n", &v),
            Some("Implemented")
        );
        assert_eq!(
            extract_status_keyword_in("**Status:** Implemented (0.1)\n", &v),
            Some("Implemented")
        );
    }

    #[test]
    fn a_policy_with_its_table_deleted_is_an_error_not_an_empty_vocabulary() {
        assert!(parse_policy("# P\n\n## Lifecycle states\n\nno table here\n").is_err());
        assert!(parse_policy("# P\n").is_err());
    }

    #[test]
    fn malformed_tables_are_refused() {
        let dup = FIXTURE.replace("**Accepted**", "**Proposed**");
        assert!(parse_policy(&dup).unwrap_err().contains("defined twice"));
        let bad_folder = FIXTURE.replace("`accepted/`", "`archive/`");
        assert!(parse_policy(&bad_folder).unwrap_err().contains("folder"));
        let bad_header = FIXTURE.replace("| State | Folder |", "| State | Where |");
        assert!(parse_policy(&bad_header).is_err());
    }
}
