//! Fjell OS host-side development tools (`cargo xtask` entry point).
//!
//! Usage: `cargo xtask <subcommand> [args]`
//!
//! Subcommands:
//!   qemu                — build fjell-kernel and launch QEMU interactively
//!   qemu-test [m2|m3]   — run QEMU smoke test, check for TEST:M*:PASS marker

mod qemu;
mod smoke;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("qemu") => qemu::cmd_qemu(),
        Some("qemu-test") => smoke::cmd_qemu_test(args.get(1).map(String::as_str)),
        Some(other) => {
            eprintln!("fjell-tools: unknown subcommand `{other}`");
            eprintln!("Usage: cargo xtask {{ qemu | qemu-test [m2|m3] }}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("fjell-tools: no subcommand given");
            eprintln!("Usage: cargo xtask {{ qemu | qemu-test [m2|m3] }}");
            ExitCode::FAILURE
        }
    }
}
