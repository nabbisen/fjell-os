//! `cargo xtask package-release` — produce the release archive.
//!
//! Archive name:      `fjell-os-v{version}.tar.gz`
//! Internal top dir:  `fjell-os-v{version}/`
//!
//! The version is read from the workspace `Cargo.toml` at the repository
//! root. The archive is written to the current working directory (expected
//! to be the repository root when invoked via `cargo xtask`).
//!
//! Excludes:
//!   target/          build artefacts
//!   .git/            version-control history
//!   *.img            disk images
//!   tests/runs/      ephemeral test logs
//!   tests/qemu/artifacts/  ephemeral QEMU serial logs
//!   provision/       operator-provisioned trust-anchor material
//!                    (ships unprovisioned; each operator provisions
//!                    explicitly with --allow-tofu-provision)

use std::process::{Command, ExitCode};

pub fn cmd_package_release() -> ExitCode {
    // Read version from workspace Cargo.toml.
    let cargo_toml = match std::fs::read_to_string("Cargo.toml") {
        Ok(s) => s,
        Err(e) => { eprintln!("package-release: cannot read Cargo.toml: {e}"); return ExitCode::FAILURE; }
    };
    let version = match parse_version(&cargo_toml) {
        Some(v) => v,
        None => { eprintln!("package-release: cannot parse version from Cargo.toml"); return ExitCode::FAILURE; }
    };

    let archive_name = format!("fjell-os-v{version}.tar.gz");
    let internal_dir = format!("fjell-os-v{version}");

    println!("package-release: version    = {version}");
    println!("package-release: archive    = {archive_name}");
    println!("package-release: top-level  = {internal_dir}/");

    // Resolve the repository root (parent of the directory containing Cargo.toml).
    // When run via `cargo xtask` the cwd is the repo root.
    let repo_root = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => { eprintln!("package-release: cannot read cwd: {e}"); return ExitCode::FAILURE; }
    };
    let src_dir = repo_root.parent().unwrap_or(&repo_root);
    let src_name = repo_root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("fjell-os");

    // --transform renames the top-level directory from the source dir name
    // (e.g. `fjell-os`) to `fjell-os-v{version}`.
    let transform = format!("s|^{src_name}|{internal_dir}|");

    // Write to a temp path first to avoid the archive being included in itself.
    let tmp_path = std::env::temp_dir().join(&archive_name);
    let status = Command::new("tar")
        .arg(format!("--transform={transform}"))
        .arg("--exclude=*/target")
        .arg("--exclude=*/.git")
        .arg("--exclude=*/*.img")
        .arg("--exclude=*/fjell-os-v*.tar.gz")   // exclude prior release archives
        .arg("--exclude=*/tests/runs")
        .arg("--exclude=*/tests/qemu/artifacts")
        .arg("--exclude=*/provision")
        .arg("-czf")
        .arg(&tmp_path)
        .arg("-C")
        .arg(src_dir)
        .arg(src_name)
        .status();

    match status {
        Ok(s) if s.success() => {
            if let Err(e) = std::fs::rename(&tmp_path, &archive_name) {
                // rename may fail across filesystems (/tmp → cwd); fall back to copy+remove
                std::fs::copy(&tmp_path, &archive_name).ok();
                std::fs::remove_file(&tmp_path).ok();
                let _ = e;
            }
            let size = std::fs::metadata(&archive_name)
                .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
                .unwrap_or_else(|_| "?".into());
            println!("package-release: wrote {archive_name} ({size})");
            ExitCode::SUCCESS
        }
        Ok(s) => {
            eprintln!("package-release: tar exited with status {s}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("package-release: failed to run tar: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_version(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("version") {
            if let Some(val) = rest.trim_start_matches(|c: char| c == ' ' || c == '=')
                                    .strip_prefix('"') {
                return Some(val.trim_end_matches('"').to_string());
            }
        }
    }
    None
}
