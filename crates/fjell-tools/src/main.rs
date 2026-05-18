//! Fjell OS host-side development tools (`cargo xtask` entry point).
//!
//! Usage: `cargo xtask <subcommand>`
//!
//! Subcommands:
//!   build-services                     — build service crates → prebuilt/*.bin
//!   build                              — build-services + fjell-kernel
//!   qemu                               — build + launch QEMU interactively
//!   qemu-test [m1..m8]                 — build + run smoke test, check
//!                                        TEST:M*:PASS  (RFC 025)
//!   qemu-negative <category>           — run a profile-driven negative
//!                                        test  (RFC 025, RFC 026, RFC 042)
//!   qemu-log-check <log-file> <marker> — substring-match a marker in a
//!                                        captured log  (RFC 025)
//!   qemu-run --profile <name>          — run an explicit profile from
//!                                        tests/qemu/profiles/<name>.toml

mod qemu;
mod qemu_log_check;
mod qemu_run;
mod smoke;
mod negative;
mod policy_eval; // RFC 040 cap-broker policy unit tests

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
        Some("qemu-test") => {
            smoke::cmd_qemu_test(args.get(1).map(String::as_str))
        }
        Some("qemu-negative") => {
            negative::cmd_qemu_negative(args.get(1).map(String::as_str))
        }
        Some("qemu-log-check") => {
            qemu_log_check::cmd_qemu_log_check(
                args.get(1).map(String::as_str),
                args.get(2).map(String::as_str),
            )
        }
        Some("qemu-run") => {
            // Accept either `qemu-run --profile NAME` or `qemu-run NAME`.
            let profile = match args.get(1).map(String::as_str) {
                Some("--profile") => args.get(2).map(String::as_str),
                Some(other)       => Some(other),
                None              => None,
            };
            qemu_run::cmd_qemu_run(profile)
        }
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
    eprintln!(
"Usage: cargo xtask <subcommand>

Subcommands:
  build-services
  build
  qemu
  qemu-test [m1|m2|m3|m4|m5|m6|m7|m8]
  qemu-negative <capability|ipc|mmio|dma|store|upgrade|...>
  qemu-log-check <log-file> <marker>
  qemu-run --profile <name>"
    );
}
