# Developer Handoff — RFC-v0.23-002

**Governing RFC:** [RFC-v0.23-002](../../proposed/RFC-v0.23-002-milestone-markers-by-identity.md)
**Milestone:** v0.23
**Status:** inherited from the governing RFC (Proposed — accepted for implementation 2026-07-31)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The rule that governs this line

> **Each marker needs two demonstrations, not one:**
> 1. it **fails** when its named service fails or does not exit cleanly, and
> 2. it does **not fire** when a different task occupies the index it used to use.

The second is the one that matters. Without it we would replace an index-keyed
unverified check with an identity-keyed unverified check and call that progress.

A slice without both demonstrations is incomplete regardless of what else it does.

## 0.1 What you are fixing

`TEST:V0.7-SYNC:PASS` does not mean syncd succeeded. It means *whatever task
occupies table index 19 exited cleanly.* Four of the eighteen `test-all` tiers
rest on a code comment staying true.

And it has already lied: `m8` has been green while init's M8 section never ran,
because `TEST:M8:PASS` attests upgraded's exit rather than the M8 path.

---

## 1. Change scope

**In scope:** `crates/fjell-kernel/src/trap/dispatch.rs` (marker emission
**only**), `crates/services/fjell-init/src/main.rs` (the M8 waits), and whatever
test files the demonstrations need.

**Explicitly NOT in scope:**

- No new syscalls. Gate 12 `syscall-surface` must stay **35/26/9** — if those
  numbers move, you have left scope.
- No change to capability, lease, IPC, MM, or crypto semantics. The kernel edit
  is confined to the marker-emission block.
- No redesign of the smoke harness or the marker vocabulary.
- **Do not fix whatever this exposes.** See §5.
- Not the other v0.23 candidates (TOML parser fail-open, doc link rot, the
  statics-are-unwritable SDK gap, the nine undispatched syscalls).

---

## 2. Slice 1 — Key the emissions on identity

Six index-keyed call sites in `dispatch.rs`, all in one block:

```
367: if exited_ok(10)        -> devmgr     (ImageId::DEVMGR    = 8)
370: if exited_ok(19)        -> syncd      (ImageId::SYNCD     = 0x1D)
373: if exited_ok(21)        -> netd       (ImageId::NETD      = 0x18)
378: if exited_ok(14)        -> upgraded   (ImageId::UPGRADED  = 12)
380: } else if exited_ok(1)  -> init       (ImageId::INIT      = 0)
383: } else if done(1)       -> init
```

**Note the indices are not the ImageIds** — index 10 is devmgr whose ImageId is
8, index 14 is upgraded whose ImageId is 12. Do not assume they correspond
anywhere; that coincidence-shaped thinking is what this RFC exists to remove.

`Task` already carries `pub image_id: fjell_abi::service::ImageId`, set by
`spawn.rs` and kernel-attested (RFC 055). Replace the index lookups with a scan
of the task table for a task whose `image_id` matches.

**Settled design decisions — do not re-open:**

1. **Scan the table; do not add an index/map.** The table is small and this runs
   on task exit. A lookup structure is more state to keep correct for no gain.
2. **Semantics: "some task with this `ImageId` is in `Exited(0)`."** If you find
   more than one live task carrying the same `ImageId` (a respawn, for example),
   that is a **finding to report**, not something to disambiguate silently.
3. Keep emission in the kernel. It is there so markers cannot be garbled by
   concurrent user-space UART writes, and that reason still holds.

**Required demonstrations, per marker** — both, per §0.

## 3. Slice 2 — Make the M8 path actually run

`init`'s M8 section calls `wait_service_ready(2)` / `(3)` / `(4)`, passing
**endpoint IDs** where `sys_ipc_recv` expects **CSpace slot numbers**. The
section therefore never runs.

Derive the correct slot numbers from where those caps are installed into init's
CSpace (`crates/fjell-kernel/src/main.rs`, the init-CSpace section) — **do not
guess them**, and do not infer them from the endpoint numbering, which is the
exact confusion being fixed.

**Demonstration required:** init's M8 narration appears in the serial log.
`TEST:M8:PASS` appearing is *not* evidence — that marker is precisely what
concealed this.

## 4. Slice 3 — Run everything and report honestly

`cargo xtask test-all` in full. Record every profile's status.

**Expect red.** R1 and R2 in the RFC are rated high and medium-high respectively,
and both are the intended outcome:

- Fixing the M8 waits should expose further breakage in a path that has never
  once executed.
- Other profiles may go red once their markers attest the truth rather than a
  coincidence.

**A profile that is honestly red is worth more than one that is dishonestly
green.** `0.23.0` may ship with recorded red profiles and an explicit
disposition. That is a better release than the alternative, and it is not a
failure of this line.

## 5. If this exposes further breakage

**Report it. Do not fix it.**

That is not a general caution here — it is the specific failure mode this slice
invites, because each exposed defect will look small and adjacent. Fixing them
in-slice would make the marker change unreviewable and turn a bounded line
unbounded, which is exactly what happened to nearly every line before this one
until it was stopped.

Escalate immediately if anything exposed looks unsafe to leave.

## 6. Prohibited shortcuts

- Do not accept a marker without **both** demonstrations.
- Do not make a red profile green by any means other than fixing the marker.
- Do not touch the syscall surface.
- Do not treat `TEST:M8:PASS` as evidence the M8 path ran.
- Do not mark unexecuted commands as passed.

## 7. Required evidence

1. `cargo xtask release-rehearsal` — full twelve-gate table
2. Gate 12 `syscall-surface` still 35/26/9
3. `cargo xtask test-all` — every tier, with red ones named and explained
4. **Each demonstration, run and shown failing** — a summary saying "test added"
   is not evidence; that is the whole point of this line
5. Serial-log excerpt showing init's M8 narration

## 8. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review: the table-scan semantics when an `ImageId` appears more
than once, the derivation of init's CSpace slot numbers, and every profile that
changed colour in either direction.
