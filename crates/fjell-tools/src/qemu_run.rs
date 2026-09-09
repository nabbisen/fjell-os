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
    /// RFC-0.29-001 D1: the structural signal that separates a negative
    /// (or smoke) profile, runnable by this module's single-QEMU
    /// `run_profile`, from a profile like `fleet-demo` that requires
    /// several simultaneous QEMU instances and a dedicated runner
    /// (`cargo xtask fleet-demo`). Anything deriving "the negative-test
    /// categories" from `tests/qemu/profiles/*.toml` must exclude these —
    /// see `discover_negative_categories`.
    pub multi_node: bool,
    /// RFC-0.29-001 D5: `true` unless the profile's own `.toml` says
    /// otherwise. A category can be swept into derived scope (D1) while
    /// still being explicitly excluded from what a release blocks on —
    /// `store` and `upgrade` have marker specifications but no emitting
    /// scenario yet (`v1-limitations.md`), and are expected to run and
    /// fail honestly rather than being silently skipped. The loader
    /// requires `not_gated_reason` whenever this is `false` — there is no
    /// way to opt out without also saying why, in the one file a reader
    /// checking this category will already have open.
    pub release_gated: bool,
    /// Required, non-empty, whenever `release_gated` is `false`.
    pub not_gated_reason: Option<String>,
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
            multi_node: false,
            release_gated: true,
            not_gated_reason: None,
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
    let profile = match load_profile(Path::new("."), name) {
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
        // RFC-0.28-003: test_ipc_blocked_recv's own D3-required exhaustion
        // markers. This list is a closed, hand-maintained set (E-014's own
        // "instruments deciding by fixed-string match" family) — a new
        // NEG:HARNESS:* marker has no effect on this gate until it is added
        // here explicitly, which is why these two are, rather than a
        // rename or a generic prefix match (the latter would also catch
        // NEG:HARNESS:CSpace_LAYOUT_VALID:PASS, a real passing marker).
        "NEG:HARNESS:BLOCKED_RECV_IDENTITY_EXCHANGE_FAILED",
        "NEG:HARNESS:BLOCKED_RECV_POLL_EXHAUSTED",
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
    // RFC-0.29-002 R3/D1: the literal "TEST:FAIL" used to live in
    // FORBIDDEN above, but it is not a substring of the real message
    // this project actually emits — `fjell-kernel`'s
    // `trap/dispatch.rs:486` prints `TEST:M7:FAIL (init did not exit
    // cleanly)`, and "TEST:" immediately followed by "FAIL" never
    // occurs; the milestone token always sits between them. Structural
    // check instead: `TEST:<token>:FAIL` for any token, not one literal
    // shape.
    if contains_test_fail_marker(&combined) {
        eprintln!(
            "[xtask] FORBIDDEN marker `TEST:<milestone>:FAIL` present in {}",
            log_path.display()
        );
        all_ok = false;
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
///   multi_node       = true|false   (RFC-0.29-001)
///   release_gated    = true|false   (RFC-0.29-001; requires not_gated_reason when false)
///   not_gated_reason = "string"     (RFC-0.29-001)
fn load_profile(root: &Path, name: &str) -> Result<Profile, String> {
    let path = root
        .join("tests/qemu/profiles")
        .join(format!("{name}.toml"));
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
    let mut multi_node_v = false;
    let mut release_gated_v = true;
    let mut not_gated_reason_v: Option<String> = None;

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
            "multi_node" => {
                multi_node_v = parse_bool(v).map_err(|e| format!("bad multi_node: {e}"))?
            }
            "release_gated" => {
                release_gated_v = parse_bool(v).map_err(|e| format!("bad release_gated: {e}"))?
            }
            "not_gated_reason" => not_gated_reason_v = Some(unquote(v)),
            _ => {} // forward-compatibility: ignore unknown keys
        }
    }

    if !release_gated_v && not_gated_reason_v.as_deref().unwrap_or("").is_empty() {
        return Err(format!(
            "{}: release_gated = false requires a non-empty not_gated_reason (RFC-0.29-001 D5) \
             — say why this category isn't release-gated, in the file a reader checking it will \
             already have open",
            path.display()
        ));
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
        multi_node: multi_node_v,
        release_gated: release_gated_v,
        not_gated_reason: not_gated_reason_v,
    })
}

fn parse_bool(v: &str) -> Result<bool, String> {
    match v.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("expected true or false, got {other:?}")),
    }
}

/// One entry of `discover_negative_categories`'s derived list.
pub struct CategoryInfo {
    pub name: String,
    pub release_gated: bool,
    pub not_gated_reason: Option<String>,
}

/// RFC-0.29-001 D1/§6: the authority for "the negative-test categories" is
/// `tests/qemu/profiles/*.toml` itself, not any of the five lists the RFC
/// found disagreeing (this project's own `NEG_CATEGORIES`, `ci.yml`'s
/// matrix, two `KNOWN_*` constants, and a stale doc-comment). A profile is
/// a negative-test category unless its own `multi_node = true` says it
/// needs a different runner entirely (`fleet-demo`, which requires three
/// simultaneous QEMU instances and is invoked through `cargo xtask
/// fleet-demo`, never through this module's single-QEMU `run_profile`) —
/// a structural exclusion that does not grow as new single-QEMU profiles
/// are added, unlike a name a person must remember to list.
///
/// Every remaining profile is included, gated or not (D5): `store` and
/// `upgrade` are swept in like any other category, distinguished only by
/// their own `release_gated = false` — the derivation does not special-
/// case them by name.
pub fn discover_negative_categories() -> Result<Vec<CategoryInfo>, String> {
    discover_negative_categories_at(Path::new("."))
}

/// `root` is `.` for the real invocation (`cargo xtask` always runs from
/// the workspace root); tests pass an absolute path computed from
/// `CARGO_MANIFEST_DIR` instead, since `cargo test` runs test binaries
/// with the crate's own directory as the working directory, not the
/// workspace's (same reasoning as `callsite_audit`'s `test_workspace_root`).
pub fn discover_negative_categories_at(root: &Path) -> Result<Vec<CategoryInfo>, String> {
    let dir = root.join("tests/qemu/profiles");
    let entries = fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;

    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();

    let mut out = Vec::new();
    for name in names {
        let p = load_profile(root, &name)?;
        if p.multi_node {
            continue;
        }
        out.push(CategoryInfo {
            name,
            release_gated: p.release_gated,
            not_gated_reason: p.not_gated_reason,
        });
    }
    Ok(out)
}

/// Does `combined` contain a `TEST:<token>:FAIL` marker, for any milestone
/// token (`M7`, `V0.4-NET`, ...) — not just the literal `"TEST:FAIL"`,
/// which is not a substring of the real message this project emits
/// (`TEST:M7:FAIL (init did not exit cleanly)`, `trap/dispatch.rs:486`).
/// A milestone token never itself contains `:`, so scanning for the next
/// `:` after each `TEST:` occurrence and checking whether it starts
/// `:FAIL` is exact, not a heuristic.
fn contains_test_fail_marker(combined: &[u8]) -> bool {
    let s = String::from_utf8_lossy(combined);
    let mut rest: &str = &s;
    while let Some(pos) = rest.find("TEST:") {
        let after = &rest[pos + "TEST:".len()..];
        if let Some(colon) = after.find(':') {
            if after[colon..].starts_with(":FAIL") {
                return true;
            }
        }
        rest = after;
    }
    false
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

#[cfg(test)]
mod forbidden_tests {
    use super::*;

    /// RFC-0.29-002 R3 required demonstration: the real message this
    /// project emits (`trap/dispatch.rs:486`) — the old literal
    /// `"TEST:FAIL"` is not a substring of it.
    #[test]
    fn catches_the_real_test_m7_fail_message() {
        assert!(!"TEST:M7:FAIL (init did not exit cleanly)".contains("TEST:FAIL"));
        assert!(contains_test_fail_marker(
            b"TEST:M7:FAIL (init did not exit cleanly)"
        ));
    }

    #[test]
    fn does_not_match_a_passing_marker() {
        assert!(!contains_test_fail_marker(b"TEST:M7:PASS"));
        assert!(!contains_test_fail_marker(b"TEST:V0.4-NET:PASS"));
    }

    #[test]
    fn matches_any_milestone_token() {
        assert!(contains_test_fail_marker(b"TEST:V0.5-PLATFORM:FAIL"));
    }

    /// E-014's own originally-filed instance, checked rather than assumed
    /// fixed: `load_profile`'s multi-line-array joiner closes the array at
    /// the first line *containing* `]`, not the first unquoted `]` — a
    /// marker string with a literal `]` in it (e.g. `"[INTENT] ..."`)
    /// truncates the array early and silently drops every later marker.
    /// **Still live** — not this RFC's R3 (which names five specific
    /// instruments, not this one) to fix; recorded as E-014's surviving
    /// instance instead.
    #[test]
    fn multiline_array_still_closes_early_on_a_bracket_inside_a_marker_string() {
        let dir = std::env::temp_dir().join(format!("qemu_run_toml_demo_{}", std::process::id()));
        let profiles_dir = dir.join("tests/qemu/profiles");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::write(
            profiles_dir.join("demo.toml"),
            "name = \"demo\"\nexpected_markers = [\n    \"[INTENT] marker one\",\n    \"marker two\",\n    \"marker three\",\n]\n",
        )
        .unwrap();
        let profile = load_profile(&dir, "demo").expect("should still parse, just wrong");
        assert_eq!(
            profile.expected_markers.len(),
            1,
            "the array closed at the `]` inside the first marker string, dropping the other two -- \
             still the live E-014 instance, not fixed by this line"
        );
        fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod discover_tests {
    use super::*;

    /// `cargo test` runs test binaries with the crate's own directory as
    /// the working directory, not the workspace root the real `cargo
    /// xtask` invocation always uses.
    fn test_workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn discover_negative_categories_excludes_fleet_demo_includes_store_upgrade() {
        let cats = discover_negative_categories_at(&test_workspace_root())
            .expect("discover should succeed against the real tree");
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert!(
            !names.contains(&"fleet-demo"),
            "fleet-demo is multi_node, must be excluded"
        );
        assert!(names.contains(&"store"));
        assert!(names.contains(&"upgrade"));
        let store = cats.iter().find(|c| c.name == "store").unwrap();
        assert!(!store.release_gated);
        assert!(store.not_gated_reason.is_some());
    }

    #[test]
    fn multi_node_profile_without_release_gated_still_loads() {
        // fleet-demo has no release_gated key at all — confirms the
        // default (true) applies and the loader does not require the key
        // on every profile, only on ones that opt out.
        let p = load_profile(&test_workspace_root(), "fleet-demo")
            .expect("fleet-demo.toml should still parse on its own");
        assert!(p.multi_node);
        assert!(p.release_gated);
        assert!(p.not_gated_reason.is_none());
    }

    #[test]
    fn release_gated_false_without_reason_is_rejected() {
        let dir = std::env::temp_dir().join(format!("qemu_run_test_{}", std::process::id()));
        let profiles_dir = dir.join("tests/qemu/profiles");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::write(
            profiles_dir.join("bad.toml"),
            "name = \"bad\"\nexpected_markers = []\nrelease_gated = false\n",
        )
        .unwrap();
        match load_profile(&dir, "bad") {
            Err(e) => assert!(e.contains("not_gated_reason")),
            Ok(_) => panic!("release_gated=false with no reason must fail closed"),
        }
        fs::remove_dir_all(&dir).ok();
    }
}
