# Fjell OS — Negative Tests

**Produced by:** RFC 026 (also known as RFC-v0.1.x-003), landed in v0.1.2.  
**Status:** v0.1.2 baseline. Each v0.2 RFC extends this document.

Negative tests are the evidence that Fjell OS rejects **invalid
operations**, not just that it performs valid ones. Positive smoke
markers (`TEST:Mx:PASS`) prove the happy path; negative markers
(`NEG:<CATEGORY>:<CASE>:PASS`) prove the rejection path.

---

## How negative tests work

Every negative test follows the same pattern:

```
1. setup: reach the state where the invalid operation can be attempted
2. attempt: call the syscall or IPC with invalid input
3. verify: confirm the kernel returns a rejection error
4. assert: the system state is unchanged (no partial effect)
5. emit:   print NEG:<CATEGORY>:<CASE>:PASS to serial
```

CI checks for the marker using `cargo xtask qemu-log-check`.

A test that does **not** print the marker (timeout, panic, or
missing the line) causes CI to fail.

A test that prints `NEG:<CATEGORY>:<CASE>:FAIL` explicitly fails CI.

A test that prints `NEG:<CATEGORY>:<CASE>:DEFERRED` is counted as
a placeholder pass. The DEFERRED annotation is removed when the
test is fully implemented.

---

## Marker naming convention

```
NEG : <CATEGORY> : <CASE> : PASS | FAIL | DEFERRED
```

- `CATEGORY` is upper-case: `CAP`, `IPC`, `MMIO`, `DMA`,
  `STORE`, `UPGRADE`, `LEASE` (added by RFC 033), `USER_COPY`
  (RFC 039), `AUDIT` (RFC 039), `POLICY` (RFC 040), `EVIDENCE`
  (RFC 041), `SVC` (RFC 038).
- `CASE` is upper-case with underscores.
- `:PASS` means the test ran and the invalid operation was rejected.
- `:FAIL` means the test ran and the invalid operation was **not**
  rejected (this is a regression — it always fails CI).
- `:DEFERRED` means the enforcement is not yet implemented;
  this counts as placeholder-PASS per RFC 025.

---

## Category: CAP (Capability)

**Enforcement status:** Partially enforced at v0.1.2.  
Full enforcement via `require_cap()` lands in v0.2 (RFC 031).

| Marker | Description | v0.1.x testable |
|---|---|---|
| `NEG:CAP:INVALID_HANDLE:PASS` | syscall with an out-of-range handle rejects | Partial |
| `NEG:CAP:GENERATION_MISMATCH:PASS` | stale generation in handle rejects | Partial |
| `NEG:CAP:MISSING_RIGHT:PASS` | call without required right rejects | Deferred to v0.2 |
| `NEG:CAP:REVOKED_LEASE:PASS` | use of a lease-revoked capability rejects | Deferred to v0.2 |
| `NEG:CAP:DROPPED_HANDLE:PASS` | use after `sys_cap_drop` rejects | Deferred to v0.2 |
| `NEG:CAP:WRONG_KIND:PASS` | wrong CapKind rejects | Deferred to v0.2 |
| `NEG:CAP:SCOPE_MISMATCH:PASS` | wrong ObjectScope rejects | Deferred to v0.2 |
| `NEG:CAP:STALE_AFTER_DROP:PASS` | stale handle after drop fails generation check | Deferred to v0.2 |
| `NEG:CAP:DROP_REVOKED_CAP:PASS` | drop of a revoked cap succeeds | Deferred to v0.2 |
| `NEG:CAP:CSpace_REUSE_AFTER_DROP:PASS` | grant/revoke/drop cycle does not exhaust CSpace | Deferred to v0.2 |
| `NEG:CAP:DROP_INVALID_HANDLE_REJECTED:PASS` | drop with invalid handle rejects | Deferred to v0.2 |

---

## Category: IPC

**Enforcement status:** Partially enforced at v0.1.2.  
Full enforcement via `require_cap()` lands in v0.2 (RFC 031).
Blocked-IPC wake/cancel lands in v0.2 (RFC 034).

| Marker | Description | v0.1.x testable |
|---|---|---|
| `NEG:IPC:SEND_WITHOUT_RIGHT:PASS` | `ipc_send` without SEND right rejects | Deferred to v0.2 |
| `NEG:IPC:CALL_WITHOUT_RIGHT:PASS` | `ipc_call` without CALL right rejects | Deferred to v0.2 |
| `NEG:IPC:REPLY_INVALID_CALL_ID:PASS` | reply with stale CallId rejects | Partial |
| `NEG:IPC:REPLY_AFTER_REVOKE:PASS` | reply after lease revoke rejects | Deferred to v0.2 |
| `NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE:PASS` | blocked caller returns `LeaseRevoked` | Deferred to v0.2 |
| `NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE:PASS` | blocked receiver wakes on revoke | Deferred to v0.2 |
| `NEG:IPC:LATE_REPLY_REJECTED:PASS` | reply after CallFrame cancellation rejects | Deferred to v0.2 |
| `NEG:IPC:TRY_RECV_EMPTY_RETURNS_WOULDBLOCK:PASS` | `ipc_try_recv` returns WouldBlock when empty | Partial |
| `NEG:IPC:TRY_RECV_WITHOUT_RIGHT:PASS` | `ipc_try_recv` without RECV right rejects | Deferred to v0.2 |

---

## Category: MMIO

**Enforcement status:** Not enforced at v0.1.2.  
MmioRegion capability ABI lands in v0.2 (RFC 035).

| Marker | Description | v0.1.x testable |
|---|---|---|
| `NEG:MMIO:MAP_WITHOUT_CAP:PASS` | `sys_mmio_map` without cap rejects | Deferred to v0.2 |
| `NEG:MMIO:MAP_RAM_REJECTED:PASS` | mapping RAM range via mmio_map rejects | Partial (RAM guard, RFC 005) |
| `NEG:MMIO:OFFSET_OUT_OF_RANGE:PASS` | offset + size beyond region rejects | Deferred to v0.2 |
| `NEG:MMIO:REVOKED_REGION_REJECTED:PASS` | revoked MmioRegion cap rejects | Deferred to v0.2 |

---

## Category: DMA

**Enforcement status:** Not enforced at v0.1.2.  
DmaRegion capability ABI lands in v0.2 (RFC 036).

| Marker | Description | v0.1.x testable |
|---|---|---|
| `NEG:DMA:ALLOC_WITHOUT_CAP:PASS` | `dma_alloc` without cap rejects | Deferred to v0.2 |
| `NEG:DMA:SIZE_TOO_LARGE:PASS` | DMA alloc > 1 page rejects | Deferred to v0.2 |
| `NEG:DMA:REVOKED_REGION_REJECTED:PASS` | revoked DmaRegion cap rejects | Deferred to v0.2 |
| `NEG:DMA:ZEROIZED_ON_EXIT:PASS` | DMA page is zeroized on task exit | Deferred to v0.2 |
| `NEG:DMA:QUARANTINE_TIMEOUT:PASS` | quarantine timeout fires audit event | Deferred to v0.2 |
| `NEG:DMA:QUARANTINED_PAGE_NOT_REUSED:PASS` | quarantined page not reused before zeroize | Deferred to v0.2 |

---

## Category: STORE

**Enforcement status:** Testable at v0.1.2.  
Store corruption rejection is part of the M5 + M6 recovery path.

| Marker | Description | v0.1.x testable |
|---|---|---|
| `NEG:STORE:CORRUPT_RECORD_REJECTED:PASS` | corrupt CRC32 record is skipped on recovery | Yes |
| `NEG:STORE:PARTIAL_TAIL_IGNORED:PASS` | partial tail record is not replayed | Yes |
| `NEG:STORE:BAD_SUPERBLOCK_MIRROR_REJECTED:PASS` | corrupt superblock mirror is not selected | Yes |
| `NEG:STORE:VALID_PREFIX_RECOVERED:PASS` | valid prefix up to corruption is recovered | Yes |

---

## Category: UPGRADE

**Enforcement status:** Testable at v0.1.2.  
Signature rejection is part of the M7 / M8 verification path.

| Marker | Description | v0.1.x testable |
|---|---|---|
| `NEG:UPGRADE:UNSIGNED_RELEASE_REJECTED:PASS` | unsigned release bundle rejects | Yes |
| `NEG:UPGRADE:INVALID_SIGNATURE_REJECTED:PASS` | tampered signature rejects | Yes |
| `NEG:UPGRADE:ACTIVE_SLOT_WRITE_REJECTED:PASS` | writing to the active slot rejects | Yes |
| `NEG:UPGRADE:HEALTH_FAILURE_NOT_CONFIRMED:PASS` | unhealthy system does not confirm active slot | Yes |

---

## v0.2 additions

The following categories are registered for v0.2 but contain no
markers in v0.1.2. Each category's profile exists with
`expected_markers = []` (placeholder per RFC 025).

| Category | Introducing RFC | Target |
|---|---|---|
| `NEG:LEASE:*` | RFC 033 | v0.2.0 |
| `NEG:USER_COPY:*` | RFC 039 | v0.2.0 |
| `NEG:AUDIT:*` | RFC 039 | v0.2.0 |
| `NEG:POLICY:*` | RFC 040 | v0.2.0 |
| `NEG:EVIDENCE:*` | RFC 041 | v0.2.0 |
| `NEG:SVC:*` | RFCs 037, 038 | v0.2.0 |

---

## CI integration

Each category maps to:

- `tests/qemu/profiles/<category>.toml` — machine-readable marker list
- `cargo xtask qemu-negative <category>` — CI runner
- Upload artefact: `tests/qemu/artifacts/negative-<category>/`

See `.github/workflows/ci.yml` §`ci-qemu-negative` for the
matrix definition and artefact paths.
