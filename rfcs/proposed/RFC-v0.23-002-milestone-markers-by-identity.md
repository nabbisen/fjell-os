# RFC-v0.23-002: Milestone Markers by Identity

**Status:** Proposed
**Milestone:** v0.23
**Tracks.** Gate integrity. The smoke-test suite's PASS markers do not attest
what they name.
**Touches.** `crates/fjell-kernel/src/trap/dispatch.rs`,
`crates/services/fjell-init/src/main.rs`.
**Relates to:** RFC-v0.22-001 (same defect class, and its governing principle
applies here), RFC-v0.23-001 (whose added latency exposed this), RFC 055
(kernel-attested `image_id`).

## Summary

`TEST:V0.7-SYNC:PASS` does not mean syncd succeeded. It means *whatever task
occupies table index 19 exited cleanly*.

All four QEMU smoke profiles rest on markers emitted this way. None verifies the
service it names. This RFC keys them on the kernel-attested `image_id` each task
already carries, so that a marker attests the thing it is named after.

## Motivation

### The mechanism

`crates/fjell-kernel/src/trap/dispatch.rs` emits the milestone markers from the
kernel, keyed on **hardcoded task-table indices**, with the spawn order recorded
in a *comment*:

```rust
// Spawn order: devmgr(10) upgraded(14) syncd(19) netd(21)
if exited_ok(19) { kprintln!("TEST:V0.7-SYNC:PASS"); }
```

`exited_ok(idx)` looks up `TaskId::new(idx, 0)` and checks for `Exited(0)`. The
service's identity is nowhere in the check.

### The blast radius

Every smoke profile depends on one of these five emissions
(`crates/fjell-tools/src/smoke.rs`):

| Profile | Marker | Emitted when index… |
|---|---|---|
| `v0.5-platform` | `TEST:V0.5-PLATFORM:PASS` | 10 exits cleanly |
| `v0.7-sync` | `TEST:V0.7-SYNC:PASS` | 19 exits cleanly |
| `v0.4-net` | `TEST:V0.4-NET:PASS` | 21 exits cleanly |
| `m8` | `TEST:M8:PASS` | 14 exits cleanly |
| — | `TEST:M7:PASS` | 1 exits cleanly |

That is four of the eighteen `test-all` tiers resting on a comment staying true.

### This has already caused harm, not merely risked it

`fjell-init`'s M8 section passes **endpoint IDs where `sys_ipc_recv` expects
CSpace slot numbers**, so that section never runs. The `m8` profile stayed green
throughout — because `TEST:M8:PASS` attests *upgraded (index 14)* exiting, not
the M8 path completing.

So the flagship milestone profile has been passing while the thing it is named
after never executed. That is not a hypothetical failure mode; it is the current
state, found by accident.

### How it surfaced

RFC-v0.23-001's chunked transfer adds ~314 blocking round trips early in boot.
That perturbed task-allocation timing enough to shift what sits at index 19 — a
fault that reported `[task#28 …]` before now reports `[task#20 …]` for the same
event. `v0.7-sync` went red.

**This RFC did not break v0.7-sync.** It revealed that the marker had always
been resting on a coincidence, and the coincidence stopped holding.

### The fix has a key already available

`Task` already carries `pub image_id: fjell_abi::service::ImageId` — set by
`spawn.rs`, kernel-attested, and already trusted for IPC sender identity
(RFC 055). Every `ImageId` the five markers need exists: `INIT` = 0,
`DEVMGR` = 8, `UPGRADED` = 12, `NETD` = 0x18, `SYNCD` = 0x1D.

So this is a small change with a trustworthy key, not a redesign.

## Goals

1. Each marker is emitted on the exit of the **named service**, identified by
   `image_id`, not by table position.
2. A marker attests the thing it names — `TEST:M8:PASS` should mean the M8 path
   completed.
3. `init`'s M8 waits use CSpace slots, so that path actually runs.
4. Every marker touched is demonstrated failing when its named service fails,
   **and** demonstrated not firing when a different task occupies that slot.

## Non-goals

- No redesign of the smoke harness or the marker vocabulary.
- No new syscalls; no change to capability, lease, IPC, MM, or crypto semantics.
  The kernel change is confined to marker emission.
- Not the other v0.23 candidates (TOML parser fail-open, doc link rot, the
  statics-are-unwritable SDK gap, the nine undispatched syscalls).
- Not making every currently-red profile green by other means. If identity-keyed
  markers reveal further breakage, that is a finding, not this RFC's to fix.

## The governing principle carries over

From RFC-v0.22-001, and it is the reason that line found real defects:

> **Every gate added or strengthened must be demonstrated failing on a
> deliberately broken input before it is accepted.**

Applied here, each marker needs **two** demonstrations, because there are two
distinct ways to be wrong:

1. It **fails** when the named service fails or does not exit cleanly.
2. It does **not fire** when a different task occupies the index it used to use.

The second is the one that matters. Without it we would replace an
index-keyed unverified check with an identity-keyed unverified check.

## Scope

| Slice | Content |
|---|---|
| 1 | Key the five emissions on `image_id`, with both failure demonstrations per marker |
| 2 | Fix `init`'s M8 waits (slot vs endpoint ID), so the M8 path runs; demonstrate that it now does |
| 3 | Run the full smoke suite and `test-all`; report anything that goes red because a marker now attests the truth |

Slice 3 is expected to surface findings. That is the point.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Fixing the M8 waits reveals further breakage in a path that has never once run | **High** | Medium | Expected. Report; do not fix in-slice. Each becomes its own item. |
| R2 | Other profiles were also passing for the wrong reason and go red | **Medium–high** | Medium | Also the point. A red profile that is honestly red is worth more than a green one that is not. |
| R3 | The fix is itself unverified | Low | High | The two-demonstration rule above. |
| R4 | Scope creep into fixing whatever R1/R2 expose | Medium | Medium | Explicit non-goal. `0.23.0` may ship with honestly-red profiles and a recorded disposition, which is better than dishonestly-green ones. |

## Acceptance criteria

- [ ] No marker emission keys on a task-table index.
- [ ] Each of the five markers has **both** demonstrations committed: fails when
      its named service fails; does not fire when another task occupies the old
      index.
- [ ] `init`'s M8 section executes — demonstrated by its narration appearing,
      not by `TEST:M8:PASS` alone.
- [ ] `cargo xtask test-all` run in full, with every profile's status recorded
      and any red one dispositioned explicitly.
- [ ] Gate 12 `syscall-surface` still 35/26/9 — no syscall surface change.
- [ ] Twelve mechanical gates pass, or a failure is recorded with its reason.

## Alternatives considered

| Option | Assessment |
|---|---|
| **Key on `image_id`** *(chosen)* | The key exists, is kernel-attested, and is already trusted for IPC identity. Smallest change with the strongest guarantee. |
| Keep indices, add a comment-vs-reality check | Automates verification of a fragile scheme instead of removing the fragility. |
| Move marker emission to user space | The kernel emits these precisely so they cannot be garbled by concurrent UART writes. That reason still holds. |
| Accept and document the fragility | Rejected. Four of eighteen tiers would keep attesting the wrong thing, and one is already known to be lying. |

## A note on how this was found

Three separate measurement errors preceded this RFC, all mine, all in the same
direction — a tool reported less than reality and I believed it. Grep silently
skipped a file containing NUL bytes; two semantic subsystems were conflated
because both lived in one crate; and a decimal-only regex reported two `ImageId`
constants missing that are simply written in hex.

The defect this RFC fixes is the same shape, in the project's instruments rather
than in mine. That correspondence is worth recording: the reason to prefer
identity over position is not tidiness, it is that positional checks fail
silently and silence reads exactly like success.
