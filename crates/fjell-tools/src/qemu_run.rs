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

use std::process::{Command, ExitCode};
use std::path::{Path, PathBuf};
use std::fs;

use crate::qemu::{KERNEL_ELF, build_all};

/// One QEMU run.  Loaded from a profile file or built inline by the
/// smoke runner.
pub struct Profile {
    pub name:             String,
    /// Path to the kernel ELF, relative to the workspace root.
    pub kernel:           PathBuf,
    /// Path to the disk image to attach (created if missing).
    pub disk:             PathBuf,
    /// Hard timeout in seconds for the `timeout(1)` wrapper.
    pub timeout_secs:     u32,
    /// Markers that must appear in the captured serial log for the run
    /// to count as a pass.  An empty list is allowed for placeholder
    /// profiles per RFC 025 §"chicken-and-egg" exemption.
    pub expected_markers: Vec<String>,
    /// Optional extra QEMU args beyond the defaults.
    pub extra_args:       Vec<String>,
}

impl Profile {
    /// Build the default smoke profile for one milestone (`m1`..`m8`).
    pub fn smoke(milestone: &str, marker: &str) -> Self {
        Self {
            name:             format!("smoke-{milestone}"),
            kernel:           PathBuf::from(KERNEL_ELF),
            disk:             PathBuf::from("fjell-disk.img"),
            timeout_secs:     60,
            expected_markers: vec![marker.to_string()],
            extra_args:       vec![],
        }
    }
    /// Build a placeholder negative profile for one category — used
    /// when no test cases are registered yet (RFC 025 §"chicken-
    /// and-egg").  Succeeds with a no-op.
    pub fn negative_placeholder(category: &str) -> Self {
        Self {
            name:             format!("negative-{category}"),
            kernel:           PathBuf::from(KERNEL_ELF),
            disk:             PathBuf::from("fjell-disk.img"),
            timeout_secs:     1,
            expected_markers: vec![],
            extra_args:       vec![],
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
    pub fn join(&self, p: &str) -> PathBuf { self.0.join(p) }
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
    println!("[xtask] running profile `{}` (timeout {}s)",
             p.name, p.timeout_secs);

    // Build kernel if needed (smoke profiles always build_all; the
    // arg `--profile` path assumes the kernel is already built).
    if !Path::new(&p.kernel).exists() {
        eprintln!("[xtask] kernel ELF missing — running build_all()");
        let _ = build_all();
    }

    // (Re)create disk image — required by virtio-blk smoke path.
    if p.disk.exists() { let _ = fs::remove_file(&p.disk); }
    let _ = Command::new("qemu-img")
        .args(["create", "-f", "raw"])
        .arg(&p.disk).arg("16M")
        .status();

    let kernel_str = p.kernel.to_string_lossy().to_string();
    let disk_str   = p.disk.to_string_lossy().to_string();
    let drive_arg  = format!("file={disk_str},format=raw,if=none,id=hd0");

    // Build the command vector once so we can both run it and persist
    // it to qemu-command.txt.
    let mut argv: Vec<String> = vec![
        format!("{}", p.timeout_secs),
        "qemu-system-riscv64".into(),
        "-machine".into(),    "virt".into(),
        "-bios".into(),       "none".into(),
        "-nographic".into(),
        "-kernel".into(),     kernel_str.clone(),
        "-drive".into(),      drive_arg.clone(),
        "-device".into(),     "virtio-blk-device,drive=hd0".into(),
    ];
    argv.extend(p.extra_args.iter().cloned());

    let _ = fs::write(art.join("qemu-command.txt"),
                      argv.join(" ").as_bytes());
    let _ = fs::write(art.join("expected-markers.txt"),
                      p.expected_markers.join("\n").as_bytes());

    let output = Command::new("timeout")
        .args(&argv[..])
        .output()
        .expect("failed to run qemu-system-riscv64");

    // Capture combined output as serial.log.
    let mut combined = output.stdout.clone();
    combined.extend_from_slice(&output.stderr);
    let log_path = art.join("serial.log");
    let _ = fs::write(&log_path, &combined);

    // Empty marker list = placeholder profile (no cases registered).
    if p.expected_markers.is_empty() {
        let _ = fs::write(art.join("result-summary.txt"),
            b"PASS (placeholder; no expected markers)\n");
        println!("[xtask] profile `{}` is a placeholder — no markers \
                  to check (RFC 025 §chicken-and-egg). PASS.", p.name);
        return ExitCode::SUCCESS;
    }

    // Check every expected marker.
    let mut all_ok = true;
    for marker in &p.expected_markers {
        let ok = combined.windows(marker.len())
                         .any(|w| w == marker.as_bytes());
        if !ok {
            eprintln!("[xtask] missing marker `{marker}` in {}",
                      log_path.display());
            all_ok = false;
        }
    }
    let summary = if all_ok { "PASS\n" } else { "FAIL\n" };
    let _ = fs::write(art.join("result-summary.txt"), summary);

    if all_ok {
        println!("[xtask] profile `{}` PASS ({} marker(s) matched) ✓",
                 p.name, p.expected_markers.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("[xtask] profile `{}` FAIL — see {}",
                  p.name, log_path.display());
        // Print the last 60 lines of serial.log directly so failures are
        // visible without opening a separate file (RFC-v0.7.1-003 §smoke).
        if let Ok(log_bytes) = fs::read(&log_path) {
            let log_text = String::from_utf8_lossy(&log_bytes);
            let lines: Vec<&str> = log_text.lines().collect();
            let tail = if lines.len() > 60 { &lines[lines.len()-60..] } else { &lines[..] };
            eprintln!("[xtask] --- serial.log tail ({} lines) ---", tail.len());
            for line in tail {
                eprintln!("[serial] {line}");
            }
            eprintln!("[xtask] --- end serial.log ---");
        }
        ExitCode::FAILURE
    }
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
    let src  = fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let mut name_v   = name.to_string();
    let mut kernel_v = PathBuf::from(KERNEL_ELF);
    let mut disk_v   = PathBuf::from("fjell-disk.img");
    let mut timeout_v: u32 = 60;
    let mut markers: Vec<String> = Vec::new();
    let mut extra:   Vec<String> = Vec::new();

    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        match k {
            "name"             => name_v   = unquote(v),
            "kernel"           => kernel_v = PathBuf::from(unquote(v)),
            "disk"             => disk_v   = PathBuf::from(unquote(v)),
            "timeout_secs"     => timeout_v = v.parse::<u32>()
                .map_err(|e| format!("bad timeout_secs: {e}"))?,
            "expected_markers" => markers = parse_list(v),
            "extra_args"       => extra   = parse_list(v),
            _ => {} // forward-compatibility: ignore unknown keys
        }
    }

    Ok(Profile {
        name: name_v, kernel: kernel_v, disk: disk_v,
        timeout_secs: timeout_v,
        expected_markers: markers, extra_args: extra,
    })
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
       || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2) {
        t[1..t.len()-1].to_string()
    } else { t.to_string() }
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
