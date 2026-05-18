//! Fjell OS host-side development tools (`cargo xtask` entry point).
//!
//! Usage: `cargo xtask <subcommand>`
//!
//! Subcommands:
//!   build-services        — build service crates → prebuilt/*.bin
//!   build                 — build-services + fjell-kernel
//!   qemu                  — build + launch QEMU interactively
//!   qemu-test [m2|m3|m4]  — build + run smoke test, check TEST:M*:PASS

mod qemu;
mod smoke;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("build-services") => {
            if qemu::build_services() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Some("build") => {
            qemu::build_all();
            ExitCode::SUCCESS
        }
        Some("qemu") => qemu::cmd_qemu(),
        Some("qemu-test") => smoke::cmd_qemu_test(args.get(1).map(String::as_str)),
        Some(other) => {
            eprintln!("fjell-tools: unknown subcommand `{other}`");
            usage();
            ExitCode::FAILURE
        }
        None => {
            eprintln!("fjell-tools: no subcommand given");
            usage();
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!("Usage: cargo xtask {{ build-services | build | qemu | qemu-test [m2|m3|m4] }}");
}
