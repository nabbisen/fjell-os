# RFC-0.38-002 R1–R9: a lease binding needs the authority it claims

**Governing RFC:** [../done/RFC-0.38-002-a-lease-binding-needs-the-authority-it-claims.md](../done/RFC-0.38-002-a-lease-binding-needs-the-authority-it-claims.md)
**Handoff:** [../handoffs/RFC-0.38-002-a-lease-binding-needs-the-authority-it-claims/implementation-handoff.md](../handoffs/RFC-0.38-002-a-lease-binding-needs-the-authority-it-claims/implementation-handoff.md)

Written against base `1b403af`, in the handoff's order: R1, R3 and R4 against the
unfixed kernel, R2, R5, R6, then R8 and R9. R7 governs the prose.

---

## The one place the RFC is wrong, because everything else depends on it

The RFC (and E-088) say a task can rebind a revoked capability **to its own revoked
lease** and recover it: *"the capability is re-stamped with the post-revocation epoch,
and `require_cap`'s lease check passes again."* **Measured against the unfixed kernel,
that does not happen.**

The revoke-then-rebind tier (R4), run before any kernel change, printed
`NEG:HARNESS:UNEXPECTED_OK` for the bind — the kernel *accepted* a bind to a `Revoked`
lease, as the RFC says — and then `NEG:CAP:REVOKED_STAYS_REVOKED:PASS`: the capability
**still failed** `sys_cap_inspect` with `LeaseRevoked`. The reason is in
`LeaseTable::check_active` (`crates/fjell-kernel/src/lease/mod.rs`): it returns
`LeaseRevoked` unless `slot.state == Active` **and** the epoch matches, and
`LeaseTable::revoke` leaves the state `Revoked` for good (nothing returns a slot to
`Empty` or `Active`). Re-stamping the epoch can satisfy the second condition only.

What **did** recover authority before the fix is the finder's original path: bind the
dead capability to a *different*, still-`Active` lease, as a task that holds no
`LeaseAdmin`. The R5 tier was extended to do exactly that, and before the fix it
revived the capability (below). So the defect was real and the fix is the same two
checks; the "sharper still" consequence in E-088 was not. E-088 carries a dated
correction (the original text is left as written).

Two consequences for how R3 reads: refusing a bind to a `Revoked` lease is **kept**
as fail-closed hardening — a binding that can never pass is only a way to hide the
revocation from the binder, and the step-2 comment promised it — but it is not the
recovery fix. The recovery fix is R2.

## R1 — re-derived, with controls (`/usr/bin/grep -a` throughout)

| Claim | Found |
|---|---|
| Absent check | `sys_cap_bind_lease` (`crates/fjell-kernel/src/cap/syscall.rs`, was `:416`) contained no authority check; the only text where it belonged was a comment saying the check was reverted |
| Three `LeaseAdmin` grants | `task/spawn.rs:622` sample-service slot 1; `:657` neg-test slot 4; `main.rs:1141` init slot 30 — all `rights: ALL_NON_META`, which includes `LEASE_CREATE` (bit 16) |
| Ungated routing | `CapBindLease` is an arm of the `dispatch_m3` list (`trap/syscall.rs:29`) and dispatched at `:393` with no gate |
| Unconditional write | `cap.lease = Some(binding)` on the looked-up slot, no test of an existing binding |
| `get` admits `Revoked` | `LeaseTable::get` refuses only a generation mismatch or `Empty` — confirmed; and `current_epoch` goes through it |
| Not in RFC 015's list | `check_lease` call sites: `check_right`, `sys_cap_copy`, `sys_cap_mint`, `sys_cap_revoke`, `sys_cap_inspect`, `sys_dma_alloc`, and `fjell-cap`'s `require_cap`. None in `sys_cap_bind_lease` (it is a *producer* of bindings, not a user of one) |
| Callers | exactly three call sites: `fjell-neg-test/src/main.rs:176`, `fjell-sample-service/src/main.rs:214` and `:278`. neg-test obtains its lease with `sys_lease_create(SLOT_LEASE_ADMIN)` — it holds the capability; sample-service holds `LeaseAdmin` at slot 1. A third caller: none (control: the same search finds all three, and the `fjell-syscall` wrapper) |

The grants are not "all spawned services at slot 4" (the comment's claim) but three
tasks at three different slots, as the RFC says.

**The class sweep.** The defect shape is "a doc comment promises a check the body does
not perform, behind an ungated dispatch arm". Every handler reachable from the
dispatch tables was classified by script (`sys_*`/`dispatch_*` in
`trap/syscall.rs` and `cap/syscall.rs`: which of `require_cap_on_ct`, `require_cap`,
`check_right`, `cs.get`, `check_lease`, `slot_by_handle` its body contains), then the
doc comments with precondition wording were read against the bodies.

- Handlers whose doc comment names a required capability and right —
  `sys_task_spawn` (TaskCreate), `sys_platform_reboot` (Reboot/REBOOT),
  `sys_irq_bind`/`sys_irq_wait` (Interrupt/IRQ_BIND), `sys_irq_ack` (IRQ_ACK),
  `sys_dma_alloc` (DmaRegion/DMA_ALLOC), `sys_mmio_map` (MmioRegion/MMIO_MAP),
  `sys_cap_install` (CapInstall; not dispatched) — **all 8 enforce it**, with the
  right the comment names (checked in the arguments of each call, not by the presence
  of a gate). The script is only a coarse filter (it listed `slot_by_handle`, a
  lookup, in `sys_cap_bind_lease`'s body); the verdicts come from reading each doc
  comment against its body.
- Ungated by design and documenting no precondition: `sys_yield`, `sys_exit`,
  `sys_debug_write`, `sys_platform_info_get`, `sys_cap_delete`, `sys_cap_drop`,
  `sys_ipc_reply` (gated by the reply edge), and the `dispatch_*` forwarders.

**No second instance.** One thing noticed and left alone: `sys_mmio_map`'s doc lists a
"Scope check — (deferred; Any accepted)" step; it says so in the same breath, so it is
a disclosed deferral, not a promised check.

## R3 — the lease must be `Active` (done before R2)

`LeaseTable::active_epoch(id)`: `InvalidCap` for a lease that does not exist (as
`get` already did), **`LeaseRevoked`** for a lease that exists and is not `Active`.
`LeaseRevoked` because it is the error `check_active` already gives a use of a binding
to that lease — the bind and the use say the same thing about the same fact — and the
ABI has no more specific candidate (`InvalidArg`/`BadState` would describe the
caller's mistake, which this is not). `current_epoch` is unchanged and still admits a
`Revoked` slot, because `sys_lease_inspect` reads it. The step-2 comment is replaced by
the doc comment that now states the order.

## R4 — demonstrated failing first, and the assertion that was already true

`test_cap_rebind_after_revoke` (neg-test): create lease, bind a copied endpoint
capability, **control** (`sys_cap_inspect` succeeds while the lease is live), revoke,
**control** (it now fails `LeaseRevoked`), rebind to the same lease, assert the bind
is refused with exactly `LeaseRevoked` (`NEG:CAP:REBIND_AFTER_REVOKE_REFUSED`), assert
the capability is *still* `LeaseRevoked` afterwards (`NEG:CAP:REVOKED_STAYS_REVOKED`).
The capability is exercised with `sys_cap_inspect`, which runs the lease check without
blocking or touching a queue.

Pre-fix (`prefix2-capability-serial.log`): `NEG:HARNESS:UNEXPECTED_OK` for the bind,
no `REBIND_AFTER_REVOKE_REFUSED`, and `REVOKED_STAYS_REVOKED:PASS` present.
Post-fix: both markers `PASS`. So the first assertion is the one the fix changes; the
second is the property and it held throughout — see the section above.

## R2 — the check, through the borrow the function already holds

`sys_cap_bind_lease(tf, tidx, ct: &mut CapTable)` scans the caller's CSpace with
`ct.cspace(tidx)` — a shared reborrow of the `ct` it already has, ending with the
`match` that consumes it — for an occupied slot that is a `CapKind::LeaseAdmin`,
`CapState::Active`, carries `LEASE_CREATE`, and whose own lease binding (if any) still
passes `check_lease`. Only then does it call `ct.cspace_mut(tidx)` to write. There is
no second `get_kernel_state()`, no raw pointer, no `unsafe` beyond the existing
`get_lease_table()` that was already in the function; the two borrows of `ct` are
sequential, not overlapping, which is the whole of what the reverted attempt got
wrong. **No escalation was needed.**

The scan (rather than a handle argument) is forced by the ABI: the syscall's registers
are `(a0 = capability, a1 = lease)` and an ABI change is out of scope. Refusal is
`PermissionDenied`, the project's default-deny error, returned before the lease is
looked at so that an unprivileged caller learns nothing about lease state.

Two strictnesses beyond the letter of R2, stated so they can be disputed: the
authority capability must itself be unrevoked (`check_lease`) and `Active`. Both are
the properties `require_cap` demands of any other authority.

## R5 — the unprivileged caller, demonstrated failing

`test_cap_bind_lease_without_authority`: bind a copied capability to lease L1 and
revoke L1 (dead — asserted); create L2, which stays `Active`; mint a `LeaseAdmin`
capability **without** `LEASE_CREATE` from slot 4; drop slot 4; try to bind the dead
capability to L2 (refused `PermissionDenied`, `…WITHOUT_RIGHT_REJECTED`); drop the
narrow capability; try again (refused, `…WITHOUT_AUTHORITY_REJECTED`); assert the
capability is **still dead** (`NEG:CAP:REVOKED_NOT_REVIVED_BY_UNPRIVILEGED_BIND`).
It runs last: it drops neg-test's own `LeaseAdmin`.

Pre-fix (`prefix2-capability-serial.log`): **three** `NEG:HARNESS:UNEXPECTED_OK` — the
bind without the right, the bind without the capability, and the final
`sys_cap_inspect`, i.e. **the dead capability was usable again**. That third line is
the recovery the RFC set out to close, measured. Post-fix: all three markers `PASS`.
The first draft of this scenario set up its mint into scratch slot 11, which an earlier
scenario leaves occupied; it printed `NEG:HARNESS:SETUP_FAILED` rather than a false
pass, and was moved to the (always-empty) narrow-cap slot.

## R6 — the documentation

The doc comment at `cap/syscall.rs` states the three checks in order and the replace
semantics; the "has been reverted … targets v0.8.x" note is gone. `RFC 042` → `RFC 006`
at `cap/syscall.rs` and at `crates/fjell-abi/src/syscall.rs` (which now also states
the `LeaseAdmin` and `Active` requirements; the sentence "used by the neg-test service"
was true of three services' worth of callers and is dropped). A sweep for the string
across `crates/`, `docs/`, `rfcs/` found two more `RFC 042` mentions in `task/spawn.rs`
comments (`:622`, `:658`); they say the `LeaseAdmin` grant exists for RFC 042's IPC
tests, which is true, and were left. The frozen `docs/src/history/v0.2.x-dev-summary.md`
carries the same phrase as history and was left (R7). `crates/fjell-cap/src/cspace.rs`
(the `copy` doc) now says binding needs `LeaseAdmin`/`LEASE_CREATE` and an `Active`
lease; `docs/src/external-design/capability-lease.md` and `docs/src/api/syscalls.md`
(row 16) say the same in one sentence each.

## R7 — no retro-claim

Nothing written says the check was present or known-and-accepted. E-088's closure
says it was documented as present and absent; the correction paragraph says what the
tree did, not what anyone knew.

## R8 — E-088 closed

`rfcs/ERRATA.md`: status `CLOSED`, Summary row updated, with the dated correction
above. `docs/src/releasing/v1-limitations.md` needed no change (E-088 was never
ACCEPTED, so no limitation row exists to retire).

## R9 — the gates

See the review request for the exact commands and exit statuses. Rebuild then
re-record: only `fjell-neg-test.bin` moved (1 line of `tests/repro/baseline-digests.txt`),
committed with the baseline.

## Left alone, and noticed

- **Nothing frees a lease slot, a task slot, an endpoint or a frame** (`LeaseId` is the
  slot index, `0..32`; there are 32). The register owns it; not touched.
- **A holder of `LeaseAdmin` may still rebind any capability it owns to any `Active`
  lease, replacing a binding.** That is what the authority is; the ABI cannot say
  "this administrator, this lease". Whether a bound capability should be immutable
  once bound is a design question, not answered here.
- **`RFC-v0.7.4-003` specified a `LEASE_BIND` right and a scope check** for this
  syscall (`rfcs/done/RFC-v0.7.4-003-capability-authority-hardening.md:54`, `:431`).
  `CapRights` has no `LEASE_BIND`; the doc comment, RFC 049 (`:78`) and this RFC all say
  `LEASE_CREATE`, which is what was implemented. No scope check was added.
- **The kernel has no `[lib]` target**, so the inline `#[cfg(test)]` module in
  `lease/mod.rs` does not run in any tier; a unit test of `active_epoch` there would be
  exactly the "test that never runs". The QEMU tier is the only place `active_epoch`
  is exercised.
- The audit record for a successful bind is still `AuditKindInternal::CapMint`; a
  refused bind records nothing. Left as it was.
- `lease/mod.rs`'s `wake_or_cancel_blocked_ipc_for_lease` doc still says
  `QUEUE_DEPTH=8` (it is 32).
