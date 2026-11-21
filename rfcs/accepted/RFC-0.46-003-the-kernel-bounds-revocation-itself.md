# RFC-0.46-003: The kernel bounds revocation itself — a lease is revoked by its owner

**Status:** Accepted — accepted 2026-11-21
**Evidence:** marker:ipc:NEG:LEASE:REVOKE_NOT_OWNER_REFUSED:PASS, test:crates/fjell-cap/src/lease_revoke.rs::a_non_owner_without_the_named_right_is_refused, test:tools/fjell-consistency-check/src/capability_topology.rs::by_name_right_holders_counts_the_holders_and_refuses_a_second
**Milestone:** 0.46
**Tracks.** The second of the three places 0.45 put cap-broker inside the kernel's trust boundary: *"I
revoke lease L"* is accepted without the kernel asking whether the revoker owns L. Narrows the
`CAP_REVOKE` confused-deputy filing (the owner's); closes no register entry on its own.
**Touches** *(indicative)*: `crates/fjell-kernel/src/trap/syscall.rs` (`sys_lease_revoke`),
`crates/fjell-cap/src/rights.rs` (one right), `crates/fjell-kernel/src/task/spawn.rs` (two grants),
`crates/fjell-kernel/src/audit/ring.rs` and `crates/formats/fjell-audit-format/src/lib.rs` (one audit
kind), `crates/services/fjell-neg-test/src/main.rs`, `tests/qemu/profiles/{ipc,policy,audit}.toml`,
`tests/abi/snapshot.json`, prebuilts and `tests/repro/baseline-digests.txt`.
**Relates to:** **RFC-0.45-002** (check **e** already asks `lt.owner` of an installer),
**RFC-0.42-001** (owner-only `sys_cap_bind_lease`, the rule copied here), **RFC-0.46-001** (recycling —
this line makes it safer to land), `fjell_service_api::cap_broker::revoke_authorised` (stays).

---

## What is true now

Re-measured at `3f141a9`:

- `sys_lease_revoke` (`crates/fjell-kernel/src/trap/syscall.rs:615`) requires a `LeaseAdmin` with
  `LEASE_REVOKE` **and** passes `Some(&ObjectScope::Lease(id))`, so a *scoped* grant would be bounded to
  the one lease it names. **A bound on which lease a holder may revoke already exists.**
- **All four `LeaseAdmin` grants carry `ObjectScope::Any`** (`task/spawn.rs:733`, `:767`, `:1053`;
  `main.rs:1406`): `sample-service`, `neg-test`, `cap-broker`, `init`.
- `lt.owner(id)` exists and `sys_cap_install` already uses it; `sys_cap_bind_lease` uses it too.

So the broker's `Any`-scoped `LeaseAdmin` can revoke **any lease in the system**, and the only thing
standing in the way is `revoke_authorised`, a predicate *inside the broker* — a service-side check on the
most privileged client. A compromised or mistaken broker is unbounded on revocation.

### The scope mechanism is not the answer, and this RFC does not "fix" it

The grants carry `Any` **because they have to**: the leases these holders revoke are created at runtime,
so their ids cannot appear in a spawn-time grant. Narrowing the grants' scope ends at "grant a capability
naming an id that does not exist yet". **Ownership is the only workable bound for a dynamically created
lease.**

### A finding the handoff did not have: two existing tests revoke a lease they do not own

`neg-test` revokes leases that **`sample-service` created**, in both `ipc`-tier scenarios that wake a
blocked task:

- `test_ipc_blocked_recv` — sample-service creates the lease and binds its own capability to it, blocks in
  `sys_ipc_recv` on that capability, and neg-test revokes the lease to wake it (`fjell-neg-test/src/main.rs`,
  "5. Revoke the lease — this triggers cancel_blocked_ipc_for_lease");
- `test_ipc_blocked_call_and_late_reply` — the same shape (line ~1023).

An ownership check **breaks both**, and this is not an accident of how they are written. The scenario is
*a task blocked on a lease-bound capability must be woken by revocation*; the owner is the blocked task;
**an owner-only rule therefore leaves nobody able to revoke** until the owner exits. Any ownership rule has
this consequence for any blocked owner. (The broker's pattern is unaffected: the broker owns the lease, the
*grantee* is the one blocked.)

## Requirements

**R1 — the kernel asks who owns the lease.** In `sys_lease_revoke`, *after* the existing capability check
(kind, `LEASE_REVOKE`, scope) and *before* `lt.revoke`: the lease's owner must be the caller — else
`PermissionDenied`. The order is RFC-0.42-001 R2's: a caller without authority learns nothing about a lease,
not even who owns it. `lt.owner` failing (`InvalidCap`: no such lease) is returned as it is — the same error
`revoke` would have given.

**R2 — one named exception, a right granted by name.** `CapRights::LEASE_REVOKE_ANY` (bit 32), in **neither**
`ALL_NON_META` nor `ALL_DEFINED` — the `LEASE_BIND` / `CLOCK_READ` precedent: a blanket grant must not
silently confer it. A `LeaseAdmin` carrying it may revoke a lease it does not own; one without it may not.

**R3 — who holds it, and who does not.**

| Holder | Gets `LEASE_REVOKE_ANY`? | Why |
|---|---|---|
| `cap-broker` | **No** | The point of the line: its revocation is bounded to the leases it created. `revoke_authorised` stays as defence in depth. |
| `sample-service` | No | Revokes only its own. |
| `init` | **No — proposed** | `init` revokes no lease in the tree today (the grep for `sys_lease_revoke` finds the broker, `neg-test` and `sample-service`). The handoff said *"`init`'s administrative path, if you keep one, is a deliberate exception and must be named"*; I propose there is **none to keep**, on the 0.45 ruling that a kind or right grows *with a consumer, never ahead of one*. Say if you want it named anyway. |
| `neg-test` | **Yes, on a second `LeaseAdmin` capability** | The two `ipc` scenarios above cannot be written without a non-owner revoker. |

`neg-test` keeps its existing `LeaseAdmin` (slot 4, no `LEASE_REVOKE_ANY`) **and** gains a second
(slot 27, with it). Scenarios that revoke their own leases keep using slot 4, so the *refusal* is
demonstrable from the same task that holds the exception: it asks slot 4 to revoke a lease it does not own.

**R4 — the refusal is audited.** A new published `AuditKind` `LeaseRevokeRejected = 0x0606` (the next code
after `RFC-v0.7.4-003`'s list, which stops at `0x0605`), as `LeaseBindRejected = 0x0604` is for a refused
bind; `arg0` = the lease id, `arg1` = the owner's task index (or `0xFFFF`), `result` = the error. A tool
should not decode a result field to tell a revocation from a refusal (the *Readable state* principle that
ruled 0.45-002's two audit variants).

**R5 — the demonstrations, each shown red by mutation on a real boot.** `policy` already has
`NEG:POLICY:REVOKE_FOREIGN_LEASE_REFUSED:PASS` (neg-test asks the **broker** to revoke a lease the broker
did not grant). Today it passes because the broker's `revoke_authorised` refuses. After this line it must
be possible to show the **kernel** is what refuses:

1. neg-test, through slot 4 (no `ANY`), revokes a lease `sample-service` owns → `PermissionDenied`, and the
   lease's epoch is unchanged (a new marker, in `policy` or `ipc`);
2. the broker's service-side check mutated **off** (the existing B2 mutation) leaves
   `REVOKE_FOREIGN_LEASE_REFUSED` **green** — the kernel refuses what the broker no longer does. This is the
   proof that R7 became defence in depth rather than the only bound;
3. the kernel's check mutated off **and** the broker's left on keeps that marker green too (the broker still
   refuses), so a third mutation — *both* off — is the one that goes red, and the table says so;
4. the two `ipc` scenarios pass through slot 27 (`ANY`), and with the right removed from slot 27 they go red.

**R6 — the audit trail carries it.** `audit.toml` echoes `"kind":"lease.revoke_rejected"` for the refusal in
R5.1, as it does for `cap.install_rejected`.

### Non-goals

- Narrowing `Any`-scoped grants (above). `revoke_authorised` stays and stays host-tested.
- **Lease ownership transfer**, and **recursive revocation** (E-093, deliberately kept out).
- Per-owner lease quotas (that is RFC-0.46-001's concern, because the quota is a recycling question).

## Risks

- **A test-only authority.** `neg-test` holding `LEASE_REVOKE_ANY` is, in the design bar's words, a
  mechanism kept for a test. It is bounded by being the audited exception the project already has for
  `neg-test` (it holds `ALL_NON_META` `LeaseAdmin`, every MMIO region and a `DmaRegion`), and by there being
  no way to write the two scenarios without it (above). If you would rather not, the alternative is to
  rewrite the two scenarios to *exit* the owner rather than revoke — which stops testing the revoke path.
- **One more `TaskId::new(x, 0)` site.** R1 compares the owner with `TaskId::new(tidx as u16, 0)`, exactly as
  `sys_cap_bind_lease` and `sys_cap_install` do. This is correct because task generations are always 0
  (`TaskTable::remove` has no callers). It is **not** a reason to take that for granted; this RFC adds a
  site to the 26 and changes nothing about them.
- **`init`'s exit-time revocation is unaffected** (`revoke_owned_by` is kernel-internal and never goes
  through `sys_lease_revoke`).
- **The ABI moves**: a new `CapRights` constant, a new `AuditKind` variant, `LeaseAdmin` grants. Predicted;
  the snapshot is regenerated deliberately.

## What this needs from the architect

1. R3's table — in particular **`init` gets nothing**, and `neg-test` gets the second capability.
2. R4's code, `0x0606`, which is outside `RFC-v0.7.4-003`'s listed range.
3. Whether `LEASE_REVOKE_ANY` is the right name and bit (32 is the next free).

## Rulings — architect, 2026-11-21 (`.git-exclude/reviewed/review-record-0.46-stage-1.md`)

**Accepted. The decisions this RFC asked for are settled here, and the requirements below are added to it; reasoning is in the review record.**

* **`init` gets no exception: accepted** (the 0.45 ruling applied: a right grows with a consumer). Do not name
  an exception with no caller.
* **`neg-test` gets `LEASE_REVOKE_ANY` on a second `LeaseAdmin`: accepted — and it is not a new authority.**
  Four images hold `LeaseAdmin` with `ObjectScope::Any` today (`sample-service`, `neg-test`, `cap-broker`,
  `init`) and each can already revoke any lease. The exception takes **four unbounded holders to one named
  test image and bounds the other three**: a strict reduction. Say so in the code comment at the grant.
* **Bit 32, in neither aggregate: accepted.** **Audit `0x0606`: accepted — and a new requirement:** the
  published decoder implements `0x0601`, `0x0602`, `0x0604`; **`0x0603` (`LEASE_BOUND`) and `0x0605`
  (`LEASE_REVOKED`) are specified and absent.** As drafted, a revocation's *refusal* is audited and its
  *success* is recorded nowhere. **Add `0x0605` with `0x0606`** (a pair, as `CapInstalled` /
  `CapInstallRejected` are), or state in the RFC why a revocation needs no record.
* **New condition — the exception is gated.** Exactly one image may hold `LEASE_REVOKE_ANY`, and a second
  holder must not appear quietly: `capability-topology` (or a sibling assertion) counts the holders and fails
  on any other than `neg-test`. A bound nobody checks is not a bound.
* **R5's mutation table is kept exactly as written** — with two independent bounds neither single mutation
  goes red and *both off* does, and it says so.
