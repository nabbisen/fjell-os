//! RFC-0.50-002 D5: the `service-table` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::service_table::evaluate`, in the library, where its tests are.

use fjell_consistency_check::service_table::{evaluate, program_name};
use fjell_repo_scope::Scope;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "service-table";
const DOC: &str = "docs/src/external-design/services.md";

pub fn check() -> ExitCode {
    let tracked = match fjell_repo_scope::files(Path::new("."), Scope::Tracked) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot list the tracked files: {e}");
            return ExitCode::FAILURE;
        }
    };
    // A program directory is one that has a tracked Cargo.toml directly under it.
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    for f in &tracked {
        let rel = f.strip_prefix(".").unwrap_or(f).to_string_lossy().replace('\\', "/");
        let rel = rel.trim_start_matches("./");
        if let Some(dir) = rel.strip_suffix("/Cargo.toml")
            && program_name(dir).is_some()
        {
            dirs.insert(dir.to_string());
        }
    }
    let Some(doc) = crate::read_file(NAME, DOC) else {
        return ExitCode::FAILURE;
    };
    let dirs: Vec<String> = dirs.into_iter().collect();
    match evaluate(&dirs, &doc) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} programs under crates/services/ and crates/drivers/, each named by a first cell of {DOC}'s service table, and every \
                 name in those cells is a program. NOT checked: the Responsibility text, the Requirement column, stub claims, or that a row is true of its program)",
                r.programs
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
