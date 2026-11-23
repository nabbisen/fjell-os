//! 0.48 small item A (E-130): the `landing-page` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::landing_page`, in the library, where its tests are.
//!
//! It covers **two published pages**: the crates.io landing page (`crates/fjell-os/README.md`: the errata figure, and
//! *no gate count*) and the repository's root `README.md` (the readiness figures and the `done/` file count), the
//! second added at the 0.48 stage-1 review because leaving two figures ungated after discovering they drift is the
//! error E-125 recorded against E-025.

use fjell_consistency_check::landing_page::{
    ERRATA_PATH, LANDING_PAGE, READINESS_PATH, ROOT_README, gate_count_claims, judge, judge_root,
};
use fjell_consistency_check::readme_counts::count_folders;
use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "landing-page";

pub fn check() -> ExitCode {
    let Some(page) = crate::read_file(NAME, LANDING_PAGE) else {
        return ExitCode::FAILURE;
    };
    let Some(errata) = crate::read_file(NAME, ERRATA_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(root) = crate::read_file(NAME, ROOT_README) else {
        return ExitCode::FAILURE;
    };
    let Some(matrix) = crate::read_file(NAME, READINESS_PATH) else {
        return ExitCode::FAILURE;
    };
    let done = match count_folders(Path::new(".")) {
        Ok(c) => c.done_total,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot count rfcs/done: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut problems: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    match judge(&page, &errata) {
        Ok(l) => lines.push(l),
        Err(p) => problems.extend(p),
    }
    match judge_root(&root, &matrix, done) {
        Ok(l) => lines.push(l),
        Err(p) => problems.extend(p),
    }
    for g in gate_count_claims(&page) {
        problems.push(format!(
            "{LANDING_PAGE} states a gate count ({g}): a hand-maintained figure on a published page. State no number; \
             the gates are in the release-rehearsal table"
        ));
    }
    if problems.is_empty() {
        for l in lines {
            println!("{l}");
        }
        ExitCode::SUCCESS
    } else {
        eprintln!("{NAME}: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}
