# Developer Handoff — RFC-0.35-001

**Governing RFC:** [RFC-0.35-001](../../accepted/RFC-0.35-001-the-capabilities-the-update-path-never-had.md)
**Milestone:** 0.35
**Status:** inherited from the governing RFC (Accepted, 0.35)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does a gate fail when a service names a capability slot the kernel does not
install for it?** Today nothing compares the two declarations, which is why eleven
slots hold nothing and two services have spun in every boot this project has ever
taken. When that comparison exists, fails on the state we are in now, and passes
on the state we leave — and when the update path's own tier is release-gated — this
line is done.

**Not the measure:** that eleven slots got filled. Filling them is the easy half
and the wrong half to be judged on.

## 0.1 Two pieces of evidence can only be taken before you fix anything

Take both **first**, in a scratch build, and keep the transcripts:

1. **The spin.** Instrument `secure-transportd`'s loop (a counter, a debug line
   every N iterations — the console is one byte per `ecall`, so keep it sparse) and
   show it re-entering without blocking. If it turns out to **block**, say so: the
   claim in E-072 is mine, it is the part I am least able to verify from outside a
   running system, and correcting it is worth more to me than confirming it.
2. **The gate failing.** Write R4's subcheck before R3's fix and run it: it must
   name all eleven slots. A gate first seen passing is a gate nobody has tested.

## 1. Order

1. **R1 re-derivation**, with controls, including my two.
2. **R2 transcripts** (above).
3. **R4 gate, failing**, on the eleven.
4. **R3 one declaration** of the topology.
5. **R5 installs** — the update path only, each justified in writing.
6. **R6 the word and the id space.**
7. **R7 the `upgrade` tier** emitting and gated; `store`'s reason narrowed.
8. **R8 register and limitations**, in one commit.
9. **R9 gates, `test-all`, CI.**

Steps 3 and 4 are deliberately before the fix. Everything after 4 is ordinary.

## 2. Traps specific to this line

1. **Do not install a slot because it is declared.** Five of the eleven belong to
   protocols nobody serves. R5 is "what the update path needs", and the answer is
   two services, not five.
2. **Do not delete the spin before you have photographed it** (§0.1).
3. **`SERVICE_READY_SEND_SLOT` is the model, not a thing to copy blindly.** It
   works because it is one fixed slot installed for every image. A per-service
   table is a different shape and must be read by **both** sides from one place.
4. **The `upgrade` tier must assert the refusal at the deciding service**, not a
   neighbouring print. RFC-0.34-004's `semantic-liar` is the pattern: the liar's
   own read *and* the deciding service's own output, both required, both saying
   refused.
5. **Flipping `release_gated` to `true` changes what a red tier means.** Once
   gated, an `upgrade` failure blocks a cut. Make sure it passes for the right
   reason before you flip it, and say which run proved that.
6. **`store`'s `not_gated_reason` currently speaks for both tiers.** After R7 it is
   wrong by half. Narrow it in the same commit.
7. **The repro baseline.** Any rebuilt prebuilt means re-recording the baseline in
   the **same commit** (trap 2 of the release handoff), and `fjell-repro-check` has
   no `--help` and starts a two-build run on an unknown flag (E-071) — pass only
   the flags the standing instruction names, and never beside a live `test-all`.

## 3. What to put in the review request

- R1's probes **and their controls**, including whether my two held.
- R2's two transcripts, or the correction if the loop blocks.
- The gate's **failing** run on the eleven, and its passing run after.
- R5's list: which slots installed, why each was necessary, and why the other
  eight were not.
- R6's decision on the kind word — carried or unclaimed — and the statement that
  makes channel id `0` invalid by construction.
- R7: the four markers, the deciding service's own output for each, the
  `release_gated` flip, `store`'s narrowed reason, and `test-all` showing **30 of
  31 gated**.
- R8: E-070 and E-072 closed, or `attestd`/`netd` named as survivors with reasons.
- R9: each gate's own exit status, `test-all`, the baseline, a CI run id.

Hand back one path: your review request. Do not tag anything.
