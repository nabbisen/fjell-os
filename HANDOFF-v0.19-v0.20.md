# Fjell-OS v0.19.0 → v0.20.0 Handoff

**Date:** 2026-06-04
**Archive:** fjell-os-0_20_0.tar.gz
**Working tree:** /home/claude/work/fjell-os

---

## Current state

v0.20.0 is packaged. All mechanical gates pass. v1.0.0 tag is architect-gated.

---

## What v0.20.0 delivered (against the v0.19.0 architect review)

### All release blockers resolved
- **RB-01a/b** — `check_err` is fail-closed; `qemu_run` fails on forbidden markers.
- **RB-02** — Gate 11 (callsite-audit) genuinely wired into `release-rehearsal`; verified in output.
- **RB-03** — v1-limitations placeholder statement replaced with per-category truth.
- **RB-04** — `cargo xtask provision-dev --allow-tofu-provision` implemented; silent zero-key default eliminated via verifyd build.rs embed + loud unprovisioned warning.

### Two previously-unknown kernel bugs found and fixed by the fail-closed harness

The fail-closed RB-01 requirement exposed that **both ipc markers had been false passes** since introduction, concealing:

1. **IPC words ABI broken end-to-end** — `sys_ipc_call_words` never packed the word count into tag bits 16–23 (kernel's `build_msg` copied 0 words); `deliver()` wrote badge→a2 and words→a3..a6, colliding word 3 with the identity write, while userspace read w0 from a2. Every payload word was silently dropped. Label-only protocols (policy, svc) worked; the ipc scenarios were false-passing against `LeaseId(0)` (a long-revoked lease sample had mistakenly bound). Fixed: `sys_ipc_call_words` packs `tag | (3<<16)`; `deliver()` writes words to a2..a5, identity to a6, badge removed.

2. **Reply-edge cancellation missing from lease revoke** (the v0.19.0 recorded finding) — `wake_or_cancel_blocked_ipc_for_lease` walked endpoint queues only; server-side reply edges were never cleared, so callers blocked awaiting a reply were never woken. Now binds `ct`, calls `ct.cancel_replies_for_lease`, wakes cancelled callers with `LeaseRevoked`. Fixes the timing-dependent hang that was silently shadowing policy/audit/svc scenarios in some boots.

**`NEG:IPC:LATE_REPLY_REJECTED:PASS` is real for the first time (3/3 ipc markers).**

### Also fixed (inline false-pass patterns)
- sample-service `BLOCKED_CALL` arm was `Err(_)` — now requires `Err(LeaseRevoked)` specifically.
- neg-test late-reply inline match had the same false-pass structure as the old `check_err`.

---

## Validation state (v0.20.0)

```
Negative profiles (fail-closed): 9/9 PASS — 27 real markers
  capability 8 · mmio 3 · dma 3 · audit 1 · user-copy 2
  policy 4 · harness 1 · ipc 3 · svc 2
QEMU smokes:        4/4 PASS
Host tiers:         566 tests, 5/5 required tiers PASS
Repro check:        PASS (28 artefacts)
Verus:              3× MACHINE-CHECKED-PASS, version pinned + matched
Rehearsal:          ALL MECHANICAL GATES PASS (Gate 11 line visible)
```

---

## Pending (owner gates only — no code blockers)

- **Gate 9 sign-off:** review updated `docs/release/v1-limitations.md` (ipc now 3/3; item 6 provision flag implemented; svc 2/4 READY pair remains pending startup timing).
- **v1.0.0 tag:** architect-gated. The architect review (v0.19.0) listed v1 preconditions — all code items done; awaiting re-review of v0.20.0.

---

## Key invariants (standing)

- IpcReply ABI: tag in a1, not a0. IPC words: w0..w3 in a2..a5 from sender AND receiver.
- No `static mut` in services. New service crates copy link.ld/build.rs/.cargo/config.toml from fjell-storaged.
- Private-endpoint service pattern: `et.alloc()` in main.rs → spawn.rs ep_obj arm → init CSpace install → `wait_service_ready`. Endpoint object ids: 0=shared, 1=storaged, 2=measuredd, 3=attestd, 4=recoveryd, 5=cap-broker, 6=sample-service.
- Task indices: 0=idle, 1=init, 2=configd, 3=cap-broker, 4=auditd, 5=svc-manager, 6=sample, 7=neg-test.
- SysError: InvalidArg=-2, PermissionDenied=-3, InvalidCap=-10, WrongType=-11, BadState=-4, LeaseRevoked=? (check fjell_abi::error).
- MMIO-ORDER: device_kick annotation required adjacent to every write_volatile.
- Repro baseline must be re-recorded after any prebuilt/*.bin change.
- provision/ must be absent from release archives (ship unprovisioned; operator provisions explicitly).
