# ADR-0007 — Append-Only State Store


**Status:** Accepted  
**Date:** 2026-05-12  
**Milestone:** M6

---

## Context

Fjell OS requires a persistent state store for audit events, configuration snapshots,
device inventory, boot-control metadata, and upgrade transaction records.  The design
mandates an append-only model to simplify recovery and maintain auditability.

---

## Decision

### Physical layout (fixed LBA, no partition table)

```
LBA   1..32    boot-control mirror A (BootControlBlock)
LBA  33..64    boot-control mirror B
LBA  65..128   superblock A (StoreSuperblock)
LBA 129..192   superblock B
LBA 193..end   append-only log segments
```

A partition table is deferred to M8+ when a signed layout manifest is introduced.

### StoreSuperblock

`StoreSuperblock` is a fixed-size `#[repr(C)]` struct written to LBA 65 (A) and
LBA 129 (B).  Fields include magic (`FJSTORE\0`), version, generation, `log_start_lba`,
`log_tail_seq`, `active_checkpoint_seq`, and `crc32`.

**RFC 008:** `seal()` computes CRC32 before every write; `is_valid()` verifies magic
and CRC32 on read.  Recovery chooses the mirror with the higher valid generation.

### RecordHeader

Each append-only record starts with a `RecordHeader` (`#[repr(C)]`): magic
(`0x464A4C52`), version, kind (`RecordKind` enum), sequence number, total length, and
`crc32`.

**M7.1 state:** CRC32 is computed on write but not yet verified on recovery scan
(recovery scanner is a stub in M7).

### BootControlBlock (separate from log)

The boot-control block is separated from the append-only log so that the bootloader /
early kernel can read it without parsing a variable-length log.  It carries slot A/B
state (`SlotInfo`), confirmation status, remaining tries, and candidate slot.

**RFC 002:** slot B is initialised as `SlotInfo::empty()` (not `bootable`).  
**RFC 008:** CRC32 `seal()` / `is_valid()` enforced.

### storaged service

The `fjell-storaged` binary is a stub in M6/M7; all block I/O is driven inline from
`fjell-init`.  This is a deliberate simplification (see ADR 0010).

---

## Consequences

- The store format is stable within v0.1.0; migration between versions requires a new
  superblock generation.
- Recovery scan correctness is not yet verified in the smoke test (deferred to M8).
- The dual-superblock mirror model is defined but the mirror selection (higher valid
  generation) is not yet exercised — both mirrors are written identically on each
  checkpoint.

---

## Security Boundary Impact

Store corruption detection (CRC32 per record) is part of the v0.1.x
positive security surface. The audit projection through storaged is
not yet wired (v0.2: RFC 041). An attacker with write access to the
store can corrupt records; CRC32 rejection limits replay of corrupt state.

## Deferred Work

- auditd persistence through storaged: v0.2, RFC 041.
- Store capability-bound IPC enforcement: v0.2, RFC 031 + 038.
- Partition-table-backed layout: v0.3+.
- Encrypted at-rest storage: no current milestone.

## Related RFCs

- RFC 008 (BCB CRC32), RFC 023 (BCB Mirror Selection Tests)
- RFC 041 (Persistent Evidence Hardening, v0.2)
- RFC 044 (Evidence Export Audit, v0.1.3)
