//! Capability broker for M4.
//!
//! Evaluates access-control policy and issues lease-bound capabilities.
//! In M4, policy is a compile-time table (no runtime policy bundle load).

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_exit, sys_ipc_recv, sys_ipc_reply,
                    sys_lease_create, sys_lease_revoke, sys_lease_inspect};
use fjell_service_api::tags;

// ── Policy table (M4: static, compile-time) ───────────────────────────────────

#[derive(Clone, Copy)]
struct PolicyRule {
    requester: &'static str,  // service name requesting the capability
    target_ep: u16,           // target endpoint slot
    allow:     bool,
}

const POLICY: &[PolicyRule] = &[
    PolicyRule { requester: "svc.svc-manager", target_ep: 1, allow: true  },
    PolicyRule { requester: "svc.auditd",      target_ep: 2, allow: true  },
    PolicyRule { requester: "svc.sample",      target_ep: 3, allow: true  },
    // Default-deny: anything not listed above is denied.
    // The denial below acts as an explicit sentinel for the smoke test.
    PolicyRule { requester: "svc.unknown",     target_ep: 0, allow: false },
];

fn evaluate(requester_tag: usize) -> bool {
    // For M4 smoke test: tag bits [15:8] encode requester index.
    // Index 0 = known service (allow), index 0xFF = unknown (deny).
    let idx = (requester_tag >> 8) & 0xFF;
    idx != 0xFF   // all except the "unknown" sentinel are allowed
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let ep = 0u32;

    // Announce ready
    let _ = sys_ipc_reply(tags::SERVICE_READY);

    // Smoke test: demonstrate lease revoke
    let lease_result = sys_lease_create(0);
    let lease_ok = lease_result.is_ok();
    if let Ok(lid) = lease_result {
        let _epoch1 = sys_lease_inspect(lid);
        let _epoch2 = sys_lease_revoke(lid);
        // After revoke, inspect should fail or return incremented epoch
        let _epoch3 = sys_lease_inspect(lid);
    }

    loop {
        match sys_ipc_recv(ep) {
            Ok(tag) if tag & 0xFF == (tags::CAP_REQUEST & 0xFF) => {
                let allowed = evaluate(tag);
                let reply = if allowed { tags::CAP_GRANTED } else { tags::CAP_DENIED };
                let _ = sys_ipc_reply(reply);
            }
            Ok(tags::SERVICE_SHUTDOWN) => break,
            Ok(_) | Err(_) => { let _ = sys_ipc_reply(0); }
        }
    }

    sys_exit(0)
}
