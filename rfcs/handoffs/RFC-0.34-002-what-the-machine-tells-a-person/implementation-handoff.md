# Developer Handoff — RFC-0.34-002

**Governing RFC:** [RFC-0.34-002](../../proposed/RFC-0.34-002-what-the-machine-tells-a-person.md)
**Milestone:** 0.34
**Status:** inherited from the governing RFC (Proposed — **do not start until the
owner accepts it**; this handoff is written in advance so that acceptance is the
only thing between you and R1)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

Not "the junk bytes are gone". **The measure is that the console — the only thing
this system says to a person — cannot show a reader something no task wrote, cut a
line without saying so, or name an event without naming who reached it.** A fix that
removes the eight bytes without owning the slot's lifetime leaves the mechanism in
place for the next task that dies mid-line.

## 0.1 Two figures in the RFC's own register entries were wrong, and are corrected there

Read the dated corrections in E-061 and E-063 before R1:

- **E-061 said four `NoMemory` sites; there are thirteen**, and at least three
  distinct conditions behind them. The number came from counting the first four
  occurrences.
- **E-063 said tiers assert `M6: storaged ready` and pass on `init`'s line.** No
  committed marker specification asserts it at all — 45 files searched, with the
  control `driver-uart: ready` found in four. **The defect is the ambiguity, not a
  passing tier.** Do not write a closure that claims a tier was fixed.

Both corrections are mine. R1 re-derives them anyway; if a third figure is wrong,
say so the same way.

## 0.2 Settled — do not re-open

1. An error a person reads **names what failed**; two conditions a caller must tell
   apart may not share a value (D1).
2. A console line buffer **belongs to its task's lifetime** (D2).
3. A split line **says it was split** (D3).
4. An index out of range **fails** — the `% DBG_TASKS` aliasing goes (D4).
5. **One writer per marker** (D5).
6. Four demonstrations, each failing first (D6).
7. The per-byte `sys_debug_write` syscall is **not redesigned here** (D7).

## 1. Order

**R1 (re-derive, with §A's measurement) → §A–§E in writing → R3 (the error value
and `init`'s message) → R4 (the buffer: lifetime, split, index) → R5 (one writer) →
R6 (demonstrations) → R7 (the tier) → R8 (errata) → evidence.**

R4 after R3 because the error value is an ABI change and wants to be alone in its
commit; R7 last because the tier asserts the end state.

## 2. §A is a measurement, not a preference

Before choosing flush-or-clear, **measure what the buffers actually hold when tasks
leave**, across the tiers. Two reasons: a dying task's last words are what a person
debugging a fault wants, so losing them is a real cost; and if the buffers turn out
to be empty at every exit in every tier, then the eight bytes of E-062 came from
somewhere else and **the mechanism is not proven** (§E). Say which it is.

**Do not close E-062 on a fix whose mechanism you have not tied to those bytes.**
Either name the task whose buffer held them, or state plainly in the closure that
the lifetime defect is proven and the specific bytes are not attributed.

## 3. R3 — the error value is an ABI change, and Gate 4 now sees it

A new `SysError` variant moves `syscall-surface` and the snapshot. Since
RFC-0.33-004 D5 the snapshot's enum hash covers **variants**, so this is the first
ABI addition Gate 4 catches by body rather than by declaration line: run
`abi-snapshot --verify` **before** regenerating and paste what it reported, as that
line did. Name the variant for the condition (*the task table is full*), not for the
table, and give `init` the image id it could not spawn — `init: spawn error` is the
line a person actually gets today.

## 4. R4 — the three defects in one path, and the argument that must be re-made

`DBG_TASKS = MAX_TASKS`, `DBG_LINE = 160`, flush on newline or when full. The
existing safety argument is *single hart, SIE masked for the byte loop*. **Flushing
from an exit path is a new caller of that argument**: re-make it in the fix's own
comment, or the next reader will assume it was checked. §B's bound belongs in
`v1-limitations.md`, not only in a comment: a person reading a braille presentation
through the console needs to know the line can be cut.

## 5. R7 — a tier, not a compile

At least one profile must assert the console is clean where it carried the prefix.
A test that the buffer is cleared is necessary and not sufficient: this line's claim
is about what a reader sees, so a tier reads it.

## 6. Prohibited

- Redesigning `sys_debug_write`, adding levels, timestamps or a logging framework.
- Changing what any service prints beyond D5's duplicate.
- Sizing the buffer from `MAX_WIRE_BYTES` without §B's cost stated (185 KB of
  kernel `.bss` at 4,624 × 40).
- Closing E-062 with the bytes unexplained **and** unacknowledged.
- Writing a closure that says a tier was fixed (§0.1).
- Renaming `init`'s duplicate marker instead of removing it.
- A gate piped into `grep` instead of its own exit status.

## 7. Required evidence

1. R1's three re-derivations with their controls, including the marker-specification
   absence **with its positive control**.
2. §A–§E in writing, §A carrying the measurement and §E the attribution or its
   absence.
3. R3: the variant, `init`'s message, `--verify` before and after, the
   syscall-surface figures.
4. R4: the lifetime fix with its re-made safety argument, the marked split with its
   bound, the aliasing gone.
5. R5: one writer, and the check that nothing depended on two.
6. R6's four demonstrations, each reverted, with transcripts.
7. R7's tier, with the assertion quoted.
8. **E-061, E-062, E-063** resolved or survivors named; register and
   `v1-limitations.md` in the same commit; the dated corrections left in place.
9. The gates, each by **its own exit status**, `test-all`, the repro baseline
   re-recorded in the same commit as any rebuild, and a CI run id.

## 8. Review request

Standard format, in `.git-exclude/review-request/`. Flag for focused review:

- **§A's measurement and §E's attribution**, first.
- The new error value's ABI figures, before and after.
- **The tier that reads the console**, and what it asserts.
- Anything you found in these three entries that is still wrong.
- Anything outside the *Touches* list, which is indicative.
