# RFC-0.28-004: The wrapper crate worked around its own helpers instead of fixing them

**Status:** Accepted — by the owner (nabbisen), 2026-09-08; implementation may begin (RFC 000)
**Milestone:** 0.28
**Tracks.** **E-033**, widened. `fjell-syscall`'s generic `ecall` helpers
declare contracts too narrow for two of their callers, and the crate has twice
worked around that rather than repaired it — once by silently losing registers,
once by **issuing the same syscall twice**.
**Touches.** `crates/fjell-syscall`, `tests/abi/snapshot.json` if any signature
moves, `crates/fjell-tools`'s `SYSCALL-CALLSITE-001`. **Does not touch the
kernel or `fjell-abi`.**
**Relates to:** **E-032**/RFC-0.28-002 (same defect class, which that line
consolidated *into* this crate); RFC-0.28-001 (`schedule_next` after every
trap — load-bearing in §2 below); RFC-0.24-003 (ABI snapshot discipline).

## Summary

RFC-0.28-002 deleted 28 hand-rolled syscall blocks from services on one
premise: **the wrapper is correct, so call the wrapper.** That premise is now
the project's single point of correctness for syscall register contracts.

It does not hold for two of the wrappers.

`fjell-syscall` has four generic helpers — `ecall0`, `ecall1`, `ecall2`,
`ecall3`. `ecall2` declares:

```rust
in("a7") nr,
inlateout("a0") a0 => r0,
inlateout("a1") a1 => r1,
in("a2") a2,          // ← plain input
in("a3") a3,          // ← plain input
// a4, a5, a6: not declared at all
```

**27 call sites use it. Exactly two issue a syscall the kernel writes past `a1`
for**, and both are broken — a bounded, verified scope, not a suspicion about
27 wrappers.

## The two, and they fail differently

### 1. `sys_ipc_recv` loses everything the delivery carried

`lib.rs:193` routes `IpcRecv` through `ecall2`. On delivery the kernel writes
`a1` (tag), `a2`–`a5` (words) and `a6` (attested sender identity,
`cap/syscall.rs:497`). Through `ecall2`, `a2`/`a3` are declared plain inputs —
**E-032's Bug B** — and `a4`/`a5`/`a6` are undeclared — **E-032's Bug A**. So
the public wrapper carries *both* classes the previous line existed to remove.

Live in `fjell-auditd`, `fjell-bootctl`, `fjell-configd`.

### 2. `sys_cap_inspect` issues the syscall twice, and does not check the second

`lib.rs:671-695`, with the crate's own comment naming the cause:

```rust
let (r0, kind) = ecall2(SyscallNumber::CapInspect as usize, cap.0 as usize, 0, 0, 0);
to_result(r0)?;
// rights and badge returned in a2/a3; ecall2 only returns a0/a1.
// Use inline asm to read them.
…  "li a7, 13", "ecall",   // ← the same syscall, a second time
```

`kind` comes from the first call; `rights` and `badge` from the second. **The
second call's status is never examined** — no `to_result`, no branch — and the
function returns `Ok` regardless.

**That is not merely wasteful.** RFC-0.28-001 established that `trap_dispatch`
calls `schedule_next` after *every* trap, so another task runs between the two
`ecall`s as a matter of course. If the capability is revoked in that window —
and lease revocation is a thing this kernel does — the second call fails, `a2`
and `a3` hold whatever the failure path left, and
`sys_cap_inspect` returns **`Ok((kind, garbage, garbage))`** to a caller that
asked it a security question.

Live: `fjell-proxy-text:53` uses it for *"a real, kernel-verified rights"*
value on the ABDD path.

## What this actually is

Not two bugs. **A helper layer whose contract is narrower than the ABI it
fronts, and a crate that routed around it twice rather than widening it.** The
comment at `lib.rs:674` is the tell: the author knew `ecall2` was insufficient
and reached for a second `ecall` instead of fixing the helper.

`ecall3` widens `a2` to `inlateout` and stops there — `a3`–`a6` are still
undeclared. The pattern is "widen just enough for the caller in front of me."

## The settled part

**D1 — `sys_cap_inspect` issues one syscall.** Whatever the helper layer
becomes, the double-call goes. A single `ecall` whose result is checked once.

**D2 — `fjell-syscall` is in `STABLE_CRATES`.** Any signature change is an
ABI-snapshot event: **show the diff is exactly what you intend before
regenerating** (RFC-0.24-003). A regeneration that absorbs an unexplained change
is how a real break disappears and the gate reports `PASS` forever after.

**D3 — `SYSCALL-CALLSITE-001` must gain some coverage of this crate.**
RFC-0.28-002's guard exempts `fjell-syscall` entirely, which is why none of this
was caught by the line that was looking for exactly this. The exemption is not
wrong — raw asm legitimately lives here — but *total* exemption means the one
place the contract must be right is the one place nothing checks it. The rule's
shape is §5's second question.

**D4 — Evidence is a committed log** (RFC-0.27-004). E-013 applies: this changes
how three services receive IPC.

## The open questions — §5

**(a) What should the helper layer be?**

1. **One widest-contract `ecall`** declaring `a0`–`a6` clobbered, used
   everywhere. Always correct by construction; costs codegen quality at the 25
   sites that do not need it.
2. **Keep per-arity helpers, widen each to the maximum the kernel can write.**
   Smallest diff; keeps four places where the contract is written down.
3. **Per-syscall declarations** — each wrapper states exactly what its syscall
   writes. Most precise, most code, and a *fourth* copy of the ABI contract that
   can drift from the kernel.

**(b) What should the guard check inside `fjell-syscall`?** It cannot be "no raw
asm" — that is the crate's job. Candidates: every block must declare `a0`–`a6`;
or a per-syscall table the guard owns; or the weakest useful rule, that any
block issuing `IpcRecv`/`IpcCall`/`CapInspect` declares the registers those
write.

**(c) Should `sys_ipc_recv` survive at all?** `sys_ipc_recv_msg` returns
everything the delivery carries; `sys_ipc_recv` returns one word and drops the
rest by design. Three callers. Fixing its clobbers and keeping a wrapper that
discards attested identity may be the wrong repair — but removing it is an ABI
removal, so **escalate rather than decide it in code.**

**Answer all three in writing before implementing.** I have no inclination on
(a); on (c) I lean toward asking whether the wrapper earns its place, and that
is a lean, not a ruling.

## Scope

`crates/fjell-syscall/src/lib.rs`; `SYSCALL-CALLSITE-001`; `tests/abi/snapshot.json`
if a signature moves; **E-033** → `CLOSED`, widened first to name the
`sys_cap_inspect` double-call.

### Non-goals

- **The kernel and `fjell-abi`.** The ABI is what it is; this line makes the
  wrapper honest about it.
- Changing which registers any syscall writes.
- The 25 `ecall2` sites whose syscalls only write `a0`/`a1` — they are correct
  today and stay untouched unless (a) chooses shape 1.
- **E-034**, E-013, E-019, E-025, E-027, E-028, E-029, E-030.

## Risks

**Widening a clobber list can only cost codegen, never correctness — which
makes shape 1 tempting and under-argued.** If it is chosen, choose it on its
merits and say what it costs; "it is always safe" is not an argument for a
design, it is an argument against thinking.

**The `sys_cap_inspect` repair changes an observable.** Today two syscalls are
issued and the audit ring records two `CapInspect` events per call. After the
fix there will be one. Any test or expectation counting audit records will move,
and that is a **finding to report, not a number to adjust**.
