//! Profile-driven QEMU runner.
//!
//! A *profile* is a small declarative description of one QEMU run:
//! kernel, disk image, timeout, expected markers, run id.  Both the
//! smoke runner (`qemu-test`) and the negative runner (`qemu-negative`)
//! are thin wrappers around `run_profile`.
//!
//! Profiles live under `tests/qemu/profiles/<name>.toml`.  v0.1.1 ships
//! a minimal hand-parsed TOML reader to avoid pulling a heavy dep into
//! the xtask crate.  The supported subset is enough for the v0.1.x
//! profiles; v0.2 may switch to `toml` if profiles grow.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::qemu::{KERNEL_ELF, build_all};

/// One QEMU run.  Loaded from a profile file or built inline by the
/// smoke runner.
pub struct Profile {
    pub name: String,
    /// Path to the kernel ELF, relative to the workspace root.
    pub kernel: PathBuf,
    /// Path to the disk image to attach (created if missing).
    pub disk: PathBuf,
    /// Hard timeout in seconds for the `timeout(1)` wrapper.
    pub timeout_secs: u32,
    /// Markers that must appear in the captured serial log for the run
    /// to count as a pass.  An empty list is allowed for placeholder
    /// profiles per RFC 025 §"chicken-and-egg" exemption.
    pub expected_markers: Vec<String>,
    /// Optional extra QEMU args beyond the defaults.
    pub extra_args: Vec<String>,
    /// RFC-0.25-001 (Demonstration 6): once this marker appears in the
    /// captured output, write `.1`'s bytes to QEMU's stdin — `-nographic`
    /// wires the guest's UART0 RX to the host process's stdin by default,
    /// so this simulates a character typed at the console. `None` for every
    /// other profile: they keep the plain `Command::output()` path
    /// unchanged (no piped stdin, no reader threads).
    pub inject_after_marker: Option<(String, Vec<u8>)>,
}

impl Profile {
    /// Build the default smoke profile for one milestone (`m1`..`m8`).
    pub fn smoke(milestone: &str, marker: &str) -> Self {
        Self {
            name: format!("smoke-{milestone}"),
            kernel: PathBuf::from(KERNEL_ELF),
            disk: PathBuf::from("fjell-disk.img"),
            timeout_secs: 60,
            expected_markers: vec![marker.to_string()],
            extra_args: vec![],
            inject_after_marker: None,
        }
    }
}

/// Where artefacts are written for one run.
pub struct ArtifactDir(pub PathBuf);

impl ArtifactDir {
    /// `tests/qemu/artifacts/<run-id>/`.  Created on demand.
    pub fn for_run(name: &str) -> Self {
        let dir = PathBuf::from("tests/qemu/artifacts").join(name);
        let _ = fs::create_dir_all(&dir);
        ArtifactDir(dir)
    }
    pub fn join(&self, p: &str) -> PathBuf {
        self.0.join(p)
    }
    /// `tests/qemu/artifacts/<profile>/runs/<run-id>/` — RFC-0.27-004 R3.
    /// Unlike the flat `serial.log` above (still written, still overwritten
    /// by the next run of this profile — that stays true, unchanged), this
    /// path is unique per invocation, so a promotable copy survives the
    /// next tier running the same profile.
    pub fn run_dir(&self, run_id: &str) -> PathBuf {
        let dir = self.0.join("runs").join(run_id);
        let _ = fs::create_dir_all(&dir);
        dir
    }
}

/// `YYYYMMDD-HHMMSS`, same construction as `test_all::timestamp_str` —
/// duplicated rather than shared (small, self-contained, and the two
/// modules stay independent by this project's own convention; see
/// `standards_mapping::normalise`'s doc-comment for the same call made
/// elsewhere in this codebase).
fn run_id_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_secs();
    let s = secs % 86400;
    let d = secs / 86400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    let days = d + 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        year, month, day, hh, mm, ss
    )
}

/// The commit `HEAD` resolved to at run time, full 40-hex sha — a short sha
/// would be ambiguous for the ancestry check `evidence promote` and the
/// `evidence` subcheck both perform on it later. `"unknown"` if `git` is
/// unavailable or the tree is not a git checkout; provenance carries this
/// literally rather than silently omitting the field.
fn git_head_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Whether any tracked file differs from `HEAD` at run time — the
/// mechanical proxy for "was this build instrumented" (D3). Not proof by
/// itself (a dirty tree can be unrelated to the binary that produced this
/// log, and a clean tree does not rule out instrumentation from an already
/// -committed-then-amended state), so `evidence promote` still requires a
/// human `--instrumented` answer rather than trusting this alone — but a
/// mismatch between the two is worth a promoter's attention, which is why
/// this is recorded at all. Fails safe: if `git status` cannot be run,
/// reports dirty rather than silently claiming a clean tree.
fn git_tree_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(true)
}

/// Entry point: `cargo xtask qemu-run --profile <name>`.
pub fn cmd_qemu_run(profile_name: Option<&str>) -> ExitCode {
    let name = match profile_name {
        Some(n) => n,
        None => {
            eprintln!("Usage: cargo xtask qemu-run --profile <name>");
            return ExitCode::FAILURE;
        }
    };
    let profile = match load_profile(name) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[xtask] qemu-run: {e}");
            return ExitCode::FAILURE;
        }
    };
    run_profile(&profile)
}

/// The core run loop, shared by smoke / negative / explicit run.
///
/// 1. Build the kernel if missing.
/// 2. (Re)create the disk image.
/// 3. Run QEMU with `timeout(N) qemu-system-riscv64 ...`.
/// 4. Capture combined stdout + stderr to `serial.log`.
/// 5. Write `qemu-command.txt` and `expected-markers.txt`.
/// 6. Assert every expected marker via `qemu-log-check::log_check`.
/// 7. Write `result-summary.txt` and return the verdict.
pub fn run_profile(p: &Profile) -> ExitCode {
    let art = ArtifactDir::for_run(&p.name);
    println!(
        "[xtask] running profile `{}` (timeout {}s)",
        p.name, p.timeout_secs
    );

    // Build kernel if needed (smoke profiles always build_all; the
    // arg `--profile` path assumes the kernel is already built).
    if !Path::new(&p.kernel).exists() {
        eprintln!("[xtask] kernel ELF missing — running build_all()");
        let _ = build_all();
    }

    // (Re)create disk image — required by virtio-blk smoke path.
    // Pure-Rust fallback: a raw QEMU disk image is just a zero-filled
    // sparse file. This avoids requiring qemu-img (a separate package on
    // Arch Linux: `qemu-img`; on Debian/Ubuntu: `qemu-utils`).
    if p.disk.exists() {
        let _ = fs::remove_file(&p.disk);
    }
    if let Err(e) = fs::File::create(&p.disk).and_then(|f| f.set_len(16 * 1024 * 1024)) {
        eprintln!(
            "[xtask] WARNING: could not create disk image {}: {e}",
            p.disk.display()
        );
    }

    let kernel_str = p.kernel.to_string_lossy().to_string();
    let disk_str = p.disk.to_string_lossy().to_string();
    let drive_arg = format!("file={disk_str},format=raw,if=none,id=hd0");

    // Build the command vector once so we can both run it and persist
    // it to qemu-command.txt.
    let mut argv: Vec<String> = vec![
        format!("{}", p.timeout_secs),
        "qemu-system-riscv64".into(),
        "-machine".into(),
        "virt".into(),
        "-bios".into(),
        "none".into(),
        "-nographic".into(),
        "-kernel".into(),
        kernel_str.clone(),
        "-drive".into(),
        drive_arg.clone(),
        "-device".into(),
        "virtio-blk-device,drive=hd0".into(),
    ];
    argv.extend(p.extra_args.iter().cloned());

    let _ = fs::write(art.join("qemu-command.txt"), argv.join(" ").as_bytes());
    let _ = fs::write(
        art.join("expected-markers.txt"),
        p.expected_markers.join("\n").as_bytes(),
    );

    let combined = match &p.inject_after_marker {
        None => {
            let output = Command::new("timeout")
                .args(&argv[..])
                .output()
                .expect("failed to run qemu-system-riscv64");
            let mut combined = output.stdout.clone();
            combined.extend_from_slice(&output.stderr);
            combined
        }
        Some((marker, inject_bytes)) => {
            run_with_stdin_injection(&argv, marker.as_bytes(), inject_bytes)
        }
    };

    let log_path = art.join("serial.log");
    let _ = fs::write(&log_path, &combined);

    // RFC-0.27-004 R3: also retain this run under a run-id-keyed directory,
    // so the flat path above being overwritten by the *next* run of this
    // profile no longer destroys the only copy — `evidence promote` reads
    // from here, not from the flat path. Provenance captured now, at run
    // time, because the commit sha and dirty-tree state are true facts only
    // at this moment; asking for them later at promotion time would be
    // asking the wrong point in history (D2/D3).
    let run_id = run_id_now();
    let run_dir = art.run_dir(&run_id);
    let _ = fs::write(run_dir.join("serial.log"), &combined);
    let _ = fs::write(run_dir.join("qemu-command.txt"), argv.join(" ").as_bytes());
    let run_info = format!(
        "run_id = {run_id}\nprofile = {}\ncommit_sha = {}\ntree_dirty_at_run_time = {}\ncommand = {}\n",
        p.name,
        git_head_sha(),
        git_tree_dirty(),
        argv.join(" "),
    );
    let _ = fs::write(run_dir.join("run-info.txt"), run_info.as_bytes());

    // Empty marker list = placeholder profile (no cases registered).
    if p.expected_markers.is_empty() {
        let _ = fs::write(
            art.join("result-summary.txt"),
            b"PASS (placeholder; no expected markers)\n",
        );
        println!(
            "[xtask] profile `{}` is a placeholder — no markers \
                  to check (RFC 025 §chicken-and-egg). PASS.",
            p.name
        );
        return ExitCode::SUCCESS;
    }

    // Check every expected marker.
    let mut all_ok = true;
    for marker in &p.expected_markers {
        let ok = combined
            .windows(marker.len())
            .any(|w| w == marker.as_bytes());
        if !ok {
            eprintln!(
                "[xtask] missing marker `{marker}` in {}",
                log_path.display()
            );
            all_ok = false;
        }
    }

    // Fail-closed (architect review v0.19 RB-01): the run FAILS if the serial
    // log contains any harness-failure or panic marker, even when every
    // expected marker matched. A wrong-error or unexpected-success result must
    // never produce a green profile.
    const FORBIDDEN: &[&str] = &[
        "NEG:HARNESS:WRONG_ERROR",
        "NEG:HARNESS:UNEXPECTED_OK",
        "TEST:FAIL",
        "kernel panic",
        "panicked at",
    ];
    for bad in FORBIDDEN {
        let found = combined.windows(bad.len()).any(|w| w == bad.as_bytes());
        if found {
            eprintln!(
                "[xtask] FORBIDDEN marker `{bad}` present in {}",
                log_path.display()
            );
            all_ok = false;
        }
    }
    let summary = if all_ok { "PASS\n" } else { "FAIL\n" };
    let _ = fs::write(art.join("result-summary.txt"), summary);

    if all_ok {
        println!(
            "[xtask] profile `{}` PASS ({} marker(s) matched) ✓",
            p.name,
            p.expected_markers.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "[xtask] profile `{}` FAIL — see {}",
            p.name,
            log_path.display()
        );
        // Print the last 60 lines of serial.log directly so failures are
        // visible without opening a separate file (RFC-v0.7.1-003 §smoke).
        if let Ok(log_bytes) = fs::read(&log_path) {
            let log_text = String::from_utf8_lossy(&log_bytes);
            let lines: Vec<&str> = log_text.lines().collect();
            let tail = if lines.len() > 60 {
                &lines[lines.len() - 60..]
            } else {
                &lines[..]
            };
            eprintln!("[xtask] --- serial.log tail ({} lines) ---", tail.len());
            for line in tail {
                eprintln!("[serial] {line}");
            }
            eprintln!("[xtask] --- end serial.log ---");
        }
        ExitCode::FAILURE
    }
}

/// Run `timeout <argv...>` with piped stdio, writing `inject_bytes` to the
/// child's stdin the first time `marker` appears in its combined
/// stdout+stderr (RFC-0.25-001 Demonstration 6: simulates a character typed
/// at the QEMU console, since `-nographic` wires UART0's RX to host stdin).
///
/// Returns the full combined output, same shape as the plain
/// `Command::output()` path (stdout bytes followed by stderr bytes would
/// lose ordering across the two streams; this merges them as they actually
/// arrive instead, which is more accurate, not less).
fn run_with_stdin_injection(argv: &[String], marker: &[u8], inject_bytes: &[u8]) -> Vec<u8> {
    let mut child = Command::new("timeout")
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn qemu-system-riscv64");

    let child_stdin = child.stdin.take().expect("piped stdin");
    let mut child_stdout = child.stdout.take().expect("piped stdout");
    let mut child_stderr = child.stderr.take().expect("piped stderr");

    let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let injected = Arc::new(AtomicBool::new(false));

    let stdout_thread = {
        let buf = Arc::clone(&buf);
        let injected = Arc::clone(&injected);
        let marker = marker.to_vec();
        let inject_bytes = inject_bytes.to_vec();
        let mut stdin = child_stdin;
        thread::spawn(move || {
            let mut chunk = [0u8; 256];
            loop {
                match child_stdout.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut b = buf.lock().unwrap();
                        b.extend_from_slice(&chunk[..n]);
                        if !injected.load(Ordering::SeqCst)
                            && !marker.is_empty()
                            && b.windows(marker.len()).any(|w| w == marker.as_slice())
                        {
                            let _ = stdin.write_all(&inject_bytes);
                            let _ = stdin.flush();
                            injected.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
        })
    };

    let stderr_thread = {
        let buf = Arc::clone(&buf);
        thread::spawn(move || {
            let mut chunk = [0u8; 256];
            loop {
                match child_stderr.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.lock().unwrap().extend_from_slice(&chunk[..n]),
                }
            }
        })
    };

    let _ = child.wait();
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    Arc::try_unwrap(buf)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default()
}

/// Minimal TOML reader for the v0.1.x profile schema.
///
/// Supports:
///   name             = "string"
///   kernel           = "path"
///   disk             = "path"
///   timeout_secs     = integer
///   expected_markers = ["a", "b", "c"]
///   extra_args       = ["-d", "trace:..."]
fn load_profile(name: &str) -> Result<Profile, String> {
    let path = PathBuf::from(format!("tests/qemu/profiles/{name}.toml"));
    let src =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let mut name_v = name.to_string();
    let mut kernel_v = PathBuf::from(KERNEL_ELF);
    let mut disk_v = PathBuf::from("fjell-disk.img");
    let mut timeout_v: u32 = 60;
    let mut markers: Vec<String> = Vec::new();
    let mut extra: Vec<String> = Vec::new();
    let mut inject_after_marker_v: Option<String> = None;
    let mut inject_bytes_v: Option<String> = None;

    let mut lines = src.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        // Multi-line array support: `key = [` opens an array that is closed
        // by a line whose content is `]`. (The original single-line reader
        // silently parsed these as empty lists, which degraded every real
        // negative profile to a placeholder — architect review v0.18
        // follow-up.)
        let v_owned: String = if v.starts_with('[') && !v.contains(']') {
            let mut acc = String::from(v);
            for cont in lines.by_ref() {
                let c = cont.trim();
                acc.push(' ');
                acc.push_str(c);
                if c.contains(']') {
                    break;
                }
            }
            acc
        } else {
            v.to_string()
        };
        let v = v_owned.as_str();
        match k {
            "name" => name_v = unquote(v),
            "kernel" => kernel_v = PathBuf::from(unquote(v)),
            "disk" => disk_v = PathBuf::from(unquote(v)),
            "timeout_secs" => {
                timeout_v = v
                    .parse::<u32>()
                    .map_err(|e| format!("bad timeout_secs: {e}"))?
            }
            "expected_markers" => markers = parse_list(v),
            "extra_args" => extra = parse_list(v),
            // RFC-0.25-001 (Demonstration 6): a byte is written to QEMU's
            // stdin once `inject_after_marker` appears in the output.
            // `inject_bytes` is the literal string whose bytes are sent —
            // both keys must be present or injection is disabled.
            "inject_after_marker" => inject_after_marker_v = Some(unquote(v)),
            "inject_bytes" => inject_bytes_v = Some(unquote(v)),
            _ => {} // forward-compatibility: ignore unknown keys
        }
    }

    let inject_after_marker = match (inject_after_marker_v, inject_bytes_v) {
        (Some(m), Some(b)) => Some((m, b.into_bytes())),
        _ => None,
    };

    Ok(Profile {
        name: name_v,
        kernel: kernel_v,
        disk: disk_v,
        timeout_secs: timeout_v,
        expected_markers: markers,
        extra_args: extra,
        inject_after_marker,
    })
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn parse_list(v: &str) -> Vec<String> {
    let t = v.trim();
    let t = t.strip_prefix('[').unwrap_or(t);
    let t = t.strip_suffix(']').unwrap_or(t);
    t.split(',')
        .map(|item| unquote(item.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}
