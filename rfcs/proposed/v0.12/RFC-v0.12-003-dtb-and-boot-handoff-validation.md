# RFC-v0.12-003 — DTB and Boot Handoff Validation

**Status:** Proposed
**Target version:** v0.12.0
**Parent:** v0.12-001.
**Cross-refs:** RFC v0.5-002 (DTB derivation), v0.12-002 (target).

## 1. Problem

On QEMU `virt` the device tree is essentially controlled by the test
harness; mismatches between expected and actual devices are rare. Real
firmware (OpenSBI, U-Boot, vendor boot ROMs) hands the kernel a DTB
constructed by code Fjell did not author, and the contents drift across
revisions. A boot whose DTB says "no UART" cannot survive the first
`sys_debug_writeln`.

v0.12 requires the kernel to *validate* the inbound DTB against the
`BoardProfile` (RFC-v0.12-002) before it commits to running, and to
fail closed on mismatch rather than running with subtly wrong device
assumptions.

## 2. Boot handoff contract

Today the kernel accepts whatever DTB pointer the platform layer
hands it. v0.12 makes the handoff contract explicit:

```text
firmware ──► kernel _start(hartid, dtb_phys_addr)
                │
                ▼
            validate_dtb(dtb_phys_addr, BOARD_PROFILE)
                │
       ┌────────┴────────┐
       │                 │
       ▼                 ▼
     match           mismatch
       │                 │
  proceed boot     emit BOOT.DTB_MISMATCH
                   refuse boot (configurable: halt or fail-safe)
```

The `validate_dtb` function lives in `fjell-kernel/src/platform/dtb.rs`
and is called from the architecture-specific `_start` after the trap
table is installed but before any other subsystem initialises.

## 3. Validator checks

Required:

- **R1.** DTB header magic and version supported.
- **R2.** DTB structure block parses without out-of-bounds access.
- **R3.** Memory node present; declared range contains the kernel
  image and `BOARD_PROFILE.ram_base_pa..ram_base_pa+ram_size_bytes`.
- **R4.** For each device named in `BOARD_PROFILE.required_devices`:
  a compatible node exists, with a memory-mapped reg range that lies
  within the MMIO regions allowed by `mmio_region_table` (RFC 005).
- **R5.** No two required devices overlap MMIO ranges.
- **R6.** Interrupt controller node present and consistent with the
  kernel's expected IRQ model (PLIC on RISC-V).

Advisory (logged, not refused):

- **A1.** Optional devices missing — recorded for `devmgr`.
- **A2.** Extra devices not in `BoardProfile` — recorded; do not bind
  drivers unless an explicit policy allows it.
- **A3.** CPU node count differs from `BOARD_PROFILE` expectation —
  recorded; kernel proceeds with the lesser.

## 4. Failure handling

On a required-check failure:

1. Emit a fixed, parseable diagnostic string on the primary UART:
   `FJELL-BOOT-FAIL: DTB <code> <detail>`.
2. Emit a `BOOT.DTB_MISMATCH` semantic record (new tag) into the
   pre-IPC audit ring.
3. Take the configured action:
   - `halt` — WFI loop, do not boot.
   - `fail_safe` — boot only the recovery service path (`recoveryd`),
     refusing to start any other service.
4. The choice between `halt` and `fail_safe` is part of the
   `BoardProfile` (`boot_failure_policy = "halt" | "fail_safe"`).

Default policy: `halt`. `fail_safe` is opt-in for archetype A3 nodes
that prioritise field recovery over fail-closed.

## 5. Implementation surface

New module `fjell-kernel/src/platform/dtb.rs`:

```rust
pub struct DtbValidationError { /* code + detail */ }

pub fn validate_dtb(
    dtb_phys: usize,
    profile: &BoardProfile,
) -> Result<DtbDigest, DtbValidationError>;
```

`DtbDigest` is a 32-byte SHA-256 of the canonical DTB bytes, retained
in the audit chain so the Trust Report can record the DTB the system
booted under.

The parser is fixed-size, zero-allocation, and runs early enough that
it cannot rely on heap allocators. A reference impl in the host
workspace (under `crates/fjell-dtb-derive`, RFC v0.5-002) already
parses DTBs; this RFC reuses its parser logic via a `no_std` feature.

## 6. Integration with existing systems

- `devmgr` (RFC v0.5-002) continues to be responsible for binding
  drivers; it now also consumes the DTB digest from the kernel
  hand-off block and records it in the discovery audit chain.
- The reference fleet demo (RFC-v0.10-005) gains a Trust Report line
  showing the DTB digest seen at boot for each node.
- `BOOT.DTB_MISMATCH` is added to the catalog reserved-tags range and
  emits with sufficient context to identify the missing or
  overlapping resource.

## 7. Acceptance criteria

1. `validate_dtb` exists in the kernel, runs from `_start`, and
   refuses boot on R1–R6 failure with the documented diagnostic.
2. `BoardProfile.boot_failure_policy` field added and honoured.
3. `BOOT.DTB_MISMATCH` semantic tag is allocated and emits with
   detail.
4. A QEMU test exercises:
   - Valid DTB → boot proceeds.
   - DTB missing UART → boot halts with documented diagnostic.
   - DTB with MMIO overlap → boot halts with documented diagnostic.
5. Trust Report records the DTB digest per node.
6. On the chosen v0.12 target (Path A or B), validation passes against
   the firmware-provided DTB and the documented `BoardProfile`.

## 8. Out of scope

- Rewriting DTBs at runtime.
- Cross-firmware bug-for-bug compatibility tables.
- ACPI handoff (deferred to ARM64 work).
- Hot-plug device changes (post-v1.0).
