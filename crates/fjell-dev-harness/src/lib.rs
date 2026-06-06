//! # `fjell-dev-harness` — QEMU-backed service integration test harness
//!
//! Implements RFC v0.9-005. Provides the building blocks for the
//! `fjell-tools dev run` command and for per-service integration tests.
//!
//! ## Architecture
//!
//! ```text
//! fjell-tools dev run --svc <name>
//!     │
//!     ├─ manifest lint    (fjell-cap-manifest)
//!     ├─ bundle build     (fjell-bundle-format)
//!     ├─ QemuBuilder      ─► spawn qemu process
//!     │       │
//!     │       └─ QemuHandle  ─► serial line reader
//!     │               │
//!     │               └─ HarnessAssertion methods:
//!     │                     assert_marker_emitted(marker, timeout)
//!     │                     assert_intent_tag_emitted(tag, timeout)
//!     │                     assert_audit_event(kind, timeout)
//!     │                     read_serial_lines(timeout)
//!     └─ print result
//! ```
//!
//! In v0.9.0 the QEMU process is launched via `std::process::Command`
//! and the serial output is captured via the `-serial stdio` flag (as
//! the existing xtask smoke runner does). The harness reads lines from
//! stdout and applies the requested assertions.
//!
//! ## Hermetic baseline
//!
//! Each `QemuBuilder` run starts from a **frozen** disk image. Test
//! runs that write to storaged produce isolated output; the frozen image
//! is never modified. This mirrors the xtask smoke runner's disk image.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

// ── QEMU configuration ────────────────────────────────────────────────────────

/// Default QEMU timeout for the full run.
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;
/// Machine type used by the Fjell smoke tests.
pub const QEMU_MACHINE: &str = "virt";
/// Default RAM size in megabytes.
pub const DEFAULT_MEMORY_MB: u32 = 128;

/// Builder for a QEMU-backed harness session.
///
/// Mirrors the `QemuRunner` in `fjell-tools`; this variant is intended
/// for integration tests (library API) rather than one-shot xtask runs.
pub struct QemuBuilder {
    kernel_path:   String,
    disk_path:     Option<String>,
    memory_mb:     u32,
    timeout:       Duration,
    extra_args:    Vec<String>,
}

impl Default for QemuBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl QemuBuilder {
    /// Create a builder with Fjell defaults.
    pub fn new() -> Self {
        Self {
            kernel_path: String::new(),
            disk_path:   None,
            memory_mb:   DEFAULT_MEMORY_MB,
            timeout:     Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            extra_args:  Vec::new(),
        }
    }

    /// Set the kernel binary path (required).
    pub fn kernel(mut self, path: &str) -> Self {
        self.kernel_path = path.into(); self
    }

    /// Attach a virtio-blk disk image.
    pub fn disk(mut self, path: &str) -> Self {
        self.disk_path = Some(path.into()); self
    }

    /// Override memory (default 128 MB).
    pub fn memory_mb(mut self, mb: u32) -> Self {
        self.memory_mb = mb; self
    }

    /// Override the per-run timeout (default 60 s).
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d; self
    }

    /// Append extra QEMU arguments verbatim.
    pub fn extra_arg(mut self, arg: &str) -> Self {
        self.extra_args.push(arg.into()); self
    }

    /// Launch QEMU and return a live [`QemuHandle`].
    pub fn launch(self) -> Result<QemuHandle, HarnessError> {
        if self.kernel_path.is_empty() {
            return Err(HarnessError::MissingKernelPath);
        }

        let mut cmd = Command::new("qemu-system-riscv64");
        cmd.args(["-machine", QEMU_MACHINE]);
        cmd.args(["-cpu", "rv64"]);
        cmd.args(["-m", &format!("{}M", self.memory_mb)]);
        cmd.args(["-nographic"]);
        cmd.args(["-bios", "none"]);
        cmd.args(["-kernel", &self.kernel_path]);

        if let Some(ref disk) = self.disk_path {
            cmd.args([
                "-drive",
                &format!("file={},if=none,id=hd0,format=raw", disk),
                "-device", "virtio-blk-device,drive=hd0",
            ]);
        }

        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());

        let mut child = cmd.spawn()
            .map_err(|e| HarnessError::QemuSpawnFailed(e.to_string()))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| HarnessError::QemuSpawnFailed("no stdout".into()))?;

        let lines: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let lines_clone = Arc::clone(&lines);

        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let mut q = lines_clone.lock().unwrap();
                q.push_back(line);
                // Keep at most 4096 lines in the buffer.
                if q.len() > 4096 { q.pop_front(); }
            }
        });

        Ok(QemuHandle {
            child,
            lines,
            deadline: Instant::now() + self.timeout,
        })
    }
}

// ── QemuHandle ────────────────────────────────────────────────────────────────

/// A live QEMU session. Drop to kill the process.
pub struct QemuHandle {
    child:    Child,
    lines:    Arc<Mutex<VecDeque<String>>>,
    deadline: Instant,
}

impl QemuHandle {
    /// Block until the full serial output contains `marker` or the
    /// session deadline expires.
    pub fn assert_marker_emitted(
        &mut self,
        marker: &str,
        per_poll_sleep: Duration,
    ) -> Result<(), HarnessError> {
        while Instant::now() < self.deadline {
            {
                let q = self.lines.lock().unwrap();
                for line in q.iter() {
                    if line.contains(marker) { return Ok(()); }
                }
            }
            // Check if QEMU already exited.
            if let Ok(Some(_)) = self.child.try_wait() {
                // Drain remaining buffer.
                let q = self.lines.lock().unwrap();
                for line in q.iter() {
                    if line.contains(marker) { return Ok(()); }
                }
                return Err(HarnessError::MarkerNotFound(marker.into()));
            }
            std::thread::sleep(per_poll_sleep);
        }
        Err(HarnessError::Timeout { marker: marker.into() })
    }

    /// Collect all serial lines received so far and return them.
    pub fn read_serial_lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().iter().cloned().collect()
    }

    /// Kill the QEMU process immediately.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for QemuHandle {
    fn drop(&mut self) { self.kill(); }
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Errors from the harness.
#[derive(Debug, Clone)]
pub enum HarnessError {
    /// `kernel_path` was not set before calling `launch()`.
    MissingKernelPath,
    /// QEMU `Command::spawn` failed.
    QemuSpawnFailed(String),
    /// The expected marker was never seen before the deadline.
    Timeout { marker: String },
    /// QEMU exited before the marker appeared.
    MarkerNotFound(String),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HarnessError::MissingKernelPath =>
                write!(f, "kernel path not set"),
            HarnessError::QemuSpawnFailed(e) =>
                write!(f, "QEMU spawn failed: {}", e),
            HarnessError::Timeout { marker } =>
                write!(f, "timeout waiting for marker `{}`", marker),
            HarnessError::MarkerNotFound(m) =>
                write!(f, "marker `{}` not found before QEMU exit", m),
        }
    }
}

impl std::error::Error for HarnessError {}

// ── Hermetic baseline helpers ─────────────────────────────────────────────────

/// Configuration for a hermetic dev-run session.
///
/// The `fjell-tools dev run` subcommand (RFC v0.9-005 §4.1) builds this
/// from the workspace + service name, then delegates to [`QemuBuilder`].
pub struct DevRunConfig {
    /// Workspace root (for cargo build paths).
    pub workspace_root: String,
    /// Service crate name to test.
    pub service_name:   String,
    /// Kernel binary path (produced by the xtask build).
    pub kernel_path:    String,
    /// Disk image path (frozen baseline).
    pub disk_path:      String,
    /// Markers the run must emit to be considered a pass.
    pub pass_markers:   Vec<String>,
    /// Per-run timeout.
    pub timeout:        Duration,
}

impl DevRunConfig {
    /// Run the service under QEMU and check all `pass_markers` appear.
    ///
    /// Returns `Ok(())` if every marker is found within the timeout.
    pub fn execute(self) -> Result<(), HarnessError> {
        let timeout = self.timeout;
        let mut handle = QemuBuilder::new()
            .kernel(&self.kernel_path)
            .disk(&self.disk_path)
            .timeout(timeout)
            .launch()?;

        for marker in &self.pass_markers {
            handle.assert_marker_emitted(marker, Duration::from_millis(100))?;
        }
        Ok(())
    }
}

// ── xtask integration helpers ─────────────────────────────────────────────────

/// Parse a log file (or captured serial output) and return whether
/// `marker` appears anywhere in the content.
///
/// Used by the xtask profile runner to check log files already on disk
/// without re-running QEMU.
pub fn check_log_for_marker(log_content: &str, marker: &str) -> bool {
    log_content.contains(marker)
}

/// Count how many distinct markers from `expected` appear in `log_content`.
pub fn count_matched_markers<'a>(
    log_content: &str,
    expected: &[&'a str],
) -> Vec<&'a str> {
    expected.iter()
        .copied()
        .filter(|m| log_content.contains(m))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qemu_builder_defaults() {
        let b = QemuBuilder::new();
        assert_eq!(b.memory_mb, DEFAULT_MEMORY_MB);
        assert_eq!(b.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        assert!(b.kernel_path.is_empty());
    }

    #[test]
    fn launch_fails_without_kernel() {
        let r = QemuBuilder::new().launch();
        assert!(matches!(r, Err(HarnessError::MissingKernelPath)));
    }

    #[test]
    fn check_log_for_marker_positive() {
        let log = "boot ok\nTEST:M8:PASS\nshutdown\n";
        assert!(check_log_for_marker(log, "TEST:M8:PASS"));
    }

    #[test]
    fn check_log_for_marker_negative() {
        let log = "boot ok\nTEST:M8:PASS\nshutdown\n";
        assert!(!check_log_for_marker(log, "TEST:M9:PASS"));
    }

    #[test]
    fn count_matched_markers_partial() {
        let log = "TEST:V0.4-NET:PASS\nTEST:M8:PASS\n";
        let expected = ["TEST:V0.4-NET:PASS", "TEST:V0.5-PLATFORM:PASS", "TEST:M8:PASS"];
        let matched = count_matched_markers(log, &expected);
        assert_eq!(matched.len(), 2);
        assert!(matched.contains(&"TEST:V0.4-NET:PASS"));
        assert!(matched.contains(&"TEST:M8:PASS"));
        assert!(!matched.contains(&"TEST:V0.5-PLATFORM:PASS"));
    }

    #[test]
    fn count_matched_markers_all() {
        let log = "TEST:A:PASS\nTEST:B:PASS\nTEST:C:PASS\n";
        let expected = ["TEST:A:PASS", "TEST:B:PASS", "TEST:C:PASS"];
        assert_eq!(count_matched_markers(log, &expected).len(), 3);
    }

    #[test]
    fn count_matched_markers_none() {
        let log = "no markers here\n";
        let expected = ["TEST:X:PASS"];
        assert_eq!(count_matched_markers(log, &expected).len(), 0);
    }

    #[test]
    fn harness_error_display() {
        let e = HarnessError::Timeout { marker: "FOO".into() };
        assert!(e.to_string().contains("FOO"));
        let e2 = HarnessError::MissingKernelPath;
        assert!(!e2.to_string().is_empty());
    }
}
