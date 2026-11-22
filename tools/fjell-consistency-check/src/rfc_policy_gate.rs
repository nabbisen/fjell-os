//! RFC-0.47-002 R5: the `rfc-policy` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::rfc_policy::evaluate`, in the library, where its tests are.

use fjell_consistency_check::rfc_policy::{Inputs, evaluate};
use std::process::ExitCode;

const NAME: &str = "rfc-policy";
const ERRATA_PATH: &str = "rfcs/ERRATA.md";

pub fn check() -> ExitCode {
    let Some(policy) = crate::read_file(NAME, fjell_consistency_check::status::POLICY_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(errata) = crate::read_file(NAME, ERRATA_PATH) else {
        return ExitCode::FAILURE;
    };
    let mut rfcs: Vec<(String, String, String)> = Vec::new();
    for dir in ["proposed", "accepted", "done"] {
        let path = format!("rfcs/{dir}");
        let Some(entries) = crate::read_dir_named(NAME, &path) else {
            return ExitCode::FAILURE;
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        paths.sort();
        for p in paths {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            rfcs.push((p.to_string_lossy().to_string(), dir.to_string(), text));
        }
    }
    match evaluate(&Inputs {
        policy: &policy,
        errata: &errata,
        rfcs: &rfcs,
    }) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} states defined by RFC 000; the drift rule is present in RFC 000 and linked from the register; \
                 {} RFCs' Status words all defined; {} Implemented-with-Errata all named in the register; \
                 {} entries carry an RFC field)",
                r.states, r.rfcs, r.with_errata, r.fielded_entries
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
