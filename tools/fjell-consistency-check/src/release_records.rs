//! RFC-0.48-003 D5: **`release-records`** -- the tree-pure half of "a released version has a CHANGELOG section and a release
//! record, and the workflow that publishes it is the one the RFC describes". It also holds the **shared pure helpers** the
//! publishing path (`cargo xtask release-preflight`) uses, so the gate and the thing it gates cannot read a tag, a section or
//! a filter differently.
//!
//! **Why this is not a check of tags.** `consistency-check` is *a pure function of the committed tree* -- the project's own
//! written rule (`release-handoff.md`: *"so that re-running it on an old tree still answers the same question"*), and at the
//! moment Gate 12 runs in a cut **the new tag does not exist yet**. Tags are refs, not tree. The facts that need a tag
//! (annotated, signed, verifies, no `v*` tag exists, ancestor of `main`) are judged by `release-preflight`, which the workflow
//! runs and the owner can run first. This check judges what the *tree* can say.
//!
//! 1. **Every `releases/<v>.md` has a `## [<v>] — ` CHANGELOG section.**
//! 2. **Every CHANGELOG section for a version strictly below the workspace version, and at or above the *floor*, has a
//!    non-empty record.** The floor is **derived** -- the lowest version that has a record -- never written: of 105 tags only
//!    35 have a section and 27 a record, total from `0.21.3` on and absent before (the convention did not exist), and a
//!    hand-written floor would fossilise today's history.
//! 3. **`release.yml` triggers on exactly the bare three-component tag filter** ([`BARE_TAG_PATTERN`]), so a `v0.48.0` tag does
//!    not publish -- silently, which is why the filter itself is judged.
//! 4. **`contents: write` appears exactly once, in a job that `needs:` the verifying job; the workflow-level permissions are
//!    empty; and every `uses:` is pinned to a 40-hex commit SHA** (the first workflow whose token can write).
//!
//! **What it does not do:** it does not show the workflow *runs*, or that a tag is signed. The first real fire is the owner's.

use std::collections::BTreeSet;

/// The GitHub tag filter the release workflow must use: bare `MAJOR.MINOR.PATCH`, no `v`, no pre-release suffix.
pub const BARE_TAG_PATTERN: &str = "[0-9]+.[0-9]+.[0-9]+";

pub const WORKFLOW_PATH: &str = ".github/workflows/release.yml";
pub const VERIFY_JOB: &str = "verify";
pub const PUBLISH_JOB: &str = "publish";

/// `0.48.0`: exactly three dot-separated all-digit components.
pub fn is_bare_tag(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// `(0, 48, 0)` for ordering; `None` if not a bare tag.
pub fn version_key(v: &str) -> Option<(u64, u64, u64)> {
    if !is_bare_tag(v) {
        return None;
    }
    let mut it = v.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

/// A matcher for the subset of GitHub's tag-filter grammar the release trigger uses: a literal, `[a-z]` classes, and `+` /
/// `*` / `?` applied to the preceding atom. **Full match**, as GitHub does. It exists so the trigger -- which fails by
/// *never firing* -- can be tested against the tags it must and must not match.
pub fn filter_matches(pattern: &str, name: &str) -> bool {
    #[derive(Clone)]
    enum Atom {
        Lit(char),
        Class(Vec<(char, char)>),
        AnyRun, // `*` : any characters except '/'
    }
    fn atom_matches(a: &Atom, c: char) -> bool {
        match a {
            Atom::Lit(l) => *l == c,
            Atom::Class(rs) => rs.iter().any(|(lo, hi)| *lo <= c && c <= *hi),
            Atom::AnyRun => c != '/',
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    // parse into (atom, quantifier) where quantifier is one of ' ', '+', '?', '*'
    let mut atoms: Vec<(Atom, char)> = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let atom = match p[i] {
            '[' => {
                let mut j = i + 1;
                let mut ranges = Vec::new();
                while j < p.len() && p[j] != ']' {
                    if j + 2 < p.len() && p[j + 1] == '-' && p[j + 2] != ']' {
                        ranges.push((p[j], p[j + 2]));
                        j += 3;
                    } else {
                        ranges.push((p[j], p[j]));
                        j += 1;
                    }
                }
                i = j + 1;
                Atom::Class(ranges)
            }
            '*' => {
                i += 1;
                Atom::AnyRun
            }
            c => {
                i += 1;
                Atom::Lit(c)
            }
        };
        let q = match p.get(i) {
            Some(&'+') if !matches!(atom, Atom::AnyRun) => {
                i += 1;
                '+'
            }
            Some(&'?') => {
                i += 1;
                '?'
            }
            _ => ' ',
        };
        let q = if matches!(atom, Atom::AnyRun) { '*' } else { q };
        atoms.push((atom, q));
    }
    let s: Vec<char> = name.chars().collect();
    fn go(atoms: &[(Atom, char)], s: &[char]) -> bool {
        let Some(((a, q), rest)) = atoms.split_first() else {
            return s.is_empty();
        };
        match q {
            ' ' => !s.is_empty() && atom_matches(a, s[0]) && go(rest, &s[1..]),
            '?' => go(rest, s) || (!s.is_empty() && atom_matches(a, s[0]) && go(rest, &s[1..])),
            '+' | '*' => {
                let min = usize::from(*q == '+');
                let mut n = 0;
                while n < s.len() && atom_matches(a, s[n]) {
                    n += 1;
                }
                (min..=n).rev().any(|k| go(rest, &s[k..]))
            }
            _ => false,
        }
    }
    go(&atoms, &s)
}

/// The CHANGELOG section for `version`: the text after its `## [<version>] — ` heading line up to (not including) the next
/// `## [` heading. `Err` says whether the heading is absent or the body is empty -- **a heading with nothing under it
/// satisfies "a section exists" and would publish a blank Release**, so emptiness is its own failure.
pub fn extract_changelog_section(changelog: &str, version: &str) -> Result<String, String> {
    let head = format!("## [{version}] — ");
    let mut lines = changelog.lines();
    let mut found = false;
    let mut body: Vec<&str> = Vec::new();
    for l in lines.by_ref() {
        if l.starts_with(&head) {
            found = true;
            break;
        }
    }
    if !found {
        return Err(format!("CHANGELOG.md has no `{head}` section"));
    }
    for l in lines {
        if l.starts_with("## [") {
            break;
        }
        body.push(l);
    }
    let text = body.join("\n").trim().to_string();
    if text.is_empty() {
        return Err(format!("CHANGELOG.md's `{head}` section is empty"));
    }
    Ok(text)
}

/// `[workspace.package] version = "X"` from a root `Cargo.toml`.
pub fn workspace_version(cargo_toml: &str) -> Option<String> {
    let mut in_table = false;
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_table = t == "[workspace.package]";
            continue;
        }
        if in_table && let Some(rest) = t.strip_prefix("version") {
            let rest = rest.trim_start().strip_prefix('=')?.trim();
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

/// Everything the tree can say.
#[derive(Clone, Copy)]
pub struct Inputs<'a> {
    /// `(version, record text)` for each `releases/<semver>.md`.
    pub records: &'a [(String, String)],
    pub changelog: &'a str,
    pub workspace_version: &'a str,
    pub workflow: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub records: usize,
    pub sections_checked: usize,
    pub floor: String,
}

/// The `tags:` entries under `on: push:` in the workflow, as written.
fn trigger_tags(workflow: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_tags = false;
    for l in workflow.lines() {
        let t = l.trim();
        if t.starts_with("tags:") {
            in_tags = true;
            if let Some(i) = t.find('[') {
                let inner = &t[i + 1..t.rfind(']').unwrap_or(t.len())];
                out.extend(
                    inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
                        .filter(|s| !s.is_empty()),
                );
                in_tags = false;
            }
            continue;
        }
        if in_tags {
            if let Some(item) = t.strip_prefix("- ") {
                out.push(item.trim().trim_matches('\'').trim_matches('"').to_string());
            } else if !t.is_empty() && !t.starts_with('#') {
                in_tags = false;
            }
        }
    }
    out
}

/// `(job name, body lines)` for each job under `jobs:` (jobs sit at two-space indent).
fn jobs(workflow: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut in_jobs = false;
    for l in workflow.lines() {
        if l.trim_end() == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        let indent = l.len() - l.trim_start().len();
        if indent == 2 && l.trim_end().ends_with(':') && !l.trim_start().starts_with('#') {
            out.push((l.trim().trim_end_matches(':').to_string(), Vec::new()));
        } else if let Some((_, body)) = out.last_mut() {
            body.push(l.to_string());
        }
    }
    out
}

fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// The workflow with YAML comments removed (` #` to end of line, and whole-line comments): prose *about* `contents: write` is
/// not a grant of it, and the file's own header says so in so many words.
fn without_comments(workflow: &str) -> String {
    workflow
        .lines()
        .map(|l| {
            let t = l.trim_start();
            if t.starts_with('#') {
                String::new()
            } else {
                match l.find(" #") {
                    Some(i) => l[..i].to_string(),
                    None => l.to_string(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn evaluate(i: &Inputs<'_>) -> Result<Report, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    let workflow_code = without_comments(i.workflow);
    let i = &Inputs {
        workflow: &workflow_code,
        ..*i
    };

    // 1 + 2: records and sections
    let have_record: BTreeSet<&str> = i
        .records
        .iter()
        .filter(|(_, t)| !t.trim().is_empty())
        .map(|(v, _)| v.as_str())
        .collect();
    for (v, t) in i.records {
        if t.trim().is_empty() {
            problems.push(format!("releases/{v}.md is empty"));
        }
        if let Err(e) = extract_changelog_section(i.changelog, v) {
            problems.push(format!("releases/{v}.md exists but {e}"));
        }
    }
    let floor = i
        .records
        .iter()
        .filter_map(|(v, _)| version_key(v).map(|k| (k, v.clone())))
        .min()
        .map(|(_, v)| v);
    let mut sections_checked = 0;
    if let (Some(floor_v), Some(ws)) = (floor.as_ref(), version_key(i.workspace_version)) {
        let fk = version_key(floor_v).unwrap();
        for l in i.changelog.lines() {
            let Some(rest) = l.strip_prefix("## [") else {
                continue;
            };
            let Some(end) = rest.find("] — ") else {
                continue;
            };
            let v = &rest[..end];
            let Some(k) = version_key(v) else { continue };
            if k >= fk && k < ws {
                sections_checked += 1;
                if !have_record.contains(v) {
                    problems.push(format!(
                        "CHANGELOG.md has a section for {v}, which is released (below the workspace version {}) and at or above the floor \
                         {floor_v} (the lowest version with a record), but releases/{v}.md is missing or empty",
                        i.workspace_version
                    ));
                }
            }
        }
    } else if floor.is_none() {
        problems.push("no releases/<version>.md exists, so the floor cannot be derived".into());
    }

    // 3. the trigger
    let tags = trigger_tags(i.workflow);
    if tags != [BARE_TAG_PATTERN.to_string()] {
        problems.push(format!(
            "{WORKFLOW_PATH}: `on.push.tags` is {tags:?}, but must be exactly [\"{BARE_TAG_PATTERN}\"] -- a `v*` filter would never fire for a \
             bare tag, and accepting both blesses a tag the convention forbids"
        ));
    } else {
        for (name, want) in [
            ("0.48.0", true),
            ("12.345.6", true),
            ("v0.48.0", false),
            ("0.2.0-alpha.1", false),
            ("0.48", false),
        ] {
            if filter_matches(BARE_TAG_PATTERN, name) != want {
                problems.push(format!(
                    "the trigger pattern {BARE_TAG_PATTERN} should {} `{name}`",
                    if want { "match" } else { "not match" }
                ));
            }
        }
    }

    // 4. permissions and pins
    let js = jobs(i.workflow);
    let writers: Vec<&str> = js
        .iter()
        .filter(|(_, b)| b.iter().any(|l| l.contains("contents: write")))
        .map(|(n, _)| n.as_str())
        .collect();
    let total_writes = i.workflow.matches("contents: write").count();
    if total_writes != 1 || writers != [PUBLISH_JOB] {
        problems.push(format!(
            "{WORKFLOW_PATH}: `contents: write` must appear exactly once, in the `{PUBLISH_JOB}` job (found {total_writes} time(s); in jobs {writers:?})"
        ));
    }
    match js.iter().find(|(n, _)| n == PUBLISH_JOB) {
        Some((_, b)) if b.iter().any(|l| l.trim() == format!("needs: {VERIFY_JOB}")) => {}
        Some(_) => problems.push(format!(
            "{WORKFLOW_PATH}: `{PUBLISH_JOB}` must have `needs: {VERIFY_JOB}`"
        )),
        None => problems.push(format!("{WORKFLOW_PATH}: no `{PUBLISH_JOB}` job")),
    }
    let top_level_perm_ok = i
        .workflow
        .lines()
        .any(|l| l.trim_end() == "permissions: {}");
    if !top_level_perm_ok {
        problems.push(format!("{WORKFLOW_PATH}: the workflow-level permissions must be `permissions: {{}}` (each job grants what it needs)"));
    }
    for l in i.workflow.lines() {
        if let Some(rest) = l
            .trim()
            .strip_prefix("- uses:")
            .or_else(|| l.trim().strip_prefix("uses:"))
        {
            let spec = rest.split_whitespace().next().unwrap_or("");
            if spec.starts_with("./") {
                continue; // a local action
            }
            let sha = spec.rsplit('@').next().unwrap_or("");
            if !is_sha(sha) {
                problems.push(format!("{WORKFLOW_PATH}: `uses: {spec}` is not pinned to a 40-hex commit SHA (this workflow's token can write)"));
            }
        }
    }

    if problems.is_empty() {
        Ok(Report {
            records: i.records.len(),
            sections_checked,
            floor: floor.unwrap_or_default(),
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trigger_pattern_matches_what_it_must_and_nothing_else() {
        for (t, want) in [
            ("0.48.0", true),
            ("0.47.0", true),
            ("10.200.3", true),
            ("v0.48.0", false),
            ("0.48", false),
            ("0.48.0.1", false),
            ("0.2.0-alpha.1", false),
            ("0.48.0-rc1", false),
            ("", false),
            ("a.b.c", false),
        ] {
            assert_eq!(filter_matches(BARE_TAG_PATTERN, t), want, "{t}");
        }
    }

    /// GitHub's own documented examples for the filter grammar, so the matcher is held to the documentation and not only to
    /// its author's reading of it.
    #[test]
    fn the_matcher_agrees_with_the_documented_examples() {
        assert!(
            filter_matches("v2*", "v2")
                && filter_matches("v2*", "v2.0")
                && filter_matches("v2*", "v2.9")
        );
        assert!(!filter_matches("v2*", "v3"));
        assert!(
            filter_matches("v[12].[0-9]+.[0-9]+", "v1.10.1")
                && filter_matches("v[12].[0-9]+.[0-9]+", "v2.0.0")
        );
        assert!(!filter_matches("v[12].[0-9]+.[0-9]+", "v3.0.0"));
        assert!(filter_matches("*", "anything") && !filter_matches("*", "a/b"));
        assert!(filter_matches("colou?r", "color") && filter_matches("colou?r", "colour"));
    }

    #[test]
    fn bare_tags_and_version_keys() {
        assert!(
            is_bare_tag("0.48.0")
                && !is_bare_tag("v0.48.0")
                && !is_bare_tag("0.48")
                && !is_bare_tag("0.48.0-rc1")
        );
        assert!(version_key("0.21.3").unwrap() < version_key("0.47.0").unwrap());
        assert!(version_key("0.9.1").unwrap() < version_key("0.10.0").unwrap());
        assert_eq!(version_key("x"), None);
    }

    const CL: &str = "# Changelog\n\n## [0.48.0] — 2026-11-25 — Subject\n\nBody of 48.\n\n## [0.47.0] — 2026-11-23 — Older\n\nBody of 47\nmore\n\n## [0.46.0] — 2026-11-21 — Oldest\n\n";

    #[test]
    fn a_section_is_extracted_exactly_and_an_absent_or_empty_one_is_an_error() {
        assert_eq!(
            extract_changelog_section(CL, "0.47.0").unwrap(),
            "Body of 47\nmore"
        );
        assert_eq!(
            extract_changelog_section(CL, "0.48.0").unwrap(),
            "Body of 48."
        );
        assert!(
            extract_changelog_section(CL, "0.49.0")
                .unwrap_err()
                .contains("no `## [0.49.0] — ` section")
        );
        assert!(
            extract_changelog_section(CL, "0.46.0")
                .unwrap_err()
                .contains("is empty")
        );
        // 0.4.0 must not match inside 0.47.0
        assert!(extract_changelog_section(CL, "0.4.0").is_err());
    }

    #[test]
    fn the_workspace_version_is_the_workspace_tables() {
        let t = "[package]\nversion = \"9.9.9\"\n\n[workspace.package]\nversion = \"0.48.0\"\nedition = \"2024\"\n";
        assert_eq!(workspace_version(t).as_deref(), Some("0.48.0"));
        assert_eq!(workspace_version("[package]\nversion = \"1.0.0\"\n"), None);
    }

    const WF: &str = "name: release\non:\n  push:\n    tags: ['[0-9]+.[0-9]+.[0-9]+']\npermissions: {}\njobs:\n  verify:\n    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n      - uses: ./.github/actions/toolchain\n  publish:\n    needs: verify\n    runs-on: ubuntu-latest\n    permissions:\n      contents: write\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n";

    fn recs() -> Vec<(String, String)> {
        vec![
            ("0.47.0".into(), "record 47".into()),
            ("0.46.0".into(), "record 46".into()),
        ]
    }
    fn cl2() -> String {
        "## [0.48.0] — d — s\n\nx\n\n## [0.47.0] — d — s\n\nx\n\n## [0.46.0] — d — s\n\nx\n\n## [0.20.0] — d — s\n\nold, below the floor\n".to_string()
    }
    fn run(
        records: &[(String, String)],
        cl: &str,
        ws: &str,
        wf: &str,
    ) -> Result<Report, Vec<String>> {
        evaluate(&Inputs {
            records,
            changelog: cl,
            workspace_version: ws,
            workflow: wf,
        })
    }

    #[test]
    fn the_control_is_green_and_the_floor_is_derived() {
        let r = run(&recs(), &cl2(), "0.48.0", WF).unwrap();
        assert_eq!(
            (r.records, r.sections_checked, r.floor.as_str()),
            (2, 2, "0.46.0")
        );
    }

    #[test]
    fn a_record_with_no_section_and_a_released_section_with_no_record_are_red() {
        let mut r = recs();
        r.push(("0.45.0".into(), "x".into()));
        let e = run(&r, &cl2(), "0.48.0", WF).unwrap_err();
        assert!(
            e.iter().any(|p| p.contains("releases/0.45.0.md exists but")
                && p.contains("no `## [0.45.0] — ` section")),
            "{e:?}"
        );
        let only47 = vec![("0.47.0".to_string(), "x".to_string())];
        // a section *below the floor* (the lowest version with a record) is history the convention did not cover: not judged
        assert!(
            run(&only47, &cl2(), "0.48.0", WF).is_ok(),
            "0.46.0 is below a floor of 0.47.0"
        );
        let e = run(
            &recs(),
            &cl2().replace("## [0.20.0]", "## [0.46.5]"),
            "0.48.0",
            WF,
        )
        .unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("section for 0.46.5") && p.contains("floor 0.46.0")),
            "{e:?}"
        );
    }

    #[test]
    fn the_unreleased_current_version_needs_no_record_yet() {
        assert!(run(&recs(), &cl2(), "0.48.0", WF).is_ok());
    }

    #[test]
    fn a_v_filter_is_red_and_named() {
        let e = run(
            &recs(),
            &cl2(),
            "0.48.0",
            &WF.replace("'[0-9]+.[0-9]+.[0-9]+'", "'v*'"),
        )
        .unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("on.push.tags") && p.contains("v*")),
            "{e:?}"
        );
    }

    #[test]
    fn contents_write_anywhere_but_the_publish_job_is_red() {
        let e = run(
            &recs(),
            &cl2(),
            "0.48.0",
            &WF.replace("contents: read", "contents: write"),
        )
        .unwrap_err();
        assert!(e.iter().any(|p| p.contains("exactly once")), "{e:?}");
        let e = run(
            &recs(),
            &cl2(),
            "0.48.0",
            &WF.replace("permissions: {}", "permissions:\n  contents: write"),
        )
        .unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("exactly once") || p.contains("workflow-level")),
            "{e:?}"
        );
        let e = run(
            &recs(),
            &cl2(),
            "0.48.0",
            &WF.replace("    needs: verify\n", ""),
        )
        .unwrap_err();
        assert!(e.iter().any(|p| p.contains("needs: verify")), "{e:?}");
        let e = run(
            &recs(),
            &cl2(),
            "0.48.0",
            &WF.replace("permissions: {}\n", ""),
        )
        .unwrap_err();
        assert!(e.iter().any(|p| p.contains("workflow-level")), "{e:?}");
    }

    #[test]
    fn prose_about_contents_write_in_a_comment_is_not_a_grant() {
        let wf = format!(
            "# contents: write is in the publish job only\n{}",
            WF.replace("name: release\n", "name: release # contents: write\n")
        );
        assert!(
            run(&recs(), &cl2(), "0.48.0", &wf).is_ok(),
            "{:?}",
            run(&recs(), &cl2(), "0.48.0", &wf)
        );
    }

    #[test]
    fn an_action_not_pinned_by_sha_is_red_and_a_local_action_is_not() {
        let e = run(
            &recs(),
            &cl2(),
            "0.48.0",
            &WF.replace(
                "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
                "actions/checkout@v7",
            ),
        )
        .unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("actions/checkout@v7") && p.contains("40-hex")),
            "{e:?}"
        );
    }

    #[test]
    fn an_empty_record_set_is_an_error() {
        assert!(run(&[], &cl2(), "0.48.0", WF).is_err());
    }

    fn real(path: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path),
        )
        .expect(path)
    }

    #[test]
    fn the_real_tree_has_a_section_for_every_record_from_the_derived_floor() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut records = Vec::new();
        for e in std::fs::read_dir(root.join("releases")).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if let Some(v) = n.strip_suffix(".md")
                && is_bare_tag(v)
            {
                records.push((v.to_string(), std::fs::read_to_string(e.path()).unwrap()));
            }
        }
        // the workflow does not exist until the line lands; this test judges it from then on.
        let wf = std::fs::read_to_string(root.join(WORKFLOW_PATH)).unwrap_or_default();
        if wf.is_empty() {
            let r = run(
                &records,
                &real("CHANGELOG.md"),
                &workspace_version(&real("Cargo.toml")).unwrap(),
                WF,
            );
            assert!(r.is_ok(), "{r:?}");
        } else {
            let r = run(
                &records,
                &real("CHANGELOG.md"),
                &workspace_version(&real("Cargo.toml")).unwrap(),
                &wf,
            );
            assert!(r.is_ok(), "{r:?}");
        }
    }
}
