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
    /// Test-only (RFC-0.34-001 D11): registers as a presentation and faults on
    /// its first message, so a presentation that dies mid-run is evidence in a
    /// tier. `init` starts it only on the console byte `P`.
    pub const SVC_PRESENTATION_FAULT: ImageId = ImageId(0x20); // 32
    /// Test-only (RFC-0.34-004 D5/R6): installed with no dispatch capability
    /// at all, calls `DISPATCH_ACTION` claiming every right there is, so the
    /// claim's refusal is evidence in a tier that nothing it sends is ever
    /// read. `init` starts it only on the console byte `L`.
    pub const SVC_PRESENTATION_LIAR: ImageId = ImageId(0x21); // 33
}

/// The endpoint object `proxy-braille` receives its wake on.
pub const PROXY_BRAILLE_EP_OBJECT: u32 = 13;

// ── RFC-0.37-001 (closes E-078, E-081, E-083): fleetd becomes a service ──────
//
// `fjell-fleetd` was never spawned at all before this line — not merely a
// stub with a dead loop (E-081's own description), but absent from
// `image.rs`'s table, `qemu.rs`'s build list, and `init`'s own spawn
// sequence. "fleetd becomes a service" (R3) is literal.

impl ImageId {
    /// RFC-0.37-001: the node-side endpoint RFC-v0.8-005 specifies. `init`
    /// starts it only on the console byte `D`.
    pub const FLEETD: ImageId = ImageId(0x22); // 34
}

/// The one shared endpoint object `fjell-diagnosticsd` (slot 1,
/// `CAP_TOOLS_EP`) and `fjell-fleetd` (slot 1, `CAP_DIAGNOSTICSD_EP`) both
/// hold a SEND|RECV capability to — `fleetd` sends `BUILD_BUNDLE`/`PUSH` and
/// blocks for a reply, `diagnosticsd` receives and replies on the same
/// capability, the same shape as `UPDATE_CHANNEL_EP_OBJECT` above (not a
/// one-way wake: both sides both send a request and block on a reply).
pub const DIAGNOSTICSD_TOOLS_EP_OBJECT: u32 = 19;

/// The endpoint object `fleetd` receives its console-byte trigger on,
/// deliberately not object 0 (shared) — the same reasoning as
/// `NEG_TEST_EP_OBJECT`.
pub const FLEETD_TRIGGER_EP_OBJECT: u32 = 20;

/// The CSpace slot in `fleetd` that receives on `FLEETD_TRIGGER_EP_OBJECT`.
/// Not declared as a named `CapHandle` constant in `fleetd`'s own source —
/// `NEG_TEST_TRIGGER_RECV_SLOT` sets the precedent: a trigger-only slot is
/// passed as a raw slot number at its one call site, not given a local
/// name, because it addresses no peer `fleetd` needs to name for itself
/// (contrast `CAP_DIAGNOSTICSD_EP`, which does).
pub const FLEETD_TRIGGER_RECV_SLOT: u32 = 2;

/// The CSpace slot in `init` that sends to `FLEETD_TRIGGER_EP_OBJECT`, SEND
/// only — mirrors `INIT_NEG_TEST_SEND_SLOT`.
pub const INIT_FLEETD_SEND_SLOT: u32 = 10;

/// The endpoint object `svc-presentation-fault` receives its wake on.
pub const SVC_PRESENTATION_FAULT_EP_OBJECT: u32 = 15;

/// The endpoint object `svc-presentation-liar` receives on. Never woken (it
/// never asks), but every task still gets its own endpoint object allocated
/// and asserted at boot, the same as every other service.
pub const SVC_PRESENTATION_LIAR_EP_OBJECT: u32 = 16;

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

// ── RFC-0.35-001: the update path's metadata channel (E-072, E-070) ──────────
//
// `secure-transportd` and `fjell-upgraded` each address the other through a
// `CapHandle` their own source has always named — slot 2 on transportd's
// side, slot 6 on upgraded's — and the kernel has never installed a
// capability at either. Both sides both send and receive on their one slot
// (a request, then a blocking wait for the reply, in both directions), so
// this is one shared Endpoint object with a SEND|RECV capability in both
// CSpaces, not a one-way wake pattern like `semantic-stream`'s presentations.

/// The endpoint object the update path's metadata channel rendezvouses on.
/// `secure-transportd` (CSpace slot 2) and `fjell-upgraded` (CSpace slot 6)
/// each hold a SEND|RECV capability to it — see `spawn.rs`'s install, the
/// only other place this object id appears.
pub const UPDATE_CHANNEL_EP_OBJECT: u32 = 17;

/// `verifyd`'s own dedicated endpoint object (RFC-0.35-001 R7). Before this
/// line `verifyd` was never in `spawn.rs`'s `ep_obj` match, so its slot-0
/// capability defaulted to the shared object every untouched image falls
/// back to — harmless only because nothing had ever called it
/// (`VERIFY`'s dispatch arm was dead code). `neg-test` is the first real
/// caller; a shared object would have risked it, or `verifyd`, receiving a
/// message meant for whichever other default-path service got there first.
pub const VERIFYD_EP_OBJECT: u32 = 18;

// ── RFC-0.41-002: no service receives on a shared endpoint object ────────────
//
// Until this line `spawn.rs`'s `ep_obj` match ended `_ => 0`: every image without a
// dedicated arm was handed slot 0 pointing at endpoint object 0, with
// `CapRights::ALL_NON_META` (which includes SEND). Three of those images actually
// used it to RECEIVE -- `configd`'s serving loop, `neg-test`'s blocked-receive tests
// (on copies of its slot 0), and `netd`'s receive (dead code behind its own early
// exit) -- and nothing sent to the object, which is the only reason the race had
// never fired. The default is gone: an image without a dedicated arm gets **no**
// endpoint capability in slot 0, and these three earn theirs.

/// `netd`'s own endpoint object (its slot 0). `netd` exits before its event loop by
/// design (a smoke-test stub), so nothing receives on it today; the object exists
/// so that the day the loop runs it owns what it waits on.
pub const NETD_EP_OBJECT: u32 = 21;

/// `configd`'s own endpoint object (its slot 0), where its serving loop receives.
/// Nothing sends to it today either.
pub const CONFIGD_EP_OBJECT: u32 = 22;

/// `neg-test`'s *scratch* endpoint object (its slot 0): the capability its
/// negative tests copy, mint from, hand to the wrong-kind syscalls and receive on
/// (through copies). Deliberately **not** `NEG_TEST_EP_OBJECT` (14), where `init`'s
/// reboot/DMA trigger arrives: a test blocked in a receive on its scratch endpoint
/// must not be able to swallow the trigger.
pub const NEG_TEST_OWN_EP_OBJECT: u32 = 23;

/// How many endpoint objects the kernel allocates at boot: the highest object
/// id above, plus one. `crates/fjell-kernel/src/main.rs` asserts that each
/// `et.alloc()` returns the object its constant names, and a host test counts
/// the `et.alloc()` calls against this number — because a constant that names
/// an object nobody allocated fails every IPC to it with `InvalidCap`, from a
/// capability that is itself perfectly valid, and that mistake is recorded in
/// `main.rs`'s own comments for cap-broker, sample-service, the service-manager
/// pair and `bootctl` (RFC-0.33-001 D8) — repeatedly.
pub const ENDPOINT_OBJECT_COUNT: u32 = 24;

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

    #[test]
    fn image_id_fleetd_value_stable() {
        // RFC-0.37-001: one past SVC_PRESENTATION_LIAR (0x21 = 33), the
        // highest id before this line.
        assert_eq!(ImageId::FLEETD.0, 0x22);
        assert!(ImageId::FLEETD.0 > ImageId::SVC_PRESENTATION_LIAR.0);
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
            (
                "SVC_PRESENTATION_FAULT_EP_OBJECT",
                SVC_PRESENTATION_FAULT_EP_OBJECT,
            ),
            (
                "SVC_PRESENTATION_LIAR_EP_OBJECT",
                SVC_PRESENTATION_LIAR_EP_OBJECT,
            ),
            ("UPDATE_CHANNEL_EP_OBJECT", UPDATE_CHANNEL_EP_OBJECT),
            ("VERIFYD_EP_OBJECT", VERIFYD_EP_OBJECT),
            ("DIAGNOSTICSD_TOOLS_EP_OBJECT", DIAGNOSTICSD_TOOLS_EP_OBJECT),
            ("FLEETD_TRIGGER_EP_OBJECT", FLEETD_TRIGGER_EP_OBJECT),
            ("NETD_EP_OBJECT", NETD_EP_OBJECT),
            ("CONFIGD_EP_OBJECT", CONFIGD_EP_OBJECT),
            ("NEG_TEST_OWN_EP_OBJECT", NEG_TEST_OWN_EP_OBJECT),
        ] {
            assert!(id < ENDPOINT_OBJECT_COUNT, "{name} = {id} is not allocated");
        }
    }

    #[test]
    fn the_spawn_tables_literal_endpoint_ids_are_allocated() {
        // The `ep_obj` match in spawn.rs: `ImageId::X => Some(N),` arms.
        let start = SPAWN
            .find("let ep_obj: Option<u32> = match image_id {")
            .expect("spawn.rs no longer has the ep_obj table this test reads");
        // RFC-0.41-002 R3: the default arm grants NOTHING. This is the host-side half of
        // that property (the other half is `capability-topology`): `_ => None`, not a
        // number -- a default that names an object is a shared object again.
        let end = start
            + SPAWN[start..]
                .find("_ => None,")
                .expect("ep_obj's default arm is not `_ => None` (RFC-0.41-002 R3)");
        let mut seen = 0;
        for line in SPAWN[start..end].lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("fjell_abi::service::ImageId::") {
                if let Some(n) = rest.split("=>").nth(1) {
                    let n = n.trim().trim_end_matches(',');
                    let n = n.strip_prefix("Some(").and_then(|r| r.strip_suffix(')'));
                    if let Some(Ok(id)) = n.map(|n| n.trim().parse::<u32>()) {
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
