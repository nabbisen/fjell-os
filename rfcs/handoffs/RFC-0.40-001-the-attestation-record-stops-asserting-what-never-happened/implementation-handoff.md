# Developer Handoff — RFC-0.40-001

**Governing RFC:** [RFC-0.40-001](../../accepted/RFC-0.40-001-the-attestation-record-stops-asserting-what-never-happened.md)
**Milestone:** 0.40 — **the lead line, and the only one of the three with design content**
**Status:** inherited from the governing RFC (Accepted, 0.40)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does the signed attestation record assert anything that did not happen?** Today it
asserts three verifications succeeded, over five literal digests, with a measurement
section of `head_seq: 1` and `chain_digest: 0x55`. When every field is either a real value
or zero-and-unavailable, and the three flags read `false`, this line is done.

**Not the measure:** that the record is *complete*. It will not be — the kernel's own image
is unmeasured and release/rootfs/policy verification does not exist. The measure is that
nothing in it is false.

## 0.1 The one thing to hold on to

There will be a pull toward finding *something* to put in `kernel_digest`. 0.39 gave the
kernel a real digest of the **service images**, and it is right there. **It is not a kernel
digest**, and putting it there would be this entry again with a better-looking value.

Zero plus an availability flag is the honest answer for every field nothing computes. The
project already has that pattern, stated in `bundle-v1.frozen` and implemented in
`fjell-diag-format`: *a field that may be unavailable carries its own flag, adjacent to it,
zeroed by the encoder.* **Copy it; do not invent a second one.**

## 0.2 The half that would be found too late

R5 is the requirement I would most expect to be skipped, because the RFC asks you to
*narrow* something while you are busy adding something.

`GET_HEAD` is ungated by sender, so reading the head needs only the endpoint. But
`ATTESTD` **is** in RFC-0.39-001's `source_for_sender` mapping, so the `CALL` capability R4
grants would also let `attestd` **append** chain events recording `source: Attestd`.
Nothing appends as `Attestd` — the only occurrence outside the mapping is its own test
table. So the capability must arrive with `ATTESTD` removed from the mapping, in the same
commit, and `the_services_that_must_be_refused_are` extended to name it.

Grant the read, refuse the append.

## 1. Order

1. **R1 completely**, including the sweep for a third signed-or-exported artefact
   asserting a positive result from a literal. Escalate if you find one.
2. **R6's tier, demonstrated failing**, before the changes — the flags read `true` and the
   head is `0x55` today. That transcript is the evidence E-094 was real.
3. **R2 and R3** — the flags to `false`, the digests to zero-plus-flag, version bumped.
   These need no capability and no new route, so they land first and the record stops lying
   even if R4 runs into something.
4. **R4 and R5 together** — the `CALL` capability and the mapping removal. Not in separate
   commits.
5. **Re-run R6 green.**
6. **R7**, the limitations rows and the no-retro-claim prose, and **R8**, closing E-094.
7. **R9**, the gates, with the new `capability-topology` figures stated.

R2/R3 before R4/R5 deliberately: the truthfulness fix does not depend on the route, and
ordering it first means a stuck R4 cannot leave the record asserting `true`.

## 2. Traps specific to this line

1. **Completing the record instead of making it truthful.** §0.1. The service-image digest
   is not a kernel digest.
2. **The capability without the mapping removal.** §0.2. Same commit.
3. **A tier that checks the head is non-zero.** Third milestone running where this decides
   a requirement. Compare against an independent derivation, as `remote-diag` does.
4. **A second availability pattern.** `fjell-diag-format` has one; its frozen schema states
   the rule. Reuse the shape, including encoder-side zeroing rather than trusting a caller.
5. **A silent fallback when `measuredd` cannot be reached.** If the query fails, the
   measurement section is **unavailable**. A record reporting a stale or literal head on a
   failed query is E-094 with extra steps.
6. **`sign_v2`'s unused parameter.** Either it uses the sequence or it does not take one.
   Leaving `#[allow(dead_code)] _meas_seq` in place after this line would be odd.
7. **Rebuild before re-recording the repro baseline**, and commit the `.bin`s with it.

## 3. What to put in the review request

- **R1's sweep as a conclusion with its command, control and boundary** — other signed or
  exported artefacts asserting a positive result from a literal, including the negative
  case.
- **What you zeroed and what you made real**, field by field, against the RFC's table.
- **The record's version move and what it cost** — the attestation format is not in the ABI
  snapshot, so say what *does* notice.
- **Confirmation that `ATTESTD` is out of the mapping** and that a refusal is asserted.
- **Both R6 transcripts**, pre-change (flags `true`, head `0x55`) and post-change.
- **The new `capability-topology` figures**, for the next cut's handoff to pre-state.
- **Exact commands and exit statuses** for every gate in R9.
- **Any place my RFC is wrong.** Every line in 0.38 and 0.39 found at least one — in
  0.39-004 you found that my required step order would have cleared a wrong PTE before
  detecting it, which defeated the line's own argument. Keep doing that.
