# RFC-v0.5-002: Device Discovery Strategy — DTB / ACPI and devmgr Boundary

**Status.** Implemented (v0.5.0)

## Status

Draft (revised, supersedes pack v0.5-002 draft)

## Target Version

`v0.5.0`.

## Phase

Platform Surface and Semantic Stabilization — Epic B (Device Discovery).

## Related Work

- v0.5 RFC 001 — PlatformProfile / BoardProfile (the *consumer* of derived
  device tables).
- v0.5 RFC 003 — architecture boundary cleanup.
- v0.2 RFC 035 — MMIO capability shape.

---

## 1. Summary

Define how Fjell consumes platform-supplied device descriptions (DTB on
RISC-V/ARM, ACPI on x86). The design is firm: **DTB/ACPI parsing happens
offline**, at the release-builder stage, producing a `BoardProfile`
(RFC v0.5-001). At runtime, `devmgr` consults *only* the BoardProfile.

This RFC defines:

- the offline parser (`fjell-tools profile derive`) that turns a DTB file
  into a BoardProfile;
- the runtime path: no DTB parser is linked into the kernel or `devmgr`;
- a strict subset of DTB nodes / properties that the offline parser
  understands;
- audit / acceptance criteria for the derivation step (so a malicious DTB
  cannot smuggle in a wider MMIO range).

---

## 2. Motivation

DTB parsers are complex. Past projects have shipped CVEs in their DT
parsers. Fjell can avoid this whole class by parsing offline:

- the build pipeline runs the parser;
- a human (and later v0.8 fleet governance) signs off on the produced
  BoardProfile;
- the device only ever sees the signed, validated profile.

This RFC defines the contract that makes that offline path safe.

---

## 3. Goals

```text
- A host-side parser that ingests DTB and emits BoardProfile.
- Strict allow-list: unknown nodes/properties cause derivation to fail.
- Output deterministic: same DTB → byte-identical BoardProfile.
- Built into fjell-tools so any developer can run it.
- No DTB-related code in the runtime kernel or in devmgr.
- ACPI scaffolding deferred but the format is forward-compatible.
```

## 4. Non-Goals

```text
- No runtime DTB parsing.
- No partial DTB acceptance (all-or-nothing derivation).
- No "fix-up" or "merge" of overlays at runtime.
- No PCI device enumeration; deferred.
```

---

## 5. External Design

### 5.1 Operator workflow (build host)

```text
$ fjell-tools profile derive \
      --platform riscv64gc-v1 \
      --dtb       boards/qemu-virt.dtb \
      --out       boards/qemu-virt-v0.4.board.profile.bin
$ fjell-tools profile sign \
      --signing-key dev-board-anchor.key \
      --out         boards/qemu-virt-v0.4.profiles.sig

# Both outputs land in /boot during image build.
```

### 5.2 Derivation rules

The parser accepts only nodes matching:

```text
/cpus/cpu@*               — extracts riscv,isa string and timebase
/soc/uart@*               — class Uart8250 (compatible = "ns16550a")
/soc/virtio_mmio@*        — class VirtioNetMmio if interrupt is bound,
                            VirtioBlkMmio if there's a block alias
/soc/plic@*               — PLIC layout
/soc/clint@*              — Clint device class
/aliases                  — used to disambiguate virtio_mmio classes
/chosen/stdout-path       — must point to a uart we already accepted
/reserved-memory          — must not overlap kernel heap or DMA window
```

Every other node causes derivation to fail with
`DeriveError::UnknownNode { path }`.

### 5.3 DMA window assignment

DTB rarely lists explicit DMA windows. The derivation step assigns DMA
windows in a deterministic order:

```text
For each MMIO device in node order:
   if class needs DMA (virtio-net, virtio-blk):
       window = next free 4 KiB-aligned slot in [DMA_BASE, DMA_BASE+DMA_TOTAL]
                of size DMA_PER_DEVICE_DEFAULT (32 KiB)
```

`DMA_BASE`, `DMA_TOTAL`, `DMA_PER_DEVICE_DEFAULT` come from the
PlatformProfile's `mem_map.heap_*` plus a fixed reservation table. The
algorithm is documented and reproducible.

---

## 6. Data Model

### 6.1 Derivation context

```rust
pub struct DeriveContext {
    pub platform:      PlatformProfile,
    pub dma_base:      u64,
    pub dma_total:     u64,
    pub dma_per_device: u64,
    pub allowed_compat: &'static [&'static [u8]],
}
```

### 6.2 Derivation errors

```rust
pub enum DeriveError {
    DtbParseFailed { offset: u32 },
    UnknownNode { path: ArrayString<128> },
    UnknownCompatible { value: ArrayString<64> },
    OverlappingRanges,
    OutOfDmaSpace,
    IsaParseFailed,
    MissingPlic,
    MissingStdout,
    OutputCapacityExceeded,
}
```

### 6.3 Reproducibility metadata

The derived `BoardProfile` carries no derivation metadata in v0.5 (the
profile is a runtime artefact, not a build manifest). A separate
`DerivationManifest` file records:

```rust
pub struct DerivationManifest {
    pub source_dtb_digest: Digest32,
    pub platform_digest:   Digest32,
    pub tool_version:      [u8; 8],
    pub derived_at_tick:   u64,    // host monotonic clock
    pub board_digest:      Digest32,
    pub manifest_digest:   Digest32,
}
```

The manifest is signed by the release anchor and stored alongside the
profile in the project's `boards/` directory; runtime never reads it.

---

## 7. Internal Design

### 7.1 Parser structure

The parser is a single host crate `fjell-dtb-derive`. It is written in safe
Rust (`#![deny(unsafe_code)]`) and limited to ≤ 2000 ELOC.

Internal modules:

```text
fjell-dtb-derive/
   src/
      lib.rs             — entry points
      header.rs          — FDT header parsing
      strings.rs         — string-table accessor
      nodes.rs           — node walk
      compat.rs          — compatible-string allow-list
      reserved_mem.rs    — /reserved-memory handling
      assigner.rs        — DMA window assigner
      isa.rs             — RISC-V ISA string parser
```

### 7.2 Allow-list

`allowed_compat` is exposed publicly so the developer can extend it for a
new board (subject to ADR review). Initial table:

```text
"ns16550a"
"virtio,mmio"
"riscv,plic0"
"sifive,plic-1.0.0"
"riscv,clint0"
"riscv"
"simple-bus"
"qemu,virtio-mmio"
```

### 7.3 Failure semantics

The derivation tool exits non-zero with a structured JSON error on any
DeriveError. CI consumes the JSON to ensure regressions are caught before
sign-off.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-120: Malicious DTB widens MMIO range or maps unrelated memory.
Mitigation:  derivation rejects overlap with kernel heap / DMA window;
             reserved-memory must not overlap; signed profile is the only
             input to runtime.

Threat T-121: Compromised build host produces forged BoardProfile.
Mitigation:  build pipeline runs derivation on a clean host; output is
             signed by an offline key; CI compares against expected
             baseline.

Threat T-122: Two DTBs producing different DMA assignments for the same
             board.
Mitigation:  deterministic assigner; identical DTBs produce byte-identical
             BoardProfile (CI checks).
```

---

## 9. Memory / Resource Design

- All host-side; no runtime cost.
- ArrayString<128> caps avoid alloc-free Rust pain; derivation can be
  ported to `no_std` if needed.

---

## 10. Compatibility and Migration

- v0.4 inlined device tables are replaced by `qemu-virt-v0.4.board.profile.bin`
  produced by this RFC's tool.
- Older boards added during v0.5 use the same tool with a per-board DTB.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- header_parse_magic
- strings_lookup_known
- nodes_walk_count
- compat_unknown_rejected
- isa_string_riscv_gc_accepted
- isa_string_unknown_extension_rejected
- dma_assigner_deterministic
- reserved_memory_overlap_rejected
- multiple_uarts_chosen_by_stdout
- virtio_class_disambiguated_by_aliases
```

### 11.2 Snapshot test

CI runs derivation against checked-in DTBs and compares produced
BoardProfile bytes to checked-in baselines. Any drift fails CI.

### 11.3 QEMU smoke

```text
- SMOKE:DERIVE:QEMU_VIRT_MATCHES_BASELINE
- SMOKE:DERIVE:BOOTS_WITH_DERIVED_PROFILE
```

### 11.4 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:DERIVE:UNKNOWN_COMPATIBLE_REJECTED`                | derive   |
| `NEG:DERIVE:RESERVED_MEM_OVERLAP_REJECTED`              | derive   |
| `NEG:DERIVE:DMA_OUT_OF_SPACE_REJECTED`                  | derive   |
| `NEG:DERIVE:UNKNOWN_NODE_REJECTED`                      | derive   |
| `NEG:DERIVE:MISSING_PLIC_REJECTED`                      | derive   |

---

## 12. Acceptance Criteria

```text
- fjell-dtb-derive crate ships; ≥ 10 host unit tests pass.
- Snapshot test green against checked-in DTBs.
- CI uses derived profile for v0.5 QEMU boot.
- 5 NEG markers green.
- ADR-v0.5-002 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.5-002-device-discovery.md
docs/src/development/v0.5-002-derive-tool.md
docs/src/adr/v0.5-002-no-runtime-dtb-parse.md
```

---

## 14. Open Questions

1. **PCI** — once Fjell ships on PCI-bearing platforms (likely a real ARM
   board), the offline-derivation model needs to track BARs and capability
   chains. Out of scope for v0.5.
2. **ACPI** — when ARM lands, ACPI tables join DTB as a permitted input.
   The schema for AcpiDerive is reserved but not implemented.
3. **Tool version drift** — if `fjell-tools profile derive` evolves, old
   DTBs may produce different outputs. Snapshot tests catch this; a
   DerivationManifest's `tool_version` field is the authoritative pin.

---

## 15. Release Gate (RFC-local)

```text
- Derivation tool builds and runs in CI.
- Snapshot baseline checked in.
- QEMU boots with derived profile.
- 5 NEG markers green.
- ADR Accepted.
```
