//! RFC-0.48-003 D5: the `release-records` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::release_records::evaluate`, in the library, where its tests are.

use fjell_consistency_check::criterion9::{self, Exemption};
use fjell_consistency_check::release_records::{
    Inputs, WORKFLOW_PATH, evaluate, is_bare_tag, workspace_version,
};
use std::process::ExitCode;

const NAME: &str = "release-records";

pub fn check() -> ExitCode {
    let Some(changelog) = crate::read_file(NAME, "CHANGELOG.md") else {
        return ExitCode::FAILURE;
    };
    let Some(cargo) = crate::read_file(NAME, "Cargo.toml") else {
        return ExitCode::FAILURE;
    };
    let Some(workflow) = crate::read_file(NAME, WORKFLOW_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(ws) = workspace_version(&cargo) else {
        eprintln!("{NAME}: FAIL — Cargo.toml has no [workspace.package] version");
        return ExitCode::FAILURE;
    };
    let Some(dir) = crate::read_dir_named(NAME, "releases") else {
        return ExitCode::FAILURE;
    };
    let mut records: Vec<(String, String)> = Vec::new();
    for e in dir.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if let Some(v) = n.strip_suffix(".md")
            && is_bare_tag(v)
        {
            let Some(text) = crate::read_file(NAME, &e.path().to_string_lossy()) else {
                return ExitCode::FAILURE;
            };
            records.push((v.to_string(), text));
        }
    }
    records.sort();
    // 0.50 small item C: criterion 9 names two runs. Judged beside the structure checks and reported in the same line.
    let c9 = criterion9::evaluate(&records, CRITERION_9_EXEMPTIONS);
    match evaluate(&Inputs {
        records: &records,
        changelog: &changelog,
        workspace_version: &ws,
        workflow: &workflow,
    }) {
        Ok(r) if c9.is_ok() => {
            let c = c9.as_ref().unwrap();
            println!(
                "{NAME}: PASS ({} release records, each with a CHANGELOG section; {} released sections from the derived floor {} each with a record; \
                 {WORKFLOW_PATH} triggers on exactly the bare tag filter, holds `contents: write` once in a job that needs `verify`, and pins every \
                 action by SHA. This judges the tree: whether a tag is signed, and whether the workflow runs, are `release-preflight`'s and the \
                 first real tag's)",
                r.records, r.sections_checked, r.floor
            );
            println!(
                "{NAME}: criterion 9 -- of the {} records at or above the derived floor {} (the first to name both), {} name a push run and a \
                 dispatch run in their criterion-9 section and {} are exempt by name: {}; the {} below it, up to {}, are not asked (criterion 9 was \
                 first exercised at 0.31.0 and the dispatch step added at 0.32.0). NOT covered: whether an id is a real `ci.yml` run, or the run \
                 was green -- that is the record's table",
                c.records - c.below_floor.len(),
                c.floor,
                c.naming_both,
                c.exempt.len(),
                c.exempt
                    .iter()
                    .map(|(v, why)| format!("{v} ({why})"))
                    .collect::<Vec<_>>()
                    .join("; "),
                c.below_floor.len(),
                c.below_floor.last().map(String::as_str).unwrap_or("?"),
            );
            ExitCode::SUCCESS
        }
        r => {
            eprintln!("{NAME}: FAIL");
            if let Err(problems) = r {
                for p in &problems {
                    eprintln!("  {p}");
                }
            }
            if let Err(problems) = &c9 {
                for p in problems {
                    eprintln!("  criterion 9: {p}");
                }
            }
            ExitCode::FAILURE
        }
    }
}

/// **Records at or above the derived floor that do not name both runs, by version and reason.** The shape of `unsafe-audit`'s
/// exemptions: hand-written, a reason each, and a stale entry fails (the record now names both, or is below the floor, or
/// does not exist). Released records are not rewritten, so the one record that predates the rule is named here, not edited.
const CRITERION_9_EXEMPTIONS: &[Exemption<'static>] = &[Exemption {
    version: "0.49.0",
    reason: "its criterion-9 section records one run -- push 45843432462, \"the tagged commit's own run, not a dispatch\" -- and names no dispatch run, \
             so `fuzz-run` and `miri` (skipped on push) were not read for the release commit, or were read and left out of the record; the tree \
             cannot distinguish them. A released record is not rewritten: the gap is the architect's erratum (CLOSED as a record), and this \
             exemption goes stale -- and the gate fails -- the day the record names both runs",
}];
