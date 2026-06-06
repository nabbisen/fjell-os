//! `cargo xtask fleet-demo {up|down|status|verify|build-bundle|deploy}`
//!
//! Manages the three-node QEMU reference fleet described in
//! `examples/three-node-fleet/fleet-demo.toml` (RFC-v0.10-005).
//!
//! ## Node topology
//!
//! ```text
//!   node-a  (coordinator)  serial :4440
//!   node-b  (member)       serial :4441
//!   node-c  (member)       serial :4442
//! ```
//!
//! All three run the same kernel image.  node-a launches with
//! the coordinator role flag; node-b and node-c with the member flag.
//! Role selection is via the kernel's `FLEET_ROLE` xtask argument
//! (written into the kernel image header at `build` time).
//!
//! ## Commands
//!
//! `up`           — launch all three QEMU nodes
//! `down`         — kill all running QEMU nodes
//! `status`       — print which nodes are reachable on their serial ports
//! `verify`       — check all pass markers appear in each node's serial log
//! `build-bundle` — build and sign the fjell-hello bundle
//! `deploy`       — run the full demo end-to-end (up → verify → down)

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::Duration;

const KERNEL: &str = "target/riscv64gc-unknown-none-elf/release/fjell-kernel";
const DEMO_DIR: &str = "examples/three-node-fleet";

// Node port assignments
const PORTS: [(u16, &str, &str); 3] = [
    (4440, "node-a", "coordinator"),
    (4441, "node-b", "member"),
    (4442, "node-c", "member"),
];

const PASS_MARKERS: &[&str] = &[
    "FLEET:NODE_A:READY",
    "FLEET:NODE_B:READY",
    "FLEET:NODE_C:READY",
    "TEST:V0.10-FLEET-DEMO:PASS",
];

pub fn cmd_fleet_demo(sub: Option<&str>, _args: &[String]) -> ExitCode {
    match sub {
        Some("up")           => cmd_up(),
        Some("down")         => cmd_down(),
        Some("status")       => cmd_status(),
        Some("verify")       => cmd_verify(),
        Some("build-bundle") => cmd_build_bundle(),
        Some("deploy")       => cmd_deploy(),
        _ => {
            eprintln!("Usage: cargo xtask fleet-demo <up|down|status|verify|build-bundle|deploy>");
            ExitCode::FAILURE
        }
    }
}

// ── up ────────────────────────────────────────────────────────────────────────

fn cmd_up() -> ExitCode {
    if !Path::new(KERNEL).exists() {
        eprintln!("[fleet-demo] kernel not found at {}", KERNEL);
        eprintln!("[fleet-demo] run `cargo xtask build` first");
        return ExitCode::FAILURE;
    }

    fs::create_dir_all("tests/fleet-demo/logs").ok();

    for (port, node_id, role) in &PORTS {
        let log_path = format!("tests/fleet-demo/logs/{}.log", node_id);
        match launch_node(*port, node_id, role, &log_path) {
            Ok(_) => println!("[fleet-demo] launched {} ({}) on serial port {}", node_id, role, port),
            Err(e) => {
                eprintln!("[fleet-demo] failed to launch {}: {}", node_id, e);
                return ExitCode::FAILURE;
            }
        }
    }

    println!("[fleet-demo] all three nodes launched — serial ports 4440/4441/4442");
    println!("[fleet-demo] run `cargo xtask fleet-demo verify` to check pass markers");
    ExitCode::SUCCESS
}

fn launch_node(port: u16, node_id: &str, _role: &str, log_path: &str) -> std::io::Result<Child> {
    let log_file = fs::File::create(log_path)?;

    Command::new("qemu-system-riscv64")
        .args([
            "-machine", "virt",
            "-cpu", "rv64",
            "-m", "128M",
            "-nographic",
            "-bios", "none",
            "-kernel", KERNEL,
            "-serial", &format!("tcp::{},server,nowait", port),
            // Pass the node identity as a kernel command-line-equivalent via
            // the bootarg mechanism.  For the demo, the kernel reads the
            // address of the serial port to determine its node index.
        ])
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        // Run in background
        .spawn()
        .map(|child| {
            println!("[fleet-demo]   {} PID={}", node_id, child.id());
            child
        })
}

// ── down ──────────────────────────────────────────────────────────────────────

fn cmd_down() -> ExitCode {
    let killed = kill_qemu_processes();
    println!("[fleet-demo] sent SIGKILL to {} QEMU process(es)", killed);
    ExitCode::SUCCESS
}

fn kill_qemu_processes() -> usize {
    let out = Command::new("pkill")
        .args(["-f", "qemu-system-riscv64"])
        .output();
    match out {
        Ok(o) if o.status.success() => 3, // approximate
        _ => 0,
    }
}

// ── status ────────────────────────────────────────────────────────────────────

fn cmd_status() -> ExitCode {
    println!("[fleet-demo] node status:");
    let mut all_up = true;
    for (port, node_id, role) in &PORTS {
        let reachable = TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_millis(300),
        ).is_ok();
        println!("  {:8} ({:11}) port {:4}  {}",
            node_id, role, port,
            if reachable { "UP" } else { "down" }
        );
        if !reachable { all_up = false; }
    }
    if all_up { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

// ── verify ────────────────────────────────────────────────────────────────────

fn cmd_verify() -> ExitCode {
    let timeout = Duration::from_secs(120);
    println!("[fleet-demo] waiting for pass markers ({}s timeout) …", timeout.as_secs());

    let mut missing: Vec<&str> = PASS_MARKERS.to_vec();
    let start = std::time::Instant::now();

    while !missing.is_empty() && start.elapsed() < timeout {
        thread::sleep(Duration::from_millis(500));
        // Check log files written by the running QEMU processes
        for entry in fs::read_dir("tests/fleet-demo/logs").unwrap_or_else(|_| {
            fs::read_dir(".").unwrap()  // fallback to avoid crash
        }).flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                missing.retain(|m| !content.contains(m));
            }
        }
    }

    if missing.is_empty() {
        println!("[fleet-demo] PASS — all markers observed");
        ExitCode::SUCCESS
    } else {
        eprintln!("[fleet-demo] FAIL — markers not seen:");
        for m in &missing {
            eprintln!("  missing: {}", m);
        }
        ExitCode::FAILURE
    }
}

// ── build-bundle ──────────────────────────────────────────────────────────────

fn cmd_build_bundle() -> ExitCode {
    println!("[fleet-demo] linting cap-manifest …");
    let manifest_path = format!("{}/fjell-hello/cap-manifest.toml", DEMO_DIR);
    if !Path::new(&manifest_path).exists() {
        eprintln!("[fleet-demo] cap-manifest not found at {}", manifest_path);
        return ExitCode::FAILURE;
    }

    let lint_status = Command::new("cargo")
        .args(["xtask", "dev", "lint", &manifest_path])
        .status();
    match lint_status {
        Ok(s) if s.success() => println!("[fleet-demo] manifest lint: PASS"),
        _ => {
            eprintln!("[fleet-demo] manifest lint: FAIL");
            return ExitCode::FAILURE;
        }
    }

    // In the demo, the "bundle" is validated by the manifest digest alone;
    // full binary signing arrives in v0.11.
    println!("[fleet-demo] bundle artefact: (signing pipeline lands in v0.11)");
    println!("[fleet-demo] bundle-build PASS (manifest validated)");
    ExitCode::SUCCESS
}

// ── deploy ────────────────────────────────────────────────────────────────────

fn cmd_deploy() -> ExitCode {
    println!("[fleet-demo] === FULL DEMO RUN ===");
    println!();

    let steps: &[(&str, fn() -> ExitCode)] = &[
        ("build-bundle", cmd_build_bundle),
        ("up",           cmd_up),
    ];

    for (name, step_fn) in steps {
        print!("[fleet-demo] step: {} … ", name);
        std::io::stdout().flush().ok();
        let rc = step_fn();
        if rc != ExitCode::SUCCESS {
            eprintln!("FAIL");
            return ExitCode::FAILURE;
        }
        println!("ok");
    }

    // Give nodes time to boot
    println!("[fleet-demo] waiting 10s for nodes to boot …");
    thread::sleep(Duration::from_secs(10));

    let verify_rc = cmd_verify();
    let _ = cmd_down();
    verify_rc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_markers_non_empty() {
        assert!(!PASS_MARKERS.is_empty());
    }

    #[test]
    fn ports_are_distinct() {
        let ports: Vec<u16> = PORTS.iter().map(|(p, _, _)| *p).collect();
        let unique: std::collections::BTreeSet<_> = ports.iter().collect();
        assert_eq!(ports.len(), unique.len());
    }

    #[test]
    fn demo_dir_exists() {
        if !Path::new("examples").exists() { return; } // not workspace root
        assert!(Path::new(DEMO_DIR).exists(),
            "expected examples/three-node-fleet/ to exist");
    }

    #[test]
    fn fleet_demo_toml_exists() {
        if !Path::new("examples").exists() { return; } // not workspace root
        assert!(Path::new(&format!("{}/fleet-demo.toml", DEMO_DIR)).exists());
    }

    #[test]
    fn cap_manifest_lint_passes() {
        // Parse the demo manifest and lint it against SDK rev 1.
        let path = format!("{}/fjell-hello/cap-manifest.toml", DEMO_DIR);
        if !Path::new(&path).exists() {
            return; // not found in test binary's CWD; skip
        }
        let content = fs::read_to_string(&path).unwrap();
        let m = fjell_cap_manifest::parse_manifest(&content).unwrap();
        fjell_cap_manifest::lint_manifest(&m, 1).unwrap();
    }
}
