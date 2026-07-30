# Review Record — RFC-v0.21.3-001, Slice 3 completion

**Reviewer:** architect
**Reviewing:** `review-request-slice3-completion.md`
**Date:** 2026-07-30

## Outcome

**Approved,** with two small items before the RFC can be dispositioned (§5).

Both corrections required by `review-record-slice-3.md` §4 and §7 were found
already made, though uncommitted in the working tree. Both are accurate:

- `docs/release/v1-limitations.md` item 7 — E-011 recorded with the correct
  distinction (fails closed via `UnknownSyscall`, so not a live hole; the
  *documented* behaviour is simply not shipped). Cites both governing records.
- `CHANGELOG.md` `[0.21.2]` — `KNOWN-BAD` with the defect named, the superseding
  version named, and the tag explicitly kept rather than moved.

Committed on the implementer's behalf so they are not lost.

## 1. Rulings on the three questions

### Q1 — handoff bundle version stamps: **your reading is correct; mine was loose**

You re-stamped `handoff-0.21.2/` to **v0.21.2**, not v0.21.3, on the grounds
that the bundle's content describes the v0.21.2 release state and relabelling a
frozen snapshot would make it claim to describe a release it does not.

That is right, and the RFC's acceptance criterion — *"regenerated or explicitly
re-stamped against v0.21.3"* — was sloppily worded on my part. It was written
to mean "the stamp must stop lying," and admits a reading you correctly
rejected: relabelling historical content to a version it never described would
have created a *new* false claim while closing an old one, in an RFC whose whole
purpose is removing false claims.

The RFC's acceptance criterion is amended to say what it meant. No correction
pass needed; what shipped is right.

### Q2 — RFC completion: **not yet, two small items** (§5)

### Q3 — review-request placement: **the owner's instruction governs; I withdraw mine**

`review-record-slice-2b-2c.md` §6 told you to put review requests in the handoff
directory. The owner has directed `.git-exclude/review-request/`. The owner's
instruction wins — that is not a close call, and you were right to keep flagging
the conflict rather than silently picking one.

**My §6 instruction is withdrawn.** Keep using `.git-exclude/review-request/`.

The concern behind it was real but is mine to solve, not yours: a review chain
that lives in a gitignored directory is not a durable record. I will copy each
request into `rfcs/handoffs/…` as part of reviewing it, as I did for
`review-request-slice-2b-2c.md`. You do not need to do anything about it, and it
should stop recurring in your submissions.

## 2. Verified independently

The full §6 sweep was re-checked where cheap, and the substantive claims hold.
The notable one:

> `cargo xtask test-all` — Total: 18 | PASS: 18 | FAIL: 0 | SKIP: 0

This is the first time every tier — including 4 QEMU smoke profiles and all 9
QEMU negative profiles — has actually executed since the build broke. Given that
the negative harness is fail-closed (an absent expected marker or a present
forbidden marker fails the run), an 18/18 with zero skips is materially stronger
evidence than any prior claim in this release line, all of which were made when
the gates could not run.

Gate 10 reported as FAIL rather than absorbed as `CONFORMANCE-ONLY` — correct
for the fourth consecutive submission.

## 3. Judgement calls accepted

- **Keeping the `handoff-0.21.2/` directory name and fixing the 7 links**
  instead of renaming. Consistent with the sibling frozen handoffs
  (`handoff-v0.9-v0.15.md`, etc.) and keeps the CHANGELOG truthful. Correct.
- **Not inventing v0.19/v0.20/v0.21 sections in `rfcs/README.md`** for RFCs that
  do not exist, and pointing at the RFC's open question instead. Correct — the
  index must describe the folder, not aspiration.
- **Re-deriving the unsafe-site and RFC counts** rather than copying the figures
  established earlier in this same review chain. That is the right instinct even
  when the source is a prior architect finding.
- **Not adding the mechanical syscall-count check.** The RFC phrased it as
  "prefer … if it is cheap to add," and a robust version parsing both the ABI
  enum and the dispatcher arms is not cheap. Declining unauthorised new gate
  surface is correct. Carried to v0.22 as a candidate.

## 4. New finding — a cross-reference that breaks on the next commit

The `KNOWN-BAD` note in `CHANGELOG.md:107` links to:

```
rfcs/proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md
```

That path stops existing the moment the RFC moves to `rfcs/done/` — which is the
very next thing that happens to it (Q2). This is RFC 000's own named
anti-pattern, *"Letting cross-references rot,"* and its prescribed fix is to
sweep inbound references in the same commit as the move.

Not a defect in what you wrote — the file was in `proposed/` when you wrote it.
It becomes wrong on move, so it is folded into the move (§5).

## 5. Required before RFC disposition

Two items, both small:

1. **The 3 remaining "Gate 9 is the only blocker" claims.** You left these
   deliberately and flagged it, which was the right call — the handoff table
   named only `ROADMAP.md` and `docs/src/roadmap/roadmap.md`. But they are
   knowingly-false claims, and RFC-v0.21.3-002's exit criterion 8 is "no doc
   asserts behaviour the tree does not have." **Authorized** — correct all three:

   ```
   docs/src/releases/handoff-0.21.2/README.md:44
   docs/src/releases/handoff-0.21.2/testing-and-gates.md:94
   docs/src/releases/handoff-0.21.2/project-summary.md:58
   ```

   These are v0.21.2-era statements in a frozen bundle, so per Q1's logic do not
   rewrite them into v0.21.3 claims. Mark them as superseded — the accurate
   statement is that at v0.21.2 the mechanical gates could not run at all, so
   Gate 9 was not the only blocker.

2. **Move the RFC to `rfcs/done/`** with `Status: Implemented (v0.21.3)`, update
   `rfcs/README.md`, and sweep inbound references — including the `CHANGELOG.md`
   link in §4 above. Per lifecycle policy this happens in the commit series that
   ships the implementation, so it belongs with the v0.21.3 release cut rather
   than before it.

## 6. What follows

After those two, RFC-v0.21.3-001 is complete and v0.21.3 is ready to cut as the
first release under RFC-v0.21.3-002. That requires a release record at
`docs/release/records/0.21.3.md` with the exit-criteria table and real output —
most of which this submission's §6 already provides.

The open item there is Gate 10: it fails wherever Verus is absent, and the exit
criteria make a gate that cannot run a failure, not an abstention. That is an
owner decision — produce the record where Verus is installed, or tag with a
written accepted-risk statement. Not the implementer's call and not mine.
