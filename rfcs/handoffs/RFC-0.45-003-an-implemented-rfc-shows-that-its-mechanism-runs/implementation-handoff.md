# Developer Handoff — RFC-0.45-003

**Governing RFC:** [RFC-0.45-003](../../accepted/RFC-0.45-003-an-implemented-rfc-shows-that-its-mechanism-runs.md)
**Milestone:** 0.45 · second line — after 0.45-002
**Status:** inherited from the governing RFC (Accepted, 0.45)
**Audience:** implementation model

---

## 1. Read the RFC and its Rulings section first

The *Rulings* settle the grammar (four kinds, with the count broken down **by kind**), R6 **in**,
and `KNOWN_FALSE` **separate** from `WAIVERS`.

**Read 0.44 small item D before writing R6.** It removed a rule that was implemented twice. R6 must
call the subcheck's own predicate, exactly as `rfc promote` now calls
`readme_counts::disagreements`. **One rule, one implementation** — a second parser here would be
this milestone's own subject committed inside its own fix.

## 2. The line in the sand

**R4's wording is a requirement, not polish.** A gate whose PASS line can be read as a statement
about all 217 pre-0.45 `Implemented` RFCs would be the **E-014** class — the defect this line
exists to close. The output names what it does not cover, every run, in words. **I will read that
string as carefully as the code.**

**At landing, the required count is 0**, because no RFC from 0.45 onward is `Implemented` yet. The
line must say that rather than imply coverage.

## 3. Yours to argue, not mine to dictate

* Where the evidence field lives in the header block, and its exact spelling.
* Whether `file:` resolves a path only, or a path and an anchor.
* How `marker:` handles a profile with no committed `expected-markers.txt` — the RFC says that
  cannot resolve, which I accept; the failure message is yours to word.

## 4. Mechanics and traps

* **Order:** land this **after** 0.45-002. RFC 056's status becomes true then, which removes the
  most awkward of the three known-false entries.
* **`RFC-v0.7.3-001` and `RFC-v0.7.3-002` go in `KNOWN_FALSE`, not `WAIVERS`**, with their reasons.
  **Do not touch their status lines** — their disposition is the owner's pending lifecycle decision.
* **The live control is RFC 056's own case:** a `syscall:CapInstall` citation must be **red** while
  the syscall is undispatched. Show it red before 0.45-002 lands, or construct it in a scratch tree.
* `rfc-status-folder` accepts `Implemented-with-Errata` and `Closed`; **RFC 000 defines neither.**
  That is a finding, not your problem — do not resolve it, and do not depend on it.

## Evidence the review request must carry

* The mutation table: **every required refusal shown red**, each by its own mutation, each restored
  byte-exact afterwards.
* A full `cargo xtask test-all` on the tip with this line in it, with the tier counts.
* If a prebuilt moved: `tests/repro/baseline-digests.txt` re-recorded **in the same commit**, and
  the `numstat` compared against the binaries the build reported as modified — **exactly those and
  no others**.
* `consistency-check --all` and `fmt --check`.
* **Anything you measured that contradicts this handoff or the RFC.** Three of the last four
  milestones had one, and every time the implementer was right to raise it.

## Stop and escalate if

* a requirement cannot be met without widening authority beyond what the RFC's Rulings settle;
* a gate you add would pass for a reason other than the one it claims to test;
* you find a register row or limitations row that is **false** rather than merely due an update.

## Not yours

`rfcs/ERRATA.md`, every `docs/src/releasing/v1-limitations.md` row, and every RFC status line.
Flag them in the review request rather than editing them.
