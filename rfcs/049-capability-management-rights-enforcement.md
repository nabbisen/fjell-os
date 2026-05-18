# RFC 049: Capability management rights enforcement

**RFC ID:** 049
**Also known as:** RFC-v0.2-015
**Status:** Proposed
**Target version:** v0.2.10
**Phase:** Capability/syscall enforcement closure
**Closes review item:** RB-02
**Depends on:** RFC 031 (Unified capability enforcement), RFC 048 (handle-based require_cap)

## Problem

`crates/fjell-kernel/src/cap/syscall.rs:26-96` implements the capability
management syscalls.  They validate the source-cap's **lease** in most cases,
but they do **not** check whether the source cap carries the corresponding
management right:

| Syscall | Current check | Missing right check |
|---------|---------------|---------------------|
| `sys_cap_copy(src, dst_slot)`   | lease | `COPY` |
| `sys_cap_mint(src, dst, rights, badge)` | lease, rights ⊆ source.rights | `MINT` |
| `sys_cap_revoke(target)`        | lease | `REVOKE` |
| `sys_cap_inspect(target, buf)`  | (none) | `INSPECT` |
| `sys_cap_delete(target)`        | (none) | (delete has no right; ownership-only) |
| `sys_cap_drop(target)`          | (intentionally skipped — RFC 032) | (drop has no right; ownership-only) |

The bit positions exist in `CapRights` (`COPY=1<<7, MINT=1<<8, REVOKE=1<<9,
INSPECT=1<<10`) but no kernel call actually requires them.

Consequence: **possession of any capability is enough to derive, duplicate,
revoke, or inspect it**.  A service handed a narrow operational cap by
cap-broker can self-promote: copy it, mint a broader version, distribute it,
or revoke the broker's reference copy.  This breaks the least-authority
invariant that RFC 040 (cap-broker default-deny) is designed to enforce.

## Proposed fix

Add a right check at the entry of each management syscall.  Use
`fjell_cap::enforcement::require_cap` (RFC 031) with the source handle:

```rust
pub fn sys_cap_copy(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    let src = CapHandle(tf.gpr[REG_A0] as u32);
    let dst = tf.gpr[REG_A1];
    // RFC 049: require COPY right on source.
    if let Err(e) = require_cap_on(src, CapKind::Any, CapRights::COPY) {
        err(tf, e); return;
    }
    // ... existing copy logic ...
}
```

`require_cap_on` is a new helper that takes an explicit source handle (rather
than scanning):

```rust
pub fn require_cap_on(
    handle: CapHandle,
    expected_kind: CapKind,  // `CapKind::Any` skips kind check
    required: CapRights,
) -> Result<(), SysError>
```

It performs RFC 031 steps 1-5 (lookup, generation, state, kind, rights) and
step 7 (lease).  Scope check (step 6) is skipped for management ops — they
operate on the source cap itself, not a separate target.

### Right requirements per call

| Call | Required right | Notes |
|------|----------------|-------|
| `sys_cap_copy(src, dst)`     | `COPY`    | Copy: same rights, same generation root |
| `sys_cap_mint(src, dst, r, b)` | `MINT`  | Plus existing check: `r ⊆ src.rights` |
| `sys_cap_revoke(target)`     | `REVOKE`  | Targets the cap subtree rooted at the source |
| `sys_cap_inspect(target, buf)` | `INSPECT` | Read-only metadata access |
| `sys_cap_delete(target)`     | —          | Ownership: caller's CSpace slot |
| `sys_cap_drop(target)`       | —          | RFC 032: ownership-only; lease may be revoked |
| `sys_cap_bind_lease(target, lease)` | (LeaseAdmin holds LEASE_CREATE — caller-side, not source-side) | RFC 042; unchanged |

### CapRights bit semantics

The bits already exist:

```
COPY    = 1 << 7
MINT    = 1 << 8
REVOKE  = 1 << 9
INSPECT = 1 << 10
DROP    = 1 << 11   // unused: drop has no right (ownership-only)
```

Existing service grants (in `task/spawn.rs`) currently use `CapRights::ALL`
which contains all management bits.  After this RFC, services keep those
bits — the change is observable only when a service holds a **non-ALL** cap,
typically delivered by cap-broker.

### cap-broker grant behavior

RFC 040 grant logic builds a target cap with `requested_rights & policy.rights`.
With this RFC, when broker mints a cap for a requester, it must consider:

- Should the granted cap carry `COPY`?  Generally **no** for operational
  caps — recipient should not be able to clone broker-issued authority.
- Should it carry `MINT`?  Generally **no** for same reason.
- Should it carry `INSPECT`?  Usually yes — recipients should be able to
  query their own cap metadata.
- `REVOKE` is irrelevant on broker-issued caps (the broker holds the
  revocation authority via lease, not via REVOKE right).

The default cap-broker policy rules in `crates/fjell-cap-broker/src/main.rs`
should be reviewed for which management bits each grant includes.  The
existing constant `EP_RW = RIGHT_SEND | RIGHT_RECV` carries no management
bits — that's correct.  No mass change needed; document in RFC 056
(cap-broker installation) as a follow-up.

## Rationale

**Why bit-level rights for management?** Possession-implies-authority is
the seL4 / classic-capability model.  Fjell departs from that for the same
reason the kernel has lease epoch revocation: we want broker-issued caps to
be **operationally usable but not redelegable**.  Without management-right
checks the kernel cannot enforce that distinction.

**Why not check on the destination as well?**  The destination slot is in
the caller's own CSpace.  Slot ownership is sufficient — no separate right
needed.

**Why exempt `cap_drop` and `cap_delete`?**  RFC 032 deliberately makes
`cap_drop` work on revoked caps so user services can clean up after
revocation.  `cap_delete` is the in-CSpace counterpart and follows the same
ownership-only rule.  Both touch only the caller's own slots.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-cap` | Add `require_cap_on(handle, kind, rights)` helper |
| `fjell-kernel` | Insert right check at top of 4 syscalls (cap_copy, cap_mint, cap_revoke, cap_inspect) |
| `fjell-cap-broker` | (no required change — `EP_RW` already excludes management bits) |
| Existing services | No source change; ALL-rights caps continue to work |

### Backward compatibility

In-tree only.  Any caller using `cap_copy` etc. with a cap that has the
right bit set will continue to succeed.  Callers using a cap lacking the
right bit will now correctly fail.  No such caller exists today — but
the failure mode is now correct rather than silently wrong.

### Audit-trail change

Failed management calls now record `AuditKindInternal::PermissionDenied`
with the source cap slot index and the missing right bit in `arg1`.  This
is uniform with other RFC 031 enforcement records.

## Test plan

### Host (unit tests in `fjell-cap`)

1. `require_cap_on` returns `Ok` for cap with required right set
2. Returns `PermissionDenied` for cap missing the right
3. Returns `InvalidCap` for stale-generation handle
4. Returns `LeaseRevoked` for cap with revoked lease
5. `CapKind::Any` skips kind check (verified)

### Host (unit tests in `fjell-kernel`)

6. `sys_cap_copy` with source cap missing `COPY` → `PermissionDenied`
7. `sys_cap_mint` with source cap missing `MINT` → `PermissionDenied`
8. `sys_cap_mint` with source `MINT` but `requested ⊄ source.rights`
   → `PermissionDenied` (unchanged invariant)

### QEMU (new markers under `capability` profile)

9. `NEG:CAP:COPY_WITHOUT_RIGHT_REJECTED:PASS` — neg-test holds a cap
   missing `COPY`, calls `sys_cap_copy(cap, 11)`, receives `PermissionDenied`.
10. `NEG:CAP:MINT_WITHOUT_RIGHT_REJECTED:PASS` — same for `sys_cap_mint`.
11. `NEG:CAP:REVOKE_WITHOUT_RIGHT_REJECTED:PASS` — same for `sys_cap_revoke`.
12. `NEG:CAP:INSPECT_WITHOUT_RIGHT_REJECTED:PASS` — same for `sys_cap_inspect`.

Setup: neg-test calls `sys_cap_mint(SLOT_OWN_EP, 15, EP_RW, 0)` — produces
a cap in slot 15 with only `SEND|RECV` rights (no management bits).
Subsequent calls using slot 15 as source should fail with `PermissionDenied`.

## Implementation notes

- `require_cap_on` is implemented in `fjell-cap/src/enforcement.rs`; the
  kernel re-exports it via `fjell-cap`.  No new dependency direction.
- The 4 management syscalls share boilerplate at entry — consider a tiny
  internal macro `require_management_right!(src_arg, RIGHT_BIT)`.
- The kernel's existing `cs.copy(src, dst)`, `cs.mint(...)`, `cs.revoke(...)`
  helpers do not change — only the call-site preamble.
- Audit records for these failures use the existing
  `AuditKindInternal::CapMint` / `CapCopy` / `CapDelete` kinds; consider
  adding `AuditKindInternal::CapMgmtDenied` (one variant) if log clarity
  is preferred.  Defer to implementation; not part of this RFC's
  acceptance.

## Open questions

- Should `CapRights::DROP` be repurposed?  Currently unused.  Recommendation:
  reserve it for future possible use (e.g. "this cap may be force-dropped
  from another CSpace by holder of a delegation cap") — leave alone for now.

- Should management rights flow through `cap_copy`?  i.e. when X copies a
  cap with COPY|MINT to slot Y, does Y get COPY|MINT too?  Recommendation:
  **yes** — `cap_copy` semantics are "duplicate as-is".  To restrict, use
  `cap_mint` with narrowed rights.  This matches seL4 derivation semantics.
