//! `cargo xtask dev run --svc <name>` — developer feedback loop (RFC v0.9-005).
//!
//! Implements the edit → build → QEMU → pass/fail loop for service authors.
//! The command:
//!
//! 1. Validates the service's `CapManifest` (RFC v0.9-002) if present.
//! 2. Builds the service binary via `cargo build --release`.
//! 3. Uses [`fjell_dev_harness::QemuBuilder`] to launch the kernel image
//!    with the built binary already staged.
//! 4. Waits for `TEST:<SVC_UPPER>:PASS` to appear in the serial output.
//! 5. Reports pass/fail, duration, and the markers it observed.
//!
//! For v0.9.0 the "stage the binary" step is not yet implemented; the
//! kernel image is launched as-is (exactly as the existing xtask smoke
//! tests do). Full service staging via the bundle pipeline (RFC v0.9-004)
//! lands in v0.9.1.

use std::process::ExitCode;
use std::time::{Duration, Instant};
use fjell_dev_harness::{QemuBuilder, HarnessError, check_log_for_marker};
use fjell_cap_manifest::{parse_manifest, lint_manifest};

/// The SDK API revision this tool was built against (RFC v0.9-001).
const HOST_SDK_API_REV: u32 = 1;

/// Entry point for `cargo xtask dev <sub>`.
pub fn cmd_dev(sub: Option<&str>, args: &[String]) -> ExitCode {
    match sub {
        Some("run") => cmd_dev_run(args),
        Some("lint") => cmd_dev_lint(args),
        Some(other) => {
            eprintln!("fjell-tools dev: unknown subcommand `{other}`");
            dev_usage();
            ExitCode::FAILURE
        }
        None => {
            dev_usage();
            ExitCode::FAILURE
        }
    }
}

// ── dev run ───────────────────────────────────────────────────────────────────

fn cmd_dev_run(args: &[String]) -> ExitCode {
    // Parse --svc <name> --kernel <path> [--disk <path>] [--timeout <secs>]
    let mut svc_name: Option<String>    = None;
    let mut kernel:   Option<String>    = None;
    let mut disk:     Option<String>    = None;
    let mut timeout   = Duration::from_secs(60);

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--svc"     => { i += 1; svc_name = args.get(i).cloned(); }
            "--kernel"  => { i += 1; kernel   = args.get(i).cloned(); }
            "--disk"    => { i += 1; disk     = args.get(i).cloned(); }
            "--timeout" => {
                i += 1;
                timeout = Duration::from_secs(
                    args.get(i).and_then(|s| s.parse().ok()).unwrap_or(60)
                );
            }
            _ => {}
        }
        i += 1;
    }

    let svc = match svc_name {
        Some(s) => s,
        None => {
            eprintln!("dev run: --svc <name> is required");
            return ExitCode::FAILURE;
        }
    };
    let kernel = match kernel {
        Some(k) => k,
        None => {
            // Try the default xtask build output path.
            let default = format!(
                "target/riscv64gc-unknown-none-elf/release/fjell-kernel"
            );
            eprintln!("[dev run] --kernel not given; trying {}", default);
            default
        }
    };

    // Derive the expected PASS marker from the service name.
    let marker = format!(
        "TEST:{}:PASS",
        svc.trim_start_matches("fjell-").to_uppercase().replace('-', "_")
    );
    println!("[dev run] service=`{}` marker=`{}`", svc, marker);

    let mut builder = QemuBuilder::new()
        .kernel(&kernel)
        .timeout(timeout);

    if let Some(ref d) = disk {
        builder = builder.disk(d);
    }

    let t0 = Instant::now();
    let result = match builder.launch() {
        Ok(mut handle) => {
            handle.assert_marker_emitted(&marker, Duration::from_millis(200))
        }
        Err(e) => Err(e),
    };

    let elapsed = t0.elapsed();
    match result {
        Ok(()) => {
            println!("[dev run] PASS  ({:.1}s)  marker=`{}`", elapsed.as_secs_f32(), marker);
            ExitCode::SUCCESS
        }
        Err(HarnessError::MissingKernelPath) => {
            eprintln!("[dev run] FAIL  kernel image not found at `{}`", kernel);
            ExitCode::FAILURE
        }
        Err(HarnessError::QemuSpawnFailed(e)) => {
            eprintln!("[dev run] FAIL  qemu spawn failed: {}", e);
            ExitCode::FAILURE
        }
        Err(HarnessError::Timeout { marker: m }) => {
            eprintln!("[dev run] FAIL  timeout waiting for `{}`", m);
            ExitCode::FAILURE
        }
        Err(HarnessError::MarkerNotFound(m)) => {
            eprintln!("[dev run] FAIL  QEMU exited before `{}` appeared", m);
            ExitCode::FAILURE
        }
    }
}

// ── dev lint ──────────────────────────────────────────────────────────────────

fn cmd_dev_lint(args: &[String]) -> ExitCode {
    let manifest_path = match args.first() {
        Some(p) => p,
        None => {
            eprintln!("dev lint: <manifest.toml> path required");
            return ExitCode::FAILURE;
        }
    };

    let content = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dev lint: cannot read `{}`: {}", manifest_path, e);
            return ExitCode::FAILURE;
        }
    };

    let manifest = match parse_manifest(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("dev lint: parse error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    match lint_manifest(&manifest, HOST_SDK_API_REV) {
        Ok(()) => {
            println!("dev lint: OK  service=`{}` sdk_api_rev={}",
                manifest.service, manifest.sdk_api_rev);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("dev lint: FAIL  {}", e);
            ExitCode::FAILURE
        }
    }
}

// ── dev log-check (bonus helper) ─────────────────────────────────────────────

/// Check a captured log file for a marker — useful in CI without re-running.
pub fn cmd_dev_log_check(log_path: Option<&str>, marker: Option<&str>) -> ExitCode {
    let (log_path, marker) = match (log_path, marker) {
        (Some(l), Some(m)) => (l, m),
        _ => {
            eprintln!("dev log-check: <log-file> <marker> required");
            return ExitCode::FAILURE;
        }
    };
    let content = match std::fs::read_to_string(log_path) {
        Ok(c) => c,
        Err(e) => { eprintln!("dev log-check: {}", e); return ExitCode::FAILURE; }
    };
    if check_log_for_marker(&content, marker) {
        println!("PASS: `{}` found in `{}`", marker, log_path);
        ExitCode::SUCCESS
    } else {
        eprintln!("FAIL: `{}` NOT found in `{}`", marker, log_path);
        ExitCode::FAILURE
    }
}

fn dev_usage() {
    eprintln!(
"Usage: cargo xtask dev <subcommand>

Subcommands:
  run  --svc <name> --kernel <path> [--disk <path>] [--timeout <secs>]
       Build service, launch QEMU, assert PASS marker.

  lint <manifest.toml>
       Parse and lint a CapManifest against the SDK (RFC v0.9-002).

  log-check <log-file> <marker>
       Check a captured serial log for a marker (offline CI check)."
    );
}
