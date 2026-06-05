# RFC-v0.5-001: PlatformProfile and BoardProfile Format

## Status

Draft (revised, supersedes pack v0.5-001 draft)

## Target Version

`v0.5.0`.

## Phase

Platform Surface and Semantic Stabilization — Epic A (Profile Format).

## Related Work

- v0.2 RFCs 035–037 — MMIO/DMA/Interrupt capability model.
- v0.4 RFCs 001/002 — first user of static device descriptions.
- v0.5 RFCs 002/003 — device-discovery strategy and architecture-boundary
  cleanup.
- v0.7 RFC 001 — node identity binds to the active PlatformProfile digest.

---

## 1. Summary

Define two stable, declarative formats:

- **`PlatformProfile`** — describes the architectural family the OS expects
  (RISC-V64GC, future ARM64), including required ISA extensions, the
  memory-map shape, the PLIC/AIA layout, and the kernel boot ABI version.
- **`BoardProfile`** — describes a concrete board variant within a platform:
  device list (MMIO ranges, IRQ lines, DMA windows), boot-time clocks,
  recovery jumper location.

The profiles are loaded by `devmgr` at boot before any device is enumerated.
Both formats are content-addressable; their digests are bound into the
measurement chain and into the v0.3 attestation record (via a v3 schema
deferred to v0.7).

---

## 2. Motivation

Today Fjell hard-codes the QEMU `virt` machine in several services. v0.4
introduced network devices through inline static descriptions in `devmgr`.
This works for one board but is unmaintainable for two.

Profiles externalise the description into a content-addressable bundle that
is:

- *built* by the release tool;
- *signed* by the release anchor;
- *measured* into the chain at boot;
- *attested* by attestd.

The result: switching boards becomes a profile swap, not a code change.

---

## 3. Goals

```text
- One PlatformProfile per architecture family.
- One BoardProfile per concrete board.
- Strict binary encoding; canonical digest formula.
- Profiles bound into measurement chain.
- Schema-versioned; future-extensible.
- devmgr refuses to start without a profile pair whose digests are signed.
- Profiles describe MMIO/IRQ/DMA only; no policy, no service config.
```

## 4. Non-Goals

```text
- No DTB or ACPI parsing in this RFC. Profiles are pre-built artefacts; RFC
  v0.5-002 covers how a profile may be *derived* from DTB/ACPI offline.
- No runtime profile change. Profiles are boot-immutable.
- No multi-CPU topology beyond the existing single-hart assumption (v0.5
  RFC 003 cleans up arch boundaries; SMP is later).
- No PCI in v0.5.0.
```

---

## 5. External Design

### 5.1 Files

```text
/boot/platform.profile.bin       — PlatformProfile, signed
/boot/board.profile.bin          — BoardProfile, signed
/boot/profiles.sig               — both profiles' signatures (Ed25519)
```

### 5.2 Boot flow

```text
1. bootloader hands kernel control with a fixed memory map.
2. kernel boots, enters Bootstrap.
3. devmgr starts; reads platform.profile.bin + board.profile.bin.
4. devmgr verifies signatures against keyring (purpose: BoardProfile).
5. devmgr measures profile digests into measurement chain.
6. devmgr enumerates devices per BoardProfile.
7. service-manager proceeds once devmgr emits PROFILES_READY.
```

### 5.3 Operator view

```text
$ fjell-tools profile show
PlatformProfile: digest=sha256:abc... family=riscv64gc version=1
BoardProfile:    digest=sha256:def... name=qemu-virt v0.4
                 devices: 3 (uart, virtio-net@10004000, virtio-blk@10005000)
```

---

## 6. Data Model

### 6.1 PlatformProfile

```rust
pub const PLATFORM_PROFILE_VERSION: u16 = 1;

pub struct PlatformProfile {
    pub schema_version: u16,
    pub family:         PlatformFamily,
    pub family_version: u16,         // monotonic within family
    pub isa_extensions: IsaExtensions,
    pub kernel_abi:     KernelAbiVersion,
    pub mem_map:        MemMap,
    pub plic_layout:    PlicLayout,
    pub profile_digest: Digest32,
}

#[repr(u8)]
pub enum PlatformFamily {
    Riscv64Gc = 0x01,
    Arm64     = 0x02,         // reserved
}

pub struct IsaExtensions(pub u64);
// bit 0: i (mandatory)
// bit 1: m (mandatory)
// bit 2: a (mandatory)
// bit 3: f
// bit 4: d
// bit 5: c
// bit 6: zbb
// bit 7: zicsr
// bit 8: zifencei
// bits 9..63: reserved (MBZ)

pub struct KernelAbiVersion {
    pub major: u8,
    pub minor: u8,
}

pub struct MemMap {
    pub kernel_load_addr:  u64,
    pub kernel_size_max:   u64,
    pub heap_start:        u64,
    pub heap_size:         u64,
    pub initrd_addr:       u64,    // 0 if no initrd
    pub initrd_size:       u64,
}

pub struct PlicLayout {
    pub base_addr:     u64,
    pub size_bytes:    u64,
    pub num_sources:   u16,
    pub num_contexts:  u16,
}
```

### 6.2 BoardProfile

```rust
pub const BOARD_PROFILE_VERSION: u16 = 1;
pub const MAX_BOARD_DEVICES:     usize = 16;

pub struct BoardProfile {
    pub schema_version: u16,
    pub board_name:     [u8; 16],     // ASCII zero-padded
    pub board_revision: [u8; 8],
    pub platform_ref:   Digest32,     // expected PlatformProfile digest
    pub device_count:   u8,
    pub devices:        [BoardDevice; MAX_BOARD_DEVICES],
    pub recovery:       RecoveryDescriptor,
    pub profile_digest: Digest32,
}

pub struct BoardDevice {
    pub class:        DeviceClass,
    pub mmio_base:    u64,
    pub mmio_size:    u64,
    pub irq_line:     u16,
    pub dma_window_start: u64,
    pub dma_window_size:  u64,
    pub name:         [u8; 16],
}

#[repr(u8)]
pub enum DeviceClass {
    Uart8250        = 0x01,
    VirtioNetMmio   = 0x02,
    VirtioBlkMmio   = 0x03,
    VirtioConsole   = 0x04,
    Plic            = 0x05,
    Clint           = 0x06,
    SystemCounter   = 0x07,
    Generic         = 0xFF,
}

pub struct RecoveryDescriptor {
    pub kind:         RecoveryKind,
    pub mmio_base:    u64,        // 0 if not MMIO
    pub gpio_pin:     u16,        // 0 if not GPIO
}

#[repr(u8)]
pub enum RecoveryKind {
    None     = 0,
    BootArg  = 1,            // kernel boot arg "recovery=1"
    Gpio     = 2,
    SerialBreak = 3,
}
```

### 6.3 Canonical digests

```text
platform_digest = SHA256(
    "FJELL-PLATFORM-V1" ||
    schema u16 LE || family u8 || family_version u16 LE ||
    isa_extensions u64 LE ||
    kernel_abi (major u8 || minor u8) ||
    mem_map (6 × u64 LE) ||
    plic_layout (base u64 || size u64 || sources u16 || contexts u16)
)

board_digest = SHA256(
    "FJELL-BOARD-V1" ||
    schema u16 LE ||
    board_name 16 B || board_revision 8 B ||
    platform_ref 32 B ||
    device_count u8 ||
    for each device: class u8 || mmio_base u64 LE || mmio_size u64 LE ||
                     irq_line u16 LE || dma_start u64 LE || dma_size u64 LE ||
                     name 16 B ||
    recovery (kind u8 || mmio_base u64 LE || gpio_pin u16 LE)
)
```

---

## 7. Internal Design

### 7.1 devmgr boot sequence

```text
1. open /boot/platform.profile.bin via storaged → bytes
2. parse → PlatformProfile { profile_digest }
3. recompute platform_digest; compare; reject on mismatch
4. open /boot/board.profile.bin → bytes
5. parse → BoardProfile { profile_digest, platform_ref }
6. recompute board_digest; compare
7. verify board.platform_ref == platform.profile_digest
8. verify keyring signatures over both digests (purpose BoardProfile)
9. measuredd.append(MeasurementKind::PlatformProfileLoaded, platform_digest)
10. measuredd.append(MeasurementKind::BoardProfileLoaded,    board_digest)
11. for device in board.devices: devmgr.register_device(device)
12. emit semantic: PLATFORM.PROFILES_READY
```

### 7.2 New KeyPurpose

```rust
pub enum KeyPurpose {
    // ... existing ...
    BoardProfile = 0x07,    // new in v0.5; signs platform + board digests
}
```

`BoardProfile` is added to RFC v0.3-002 with an ADR; tests in `fjell-keyring`
need a new test fixture.

### 7.3 New MeasurementKind values

```text
PlatformProfileLoaded = 0x10
BoardProfileLoaded    = 0x11
```

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-110: Adversary swaps board.profile.bin to widen MMIO permissions.
Mitigation:  signature over board_digest; board_digest covers all device
             rows; signature check fails.

Threat T-111: Mismatched platform_ref (BoardProfile claims a different
             PlatformProfile).
Mitigation:  devmgr requires board.platform_ref == platform.profile_digest.

Threat T-112: Adversary lowers kernel_abi major in PlatformProfile to enable
             a quirk path.
Mitigation:  kernel ABI accepted only if equal to compiled-in expected major;
             devmgr verifies and refuses to start otherwise.

Threat T-113: Profile claims absent ISA extension (e.g., zicsr off).
Mitigation:  devmgr (and the kernel via cpuid CSR read) verify the actual
             ISA matches the profile's required mask.
```

### 8.2 Audit emission

```text
PlatformProfileLoaded   { digest, family, family_version, kernel_abi }
BoardProfileLoaded      { digest, board_name, device_count }
ProfileSignatureFailed  { which, error_code }
ProfileDigestMismatch   { which, computed, claimed }
PlatformRefMismatch     { board_ref, platform_digest }
IsaMismatch             { required_mask, observed_mask }
```

---

## 9. Memory / Resource Design

- `PlatformProfile` packed ≈ 80 B.
- `BoardProfile` packed: 16 + 8 + 32 + 1 + 16 × ~50 + 11 + 32 ≈ 900 B.
- Both stored as Copy structs.

---

## 10. Compatibility and Migration

- The static device tables currently in devmgr (added by RFC v0.4-001) are
  replaced by `BoardProfile` lookups.
- The v0.4 QEMU build ships a `qemu-virt-v0.4` board profile that exactly
  reproduces the v0.4 device map.
- A migration test asserts the new boot path enumerates identical devices.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- platform_profile_serialise_round_trip
- board_profile_serialise_round_trip
- platform_digest_covers_isa_extensions
- platform_digest_covers_kernel_abi
- board_digest_covers_device_rows
- board_platform_ref_matches
- isa_extensions_bit_definitions_stable
- max_devices_enforced
```

### 11.2 QEMU smoke

```text
- SMOKE:PROFILE:LOAD_BOTH
- SMOKE:PROFILE:DEVICES_MATCH_V04_BASELINE
```

### 11.3 QEMU negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:PROFILE:PLATFORM_DIGEST_MISMATCH_REJECTED`         | profile  |
| `NEG:PROFILE:BOARD_DIGEST_MISMATCH_REJECTED`            | profile  |
| `NEG:PROFILE:BOARD_REF_MISMATCH_REJECTED`               | profile  |
| `NEG:PROFILE:UNSIGNED_PROFILE_REJECTED`                 | profile  |
| `NEG:PROFILE:ISA_REQUIRED_NOT_PRESENT_REJECTED`         | profile  |
| `NEG:PROFILE:KERNEL_ABI_MAJOR_MISMATCH_REJECTED`        | profile  |

---

## 12. Acceptance Criteria

```text
- fjell-platform-format crate lands with PlatformProfile/BoardProfile.
- devmgr boot replaces static tables with profile-driven enumeration.
- qemu-virt-v0.4 profile built and used in CI.
- 6 NEG markers green.
- New KeyPurpose::BoardProfile added.
- Two new MeasurementKind values defined.
- ADR-v0.5-001 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.5-001-platform-profile.md
docs/src/format/platform-profile.md
docs/src/format/board-profile.md
docs/src/adr/v0.5-001-platform-board-boundary.md
```

---

## 14. Open Questions

1. **Profile sealing** — should profiles be sealed against the trust
   provider as well? Resolution: not needed in v0.5; the keyring signature
   path is sufficient. May revisit when a hardware provider lands.
2. **Multiple boards per platform image** — the build pipeline can ship
   multiple `*.board.profile.bin` and bootloader chooses one. Out of scope;
   v0.5 ships one board per image.
3. **Device-class taxonomy growth** — the enum is `u8`; if classes grow
   past 256 we widen to u16 in a v0.5.x RFC.

---

## 15. Release Gate (RFC-local)

```text
- crate landed, devmgr migrated.
- 6 NEG markers green.
- CI uses profiles end-to-end.
- ADR Accepted.
```
