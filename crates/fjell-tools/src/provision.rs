//! `cargo xtask provision-dev` — RFC-v0.17-001 (Accepted) dev-profile
//! trust-anchor provisioning.
//!
//! Architect ruling (v0.18.3 review §4.4, recorded in
//! docs/verification/verus/review-records/v0.18-architect-review-decisions.md):
//!
//!   Development/QEMU profile: TOFU allowed ONLY with the explicit
//!   `--allow-tofu-provision` flag. Silent default TOFU is prohibited.
//!   Factory/field (v1.1): factory station. High-assurance (v2+):
//!   hardware-anchored.
//!
//! What this command does (with the flag):
//!   1. Writes a 32-byte dev anchor key to `provision/dev-trust-anchor.key`
//!      (64 hex chars) — either freshly generated (`--generate`) or supplied
//!      (`--key <hexfile>`).
//!   2. Writes `provision/PROVENANCE.toml` recording mechanism = "tofu-dev",
//!      the explicit-flag acknowledgement, and the date.
//!   3. The next build embeds the key into fjell-verifyd via its build.rs;
//!      unprovisioned builds keep the legacy all-zero dev key and log a
//!      loud startup warning.
//!
//! Without the flag, the command refuses and prints the policy.

use std::fs;
use std::process::ExitCode;

pub fn cmd_provision_dev(args: &[String]) -> ExitCode {
    let allow = args.iter().any(|a| a == "--allow-tofu-provision");
    if !allow {
        eprintln!("provision-dev: REFUSED.");
        eprintln!();
        eprintln!("Trust-on-first-use provisioning of the dev trust anchor requires the");
        eprintln!("explicit `--allow-tofu-provision` flag (RFC-v0.17-001, architect ruling");
        eprintln!("v0.18.3 §4.4). Silent default TOFU is prohibited for all v1-supported");
        eprintln!("profiles. This mechanism is for the development/QEMU profile ONLY;");
        eprintln!("factory/field nodes use factory-station provisioning (v1.1).");
        eprintln!();
        eprintln!("  cargo xtask provision-dev --allow-tofu-provision --generate");
        eprintln!("  cargo xtask provision-dev --allow-tofu-provision --key <hexfile>");
        return ExitCode::FAILURE;
    }

    // Resolve key material.
    let key_hex: String = if let Some(pos) = args.iter().position(|a| a == "--key") {
        let path = match args.get(pos + 1) {
            Some(p) => p,
            None => { eprintln!("provision-dev: --key requires a path"); return ExitCode::FAILURE; }
        };
        match fs::read_to_string(path) {
            Ok(s) => {
                let t = s.trim().to_string();
                if t.len() != 64 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
                    eprintln!("provision-dev: key file must contain exactly 64 hex chars");
                    return ExitCode::FAILURE;
                }
                t
            }
            Err(e) => { eprintln!("provision-dev: cannot read {path}: {e}"); return ExitCode::FAILURE; }
        }
    } else {
        // --generate (default when flag present and no --key): derive 32
        // bytes from the OS entropy source (read exactly 32 bytes — never
        // fs::read the whole device).
        use std::io::Read;
        let mut bytes = [0u8; 32];
        match fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes)) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("provision-dev: no entropy source available: {e}");
                return ExitCode::FAILURE;
            }
        }
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    };

    if let Err(e) = fs::create_dir_all("provision") {
        eprintln!("provision-dev: cannot create provision/: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = fs::write("provision/dev-trust-anchor.key", format!("{key_hex}\n")) {
        eprintln!("provision-dev: cannot write key file: {e}");
        return ExitCode::FAILURE;
    }

    let date = std::process::Command::new("date").arg("+%Y-%m-%d").output()
        .ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let provenance = format!(
        "# Trust-anchor provisioning provenance (RFC-v0.17-001)\n\
         [provision]\n\
         mechanism            = \"tofu-dev\"\n\
         profile              = \"development/QEMU\"\n\
         explicit_flag        = true   # --allow-tofu-provision acknowledged\n\
         date                 = \"{date}\"\n\
         key_file             = \"provision/dev-trust-anchor.key\"\n\
         \n\
         # NOTE: bundles must be signed with the authority matching this\n\
         # anchor for verification to succeed. This anchor is NOT valid for\n\
         # factory/field deployments (factory-station provisioning, v1.1)\n\
         # or high-assurance deployments (hardware-anchored, v2+).\n");
    if let Err(e) = fs::write("provision/PROVENANCE.toml", provenance) {
        eprintln!("provision-dev: cannot write provenance: {e}");
        return ExitCode::FAILURE;
    }

    println!("provision-dev: dev trust anchor provisioned (mechanism=tofu-dev).");
    println!("  key:        provision/dev-trust-anchor.key");
    println!("  provenance: provision/PROVENANCE.toml");
    println!("  effect:     the next `cargo xtask build` embeds this anchor into");
    println!("              fjell-verifyd (replacing the legacy all-zero dev key).");
    println!("  reminder:   sign bundles with the matching authority; this anchor");
    println!("              is for the development/QEMU profile only.");
    ExitCode::SUCCESS
}
