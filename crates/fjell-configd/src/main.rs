//! Config daemon for M4.
//!
//! Validates the embedded bootstrap manifest and serves config requests
//! over its IPC endpoint.  In M4 the "manifest" is a compile-time constant
//! (no filesystem); TOML parsing is deferred to M5.

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_exit, sys_ipc_recv, sys_ipc_reply};
use fjell_service_api::tags;

/// Embedded bootstrap service manifest (validated at compile time).
///
/// Real TOML parsing is added in M5 once a filesystem exists.
/// For M4 this is a statically validated Rust constant.
struct ServiceManifest {
    name: &'static str,
    restart: RestartPolicy,
}
enum RestartPolicy { Never, OnFailure }

const BOOTSTRAP_MANIFEST: &[ServiceManifest] = &[
    ServiceManifest { name: "svc.cap-broker",      restart: RestartPolicy::Never },
    ServiceManifest { name: "svc.auditd",           restart: RestartPolicy::Never },
    ServiceManifest { name: "svc.svc-manager",      restart: RestartPolicy::Never },
    ServiceManifest { name: "svc.sample",           restart: RestartPolicy::OnFailure },
];

fn validate_manifest() -> bool {
    // Validate: no duplicate names, all names non-empty.
    for (i, a) in BOOTSTRAP_MANIFEST.iter().enumerate() {
        if a.name.is_empty() { return false; }
        for b in &BOOTSTRAP_MANIFEST[i+1..] {
            if a.name == b.name { return false; }
        }
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let ep = 0u32;

    // Validate manifest on startup
    let valid = validate_manifest();

    // Announce ready (CONFIG_VALIDATED or CONFIG_INVALID)
    if valid {
        let _ = sys_ipc_reply(tags::CONFIG_VALIDATED);
    } else {
        let _ = sys_ipc_reply(tags::CONFIG_INVALID);
        sys_exit(1);
    }

    // Serve config requests
    loop {
        match sys_ipc_recv(ep) {
            Ok(tags::CONFIG_GET) => {
                // Return number of services in manifest
                let _ = sys_ipc_reply(tags::CONFIG_GET | (BOOTSTRAP_MANIFEST.len() << 12));
            }
            Ok(tags::SERVICE_SHUTDOWN) => break,
            Ok(_) | Err(_) => { let _ = sys_ipc_reply(0); }
        }
    }

    sys_exit(0)
}
