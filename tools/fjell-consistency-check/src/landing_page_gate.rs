//! 0.48 small item A (E-130): the `landing-page` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::landing_page::judge`, in the library, where its tests are.

use fjell_consistency_check::landing_page::{ERRATA_PATH, LANDING_PAGE, judge};
use std::process::ExitCode;

const NAME: &str = "landing-page";

pub fn check() -> ExitCode {
    let Some(page) = crate::read_file(NAME, LANDING_PAGE) else {
        return ExitCode::FAILURE;
    };
    let Some(errata) = crate::read_file(NAME, ERRATA_PATH) else {
        return ExitCode::FAILURE;
    };
    match judge(&page, &errata) {
        Ok(line) => {
            println!("{line}");
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
