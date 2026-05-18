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
