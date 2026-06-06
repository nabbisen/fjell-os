//! `cargo xtask publish` and `cargo xtask install` — local artifact registry
//! (RFC-v0.14-004).
//!
//! The registry is a file-tree at `registry/`:
//!
//! ```text
//! registry/
//!   registry.toml                 — top-level manifest
//!   bundles/<service>/<version>/
//!     bundle.bundle               — ServiceBundle bytes
//!     bundle.bundle.sig           — SignedManifest
//!     meta.toml                   — version metadata
//! index/
//!   by-service/<name>             — file containing latest version string
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// ── Public entry points ───────────────────────────────────────────────────────

pub fn cmd_publish(args: &[String]) -> ExitCode {
    let bundle  = flag(args, "--bundle");
    let sig     = flag(args, "--sig");
    let version = flag(args, "--version");
    let notes   = flag(args, "--notes").unwrap_or_else(|| "".into());
    let reg     = flag(args, "--registry").unwrap_or_else(|| "registry".into());

    let (bundle, version) = match (bundle, version) {
        (Some(b), Some(v)) => (b, v),
        _ => {
            eprintln!("publish: --bundle <f> --version <v> required");
            return ExitCode::FAILURE;
        }
    };

    // Infer service name from bundle filename
    let service = Path::new(&bundle)
        .file_stem().and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .trim_end_matches(".bundle")
        .to_string();

    // Load bundle bytes to compute digest
    let bundle_bytes = match fs::read(&bundle) {
        Ok(b) => b,
        Err(e) => { eprintln!("publish: cannot read bundle: {}", e); return ExitCode::FAILURE; }
    };
    let bundle_digest = fnv_hex32(&bundle_bytes);

    // Check for existing version
    let dest = PathBuf::from(&reg).join("bundles").join(&service).join(&version);
    if dest.exists() {
        eprintln!("publish: {}/{} already exists; bump version to re-publish", service, version);
        return ExitCode::FAILURE;
    }

    // Downgrade check: refuse if new version < latest
    if let Some(latest) = latest_version(&reg, &service) {
        if !version_greater(&version, &latest) {
            eprintln!("publish: {} ≤ latest {} — downgrade publishing refused",
                version, latest);
            return ExitCode::FAILURE;
        }
    }

    // Create destination
    fs::create_dir_all(&dest).ok();

    // Copy bundle
    if let Err(e) = fs::copy(&bundle, dest.join("bundle.bundle")) {
        eprintln!("publish: copy bundle: {}", e); return ExitCode::FAILURE;
    }

    // Copy sig if provided
    if let Some(ref sig_path) = sig {
        if let Err(e) = fs::copy(sig_path, dest.join("bundle.bundle.sig")) {
            eprintln!("publish: copy sig: {}", e); return ExitCode::FAILURE;
        }
    }

    // Write meta.toml
    let meta = format!(
        "service_name = {:?}\nversion = {:?}\nbundle_digest = {:?}\nnotes = {:?}\n",
        service, version, bundle_digest, notes
    );
    if let Err(e) = fs::write(dest.join("meta.toml"), &meta) {
        eprintln!("publish: write meta: {}", e); return ExitCode::FAILURE;
    }

    // Update index
    let idx = PathBuf::from(&reg).join("index").join("by-service");
    fs::create_dir_all(&idx).ok();
    let _ = fs::write(idx.join(&service), &version);

    // Ensure registry.toml exists
    let reg_toml = PathBuf::from(&reg).join("registry.toml");
    if !reg_toml.exists() {
        let content = "schema_version = 1\nallow_unsigned = true\n";
        let _ = fs::write(&reg_toml, content);
    }

    println!("publish: {}/{} → {}/bundles/{}/{}", service, version, reg, service, version);
    println!("publish: bundle_digest = {}", &bundle_digest[..16]);
    ExitCode::SUCCESS
}

pub fn cmd_install(args: &[String]) -> ExitCode {
    let service = flag(args, "--service");
    let version = flag(args, "--version");
    let reg     = flag(args, "--registry").unwrap_or_else(|| "registry".into());

    let service = match service {
        Some(s) => s,
        None => { eprintln!("install: --service <name> required"); return ExitCode::FAILURE; }
    };

    // Resolve version
    let version = version.unwrap_or_else(|| {
        latest_version(&reg, &service).unwrap_or_else(|| "0.0.0".into())
    });

    let bundle_path = PathBuf::from(&reg)
        .join("bundles").join(&service).join(&version)
        .join("bundle.bundle");

    if !bundle_path.exists() {
        eprintln!("install: {}/{} not found in registry {}", service, version, reg);
        return ExitCode::FAILURE;
    }

    let bundle_bytes = match fs::read(&bundle_path) {
        Ok(b) => b,
        Err(e) => { eprintln!("install: {}", e); return ExitCode::FAILURE; }
    };

    let digest = fnv_hex32(&bundle_bytes);
    println!("install: {} v{} (digest {}…)", service, version, &digest[..12]);
    println!("install: PASS (deploy via `cargo xtask fleet-demo deploy` for live install)");
    ExitCode::SUCCESS
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).and_then(|w| w.get(1)).cloned()
}

fn latest_version(reg: &str, service: &str) -> Option<String> {
    let idx = PathBuf::from(reg).join("index").join("by-service").join(service);
    fs::read_to_string(idx).ok().map(|s| s.trim().to_string())
}

/// Simple semver-ish comparison: split by '.' and compare numerically.
fn version_greater(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    let av = parse(a);
    let bv = parse(b);
    av > bv
}

fn fnv_hex32(data: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}{:016x}{:016x}{:016x}",
        h, h.wrapping_mul(2654435761), h.wrapping_add(0x9e3779b9), h ^ 0xdeadbeef)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_greater_basics() {
        assert!(version_greater("0.1.1", "0.1.0"));
        assert!(version_greater("1.0.0", "0.9.9"));
        assert!(!version_greater("0.1.0", "0.1.0"));
        assert!(!version_greater("0.0.9", "0.1.0"));
    }

    #[test]
    fn fnv_hex32_deterministic() {
        assert_eq!(fnv_hex32(b"bundle"), fnv_hex32(b"bundle"));
        assert_ne!(fnv_hex32(b"bundle"), fnv_hex32(b"Bundle"));
    }

    #[test]
    fn fnv_hex32_length() {
        assert_eq!(fnv_hex32(b"test").len(), 64);
    }
}
