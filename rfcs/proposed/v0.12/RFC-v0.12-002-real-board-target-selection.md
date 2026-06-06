# RFC-v0.12-002 — Real-Board Target Selection (or Hardened QEMU Fallback)

**Status:** Proposed
**Target version:** v0.12.0
**Parent:** v0.12-001.
**Cross-refs:** RFC v0.5-001 (PlatformProfile), v0.5-002 (DTB discovery).

## 1. Problem

v0.12 needs a deployment target. The choice is the single most consequential
non-identity decision of v0.x — it determines which devices Fjell will
exercise, which firmware quirks it must absorb, and which operator
workflow it inherits.

This RFC sets selection criteria, names candidates, and commits to one
primary + one secondary.

## 2. Selection criteria

A candidate target must satisfy all of:

- **C1. Open documentation.** Datasheet and reference manual freely
  available; no NDA-gated material on the critical boot path.
- **C2. Supply.** Available in small quantities from at least two
  vendors at the time of v0.12 development.
- **C3. RISC-V profile.** Cores implement RV64GC at minimum; ideally
  conform to a standard profile (RVA20 / RVA22) so behaviour is
  contract-defined.
- **C4. Firmware story.** Boots under OpenSBI or U-Boot; firmware
  source is auditable.
- **C5. Storage path.** Exposes a virtio-blk-compatible interface or
  an SD/eMMC controller for which a minimal driver is feasible within
  v0.12 scope.
- **C6. Serial console.** UART available from boot; can be the
  primary diagnostic channel.
- **C7. Fjell-archetype fit.** Sized appropriately for A1/A2/A3
  workloads — not a server-class system, not a sub-MB MCU.

Desirable but not required:

- D1. Onboard secure element (TPM, EdgeLock, etc.) — would unblock
  hardware-rooted attestation in v0.13.
- D2. Ethernet for v0.4 networking validation.
- D3. Watchdog timer for `powerd`/recovery.

## 3. Candidate evaluation

The candidates considered:

| Candidate | C1 | C2 | C3 | C4 | C5 | C6 | C7 | D1 | D2 | D3 |
|-----------|----|----|----|----|----|----|----|----|----|-----|
| StarFive VisionFive 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ |
| Milk-V Mars (Lichee Pi) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ |
| BeagleV-Ahead | ✓ | partial | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ |
| SiFive HiFive Unmatched | ✓ | EOL | ✓ | ✓ | ✓ | ✓ | partial | ✗ | ✓ | ✓ |
| QEMU `sifive_u` | n/a | n/a | ✓ | ✓ | ✓ | ✓ | n/a | n/a | ✓ | n/a |

Final selection is deferred to landing time of this RFC (supply status
will be re-verified). v0.12-002 commits to:

- **Primary (Path A):** the candidate satisfying all of C1–C7 with the
  best documentation quality at landing time. Initial preference:
  StarFive VisionFive 2.
- **Secondary (Path A backup):** Milk-V Mars or BeagleV-Ahead,
  whichever is in stock.
- **Fallback (Path B):** QEMU `sifive_u` if Path A is impractical.

The choice is captured at landing as a one-line update to this RFC
section before merge to `done/`.

## 4. BoardProfile content

The chosen target's `BoardProfile` (RFC v0.5-001 type) is committed at
`platform/<target>/board-profile.toml`:

```toml
schema_version  = 1
target_name     = "starfive-visionfive2"   # example
architecture    = "rv64gc"
extensions      = ["I", "M", "A", "F", "D", "C"]
ram_base_pa     = 0x40000000
ram_size_bytes  = 0x100000000              # 4 GiB
uart_mmio_base  = 0x10000000               # actual address
virtio_blk_base = ...
required_devices = ["uart", "rtc", "virtio-blk"]
optional_devices = ["virtio-net", "wdt"]
firmware_kind    = "opensbi"
firmware_min_ver = "1.5"
boot_protocol    = "linux-image-hdr-v6+dtb"
```

The structure is exactly the type defined by RFC v0.5-001; v0.12
provides the first non-QEMU value.

## 5. What changes in the codebase

- `crates/fjell-platform-format` already has the type. v0.12 ships a
  real instance (no code change).
- `crates/fjell-devmgr` learns to consume the target's `required_devices`
  table and refuse boot if a required device is missing post-discovery.
- The kernel platform layer gains a new `crates/fjell-platform-<target>/`
  with target-specific MMIO bases and IRQ numbers, behind a feature
  flag selected at build time.
- Build artefacts include a `<target>.bin` derived from the kernel ELF
  via the procedure documented in v0.12-005.

## 6. Acceptance criteria

1. A specific target is named, with rationale, in this RFC section
   before merge to `done/`.
2. A complete `BoardProfile` is committed for the chosen target.
3. `crates/fjell-platform-<target>/` exists and builds the kernel.
4. The build produces flashable media following v0.12-005.
5. The kernel boots on real silicon and reaches `init: ready` over
   serial. (Path A.)
6. If Path B is taken, the same outcome holds for `qemu-system-riscv64
   -M sifive_u`, and the RFC body is annotated with the Path A
   re-attempt milestone.

## 7. Out of scope

- ARM64 target choice (v1.x or later).
- Designing custom hardware.
- Cross-board portability beyond what v0.5 already specifies.
- Negotiating with vendors for early-access hardware.
