//! RFC-0.48-003 D5: the `release-records` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::release_records::evaluate`, in the library, where its tests are.

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
    match evaluate(&Inputs {
        records: &records,
        changelog: &changelog,
        workspace_version: &ws,
        workflow: &workflow,
    }) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} release records, each with a CHANGELOG section; {} released sections from the derived floor {} each with a record; \
                 {WORKFLOW_PATH} triggers on exactly the bare tag filter, holds `contents: write` once in a job that needs `verify`, and pins every \
                 action by SHA. This judges the tree: whether a tag is signed, and whether the workflow runs, are `release-preflight`'s and the \
                 first real tag's)",
                r.records, r.sections_checked, r.floor
            );
            ExitCode::SUCCESS
        }
        Err(problems) => {
            eprintln!("{NAME}: FAIL");
            for p in &problems {
                eprintln!("  {p}");
            }
            ExitCode::FAILURE
        }
    }
}
