# RFC-0.40-002: A revocation is audited under a code the decoder knows

**Status:** Accepted (0.40) — accepted 2026-10-29
**Milestone:** 0.40
**Tracks.** **E-092** — the kernel appends `AuditKindInternal::LeaseRevoked = 40` on every
lease revocation while the published decoder has `LeaseRevoked = 16` and `_ => Internal`
with no arm for 40, so every revocation record reads as `Internal` and the ABI's own
`LeaseRevoked` is unreachable. `CapDenied = 41` has no emitter and no decoder arm.
**Touches** *(indicative)*: `crates/fjell-kernel/src/audit/ring.rs`,
`crates/formats/fjell-audit-format/src/lib.rs`, `crates/services/fjell-auditd/`,
`tests/qemu/profiles/audit.toml`, `rfcs/ERRATA.md`.
**Relates to:** **RFC-0.38-004**, which found it while establishing that a distinct audit
kind for a bind would be an ABI addition; **E-091**, which carries the unbuilt
`AUDIT_LEASE_BIND_REJECTED` code and is **not** this line.

## What is wrong

The audit trail misreports the one event class the lease mechanism exists to record.

- Kernel: `AuditKindInternal::LeaseRevoked = 40` (`audit/ring.rs:45`), **appended on every
  lease revocation** (`lease/mod.rs:343`).
- User space: `AuditKind::LeaseRevoked = 16` and `Internal = 255`
  (`fjell-audit-format/src/lib.rs:70`, `:77`), and `from_u16` ends `_ => Self::Internal`
  with **no arm for 40**.
- `fjell-auditd` decodes at exactly one site, `main.rs:51`:
  `let kind = AuditKind::from_u16(r.kind);`

So two things are true at once: **every revocation decodes as `Internal`**, and the
published `LeaseRevoked` variant is **unreachable**, because nothing anywhere emits 16.

### The scope is bounded, and the architect measured it

E-092 asks for the two enums to be compared wholesale rather than only for these values.
Done, at `42f444c`, by parsing both and diffing them:

| | |
|---|---|
| Variants | **22 each** |
| **Mismatched value for the same name** | **exactly one — `LeaseRevoked`, kernel 40, userspace 16** |
| Kernel-only name | **`CapDenied = 41`** — no userspace variant at all |
| Userspace-only name | `Internal = 255` — the fallback, correctly never emitted by name |
| Values the kernel emits that decode to nothing | **40 and 41** |

So this is one disagreement and one orphan, not an open-ended audit. **Re-derive it** (R1)
rather than taking my table, but the line's size is known.

## Requirements

**R1 — Re-derive the comparison, wholesale, with a control.** Both enums, every variant,
by a method that cannot miss one — a diff of the two value sets, not a reading. Confirm the
single mismatch, the orphan, and that nothing else disagrees. Control: the method must
find `LeaseRevoked` without being told to look for it.

**R2 — Pick a direction for `LeaseRevoked`, and say why.** Two exist:

- **(a) The kernel emits the published value, 16.** The architect's recommendation.
  `AuditKind` is the published ABI and the kernel is the implementation, so the
  implementation moves. It also leaves exactly one value meaning the event, where (b)
  leaves two.
- **(b) The decoder gains a `40 =>` arm.** Preserves the current wire value, but enshrines
  a value the published enum does not name and leaves 16 forever unemitted.

**I checked what constrains the choice and found nothing**: no committed evidence file or
expected-markers artefact contains the wire value, and `auditd:51` is the only decode site.
So (a) breaks nothing I can find. **If you find something that depends on 40, say so and
take (b)** — that is exactly the kind of thing this project has found in review four times
running.

**R3 — Dispose of `CapDenied = 41`.** It is declared in the kernel, **emitted by nothing**,
and absent from the userspace enum: dead on all three counts.

**Delete it.** A kind nothing emits and nothing decodes is not a kind, and leaving it
invites the next reader to assume capability denials are audited. **Adding an emitter is
out of scope** — that is new behaviour and a different line. If you conclude a
capability-denial audit event should exist, **say so in the review request** and leave the
variant deleted; I would rather rule on a proposal than inherit a placeholder.

*(Note the neighbour that is not this line: `AUDIT_LEASE_BIND_REJECTED = 0x0604`, specified
by `RFC-v0.7.4-003` and never built, belongs to **E-091**.)*

**R4 — A tier asserts the decoded name, demonstrated failing.** The `audit` tier must
assert that a lease revocation appears **as a revocation** in `auditd`'s decoded output,
not as `Internal`. Shown red against the pre-change tree — where it reads `Internal` — and
green after. Both transcripts in the review request.

Assert the **decoded name**, not the raw `kind` word. The raw value is what changes; the
property is what a consumer reads.

**R5 — No ABI addition, and confirm it.** Neither direction adds a variant: (a) changes a
value the kernel emits, (b) adds a match arm. `abi-snapshot --verify` should report
`Added 0, Removed 0, Changed 0`. **If it does not, stop** — a changed signature here means
the enum's shape moved, which is not what either direction does.

**R6 — E-092 closes in this line's commit.**

**R7 — Gates by exit status.** `release-rehearsal`, `consistency-check --all`,
`readiness-check`, `abi-snapshot --verify`, `fjell-repro-check`, `test-all` to completion,
`callsite-audit`, `cargo fmt --check`. A kernel change moves the prebuilts: rebuild
**before** re-recording the repro baseline and commit the binaries with it.

### Non-goals

- **E-091's `AUDIT_LEASE_BIND_REJECTED`**, and auditing refused operations generally. A
  refusal that leaves no trace was raised at the 0.38-004 review and deliberately left;
  it needs a decision about attacker-controlled log volume, not a code.
- **The `CapMint`-for-a-bind mislabel.** RFC-0.38-004 established that a distinct bind kind
  **is** an ABI addition and correctly left it. Unchanged here.
- **Any other audit-record field.** This line is about the kind word.

## Risks

- **Fixing the decoder because it is the smaller edit.** (b) is one line and leaves the
  published enum's own value permanently unemitted. If you take it, the reason must be
  something that depends on 40, not its size.
- **Deleting `CapDenied` and quietly adding an emitter.** Two changes wearing one name. R3
  forbids the second half.
- **A tier that asserts the raw `kind`.** It would pass under (a) and fail under (b) while
  telling a consumer nothing. R4 asserts the decoded name.
