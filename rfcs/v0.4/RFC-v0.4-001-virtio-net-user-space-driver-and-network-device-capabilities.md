# RFC-v0.4-001: virtio-net User-Space Driver and Network Device Capabilities

## Status

Draft (revised, supersedes pack v0.4-001 draft)

## Target Version

`v0.4.0`.

## Phase

Minimal Secure Control-Plane Networking — Epic A (Network Device Driver).

## Related Work

- v0.2 RFC 035 — `MmioRegion` capability (the driver consumes one).
- v0.2 RFC 036 — `DmaRegion` capability with zeroize/quarantine (the driver
  consumes RX/TX rings as DMA capabilities).
- v0.2 RFC 052 — DMA table-full rollback and cap-based revoke.
- v0.4 RFCs 002, 003 — consumers of this driver's `NetDeviceHandle`.
- v0.5 RFC 002 — devmgr device-discovery strategy; in v0.4 the device table is
  static-but-bounded.

---

## 1. Summary

Introduce a user-space `driver-virtio-net` service that owns a single
virtio-mmio network interface through:

- one `MmioRegion` capability for the device registers;
- two `DmaRegion` capabilities (RX ring, TX ring);
- one `Interrupt` capability for the device's IRQ line;
- one `NetDevice` capability that downstream services (`netd`) use to address
  the interface.

The driver exposes a narrow, packet-shaped IPC protocol to `netd` and **never
parses packet payloads**. All higher-layer protocol handling lives in `netd`
(RFC v0.4-002) and beyond.

The kernel gains:

- a new capability kind `Interrupt` with rights `BIND | UNBIND | ACK`;
- a new capability kind `NetDevice` with rights `SEND_PACKET |
  RECV_PACKET | CONTROL`;
- one new syscall: `sys_irq_wait`.

No protocol stack lives in the kernel.

---

## 2. Motivation

Networking has been deferred since v0.1. The v0.2 boundary hardening is
complete and adding networking now is safe only if every piece of attack
surface is capability-bounded.

Putting the driver in user space:

- preserves the mechanism-only kernel commitment;
- inherits DMA zeroize/quarantine for free (v0.2 RFC 036);
- localises crash recovery — driver restart cannot panic the kernel.

A kernel-side stack would re-couple Fjell to a generic-OS model. The
virtio-mmio profile is chosen because QEMU's `-device virtio-net-device`
maps to a stable, well-documented register layout and works on the existing
`virt` machine target.

---

## 3. Goals

```text
- User-space driver. Kernel never touches packet bytes.
- All driver authority via explicit capabilities (no ambient).
- RX/TX rings as DmaRegion capabilities; revoke unmaps, zeroizes,
  and quarantines per v0.2 RFC 036.
- IRQ delivery via a new sys_irq_wait that blocks the calling task until the
  bound IRQ fires.
- NetDevice capability hands packets to/from netd over a fixed protocol.
- Driver fault triggers DeviceRevoked event; devmgr restart policy applies.
- ≥ 8 negative tests covering the device boundary.
```

## 4. Non-Goals

```text
- No virtio-pci. QEMU virt's pci path is more code surface; deferred.
- No multi-queue. One RX queue, one TX queue.
- No offload features (no TSO, no GSO, no checksum offload).
- No DHCP, no IPv6, no IPv4 routing (netd's protocol set is RFC v0.4-002).
- No promiscuous mode.
- No driver hot-plug (devices discovered once at boot).
- No raw sockets exposed to user services.
```

---

## 5. External Design

### 5.1 Capability model

```text
devmgr                        cap-broker
  │                              │
  │ MMIO + DMA + IRQ caps        │ NetDevice cap
  ▼                              ▼
driver-virtio-net  ───────►  netd
  ▲                              ▲
  │ sys_irq_wait                 │
  └─────── kernel (mechanism) ───┘
```

The driver receives 4 capabilities at startup (sent by `devmgr` over a
bootstrap endpoint) and one synthesised capability (`NetDevice`) it produces
itself and grants to `netd` via `cap-broker`.

### 5.2 NetDevice protocol (driver ↔ netd)

The driver exposes a single endpoint. Tags use the v0.2 16-bit space:

```text
Tag                  Direction       Payload (w0..w3)
NET_PACKET_RX        driver → netd   pkt_len u16, ring_idx u16, flags u32, opaque
NET_PACKET_TX        netd → driver   pkt_len u16, ring_idx u16, flags u32, opaque
NET_TX_DONE          driver → netd   ring_idx u16, status u8
NET_LINK_UP          driver → netd   mtu u16, mac0_u32, mac1_u32, _
NET_LINK_DOWN        driver → netd   reason u8
NET_REVOKED          driver → netd   reason u8
NET_QUERY_STATE      netd → driver   _
NET_QUERY_REPLY      driver → netd   state u8, mtu u16, queue_avail u16
NET_DRIVER_READY     driver → smgr   _
```

Packet bytes are transferred through *shared DMA pages*. The protocol carries
only ring indices.

### 5.3 IRQ model

```rust
// New syscall:
fn sys_irq_wait(irq_handle: CapHandle) -> Result<(), AbiError>;
```

`sys_irq_wait` blocks the calling task until the IRQ bound to the
`Interrupt` capability fires, then returns. The kernel acks the IRQ at the
PLIC level only after the user-space handler calls a follow-up `sys_irq_ack`.

```rust
fn sys_irq_ack(irq_handle: CapHandle) -> Result<(), AbiError>;
```

This lets the driver coalesce work before re-arming.

---

## 6. Data Model

### 6.1 Kernel side: new capability kinds

```rust
#[repr(u16)]
pub enum CapKind {
    // ... existing kinds (TaskControl, Endpoint, MmioRegion, DmaRegion, ...) ...
    Interrupt = 0x0014,
    NetDevice = 0x0015,
}

pub struct CapRights(pub u32);

impl CapRights {
    // existing rights ...
    pub const IRQ_BIND:    Self = Self(1 << 27);
    pub const IRQ_UNBIND:  Self = Self(1 << 28);
    pub const IRQ_ACK:     Self = Self(1 << 29);
    pub const NET_SEND:    Self = Self(1 << 30);
    pub const NET_RECV:    Self = Self(1 << 31);
    // NET_CONTROL reuses CAP_INSTALL — see open question 14.1
}
```

### 6.2 Interrupt object

```rust
pub struct InterruptObject {
    pub irq_line:     u16,
    pub plic_priority: u8,
    pub bound_task:   Option<TaskId>,
    pub waker:        Option<TaskId>,    // currently sys_irq_wait-blocked
    pub state:        InterruptState,    // Active | Quiesced | Faulted
    pub lease:        LeaseId,
}
```

### 6.3 NetDevice object

```rust
pub struct NetDeviceObject {
    pub device_id:    DeviceId,
    pub mac:          [u8; 6],
    pub mtu:          u16,
    pub link_up:      bool,
    pub rx_ring:      DmaRegionId,
    pub tx_ring:      DmaRegionId,
    pub mmio_region:  MmioRegionId,
    pub irq:          InterruptId,
    pub lease:        LeaseId,
    pub state:        NetDeviceState,    // Initialising | Ready | Faulted | Revoked
}
```

### 6.4 Shared ring layout (DMA)

```text
RX ring (one DMA page, 4 KiB):
  - 16 descriptors, each 256 B (header + payload up to ~240 B).
  - Header:
      u16 len (BE on virtio-net spec, LE on the inner Fjell encoding)
      u16 flags
      u32 used_seq
  - Driver writes payload bytes into the descriptor and posts ring_idx.

TX ring (one DMA page, 4 KiB):
  - Same shape; netd writes payload, driver consumes.
```

Single-page rings keep the DMA capability single-frame. Larger MTU support
requires multi-frame DMA (DmaRegion already supports it) and is deferred to a
patch RFC if traffic shapes demand it.

---

## 7. Internal Design

### 7.1 Driver state machine

```text
[Boot] ──cap install──► [Init]
                          │ read device registers, negotiate features
                          ▼
                        [Ready] ────── irq ──► [HandleRx]
                          │                       │
                          │ ◄── irq_ack ──────────┘
                          │
                          │ revoke / fault
                          ▼
                        [Faulted] ───── reset ────► [Quiesced]
                                                       │
                                                       ▼
                                                    [Withdrawn]
```

### 7.2 Driver crash policy

- Panic in driver → kernel terminates the driver task and emits `TaskFaulted`.
- devmgr observes the fault, increments restart counter.
- If restart count exceeds threshold (`MAX_RESTARTS=3`), devmgr marks device
  `Quarantined` and refuses further restarts until operator intervention via
  the recovery flow.
- DMA zeroize/quarantine (v0.2 RFC 036) runs on every fault — buffers are
  never reused without zeroization.

### 7.3 IRQ wait semantics

```text
sys_irq_wait blocks:
  - kernel checks: cap valid? right IRQ_BIND? lease active?
  - if irq has pending count > 0, decrement and return immediately
  - else mark task as waker; set state Blocked
  - on irq fire: kernel marks task Runnable; user-space resumes

sys_irq_ack:
  - kernel checks IRQ_ACK right
  - re-enables the PLIC line
  - if more events buffered, wakeable count incremented
```

Negative cases: revoke of the `Interrupt` capability wakes the blocked task
with `AbiError::LeaseRevoked` and clears the waker.

### 7.4 Reset path

A driver-initiated reset is required when zeroize-on-revoke runs while the
device might still be DMAing. Sequence:

```text
1. driver issues virtio reset (write 0 to status register)
2. driver waits up to MAX_RESET_NS for device-ack
3. if device fails to ack, devmgr triggers PLIC-level mask and emits
   DeviceQuarantined; DMA region transitions to Quarantined per v0.2 RFC 036
4. eventually device-reset timeout zeroizes pages and frees the region
```

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-60: User task obtains a NetDevice cap without going through
             cap-broker and exfiltrates packets.
Mitigation:  cap-broker is the only path that mints NetDevice handles for
             non-driver tasks; require_cap checks NET_SEND/NET_RECV.

Threat T-61: Driver task is compromised and writes arbitrary device-register
             values to disable MMU.
Mitigation:  MmioRegion capability covers only the device-register page; RAM
             MMIO mapping is rejected (existing v0.2 invariant).

Threat T-62: Compromised driver poisons RX ring with malformed packets.
Mitigation:  netd parses all packet bytes; driver passes through only.
             Driver crash → DeviceRevoked → netd discards any in-flight
             rings; DMA pages zeroized.

Threat T-63: IRQ storm from a malicious or broken device.
Mitigation:  kernel applies PLIC-level rate-limit (configurable per IRQ); on
             excess, sys_irq_wait returns IrqStorm and IRQ is masked.

Threat T-64: Driver enters infinite loop without acking IRQs (livelock).
Mitigation:  timer preemption (v0.2) still preempts driver task; service-
             manager observes missed health and restarts driver after
             threshold.
```

### 8.2 Default-deny posture

- No task obtains any `Interrupt`, `MmioRegion`, `DmaRegion`, or `NetDevice`
  cap without an explicit `cap-broker` grant.
- The driver itself receives its 4 caps via a one-shot bootstrap endpoint
  whose authority is provisioned by devmgr at start-up.

### 8.3 Audit emission

```text
NetDriverStarted          { device_id, mac }
NetDriverFaulted          { device_id, reason_code }
NetDeviceRevoked          { device_id, reason_code }
IrqBindFailed             { irq_line, error }
IrqStormDetected          { irq_line, observed_rate }
DmaQuarantineNet          { region_id, reason_code }
```

---

## 9. Memory / DMA Design

- RX ring: 1 × 4 KiB DmaRegion, 16 descriptors of 256 B.
- TX ring: 1 × 4 KiB DmaRegion, 16 descriptors of 256 B.
- Driver private scratch: 1 × 4 KiB user page, *not* DMA (no device access).
- Stack: 64 KiB per existing service convention.

Both rings revoke independently. Revoke of RX leaves TX usable; the driver
emits a partial `NetDeviceState::Degraded` event. Full revoke (both rings or
device cap) → `NET_REVOKED`.

---

## 10. Compatibility and Migration

### 10.1 ABI compatibility

- Two new syscall numbers (`sys_irq_wait`, `sys_irq_ack`) appended.
- Two new `CapKind` discriminants appended.
- Five new `CapRights` bits appended.

All additive; no v0.2 surface changes.

### 10.2 Migration plan

| Step | Action |
|------|--------|
| 1    | Add Interrupt/NetDevice cap kinds + rights to `fjell-cap`. |
| 2    | Add `sys_irq_wait`/`sys_irq_ack` to the kernel. |
| 3    | Add driver-virtio-net crate. |
| 4    | Add devmgr boot-time wiring (static MMIO/IRQ/DMA descriptions). |
| 5    | Add the NetDevice protocol crate (shared between driver and netd). |
| 6    | RFC v0.4-002 lands netd on top. |

---

## 11. Test Strategy

### 11.1 Host unit tests (driver core, host-buildable)

A small subset of the driver — descriptor walking, virtio feature
negotiation, ring index math — is factored into a host-testable library
(`fjell-driver-virtio-net-core`).

```text
- feature_negotiation_picks_legacy_when_modern_unset
- rx_ring_index_wraps
- tx_ring_index_wraps
- packet_too_large_rejected
- malformed_descriptor_marks_ring_faulted
```

### 11.2 QEMU smoke tests

```text
- SMOKE:NET:DRIVER_READY           — driver emits NET_DRIVER_READY in QEMU.
- SMOKE:NET:LINK_UP                — link comes up within 200 ms.
- SMOKE:NET:LOOPBACK               — netd sends a packet; driver loops it.
```

### 11.3 QEMU negative tests

| Marker                                                | Profile |
|-------------------------------------------------------|---------|
| `NEG:NET:NO_SEND_CAP_REJECTED`                        | net     |
| `NEG:NET:NO_RECV_CAP_REJECTED`                        | net     |
| `NEG:NET:REVOKED_NETDEVICE_USE_REJECTED`              | net     |
| `NEG:IRQ:NO_BIND_RIGHT_REJECTED`                      | net     |
| `NEG:IRQ:REVOKED_INTERRUPT_WAKES_BLOCKED_TASK`        | net     |
| `NEG:NET:DRIVER_PANIC_TRIGGERS_REVOKED_EVENT`         | net     |
| `NEG:NET:DMA_RX_REVOKE_QUARANTINES_PAGES`             | net     |
| `NEG:NET:IRQ_STORM_MASKS_LINE`                        | net     |

### 11.4 Property tests (deferred to v0.6 RFC 001)

```text
- ring index sequence under random push/pop never exceeds capacity.
- revoke during in-flight wait wakes the task with LeaseRevoked.
```

---

## 12. Acceptance Criteria

```text
- driver-virtio-net crate exists, builds host + cross.
- QEMU smoke: NET:DRIVER_READY + NET:LINK_UP + NET:LOOPBACK green.
- 8 QEMU negative markers green.
- Kernel cap-kinds and syscalls added; existing v0.2 negative tests still
  pass (no regressions).
- Driver crash recovery confirmed (induced panic → DeviceRevoked → smoke
  retry succeeds after restart).
- ADR-v0.4-001 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.4-001-virtio-net.md
docs/src/development/v0.4-001-virtio-net.md
docs/src/verification/v0.4-001-virtio-net-invariants.md
docs/src/format/net-device-protocol.md
docs/src/adr/v0.4-001-net-device-boundary.md
docs/src/adr/v0.4-001-irq-syscall.md
```

---

## 14. Open Questions

1. **NET_CONTROL rights bit position** — only 32 bits are available in
   `CapRights(u32)`. After v0.4 adds 5 new bits we are at 32/32. Resolution:
   widen `CapRights` to `u64` in v0.5 RFC 003 (architecture boundary cleanup);
   meanwhile NET_CONTROL aliases `CAP_INSTALL` for NetDevice scope only.
2. **PLIC priority assignment** — currently fixed-priority. Driver pre-emption
   between interrupts is acceptable but means a slow driver can delay another
   device's interrupts. v0.5 RFC 002 addresses dynamic priority during
   device discovery.
3. **virtio-modern vs legacy** — QEMU 9.0 defaults to modern; some boards
   only support legacy. The driver negotiates and falls back. The chosen
   profile is bound into the audit `NetDriverStarted` event.

---

## 15. Release Gate (RFC-local)

```text
- Code merged; cross-builds.
- Smoke + negative markers green.
- ADRs Accepted.
- CHANGELOG entries filed.
- No new ambient-authority paths introduced.
```
