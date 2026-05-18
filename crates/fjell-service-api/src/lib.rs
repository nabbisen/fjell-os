//! User-space service SDK for Fjell OS.
//!
//! Provides helpers for IPC, debug output, and service protocol constants.

#![no_std]

pub mod tags {
    pub const SERVICE_READY:      usize = 0x001;
    pub const SERVICE_HEARTBEAT:  usize = 0x002;
    pub const SERVICE_SHUTDOWN:   usize = 0x003;
    pub const CONFIG_VALIDATE:    usize = 0x010;
    pub const CONFIG_VALIDATED:   usize = 0x011;
    pub const CONFIG_INVALID:     usize = 0x012;
    pub const CONFIG_GET:         usize = 0x013;
    pub const CAP_REQUEST:        usize = 0x020;
    pub const CAP_GRANTED:        usize = 0x021;
    pub const CAP_DENIED:         usize = 0x022;
    pub const AUDIT_EVENT:        usize = 0x030;
    pub const AUDIT_DRAIN_READY:  usize = 0x031;
    pub const SM_START_SERVICE:   usize = 0x040;
    pub const SM_STOP_SERVICE:    usize = 0x041;
    pub const SM_STATUS_QUERY:    usize = 0x042;
    pub const SM_STATUS_REPLY:    usize = 0x043;
    pub const SM_CORE_TARGET_READY: usize = 0x044;
    pub const BOOTSTRAP_COMPLETE: usize = 0x100;

    // ── RFC 042: neg-test IPC protocol ───────────────────────────────────────
    /// Sent by neg-test to a helper service: "bind lease_id (in w0) to your
    /// endpoint cap and block in ipc_recv so we can test revocation wakeup."
    pub const BIND_LEASE_FOR_IPC_TEST: usize = 0x060;
    /// Sent by neg-test (as server): bind lease_id (w0) to a copied endpoint
    /// cap (slot 6) and immediately call neg-test back on that leased cap.
    /// Demonstrates BLOCKED_CALL_WAKES and LATE_REPLY_REJECTED in one exchange.
    pub const BIND_LEASE_AND_CALL_BACK: usize = 0x061;
    /// Callback message sent by sample-service back to neg-test.
    pub const CALL_BACK_MSG: usize = 0x062;
}

// ── RFC 019: storaged IPC protocol ────────────────────────────────────────────
pub mod storaged {
    /// Storaged is ready; init may proceed with storage operations.
    pub const READY:            usize = 0x200;
    /// Begin a 512-byte sector write. words[1]=lba_lo, words[2]=lba_hi.
    pub const WRITE_BEGIN:      usize = 0x201;
    /// One 64-byte chunk of sector data. words[0..8] = data bytes (little-endian).
    pub const WRITE_CHUNK:      usize = 0x202;
    /// Commit the staged write. Reply: WRITE_OK or WRITE_ERR.
    pub const WRITE_COMMIT:     usize = 0x203;
    pub const WRITE_ACK:        usize = 0x204;  // ack for BEGIN/CHUNK
    pub const WRITE_OK:         usize = 0x205;
    pub const WRITE_ERR:        usize = 0x206;
    // Read protocol
    pub const READ_BEGIN:  usize = 0x207;
    pub const READ_CHUNK:  usize = 0x208;
    pub const READ_COMMIT: usize = 0x209;
    pub const READ_ACK:    usize = 0x20A;
    pub const READ_DATA:   usize = 0x20B;
    pub const READ_OK:     usize = 0x20C;
    pub const READ_ERR:    usize = 0x20D;
}

// ── RFC 019: bootctl IPC protocol ─────────────────────────────────────────────
pub mod bootctl {
    pub const READY:            usize = 0x210;
    /// Read the BCB; reply is READ_OK with 8-chunk transfer, then BCB_DATA.
    pub const READ_BCB:         usize = 0x211;
    /// Write the BCB; follow with 8 WRITE_CHUNK messages then WRITE_COMMIT.
    pub const WRITE_BCB:        usize = 0x212;
    pub const READ_OK:          usize = 0x213;
    pub const WRITE_OK:         usize = 0x214;
    pub const ERR:              usize = 0x215;
}

// ── M8: measuredd IPC protocol ────────────────────────────────────────────────
pub mod measuredd {
    /// Service is ready.
    pub const READY:            usize = 0x300;
    /// Append one measurement event.
    /// words[0] = kind<<24|source<<16|subject<<8|flags
    /// words[1] = subject_digest lo64
    /// words[2] = subject_digest hi64 (bytes 8-15)
    /// Reply: APPEND_OK (seq in words[0]) or ERR.
    pub const APPEND_EVENT:     usize = 0x301;
    pub const APPEND_OK:        usize = 0x302;
    /// Get chain head (latest seq + chain_digest).
    pub const GET_HEAD:         usize = 0x303;
    pub const HEAD_REPLY:       usize = 0x304;
    /// Get a specific event by seq.
    pub const GET_EVENT:        usize = 0x305;
    pub const EVENT_REPLY:      usize = 0x306;
    /// Start log export.
    pub const EXPORT_LOG:       usize = 0x307;
    pub const EXPORT_CHUNK:     usize = 0x308;
    pub const EXPORT_DONE:      usize = 0x309;
    pub const ERR:              usize = 0x30F;
}

// ── M8: attestd IPC protocol ──────────────────────────────────────────────────
pub mod attestd {
    pub const READY:            usize = 0x310;
    /// Generate a local attestation record.
    pub const GENERATE:         usize = 0x311;
    pub const GENERATED:        usize = 0x312;
    /// Verify the latest record.
    pub const VERIFY_LATEST:    usize = 0x313;
    pub const VERIFY_OK:        usize = 0x314;
    pub const VERIFY_FAIL:      usize = 0x315;
    /// Export attestation record (PlainText projection).
    pub const EXPORT:           usize = 0x316;
    pub const EXPORT_CHUNK:     usize = 0x317;
    pub const EXPORT_DONE:      usize = 0x318;
    pub const ERR:              usize = 0x31F;
}

// ── M8: recoveryd IPC protocol ────────────────────────────────────────────────
pub mod recoveryd {
    pub const READY:               usize = 0x320;
    /// List snapshots.
    pub const LIST_SNAPSHOTS:      usize = 0x321;
    pub const SNAPSHOT_LIST:       usize = 0x322;
    /// Inspect a slot (words[0] = SlotId).
    pub const INSPECT_SLOT:        usize = 0x323;
    pub const SLOT_INSPECTION:     usize = 0x324;
    /// Inspect latest failure.
    pub const INSPECT_FAILURE:     usize = 0x325;
    pub const FAILURE_SUMMARY:     usize = 0x326;
    /// Enter recovery target (words[0] = reason).
    pub const ENTER_RECOVERY:      usize = 0x327;
    pub const RECOVERY_ENTERED:    usize = 0x328;
    /// Request manual rollback (words[0]=slot, words[1]=reason, words[2]=confirmed).
    pub const SELECT_ROLLBACK:     usize = 0x329;
    pub const ROLLBACK_SELECTED:   usize = 0x32A;
    /// Export diagnostics (words[0] = format).
    pub const EXPORT_DIAGNOSTICS:  usize = 0x32B;
    pub const DIAGNOSTICS_CHUNK:   usize = 0x32C;
    pub const DIAGNOSTICS_DONE:    usize = 0x32D;
    pub const ERR:                 usize = 0x32F;
}

// ── M8: verifyd freshness extension ──────────────────────────────────────────
pub mod verifyd {
    pub const READY:              usize = 0x330;
    pub const CHECK_FRESHNESS:    usize = 0x331;
    pub const FRESHNESS_OK:       usize = 0x332;
    pub const FRESHNESS_REJECTED: usize = 0x333;
    pub const ERR:                usize = 0x33F;
}

// ── RFC 038 (v0.2.0): Service Plane Separation Foundation ────────────────────

/// Service READY protocol (RFC 038 §"Service-ready protocol").
///
/// Every separated service, on start:
/// 1. Performs minimum initialisation.
/// 2. Sends a `READY` message on its private endpoint.
/// 3. Enters its cooperative service loop (RFC 037 shape).
///
/// `service-manager` watches:
/// - `READY` message within `START_TIMEOUT_MS` → service is up.
/// - Timeout without `READY` → service start failed (audit event emitted).
/// - Fault propagated from kernel → service-manager records as Failed.
pub mod ready {
    /// IPC message label for the service READY signal.
    ///
    /// ```text
    /// ipc_send(service_control_ep, label=SERVICE_READY_LABEL, words=0)
    /// ```
    pub const LABEL: usize = crate::tags::SERVICE_READY;

    /// Default start timeout in milliseconds (RFC 038 §"Service manifest").
    pub const START_TIMEOUT_MS: u64 = 1000;

    /// Service fault notification from service-manager to auditd/semantic-stream.
    pub const FAULT_LABEL: usize = 0x050;

    /// Service start timeout notification.
    pub const TIMEOUT_LABEL: usize = 0x051;
}

/// Service lifecycle tracked by `fjell-service-manager` (RFC 038).
///
/// Matches `fjell_abi::service::ServiceState` at the kernel level but adds
/// the RFC 038-specific states for READY-protocol tracking.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvcLifecycle {
    /// Service slot is unused.
    Empty       = 0,
    /// Spawned; waiting for READY message.
    Spawned     = 1,
    /// READY received within `START_TIMEOUT_MS`.
    Ready       = 2,
    /// Running normally.
    Running     = 3,
    /// READY not received within the timeout — start failed.
    StartFailed = 4,
    /// Service faulted after going Ready.
    Faulted     = 5,
}

/// Required extraction order for cooperative services (RFC 038 §"Required
/// initial separation order").
///
/// Each constant is the human-readable name used in manifest TOMLs and logs.
pub mod extraction_order {
    pub const ORDER: &[&str] = &[
        "storaged",
        "bootctl",
        "verifyd",
        "upgraded",
        "rootfsd",
        "snapshotd",
    ];
}

/// Manifest entry for a separated service (RFC 038 §"Service manifest").
///
/// The TOML loader in `fjell-service-manager` populates these.
#[derive(Clone, Debug)]
pub struct ServiceManifestEntry {
    pub name: [u8; 16],            // ASCII null-padded
    pub image_id: u16,
    pub start_timeout_ms: u64,
    pub ready_endpoint: u16,       // CSpace slot index of its ready endpoint
}

impl ServiceManifestEntry {
    /// Build a manifest entry with the default timeout.
    pub fn new(name: &[u8], image_id: u16, ready_endpoint: u16) -> Self {
        let mut n = [0u8; 16];
        for (i, &b) in name.iter().enumerate().take(15) { n[i] = b; }
        ServiceManifestEntry {
            name: n,
            image_id,
            start_timeout_ms: ready::START_TIMEOUT_MS,
            ready_endpoint,
        }
    }
}

// ── RFC 042 (v0.2.0): Negative-test marker constants ─────────────────────────
//
// Each constant is the exact string the relevant service or kernel prints to
// the QEMU serial log when a negative-test scenario is confirmed to behave
// correctly.  The `qemu-log-check` tool verifies these at CI time.
//
// Format: `NEG:<CATEGORY>:<DESCRIPTION>:PASS`
//
// Host tests that exercise the same logic at compile time are noted inline.

pub mod negative_markers {
    // ── capability enforcement ────────────────────────────────────────────────
    /// `require_cap` rejects a capability with the wrong kind.
    pub const CAP_WRONG_KIND: &str         = "NEG:CAP:WRONG_KIND_REJECTED:PASS";
    /// `require_cap` rejects a capability with insufficient rights.
    pub const CAP_RIGHTS_DENIED: &str      = "NEG:CAP:RIGHTS_DENIED:PASS";
    /// A lease-bound capability is rejected after the lease is revoked.
    pub const CAP_LEASE_REVOKED: &str      = "NEG:CAP:LEASE_REVOKED:PASS";
    /// `sys_cap_drop` succeeds even on a revoked capability.
    pub const CAP_DROP_ON_REVOKED: &str    = "NEG:CAP:DROP_ON_REVOKED:PASS";

    // ── blocked IPC revocation (RFC 034) ─────────────────────────────────────
    /// A task blocked in `ipc_call` is woken with `LeaseRevoked` when its
    /// endpoint cap's lease is revoked.
    pub const IPC_BLOCKED_CALL: &str       = "NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE:PASS";
    /// A task blocked in `ipc_recv` is woken with `LeaseRevoked`.
    pub const IPC_BLOCKED_RECV: &str       = "NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE:PASS";
    /// `ipc_reply` is rejected (silently dropped) when the call's lease
    /// was revoked while the caller was blocked.
    pub const IPC_LATE_REPLY: &str         = "NEG:IPC:LATE_REPLY_REJECTED:PASS";

    // ── MMIO boundary (RFC 035) ───────────────────────────────────────────────
    /// `sys_mmio_map` rejects a cap without `MMIO_MAP` right.
    pub const MMIO_RIGHTS: &str            = "NEG:MMIO:RIGHTS_CHECK:PASS";
    /// `sys_mmio_map` rejects an out-of-bounds offset+size.
    pub const MMIO_BOUNDS: &str            = "NEG:MMIO:BOUNDS_REJECTED:PASS";
    /// `sys_mmio_map` rejects a request that would map into kernel RAM.
    pub const MMIO_RAM_GUARD: &str         = "NEG:MMIO:RAM_GUARD_REJECTS:PASS";

    // ── DMA boundary (RFC 036) ────────────────────────────────────────────────
    /// Physical DMA page is zeroed when the owning task exits.
    pub const DMA_ZEROIZE_ON_EXIT: &str    = "NEG:DMA:ZEROIZE_ON_EXIT:PASS";
    /// `sys_dma_revoke` correctly zeroizes and frees the page.
    pub const DMA_REVOKE_EXPLICIT: &str    = "NEG:DMA:REVOKE_EXPLICIT:PASS";
    /// `sys_dma_alloc` rejects a cap without `DMA_ALLOC` right.
    pub const DMA_RIGHTS: &str             = "NEG:DMA:RIGHTS_CHECK:PASS";

    // ── cap-broker policy (RFC 040) ───────────────────────────────────────────
    /// An unknown service is denied by the default-deny policy.
    pub const POLICY_DEFAULT_DENY: &str    = "NEG:POLICY:DEFAULT_DENY:PASS";
    /// `CAP_REQUEST` sent before `BOOTSTRAP_COMPLETE` is rejected.
    pub const POLICY_BOOTSTRAP_GUARD: &str = "NEG:POLICY:BOOTSTRAP_GUARD:PASS";
    /// An explicit `Deny` rule takes precedence over an `Allow` rule.
    pub const POLICY_DENY_PRIORITY: &str   = "NEG:POLICY:DENY_PRIORITY:PASS";

    // ── safe user copy (RFC 039) ──────────────────────────────────────────────
    /// `copy_to_user` rejects a null destination pointer.
    pub const USER_COPY_NULL: &str         = "NEG:USER_COPY:NULL_REJECTED:PASS";
    /// `copy_to_user` rejects a kernel-space destination address.
    pub const USER_COPY_KERNEL_ADDR: &str  = "NEG:USER_COPY:KERNEL_ADDR_REJECTED:PASS";

    // ── service separation (RFC 038) ──────────────────────────────────────────
    /// Service-manager detects a service that failed to send READY in time.
    pub const SVC_START_TIMEOUT: &str      = "NEG:SVC:START_TIMEOUT_DETECTED:PASS";
    /// Service-manager detects a service fault after READY.
    pub const SVC_FAULT: &str             = "NEG:SVC:FAULT_DETECTED:PASS";

    // ── audit evidence (RFC 041) ──────────────────────────────────────────────
    /// Snapshot continuity check reports dropped records correctly.
    pub const AUDIT_EVIDENCE_GAP: &str     = "NEG:AUDIT:EVIDENCE_GAP_DETECTED:PASS";

    // ── v0.2 release gate ─────────────────────────────────────────────────────
    /// All v0.2 negative-test categories have been exercised.
    pub const V02_RELEASE_GATE: &str       = "TEST:V02:PASS";
}
