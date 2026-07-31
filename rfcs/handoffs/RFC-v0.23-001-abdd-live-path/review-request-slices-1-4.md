# Review Request — RFC-v0.23-001 Slices 1–4: core path working, one regression found and not resolved

**Governing RFC:** [rfcs/proposed/RFC-v0.23-001-abdd-live-path.md](../../rfcs/proposed/RFC-v0.23-001-abdd-live-path.md)
**Handoff:** [rfcs/handoffs/RFC-v0.23-001-abdd-live-path/implementation-handoff.md](../../rfcs/handoffs/RFC-v0.23-001-abdd-live-path/implementation-handoff.md)
**Prior review:** [review-record-design-conflict.md](../../rfcs/handoffs/RFC-v0.23-001-abdd-live-path/review-record-design-conflict.md) — Subsystem A confirmed, Slice 1 unblocked
**Submitted by:** implementation model
**Status:** Slices 1–4 functionally complete and evidenced live (not just unit-tested). **One commit (Slice 1) already landed** (`d67cbfc`). Slices 2–4 are implemented and working but **not yet committed** — a real regression in an unrelated smoke test was found while gathering final evidence, and I'm stopping to report it rather than guessing at a fix, since the root cause sits in kernel code this RFC's scope explicitly excludes.

Branch: `docs/v0.21.3-rfc-and-design-baseline`

---

## 1. What works — confirmed live, not asserted

The full ABDD path runs end-to-end. From a real QEMU boot (`cargo xtask qemu-test m8`), in order:

```
M5: semantic-stream started / M5: proxy-text started      (Slice 1: both live)
M5: semantic policy loaded / M5: semantic operations ready (both READY, init unblocked)
sample-service demo intent                                  (Slice 2: emitted by sample-service,
                                                               rendered by the proxy TASK)
Actions: [1] acknowledge [2] remap-device
proxy-text: action accepted                                  (Slice 3: valid capability → accept)
proxy-text: action DENIED (capability not held)              (Slice 3: missing capability → refuse)
[STATE][Ok] Verified boot status / Immutable rootfs / System snapshot
[EVENT][Normal][Ok] Slot confirmed after health              (Slice 2(b): init's own content,
                                                               now emitted+rendered by proxy-text,
                                                               not rendered in init's own process)
```

`init` no longer depends on `fjell-proxy-text` (removed from `Cargo.toml`; `cargo tree` confirms it's gone from the dependency graph). All four required Slice 4 markers pass in a dedicated `tests/qemu/profiles/semantic.toml`, and the profile's fail-closed behavior is independently demonstrated (see §4).

## 2. Real findings along the way — reported per handoff §6, fixed where in scope

Each was caught by actually booting QEMU and reading the serial log, not by static review. All are squarely within the RFC's listed in-scope files; none touch the boundary/protocol design the architect settled.

1. **Endpoint objects never allocated.** `spawn.rs`'s `ep_obj` arms and `main.rs`'s CSpace slots referenced endpoint objects 7/8, but nothing called `et.alloc()` for them — the identical defect class already documented in this file's own comments for cap-broker (RFC 040) and sample-service (RFC 042). Fixed: two `et.alloc()` calls added alongside the existing ones.
2. **`SemanticRing`'s 470 KiB of dead BSS.** Three 32-slot rings of `SemanticEnvelope` (kilobyte-scale) were invisible before this RFC because nothing called `publish()` over IPC, so the linker eliminated them. Wiring the service loop up for real made them reachable, and `sys_task_spawn` failed with `NoMemory` at boot. Confirmed via `[bss-pad]` build output: 117 pages before the fix, back to 0 after.
3. **Writing to *any* static is impossible for a service.** Even after shrinking the rings, `publish()` still faulted (`StorePageFault`). `spawn.rs` maps a service's entire image — text, data, bss alike — `R | X | U`, deliberately with no `W` (can't be both writable and executable). This is exactly the constraint the handoff's own design decision #4 documents for `proxy-text`'s `ProxyState` ("`static mut` is forbidden in services"); it applies equally to a `static UnsafeCell`, which nothing had exercised enough to catch before. Fixed: removed the ring/publish machinery entirely (nothing else in scope reads from it); validation is kept, storage is not; the one thing Slice 3 needs (the last intent, for the `DISPATCH_ACTION` lookup) is a loop-local stack variable instead, matching the same pattern design decision #4 already uses.
4. **A real deadlock.** `proxy-text`'s `RENDER_COMMIT` handler called back into `semantic-stream` (`DISPATCH_ACTION`) *before* replying to `RENDER_COMMIT` — but `semantic-stream` was still blocked waiting for that exact reply, so it couldn't service the new incoming call. Confirmed live as a silent hang (no fault, just never printed the dispatch result). Fixed: reply first, dispatch actions as an independent follow-up round trip.
5. **Reply word silently discarded.** The 4-word raw `ipc_call` helper for `DISPATCH_ACTION` only captured the reply *tag* (a1), not its data word (a2) — `sys_ipc_reply` copies all four reply words unconditionally, tag-packing doesn't apply to replies. Every action came back "not applicable" despite the server-side lookup succeeding. Fixed: capture a2 as a `lateout`.
6. **`sys_cap_inspect` requires `INSPECT` on the capability being inspected, not generally.** The demo capability (slot 2, deliberately `SEND | REPLY`-only) couldn't be inspected by its own holder without also holding `INSPECT` on itself — every inspection failed `PermissionDenied`, defaulting `granted_rights` to 0, denying every action regardless of its actual required right. Fixed: added `INSPECT` to slot 2's own rights.
7. **The shared TOML array parser breaks on `]` inside a marker string.** `qemu_run.rs`'s hand-rolled multi-line array reader closes the array at the first line containing `]`, even when that `]` is part of the string content (e.g. a `"[INTENT][Normal] ..."` marker). Only 2 of 4 markers loaded, silently. **Not fixed** — the parser is shared by every profile and out of this RFC's scope; worked around by choosing bracket-free marker text instead, documented inline in `semantic.toml`.
8. **Pre-existing, unrelated: the M8 `measuredd`/`attestd`/`recoveryd` wait hangs.** While investigating (1), I found `fjell-init`'s M8 section (`wait_service_ready(2)`/`(3)`/`(4)`) passes *endpoint IDs* where the kernel's `sys_ipc_recv` expects *CSpace slot numbers* — confirmed this was already broken before any RFC-v0.23-001 change (same symptom in the untouched tree: the M8 IPC narration never printed, masked only because `TEST:M8:PASS` is emitted independently by the kernel based on a *different* task's exit, not on this section completing). **Not fixed** — pre-existing, unrelated to this RFC, and outside its scope. My conversion of the 9 M8-section `render_*` call sites to `emit_*` (Slice 2(b)) is complete and correct, but this pre-existing hang means that content is currently unreachable in practice, same as before my change.

## 3. The blocking finding — kernel task-index fragility, exposed by this RFC's added latency

**`cargo xtask qemu-test v0.7-sync` now fails**, reproducibly, where it passed cleanly after Slice 1 (`d67cbfc`'s own evidence: "All 4 smoke profiles... pass").

Root cause, as far as I traced it without touching kernel code:

- `crates/fjell-kernel/src/trap/dispatch.rs` detects milestone completion by **hardcoded task-table index**, not by name or `ImageId`: `exited_ok(10)` for devmgr, `exited_ok(14)` for upgraded, `exited_ok(19)` for syncd, `exited_ok(21)` for netd.
- This RFC's chunked transfer is real overhead: `SemanticEnvelope` is kilobyte-scale, chunked at 32 bytes/message, so sample-service's one intent emission plus semantic-stream's one forward is on the order of 300 blocking IPC round trips, happening early in boot (M5). The review record already flagged this cost as "acceptable for a demonstration" (§3.2) — what wasn't anticipated is that it perturbs **task-allocation timing** downstream.
- Confirmed live: a fault message that reported `[task#28 ...]` before this RFC's changes now reports `[task#20 ...]` for the *same* underlying event (svc-fault's intentional crash) — consistently, across repeated runs. Something 8 slots earlier in the schedule now finishes (frees its table slot) sooner than before, and task-table indices compress to fill it.
- Whatever task the kernel now finds at hardcoded index 19 is not behaving like syncd used to: `TEST:V0.7-SYNC:PASS` never appears even at a 300-second timeout (5x the profile's 60s budget), and the QEMU process holds ~99.7% host CPU the entire time — consistent with some task spinning, not with syncd's own `sys_exit` simply arriving late.

**I did not fix this.** The fix would need to touch `crates/fjell-kernel/src/trap/dispatch.rs`'s milestone-detection logic, which sits outside every in-scope file the handoff lists (`crates/fjell-kernel/src/task/spawn.rs`'s `ep_obj` match **only** is the one kernel change this RFC authorizes) and is explicitly excluded: *"No kernel behaviour change beyond the two `ep_obj` match arms."* This is a real, pre-existing fragility (hardcoded index instead of tracking by identity) that this RFC's added latency was enough to expose — not something introduced by a design choice I made.

## 4. What I need from you

1. **How should the v0.7-sync regression be dispositioned?** Options as I see them, none of which I should pick unilaterally:
   - Accept it as a known cost of this RFC and record it (ERRATA / known-limitation), leaving `dispatch.rs` untouched — this RFC's scope holds, but `test-all` doesn't go fully green until a follow-up fixes the milestone-detection fragility.
   - Authorize a narrow, explicit exception to touch `dispatch.rs`'s milestone-detection logic (e.g. track by `ImageId` instead of a hardcoded index) — a real fix, but outside the scope this RFC's handoff currently grants.
   - Something else I haven't considered.
2. Everything in §1–2 is otherwise ready to review and, once §3 is resolved, to commit as Slices 2–4 (a fourth commit, following the same one-commit-per-slice pattern as Slice 1).

Flagged for focused review per handoff §10, in addition to §3 above: the chunked-transfer protocol and round-trip count (finding 2, §2 above, and the general cost noted in §3); the comment/string-content bug in the shared TOML parser (finding 7); and the pre-existing M8 wait bug (finding 8), which I noticed but is unrelated to this line.
