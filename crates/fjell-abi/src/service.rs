//! Service-layer ABI types shared between kernel and user space.

/// Packed image identifier used with `TaskSpawn`.
///
/// The kernel maintains a static table of embedded service images; this ID
/// selects which image to load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct ImageId(pub u16);

impl ImageId {
    pub const INIT:            ImageId = ImageId(0);
    pub const CONFIGD:         ImageId = ImageId(1);
    pub const CAP_BROKER:      ImageId = ImageId(2);
    pub const AUDITD:          ImageId = ImageId(3);
    pub const SERVICE_MANAGER: ImageId = ImageId(4);
    pub const SAMPLE_SERVICE:  ImageId = ImageId(5);
}

/// Task lifecycle state as reported by `TaskStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskLifecycle {
    Created   = 0,
    Runnable  = 1,
    Running   = 2,
    Blocked   = 3,
    Exited    = 4,
    Faulted   = 5,
}

/// Service-level lifecycle tracked by `fjell-service-manager`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceState {
    Declared             = 0,
    WaitingDependencies  = 1,
    Spawning             = 2,
    Starting             = 3,
    Running              = 4,
    Ready                = 5,
    Degraded             = 6,
    Restarting           = 7,
    Failed               = 8,
    Exited               = 9,
    Tombstoned           = 10,
}

/// Service identifier (16 ASCII bytes, null-padded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ServiceId(pub [u8; 16]);

impl ServiceId {
    pub const fn from_bytes(b: &[u8]) -> Self {
        let mut arr = [0u8; 16];
        let mut i = 0;
        while i < b.len() && i < 16 { arr[i] = b[i]; i += 1; }
        ServiceId(arr)
    }
    pub const fn init()            -> Self { Self::from_bytes(b"svc.init") }
    pub const fn configd()         -> Self { Self::from_bytes(b"svc.configd") }
    pub const fn cap_broker()      -> Self { Self::from_bytes(b"svc.cap-broker") }
    pub const fn auditd()          -> Self { Self::from_bytes(b"svc.auditd") }
    pub const fn service_manager() -> Self { Self::from_bytes(b"svc.svc-manager") }
    pub const fn sample_service()  -> Self { Self::from_bytes(b"svc.sample") }
}

impl ImageId {
    // M5 additions
    pub const SEMANTIC_STREAM: ImageId = ImageId(6);
    pub const PROXY_TEXT:      ImageId = ImageId(7);
}

impl ImageId {
    // M6 additions
    pub const DEVMGR:            ImageId = ImageId(8);
    pub const DRIVER_VIRTIO_BLK: ImageId = ImageId(9);
    pub const STORAGED:          ImageId = ImageId(10);
    pub const BOOTCTL:           ImageId = ImageId(11);
    pub const UPGRADED:          ImageId = ImageId(12);
    pub const POWERD:            ImageId = ImageId(13);
}

impl ImageId {
    // M7 additions
    pub const VERIFYD:   ImageId = ImageId(14);
    pub const ROOTFSD:   ImageId = ImageId(15);
    pub const SNAPSHOTD: ImageId = ImageId(16);
}

impl ImageId {
    // M8 additions
    pub const MEASUREDD:  ImageId = ImageId(17);
    pub const ATTESTD:    ImageId = ImageId(18);
    pub const RECOVERYD:  ImageId = ImageId(19);
}

impl ImageId {
    /// v0.2: dedicated negative-test service (RFC 042).
    pub const NEG_TEST: ImageId = ImageId(20);
    /// RFC 042: service that never sends READY (start-timeout test).
    pub const SVC_TIMEOUT: ImageId = ImageId(21);
    /// RFC 042: service that sends READY then faults (fault-detected test).
    pub const SVC_FAULT:   ImageId = ImageId(22);
}

// ── v0.4 networking + v0.7 distributed sync (RFC-v0.7.1-003) ─────────────────

impl ImageId {
    // v0.4 networking services
    pub const DRIVER_VIRTIO_NET:   ImageId = ImageId(0x17);  // 23
    pub const NETD:                ImageId = ImageId(0x18);  // 24
    pub const SECURE_TRANSPORTD:   ImageId = ImageId(0x19);  // 25
    pub const DIAGNOSTICSD:        ImageId = ImageId(0x1A);  // 26

    // v0.7 distributed sync services
    pub const IDENTITYD:           ImageId = ImageId(0x1B);  // 27
    pub const SUMMARYD:            ImageId = ImageId(0x1C);  // 28
    pub const SYNCD:               ImageId = ImageId(0x1D);  // 29
}

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
