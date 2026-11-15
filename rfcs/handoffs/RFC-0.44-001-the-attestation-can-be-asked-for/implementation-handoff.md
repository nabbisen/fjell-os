# Developer Handoff — RFC-0.44-001

**Governing RFC:** [RFC-0.44-001](../../done/RFC-0.44-001-the-attestation-can-be-asked-for.md)
**Milestone:** 0.44 · first line
**Status:** inherited from the governing RFC (Implemented, 0.44.0)
**Audience:** implementation model

---

## 1. Read the RFC first

Seven requirements. **R5 is what finishes the line** — the flag shown staying `false` with the
capability absent, before it is shown `true`. **R7 is the one most likely to be skipped** because
it is a doc comment, and it is a requirement.

**Read `RFC-0.39-002` before writing anything.** It did this for `measuredd`: named endpoint
constant, `CALL` capability at a named slot, a query, the flag going true, demonstrated failing
first. You are following it one service over. If you find yourself inventing a shape, you have
stopped following it.

## 2. The line in the sand

**Do not let a failed query produce a usable-looking value.** That is **E-076**, filed because a
recv error or unexpected tag yielded a 32-byte zero digest indistinguishable from a real one. A
failure leaves `last_attestation_available` **`false`** and the digest untouched.

**Do not add a transport, a remote peer, or a push.** `push_attestation` was deleted by
RFC-0.35-002 and stays deleted. This line gives the record a **local** consumer.

**Do not implement the chunked exports.** R6 deletes them. If you find yourself writing a chunk
loop, re-read the RFC's design section: the consumer wants 32 bytes.

## 3. Yours to argue, not mine to dictate

1. **The query's tag names and whether `VERIFY_LATEST` is reused or left alone.** R3 fixes the
   behaviour — a `Digest32` in four words, and *"no record yet"* distinguishable from *"here it
   is"* — not the spelling. If a new pair reads better than overloading an existing tag, say so.
   Note the ABI consequence either way (R6 removes six constants; adding two changes the count you
   predict).
2. **Which slot number `diagnosticsd` gets.** `CAP_MEASUREDD_EP` is slot 4; pick a free one and
   name it, and say what you checked to establish it is free for that image.
3. **R7's wording.** I have fixed that the warning is owed and where; the sentence is yours. The
   model to match is `measurement_events_kernel_digested`'s doc comment, which says a reader who
   treats it as "how much of the chain is trustworthy" has read more into it than it says.

## 4. Mechanics and traps

- **`capability-topology` will now have an opinion.** Adding a declared `CapHandle` constant to
  `diagnosticsd` means the instrument requires a matching install — that is the point, and it is
  also how you will know the wiring is real. Run `consistency-check --all`, not just your own
  check.
- **The `ep_obj` arm is a `match`, not an `if image_id ==` block.** 0.43-002 taught the instrument
  to read those arms; R1 replaces a literal with a named constant inside one. Confirm the subcheck
  still attributes it after the change.
- **`spawn.rs` is kernel code**, so the kernel binary moves and **the repro baseline is re-recorded
  in the same commit**, in the documented order: rebuild, delete, re-record, verify the diff
  contains only what the change explains.
- **Six constants leave the ABI and up to two arrive.** Regenerate `tests/abi/snapshot.json` in
  this line's own commit and **predict the item count in the review request** — it is 511 today.
  Gate 4 fails on any non-zero `Added`, `Removed` or `Changed sig`, in either direction.
- **`measuredd` is touched too** (R6 removes its three constants and the two-line arm). It is
  reachable and working, so check nothing else sends those tags before deleting: `EXPORT_LOG` has
  two occurrences tree-wide today, and the arm is the second.
- `/usr/bin/grep -a` when sweeping — plain `grep` here skips files containing NUL bytes.

## 5. Evidence the review request must carry

1. **R5's pair, quoted**: the capability absent → no reply → `last_attestation_available` stays
   `false`; then present → `true` with a real digest. **Taken on real boots**, as RFC-0.39-002 did.
2. **The "no record yet" case exercised** — the query answered before any record exists, with the
   flag still `false` and no zero digest presented as real (R3, R4, and E-076's rule).
3. **The ABI movement**, predicted and then shown: the count before, the count after, and
   `Added`/`Removed`/`Changed sig` all 0 after regeneration.
4. **The repro-baseline diff**, with each line explained by the kernel moving or by a prebuilt that
   moved. Nothing else may appear.
5. **R7's doc comment**, quoted, plus a sentence on what a reader of the bundle now learns and what
   they must not conclude — the record's claims are nearly all placeholders (E-095).
6. **`consistency-check --all`** at 21 subchecks, `cargo xtask build` warning count, and
   `test-all` — this line changes the kernel, so the QEMU tiers are not optional.
7. **What you checked before deleting `measuredd`'s constants**, so the removal rests on a sweep
   rather than on this handoff's say-so.

## 6. Stop and escalate if

- Making `attestd` reachable turns any tier red for a reason that is not a version or count change.
- The digest query cannot distinguish "no record yet" from a real reply without a new tag **and**
  you think the new tag is wrong.
- `diagnosticsd` turns out to have no free slot that is sensible, or the natural slot is one
  another image uses for something else.
- You conclude the consumer wants more than a digest after all. That would change the export
  ruling, and it is the one conclusion that should come back to me before code does.

## 7. Not yours

`rfcs/ERRATA.md` — **E-121**, **E-120** and **E-087**'s closures — and
`docs/src/releasing/v1-limitations.md` **rows 23 and 24** are the architect's. Row 24 currently
says the service cannot be asked for anything; when this lands it becomes false, and **flagging it
rather than editing it is correct**.
