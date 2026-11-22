//! RFC-0.47-001 R9: the `authority-funnel` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::authority_funnel::evaluate`, in the library, where its tests are.

use fjell_consistency_check::authority_funnel::{CLASSIFIED, Source, evaluate};
use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "authority-funnel";
const KERNEL_SRC: &str = "crates/fjell-kernel/src/";

pub fn check() -> ExitCode {
    let paths = match fjell_repo_scope::files_with_extension(
        Path::new("."),
        fjell_repo_scope::Scope::Working,
        "rs",
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot derive the repository scope: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut sources = Vec::new();
    for p in paths
        .iter()
        .filter(|p| p.to_string_lossy().contains(KERNEL_SRC))
    {
        let Some(text) = crate::read_file(NAME, &p.to_string_lossy()) else {
            return ExitCode::FAILURE;
        };
        sources.push(Source {
            path: p.to_string_lossy().trim_start_matches("./").to_string(),
            text,
        });
    }
    match evaluate(&sources, CLASSIFIED) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} kernel source files, {} functions; {} authority-shaped tokens, every one in a \
                 classified function ({} entries, each with a reason); {} SAFETY claims that `require_cap` validated \
                 something, every one in a function that reaches the funnel; {} functions reach it)",
                r.files,
                r.functions,
                r.tokens,
                r.classified_used,
                r.safety_claims,
                r.funnel_reaching
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
