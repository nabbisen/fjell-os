# Developer Handoff — RFC-0.40-003

**Governing RFC:** [RFC-0.40-003](../../accepted/RFC-0.40-003-the-allocators-tests-get-a-home-a-gate-can-run.md)
**Milestone:** 0.40 · **Independent of 0.40-001 and 0.40-002** — no ordering constraint
**Status:** inherited from the governing RFC (Accepted, 0.40)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Would the tests a gate runs have caught E-097?** Moving the allocator so `cargo test`
reaches it is the prerequisite. **R5 is the deliverable**: a test asserting that a reserved
range contains no free frame, demonstrated failing against a deliberately mis-reserved
allocator.

**Not the measure:** that six tests now run. Six tests that would still have missed E-097
is coverage theatre, and the line would have bought a build-system change.

## 0.1 Why this is worth a line at all

A frame-allocator reservation defect — a 4 MiB kernel stack half outside the reserved
range — lived **thirty-five milestones**, intermittently overwriting a live service's text,
and was found only because a tier went red for reasons nobody could explain. The code with
the weakest automated coverage in this tree is the code that held it, and the tests that
would speak to it have never executed.

That is the argument. Keep it in view, because it is also the argument for R5 over a
cheaper reading of the line.

## 0.2 The extraction should be clean — confirm it before trusting it

Measured by the architect at `42f444c`: `mm/frame_alloc.rs` is 309 lines with **zero**
`crate::` references, importing only `PhysFrame`, `MmError` and `fjell_abi::task::TaskId`;
`mm/address.rs` is 80 lines and `mm/error.rs` 26, neither reaching into the kernel; and ten
kernel files use the two types.

So a `no_std` crate holding those three files, re-exported from `mm`, should leave the other
ten compiling untouched. **R1 asks you to re-derive that first.** If any of the three
reaches back into the kernel, the line's shape changes and I want to hear before you move
anything.

## 1. Order

1. **R1**, the coupling check. Escalate if it fails.
2. **R2**, the move, with the kernel re-exporting what it needs.
3. **R4**, finding out what already runs the crate's tests — possibly nothing new is
   needed. Report the tier's test count **before and after**; a number that does not move
   means the crate was not picked up.
4. **R3**, stating the count that now runs (at least the existing six).
5. **R5**, the E-097 test, demonstrated failing against a mis-reserved allocator.
6. **R6** closing E-100 and removing limitations row 16, then **R7** the gates.

## 2. Traps specific to this line

1. **Stopping after the move.** R5 is the deliverable. §0.
2. **Fixing a defect the new tests find.** If they find one, **file it and escalate** — do
   not fix it in the line that moved the code, or nobody will be able to tell which change
   did what. This matters more than usual here: the whole point is that this code's
   behaviour has been under-observed.
3. **Prebuilts moving.** A pure re-home should leave every `.bin` **byte-identical**. If any
   moves, something changed beyond moving code. **Find out why before committing a
   baseline** — do not absorb it.
4. **`unsafe-audit`'s count.** It counts sites, not crates, so a move may shift it. State
   the new figure and update `README.md`'s two places if it moves. And if the allocator
   needs no `unsafe`, put `forbid(unsafe_code)` on the new crate and say so — a bitmap
   allocator that needs none is worth having on the record.
5. **Publishing the new crate.** It is internal: `publish = false`. An accidental
   `publish = true` would put kernel internals on crates.io.
6. **Scope drift into the memory manager.** These three files are self-contained; the next
   three are not.

## 3. What to put in the review request

- **R1's coupling check**, with the command — and whether my measurements held.
- **What moved and what the kernel re-exports**, and if you took a narrower cut, what it
  cost.
- **Whether the new crate needs `unsafe`**, and if not, that it forbids it.
- **R4's before/after test counts for the gate**, and whether anything had to be wired.
- **R5's test, with its failing demonstration** against a mis-reserved allocator.
- **Confirmation that every prebuilt is byte-identical**, or an explanation of why one is
  not.
- **Exact commands and exit statuses** for every gate in R7, including the `unsafe-audit`
  figure.
- **Any place my RFC is wrong.**
