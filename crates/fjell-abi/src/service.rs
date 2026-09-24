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
// rfcs/answers/RFC-0.28-001-readiness-topology-answer.md §3. Defined here
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

// ── RFC-0.33-001 D8: bootctl's own endpoint ───────────────────────────────────
//
// `bootctl` was one of the two extra receivers on object 0 the comment above
// already named (the other, auditd's audit-drain trigger, was never fixed
// either). A health report meant for `bootctl` could be received by whichever
// other shared-object-0 service reached `ipc_recv` first. Same fix as
// RFC-0.28-001, one object pair, applied here.

/// The endpoint object `bootctl` receives `BOOT_*` messages on. Installed as
/// `bootctl`'s own identity endpoint (`ep_obj` table, `spawn.rs`), replacing
/// its previous default to shared object 0.
pub const BOOTCTL_EP_OBJECT: u32 = 12;

/// The CSpace slot installed only in `service-manager`'s own CSpace, pointing
/// at `BOOTCTL_EP_OBJECT` with SEND rights, used to deliver
/// `tags::BOOT_HEALTH_OK` / `tags::BOOT_HEALTH_FAILED`. `bootctl` checks the
/// sender's kernel-attested image id against `ImageId::SERVICE_MANAGER`, not
/// this slot's existence alone, so a copy of this capability granted
/// elsewhere would still be refused — but nothing else is granted one.
pub const BOOTCTL_HEALTH_SEND_SLOT: u32 = 22;

// ── RFC-0.34-001 D8: presentations that ask, and their endpoints ──────────────
//
// `semantic-stream` never initiates a blocking IPC to a presentation: each one
// asks the stream for its next message and is answered by reply, and is woken
// through an endpoint of its own. See `fjell-semantic-fanout` and
// `rfcs/answers/RFC-0.34-001-a-second-presentation-answer.md`.

impl ImageId {
    /// The second presentation: uncontracted braille cells, written to the
    /// console as the stream a display driver would consume (RFC-0.34-001 §A).
    pub const PROXY_BRAILLE: ImageId = ImageId(0x1F); // 31
}

/// The endpoint object `proxy-braille` receives its wake on.
pub const PROXY_BRAILLE_EP_OBJECT: u32 = 13;

// ── RFC-0.33-001 D17: the reset trigger must not survive the reset ────────────
//
// `neg-test` runs a scenario that resets the machine. Its trigger used to be a
// virtio entropy device on the bus, which is still there after the reset — a
// reboot loop on hardware whose only fault is having an entropy source, hidden
// in the one place it would be observed because the harness passes `-no-reboot`.
// The trigger is now a console byte, which does not survive: injected once by an
// external agent, absent on the next boot. `init` reads it and sends
// `neg-test` one message over an endpoint that is `neg-test`'s alone.

/// The endpoint object `neg-test` receives its trigger on. Deliberately not
/// object 0, which `neg-test` shares with other services (and which raced
/// before RFC-0.28-001).
pub const NEG_TEST_EP_OBJECT: u32 = 14;

/// The CSpace slot in `neg-test` that receives on `NEG_TEST_EP_OBJECT`.
pub const NEG_TEST_TRIGGER_RECV_SLOT: u32 = 8;

/// The CSpace slot in `init` that sends to `NEG_TEST_EP_OBJECT`, SEND only.
pub const INIT_NEG_TEST_SEND_SLOT: u32 = 9;

/// How many endpoint objects the kernel allocates at boot: the highest object
/// id above, plus one. `crates/fjell-kernel/src/main.rs` asserts that each
/// `et.alloc()` returns the object its constant names, and a host test counts
/// the `et.alloc()` calls against this number — because a constant that names
/// an object nobody allocated fails every IPC to it with `InvalidCap`, from a
/// capability that is itself perfectly valid, and that mistake is recorded in
/// `main.rs`'s own comments for cap-broker, sample-service, the service-manager
/// pair and `bootctl` (RFC-0.33-001 D8) — repeatedly.
pub const ENDPOINT_OBJECT_COUNT: u32 = 15;

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

/// RFC-0.34-001 §B: the endpoint-allocation trap, closed by a test instead of a
/// comment. Endpoint objects are allocated by `et.alloc()` calls in the
/// kernel's `main.rs`; the ids they return were thrown away, so the constants
/// above and the allocations were related only by statement order. The kernel
/// now asserts each new id at boot; this test counts the allocations against
/// `ENDPOINT_OBJECT_COUNT` and checks that no object id the spawn table names
/// is beyond them — the omission that cost RFC-0.33-001 a live defect.
///
/// It reads two kernel source files as text. That is blunt, and it is here
/// because the alternative is a test that cannot see the kernel at all.
#[cfg(test)]
mod endpoint_allocation_tests {
    use super::*;

    const KERNEL_MAIN: &str = include_str!("../../fjell-kernel/src/main.rs");
    const SPAWN: &str = include_str!("../../fjell-kernel/src/task/spawn.rs");

    #[test]
    fn every_endpoint_object_is_allocated_at_boot() {
        // Every allocation's `.expect("alloc ... endpoint")` line. The control
        // is the count itself: it must be non-zero and equal the constant.
        let allocations = KERNEL_MAIN
            .lines()
            .filter(|l| l.contains(".expect(\"alloc") && l.contains("endpoint"))
            .count() as u32;
        assert!(allocations > 0, "the pattern matched nothing in main.rs");
        assert_eq!(
            allocations, ENDPOINT_OBJECT_COUNT,
            "main.rs allocates {allocations} endpoints; ENDPOINT_OBJECT_COUNT says {ENDPOINT_OBJECT_COUNT}"
        );
    }

    #[test]
    fn every_named_endpoint_object_is_below_the_count() {
        for (name, id) in [
            ("SERVICE_MANAGER_EP_OBJECT", SERVICE_MANAGER_EP_OBJECT),
            ("INIT_RELAY_EP_OBJECT", INIT_RELAY_EP_OBJECT),
            ("BOOTCTL_EP_OBJECT", BOOTCTL_EP_OBJECT),
            ("PROXY_BRAILLE_EP_OBJECT", PROXY_BRAILLE_EP_OBJECT),
            ("NEG_TEST_EP_OBJECT", NEG_TEST_EP_OBJECT),
        ] {
            assert!(id < ENDPOINT_OBJECT_COUNT, "{name} = {id} is not allocated");
        }
    }

    #[test]
    fn the_spawn_tables_literal_endpoint_ids_are_allocated() {
        // The `ep_obj` match in spawn.rs: `ImageId::X => N,` arms.
        let start = SPAWN
            .find("let ep_obj: u32 = match image_id {")
            .expect("spawn.rs no longer has the ep_obj table this test reads");
        let end = start
            + SPAWN[start..]
                .find("_ => 0,")
                .expect("ep_obj has no default arm");
        let mut seen = 0;
        for line in SPAWN[start..end].lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("fjell_abi::service::ImageId::") {
                if let Some(n) = rest.split("=>").nth(1) {
                    if let Ok(id) = n.trim().trim_end_matches(',').parse::<u32>() {
                        seen += 1;
                        assert!(
                            id < ENDPOINT_OBJECT_COUNT,
                            "spawn.rs names object {id}: `{line}`"
                        );
                    }
                }
            }
        }
        assert!(
            seen >= 8,
            "read only {seen} literal arms; the parse is not seeing the table"
        );
    }
}
