//! RFC-0.50-002 D5 (ruled in at the stage-2 review): **the service table names every program under `crates/services/` and
//! `crates/drivers/`, and only those.**
//!
//! `docs/src/external-design/services.md`'s heading no longer states a count; it states a **rule** -- *"every program under
//! `crates/services/` and `crates/drivers/`"* -- and a rule stated in a document with nothing checking it is the shape the Syscall
//! Reference had for twenty milestones. Before this, the table was five programs short of the tree (the heading's 29 matched the
//! table's 29; the tree has 34), and `doc_counts.rs` said service counts were somebody else's.
//!
//! **A set comparison only, and deliberately small.** Every program directory is named by some row's *first cell*, and every name a
//! first cell gives is a program directory. A program's name is its directory with the `fjell-` prefix removed (`fjell-init` ->
//! `init`, `fjell-driver-uart` -> `driver-uart`), which is how the table has always spelled them. **Not checked:** the Responsibility
//! text, the Requirement column, which programs are stubs, or that a row says anything true about its program. **Nothing found is a
//! failure**: a table whose heading is renamed or whose rows cannot be read is FAIL, not a pass over nothing.

use std::collections::BTreeSet;

/// The heading the table sits under; its section runs to the next heading.
pub const ANCHOR: &str = "### The service plane";

#[derive(Debug)]
pub struct Report {
    pub programs: usize,
    pub rows: usize,
}

/// `crates/services/fjell-init` -> `init`. `None` for a path that is not a program directory.
pub fn program_name(dir: &str) -> Option<String> {
    let rest = dir
        .strip_prefix("crates/services/")
        .or_else(|| dir.strip_prefix("crates/drivers/"))?;
    (!rest.is_empty() && !rest.contains('/'))
        .then(|| rest.strip_prefix("fjell-").unwrap_or(rest).to_string())
}

/// The program names the table's first cells give, in order (one entry per backtick span; a row may name several).
pub fn named_programs(doc: &str) -> Option<Vec<String>> {
    let mut in_section = false;
    let mut found_anchor = false;
    let mut out = Vec::new();
    for line in doc.lines() {
        if line.starts_with(ANCHOR) {
            in_section = true;
            found_anchor = true;
            continue;
        }
        if in_section && line.starts_with('#') {
            break;
        }
        if !in_section || !line.starts_with("| `") {
            continue; // the header row starts `| Service`, the separator `|---`
        }
        let first = line.trim_start_matches('|').split('|').next().unwrap_or("");
        let mut rest = first;
        while let Some(a) = rest.find('`') {
            let after = &rest[a + 1..];
            let Some(b) = after.find('`') else { break };
            out.push(after[..b].to_string());
            rest = &after[b + 1..];
        }
    }
    found_anchor.then_some(out)
}

/// `dirs`: the program directories in the tree (e.g. `crates/services/fjell-init`); `doc`: `services.md`.
pub fn evaluate(dirs: &[String], doc: &str) -> Result<Report, Vec<String>> {
    let mut problems = Vec::new();
    let tree: BTreeSet<String> = dirs.iter().filter_map(|d| program_name(d)).collect();
    if tree.is_empty() {
        return Err(vec!["no program directory found under crates/services/ or crates/drivers/: a check over nothing is not a pass".into()]);
    }
    let Some(named) = named_programs(doc) else {
        return Err(vec![format!(
            "services.md: the `{ANCHOR}` heading was not found: a renamed or removed table is a failure, not a pass over nothing"
        )]);
    };
    if named.is_empty() {
        problems.push("services.md: the service table has no rows under its heading".into());
    }
    let mut seen = BTreeSet::new();
    for n in &named {
        if !seen.insert(n.as_str()) {
            problems.push(format!(
                "services.md: `{n}` is named by two rows (or twice in one)"
            ));
        }
        if !tree.contains(n) {
            problems.push(format!(
                "services.md: the table names `{n}`, and there is no such program directory under crates/services/ or crates/drivers/ (a ghost row)"
            ));
        }
    }
    for p in &tree {
        if !seen.contains(p.as_str()) {
            problems.push(format!(
                "services.md: `{p}` is a program in the tree (a `fjell-{p}` directory under crates/services/ or crates/drivers/) and no row of the service table names it"
            ));
        }
    }
    if problems.is_empty() {
        Ok(Report {
            programs: tree.len(),
            rows: named.len(),
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "## 2. x\n\n### The service plane (as built)\n\ntext\n\n| Service | Responsibility | Requirement |\n|---|---|---|\n| `init` | first | FR |\n| `driver-uart` | uart | RFC |\n| `svc-fault`, `neg-test` | helpers | (testing) |\n\n## 3. next\n\n| `not-this` | x | y |\n";
    fn dirs() -> Vec<String> {
        [
            "crates/services/fjell-init",
            "crates/drivers/fjell-driver-uart",
            "crates/services/fjell-svc-fault",
            "crates/services/fjell-neg-test",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[test]
    fn a_table_that_names_every_program_and_only_those_passes() {
        let r = evaluate(&dirs(), DOC).unwrap();
        assert_eq!(
            (r.programs, r.rows),
            (4, 4),
            "a row may name several programs; a name is counted once"
        );
    }

    /// Direction 1: a program in the tree that no row names (the five the real table was missing).
    #[test]
    fn a_program_no_row_names_fails() {
        let mut d = dirs();
        d.push("crates/services/fjell-summaryd".into());
        let err = evaluate(&d, DOC).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("`summaryd` is a program in the tree")),
            "{err:?}"
        );
    }

    /// Direction 2: a row naming a program that is not in the tree.
    #[test]
    fn a_ghost_row_fails() {
        let doc = DOC.replace("`neg-test`", "`neg-test`, `ghost-svc`");
        let err = evaluate(&dirs(), &doc).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("`ghost-svc`") && e.contains("ghost row")),
            "{err:?}"
        );
    }

    #[test]
    fn a_program_named_twice_fails() {
        let doc = DOC.replace("| `driver-uart` |", "| `driver-uart`, `init` |");
        let err = evaluate(&dirs(), &doc).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("`init` is named by two rows")),
            "{err:?}"
        );
    }

    #[test]
    fn a_table_that_cannot_be_found_is_a_failure_not_a_pass() {
        let err = evaluate(
            &dirs(),
            &DOC.replace("### The service plane", "### The plane of services"),
        )
        .unwrap_err();
        assert!(err[0].contains("heading was not found"), "{err:?}");
        let err = evaluate(&[], DOC).unwrap_err();
        assert!(err[0].contains("a check over nothing"), "{err:?}");
        let err = evaluate(&dirs(), "### The service plane\n\nno table here\n").unwrap_err();
        assert!(err.iter().any(|e| e.contains("no rows")), "{err:?}");
    }

    #[test]
    fn rows_after_the_next_heading_are_not_the_table() {
        let named = named_programs(DOC).unwrap();
        assert!(!named.contains(&"not-this".to_string()), "{named:?}");
    }

    #[test]
    fn program_names_strip_the_prefix_and_only_the_two_trees_count() {
        assert_eq!(
            program_name("crates/services/fjell-init").as_deref(),
            Some("init")
        );
        assert_eq!(
            program_name("crates/drivers/fjell-driver-uart").as_deref(),
            Some("driver-uart")
        );
        assert_eq!(program_name("crates/fjell-kernel"), None);
        assert_eq!(program_name("crates/services/fjell-init/src"), None);
    }

    #[test]
    fn the_real_table_names_the_real_programs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut dirs = Vec::new();
        for kind in ["services", "drivers"] {
            for e in std::fs::read_dir(root.join("crates").join(kind))
                .unwrap()
                .flatten()
            {
                if e.path().join("Cargo.toml").is_file() {
                    dirs.push(format!("crates/{kind}/{}", e.file_name().to_string_lossy()));
                }
            }
        }
        let doc =
            std::fs::read_to_string(root.join("docs/src/external-design/services.md")).unwrap();
        let r = evaluate(&dirs, &doc);
        assert!(r.is_ok(), "{:#?}", r.err());
    }
}
