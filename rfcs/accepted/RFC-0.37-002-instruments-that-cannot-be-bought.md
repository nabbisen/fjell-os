# RFC-0.37-002: Instruments that cannot be bought

**Status:** Accepted (0.37) — accepted 2026-10-18
**Milestone:** 0.37
**Tracks.** **E-082** — exit criterion 2's *"0 warning lines"* is partly bought by
crate-level lint suppression — and **E-084**, `fjell-netd`'s complete IPC event
loop being statically unreachable behind the very lint its crate switches off. One
line because the second is what the first was hiding: the gate reports clean, and
the thing it would have reported is a whole event loop the compiler knows is dead.
**Touches** *(indicative)*: `crates/services/fjell-netd/`,
`crates/services/fjell-secure-transportd/`,
`crates/drivers/fjell-driver-virtio-net/`, the other ten crates carrying a
crate-level `#![allow(...)]`, possibly `crates/fjell-tools/` for criterion 2's own
reporting, `rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`.
**Relates to:** **E-014**, the class this belongs to — an instrument reporting on
less than it claims; **RFC-0.35-003**, the previous instruments line, whose
exit-code contract (**2** for a usage error, **1** for a finding) is the kind of
precision this one is after; **exit criterion 2** of
`docs/src/releasing/v0-release-cycle.md`, which every cut since 0.30.0 has recorded
met.

## What is wrong, measured at `226544d`

**13 crates** carry a crate-level `#![allow(...)]`. The suppressed lints, by
frequency: `unused_assignments` (10), `dead_code` (7), `unused_imports` (4),
`unused_variables` (3), **`unreachable_code` (3)**, `unused_mut` (3).

The three suppressing `unreachable_code` carry the identical six-lint block:
`fjell-netd`, `fjell-driver-virtio-net`, `fjell-secure-transportd`. **What each one
is actually hiding, measured rather than assumed:**

- **`fjell-netd`** — a complete IPC event loop, receiving messages, handling
  `NetIpcTag::LinkUp`, pre-allocating an update-metadata session for
  `secure-transportd`, **statically unreachable** after `sys_exit(0)` (which is
  `-> !`). This is **E-084**, and it is the lint doing exactly its job with the
  switch off.
- **`fjell-secure-transportd`** and **`fjell-driver-virtio-net`** — **nothing.**
  Their only `sys_exit` is the panic handler's `exit(1)` and their loops are
  reachable. The allow suppresses no current warning in either: copied boilerplate
  that would silently absorb the next instance.

**Why this is a gate problem and not a tidying task.** *"0 warning lines"* is
**exit criterion 2**. A result that can be bought by a crate-level `allow` reports
on less than it claims, and the gate cannot tell a crate with nothing to warn about
from a crate that turned the warning off. The record says *"0 warning lines"*
either way, and has for seven releases.

## Requirements

**R1 — Re-derive the inventory at your tip**, with a control: the 13 crates, the
lint frequencies, and — for each of the three suppressing `unreachable_code` —
**whether it suppresses anything**, shown by deleting the allow and building. My
measurement says one does and two do not; confirm or refute it rather than
inheriting it.

**R2 — Every allow that suppresses nothing goes**, each removal demonstrated by
the build that stays clean without it. This is the cheap half and it is most of the
inventory.

**R3 — Every allow that survives states its reason, at the narrowest scope that
works.** An item-level `#[allow(...)]` with a comment naming why, in preference to
a crate-level blanket. `fjell-measuredd`'s existing
`#![allow(unused_assignments)] // IPC polling idiom: t/w* are overwritten by
sys_ipc_recv` is the shape to copy — it says what and why on one line — but at item
scope where it can be.

**R4 — E-084: `netd`'s code says what the service does.** Either the loop runs —
which is a networking feature and **not this line's business** — or the unreachable
code goes and what remains is the exit the stub list already describes. **Deleting
a working implementation is a real loss**, so whichever you choose, state it with
its reason in the file; if the loop is kept for a future line it belongs somewhere
a reader cannot mistake for live code. Do **not** make `netd` serve traffic here.

**R5 — Criterion 2 reports what it cannot see.** The design question E-082 raises
and does not answer: should the criterion **count** surviving crate-level allows
and name the crates? My ruling: **yes, and as a figure in the record rather than a
gate that fails** — an instrument should disclose its own blind spots, and a cut
should not be blocked by a legitimate, reasoned suppression. Implement it so the
release record can state *"0 warning lines; N crate-level allows, each with a
stated reason"*. If you find a better mechanism, **escalate before building it**.

**R6 — The failing case, shown.** For R5's reporting: a scratch crate-level allow
added somewhere, the figure moving, the allow removed. A counter first seen
reporting zero has tested nothing — the lesson from RFC-0.35-003's own subcheck.

**R7 — E-082 and E-084 CLOSED**, `v1-limitations.md` in the same commit. **E-081
and E-078 are RFC-0.37-001's** — `fjell-fleetd` is not this line's business even
though it is E-084's own class.

**R8 — The gates**, each by its own exit status, as the required set. Expect
prebuilts to move if `netd`'s source changes; rebuild then re-record the baseline
in the same commit.

### Non-goals

- Making `fjell-netd` serve network traffic.
- `fjell-fleetd`'s missing loop (RFC-0.37-001).
- Removing a suppression that is doing real work, for tidiness. R3 is "state the
  reason", not "eliminate".
- Turning criterion 2 into a gate that fails on a reasoned allow (R5).

## Risks

- **This line's own temptation is to delete code to make a lint quiet.** `netd`'s
  loop is a complete implementation; R4 asks for a decision with a reason, not for
  the shortest diff.
- **A crate-level allow sometimes is right.** `#![no_std]` services with polling
  idioms have real reasons; R3 exists because the defect is the *unstated* blanket,
  not the blanket.
- **R5 could become theatre.** A count nobody reads is worth less than the sentence
  that explains why a given allow survives. If the two conflict, prefer the stated
  reasons in the source and keep the count as a figure, not a ritual.
