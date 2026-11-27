//! RFC-0.50-002: the `syscall-doc` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::syscall_doc::evaluate`, in the library, where its tests are.

use fjell_consistency_check::syscall_doc::{Inputs, evaluate};
use std::process::ExitCode;

const NAME: &str = "syscall-doc";
const ABI: &str = "crates/fjell-abi/src/syscall.rs";
const REFERENCE: &str = "docs/src/api/syscalls.md";
const KERNEL_DOC: &str = "docs/src/external-design/kernel.md";
const WRAPPERS: &str = "crates/fjell-syscall/src/lib.rs";

pub fn check() -> ExitCode {
    let Some(abi) = crate::read_file(NAME, ABI) else { return ExitCode::FAILURE };
    let Some(reference) = crate::read_file(NAME, REFERENCE) else { return ExitCode::FAILURE };
    let Some(kernel_doc) = crate::read_file(NAME, KERNEL_DOC) else { return ExitCode::FAILURE };
    let Some(wrappers) = crate::read_file(NAME, WRAPPERS) else { return ExitCode::FAILURE };
    match evaluate(&Inputs { abi: &abi, reference: &reference, kernel_doc: &kernel_doc, wrappers: &wrappers }) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} declared syscalls; {REFERENCE}: {} rows, each a variant with its own number and every variant named; {KERNEL_DOC}: \
                 {} group-table entries, every variant named; {} wrapper alias(es) resolved to the syscall they are listed under; {} wrapper name(s) in the \
                 reference's third column exist. NOT checked: the prose around the tables, the *Group* column's grouping, a wrapper's signature, and the \
                 service tables)",
                r.declared, r.reference_rows, r.kernel_entries, r.aliases_resolved, r.wrappers_checked
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
