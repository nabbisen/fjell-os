# Fjell OS — StarFive VisionFive 2 Deployment Guide

*Implements RFC-v0.12-005. This is the first hardware deployment
target beyond QEMU `virt`. Follow these steps exactly.*

---

## Prerequisites

| Requirement | Minimum | Confirmed |
|-------------|---------|-----------|
| Board | VisionFive 2 (any RAM variant) | ✓ |
| Firmware | OpenSBI ≥ 1.5, U-Boot ≥ 2023.10 | TODO v0.12.1 |
| MicroSD | Class 10 / A1, ≥ 4 GB | ✓ |
| Serial cable | USB-UART 3.3V (e.g. CH340) | ✓ |
| Host OS | Linux (amd64) | ✓ |
| Rust toolchain | Pinned in `rust-toolchain.toml` | ✓ |

---

## Step 1 — Build the kernel

```bash
cargo xtask build
# Output: target/riscv64gc-unknown-none-elf/release/fjell-kernel
```

---

## Step 2 — Validate the Trust Report

```bash
cargo xtask trust-report --dry-run | head -20
```

---

## Step 3 — Flash the SD card

*TODO: v0.12.1 — document the exact partition layout and flashing procedure
after first successful boot is confirmed. References RFC-v0.12-002 §5.*

Placeholder command (update once validated):

```bash
# Partition SD card as expected by VisionFive 2 U-Boot
# Then copy the kernel image
# dd if=target/.../fjell-kernel of=/dev/sdX bs=512 seek=<offset>
```

---

## Step 4 — Serial console setup

Connect the USB-UART adapter to the VisionFive 2 40-pin GPIO header:
- Pin 8 (UART TX from board) → adapter RX
- Pin 10 (UART RX to board) → adapter TX
- Pin 6 (GND) → adapter GND

```bash
screen /dev/ttyUSB0 115200
```

Expected boot output:
```
OpenSBI v1.x ...
U-Boot ...
Fjell kernel boot ...
TEST:M8:PASS        ← success marker
```

---

## Step 5 — Verify DTB handoff

The DTB validation (RFC-v0.12-003) fires at early boot. On mismatch:

```
FJELL-BOOT-FAIL: DTB R4 <mmio_address>
```

If this appears, the firmware DTB device tree does not match the
`platform/starfive-visionfive2/board-profile.toml`. Update the profile
or the firmware to match.

---

## Failure modes

| Symptom | Cause | Resolution |
|---------|-------|------------|
| No UART output after firmware | Kernel not loaded | Re-flash SD card |
| `FJELL-BOOT-FAIL: DTB` | DTB mismatch | Update board-profile.toml |
| Boots to prompt then hangs | Service spawn failure | Capture serial log; check init image table |
| Reboots in a loop | Boot control recovery active | Run `bootctl status` via JTAG |

---

## Trust spine

Bundles deployed to this board are signed with the v0.11 signing pipeline.
Verify before flashing:

```bash
cargo xtask verify-bundle-sig \
  --bundle <bundle.bundle> \
  --sig <bundle.bundle.sig> \
  --pubkey <release-pubkey-hex>
```

*TODO: Attestation over hardware trust anchor (TPM/secure-element)
is deferred to v0.13 once the VisionFive 2 secure-element story is
confirmed (RFC-v0.12-001 §4).*

---

## MMIO ordering note

The MMIO ordering audit (RFC-v0.12-004) was conducted on QEMU.
On real VisionFive 2 silicon, the JH7110 implements RVWMO without
the TSO extension. All MMIO-ORDER annotations were conservatively
applied for RVWMO. Any ordering regression discovered on hardware
should be filed against RFC-v0.12-004 directly.

---

*Full RFC specification: RFC-v0.12-005.*
*Board profile: `platform/starfive-visionfive2/board-profile.toml`.*
