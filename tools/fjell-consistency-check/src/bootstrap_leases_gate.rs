//! RFC-0.48-001 R6: the `bootstrap-leases` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::bootstrap_leases::evaluate`, in the library, where its tests are.

use fjell_consistency_check::bootstrap_leases::{Source, evaluate, parse_undispatched};
use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "bootstrap-leases";
const KERNEL_SRC: &str = "crates/fjell-kernel/src/";
const EXPECTED: &str = "tests/syscall/expected.toml";

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
    let Some(expected) = crate::read_file(NAME, EXPECTED) else {
        return ExitCode::FAILURE;
    };
    match evaluate(&sources, &parse_undispatched(&expected)) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} kernel source files; {} Capability literals, {} `lease: None`, every one in a classified \
                 function ({} entries, each with a reason) -- the per-image grants are bound to a boot lease in one pass; \
                 {} fixture images exempt from that pass, each with a reason; `MmioUnmap` is still undispatched, so lease \
                 revocation still does not reclaim an MMIO mapping)",
                r.files, r.literals, r.unbound_literals, r.classified_used, r.fixture_images
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
