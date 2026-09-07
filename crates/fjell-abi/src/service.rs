//! Service-layer ABI types shared between kernel and user space.

/// Packed image identifier used with `TaskSpawn`.
///
/// The kernel maintains a static table of embedded service images; this ID
/// selects which image to load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct ImageId(pub u16);

impl ImageId {
    pub const INIT: ImageId = ImageId(0);
    pub const CONFIGD: ImageId = ImageId(1);
    pub const CAP_BROKER: ImageId = ImageId(2);
    pub const AUDITD: ImageId = ImageId(3);
    pub const SERVICE_MANAGER: ImageId = ImageId(4);
    pub const SAMPLE_SERVICE: ImageId = ImageId(5);
}

/// Task lifecycle state as reported by `TaskStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskLifecycle {
    Created = 0,
    Runnable = 1,
    Running = 2,
    Blocked = 3,
    Exited = 4,
    Faulted = 5,
}

/// Service-level lifecycle tracked by `fjell-service-manager`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceState {
    Declared = 0,
    WaitingDependencies = 1,
    Spawning = 2,
    Starting = 3,
    Running = 4,
    Ready = 5,
    Degraded = 6,
    Restarting = 7,
    Failed = 8,
    Exited = 9,
    Tombstoned = 10,
}

/// Service identifier (16 ASCII bytes, null-padded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ServiceId(pub [u8; 16]);

impl ServiceId {
    pub const fn from_bytes(b: &[u8]) -> Self {
        let mut arr = [0u8; 16];
        let mut i = 0;
        while i < b.len() && i < 16 {
            arr[i] = b[i];
            i += 1;
        }
        ServiceId(arr)
    }
    pub const fn init() -> Self {
        Self::from_bytes(b"svc.init")
    }
    pub const fn configd() -> Self {
        Self::from_bytes(b"svc.configd")
    }
    pub const fn cap_broker() -> Self {
        Self::from_bytes(b"svc.cap-broker")
    }
    pub const fn auditd() -> Self {
        Self::from_bytes(b"svc.auditd")
    }
    pub const fn service_manager() -> Self {
        Self::from_bytes(b"svc.svc-manager")
    }
    pub const fn sample_service() -> Self {
        Self::from_bytes(b"svc.sample")
    }
}

impl ImageId {
    // M5 additions
    pub const SEMANTIC_STREAM: ImageId = ImageId(6);
    pub const PROXY_TEXT: ImageId = ImageId(7);
}

impl ImageId {
    // M6 additions
    pub const DEVMGR: ImageId = ImageId(8);
    pub const DRIVER_VIRTIO_BLK: ImageId = ImageId(9);
    pub const STORAGED: ImageId = ImageId(10);
    pub const BOOTCTL: ImageId = ImageId(11);
    pub const UPGRADED: ImageId = ImageId(12);
    pub const POWERD: ImageId = ImageId(13);
}

impl ImageId {
    // M7 additions
    pub const VERIFYD: ImageId = ImageId(14);
    pub const ROOTFSD: ImageId = ImageId(15);
    pub const SNAPSHOTD: ImageId = ImageId(16);
}

impl ImageId {
    // M8 additions
    pub const MEASUREDD: ImageId = ImageId(17);
    pub const ATTESTD: ImageId = ImageId(18);
    pub const RECOVERYD: ImageId = ImageId(19);
}

impl ImageId {
    /// v0.2: dedicated negative-test service (RFC 042).
    pub const NEG_TEST: ImageId = ImageId(20);
    /// RFC 042: service that never sends READY (start-timeout test).
    pub const SVC_TIMEOUT: ImageId = ImageId(21);
    /// RFC 042: service that sends READY then faults (fault-detected test).
    pub const SVC_FAULT: ImageId = ImageId(22);
}

// ── v0.4 networking + v0.7 distributed sync (RFC-v0.7.1-003) ─────────────────

impl ImageId {
    // v0.4 networking services
    pub const DRIVER_VIRTIO_NET: ImageId = ImageId(0x17); // 23
    pub const NETD: ImageId = ImageId(0x18); // 24
    pub const SECURE_TRANSPORTD: ImageId = ImageId(0x19); // 25
    pub const DIAGNOSTICSD: ImageId = ImageId(0x1A); // 26

    // v0.7 distributed sync services
    pub const IDENTITYD: ImageId = ImageId(0x1B); // 27
    pub const SUMMARYD: ImageId = ImageId(0x1C); // 28
    pub const SYNCD: ImageId = ImageId(0x1D); // 29
}

impl ImageId {
    /// RFC-0.25-001: the external interrupt plane's first console input path.
    pub const DRIVER_UART: ImageId = ImageId(0x1E); // 30
}

// ── RFC-0.28-001: readiness topology ──────────────────────────────────────────
//
// Readiness has its own dedicated endpoint objects and CSpace slots,
// distinct from object 0 ("shared, all non-special services"). Object 0
// turned out to have at least two other uncoordinated receivers (auditd's
// audit-drain trigger, bootctl's own protocol) racing service-manager for
// the same messages — see
// docs/rfcs/RFC-0.28-001-readiness-topology-answer.md §3. Defined here
// (not in `fjell-service-api`) because `spawn.rs` — kernel-side — must
// install these capabilities, and the kernel does not depend on the
// user-space service SDK crate.

/// The endpoint object service-manager receives `SERVICE_READY` on.
/// Installed as service-manager's own identity endpoint (`ep_obj` table,
/// `spawn.rs`) — replacing its previous accidental default to object 0.
pub const SERVICE_MANAGER_EP_OBJECT: u32 = 10;

/// The CSpace slot installed, unconditionally, in every spawned service's
/// own CSpace, pointing at `SERVICE_MANAGER_EP_OBJECT` with SEND rights
/// only. Every announcer sends readiness through this slot — never
/// through slot 0, whose meaning is each service's own identity endpoint
/// and is unrelated to readiness.
pub const SERVICE_READY_SEND_SLOT: u32 = 20;

/// The endpoint object service-manager relays per-service readiness onto,
/// for `init` alone. `init` holds the only receive capability here;
/// service-manager holds the only send capability. Nothing else ever
/// touches this object, so a receive here can never collide with
/// unrelated traffic the way object 0 did.
pub const INIT_RELAY_EP_OBJECT: u32 = 11;

/// The CSpace slot installed only in service-manager's own CSpace,
/// pointing at `INIT_RELAY_EP_OBJECT` with SEND rights, used to relay
/// each of storaged/measuredd/attestd/recoveryd's readiness to `init`.
pub const INIT_RELAY_SEND_SLOT: u32 = 21;

/// The CSpace slot installed only in `init`'s own CSpace, pointing at
/// `INIT_RELAY_EP_OBJECT` with RECEIVE rights — reusing the slot
/// RFC-0.26-004 vacated when it removed `init`'s capability to proxy-text
/// (object 8) entirely.
pub const INIT_RELAY_RECV_SLOT: u32 = 7;

#[cfg(test)]
mod image_id_v07_tests {
    use super::ImageId;

    #[test]
    fn image_id_v04_values_stable() {
        assert_eq!(ImageId::DRIVER_VIRTIO_NET.0, 0x17);
        assert_eq!(ImageId::NETD.0, 0x18);
        assert_eq!(ImageId::SECURE_TRANSPORTD.0, 0x19);
        assert_eq!(ImageId::DIAGNOSTICSD.0, 0x1A);
    }

    #[test]
    fn image_id_v07_values_stable() {
        assert_eq!(ImageId::IDENTITYD.0, 0x1B);
        assert_eq!(ImageId::SUMMARYD.0, 0x1C);
        assert_eq!(ImageId::SYNCD.0, 0x1D);
    }

    #[test]
    fn image_id_no_overlap_with_v03() {
        // v0.3 max was SVC_FAULT = 22 = 0x16
        assert_eq!(ImageId::SVC_FAULT.0, 22);
        assert!(ImageId::DRIVER_VIRTIO_NET.0 > ImageId::SVC_FAULT.0);
    }
}
