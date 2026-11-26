//! RFC-0.48-001 R6: the `bootstrap-leases` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::bootstrap_leases::evaluate`, in the library, where its tests are.

use fjell_consistency_check::bootstrap_leases::{Source, evaluate};
use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "bootstrap-leases";
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
    match evaluate(&sources) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} kernel source files; {} Capability literals, {} `lease: None`, every one in a classified \
                 function ({} entries, each with a reason) -- the per-image grants are bound to a boot lease in one pass; \
                 {} fixture images exempt from that pass, each with a reason; {} call sites of a device-unmap primitive, every \
                 one in a classified function (the holder's syscalls, a task's death, and a failed map's own rollback) -- so lease \
                 revocation is not among the things that take a DMA frame or an MMIO mapping back; {} production task ids carry a \
                 literal generation 0, every one in a classified function, so none can compare an owner by index once a slot is \
                 reused)",
                r.files,
                r.literals,
                r.unbound_literals,
                r.classified_used,
                r.fixture_images,
                r.device_unmap_sites,
                r.task_id_zero_sites
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
