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
//!   test-all [--no-qemu]              — run every test tier; write
//!                                        dated log bundle to tests/runs/

mod qemu;
mod qemu_log_check;
mod qemu_run;
mod smoke;
mod negative;
mod policy_eval; // RFC 040 cap-broker policy unit tests
mod dev;          // RFC v0.9-005 developer workflow
mod test_all;     // full test-all runner with log bundle
mod trust_report; // RFC 061 §6 Trust Report
mod bench;
mod fleet_demo;
mod sign_bundle;
mod registry;
mod dev_modes;   // RFC-v0.14-005 --trace/--measure/--gdb    // RFC-v0.14-004 publish/install  // RFC-v0.11-003 bundle signing pipeline   // RFC-v0.10-005 three-node fleet demo        // RFC-v0.10-004 criterion bench runner

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
        Some("dev") => {
            if args.get(1).map(String::as_str) == Some("run") {
                dev_modes::cmd_dev_run(&args[2.min(args.len())..])
            } else if args.get(1).map(String::as_str) == Some("log-check") {
                dev::cmd_dev_log_check(
                    args.get(2).map(String::as_str),
                    args.get(3).map(String::as_str),
                )
            } else {
                dev::cmd_dev(
                    args.get(1).map(String::as_str),
                    &args[2.min(args.len())..],
                )
            }
        }
        Some("publish") => { registry::cmd_publish(&args[1..]) }
        Some("install") => { registry::cmd_install(&args[1..]) }
        Some("sign-bundle") => {
            sign_bundle::cmd_sign_bundle(&args[1..])
        }
        Some("verify-bundle-sig") => {
            sign_bundle::cmd_verify_bundle_sig(&args[1..])
        }
        Some("key") => {
            sign_bundle::cmd_key(args.get(1).map(String::as_str), &args[2.min(args.len())..])
        }
        Some("fleet-demo") => {
            fleet_demo::cmd_fleet_demo(
                args.get(1).map(String::as_str),
                &args[2.min(args.len())..],
            )
        }
        Some("toolkit") => {
            if args.get(1).map(String::as_str) == Some("regenerate") {
                println!("[toolkit] regenerating typed emitters from catalog v1 …");
                let status = std::process::Command::new("python3")
                    .args(["-c", "import subprocess; subprocess.run(['cargo', 'build', '-p', 'fjell-semantic-toolkit'], check=True); print('[toolkit] regenerate: OK')"]) 
                    .status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
                if status == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
            } else {
                eprintln!("Usage: cargo xtask toolkit regenerate");
                ExitCode::FAILURE
            }
        }
        Some("bench") => {
            bench::cmd_bench(&args[1..])
        }
        Some("repro-check") => {
            let status = std::process::Command::new("cargo")
                .args(["run", "-p", "fjell-repro-check", "--", "--skip-build"])
                .status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
            if status == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Some("abi-snapshot") => {
            // RFC-v0.10-002: generate or verify the stable ABI surface snapshot
            let sub = args.get(1).map(String::as_str).unwrap_or("--verify");
            let snap = args.windows(2).find(|w| w[0]=="--snapshot").and_then(|w| w.get(1)).map(String::as_str).unwrap_or("tests/abi/snapshot.json");
            let argv: Vec<String> = vec![sub.into(), "--snapshot".into(), snap.into()];
            // delegate to the abi-snapshot binary
            let status = std::process::Command::new("cargo")
                .args(["run", "-p", "fjell-abi-snapshot", "--"])
                .args(&argv)
                .status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
            if status == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Some("readiness-check") => {
            let path = args.get(1).map(String::as_str).unwrap_or("docs/release/v1-readiness.md");
            let status = std::process::Command::new("cargo")
                .args(["run", "-p", "fjell-readiness-check", "--", "--matrix", path])
                .status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
            if status == 0 { ExitCode::SUCCESS } else { ExitCode::from(status as u8) }
        }
        Some("trust-report") => {
            trust_report::cmd_trust_report(&args[1..])
        }
        Some("test-all") => {
            test_all::cmd_test_all(&args[1..])
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
  qemu-run --profile <name>
  dev run --svc <name> --kernel <path>  (RFC v0.9-005)
  dev lint <manifest.toml>
  test-all [--no-qemu]           run every tier, save logs to tests/runs/
  trust-report [--dry-run]       RFC 061 §6 six-section trust report"
    );
}
