# Developer Handoff — RFC-0.39-002

**Governing RFC:** [RFC-0.39-002](../../accepted/RFC-0.39-002-the-head-is-readable-and-says-how-much-of-it-is-real.md)
**Milestone:** 0.39
**Status:** inherited from the governing RFC (Accepted, 0.39)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

**Do not start until RFC-0.39-001 has landed and been accepted.** The whole point of the
milestone's order is that the chain's provenance is attested before anything publishes it.

---

## 0. The measure of this line

**Can a consumer of the bundle tell how much of the measurement chain is a real
measurement?** Today it cannot ask at all — slot 4 is uninstalled and the flag reads
`false`. After this line it can read the head, and the bundle tells it one of four events
carries a digest computed by something that could compute one.

**Not the measure:** that `measurement_head_available` reads `true`. That is half the
line, and on its own it is the misleading state this line exists to avoid.

## 0.1 Why this is not the one-day change it was scoped as

0.38 deferred "install slot 4 and let the flag read true". Scoping 0.39 against the
owner's standard — *what is a consumer entitled to believe?* — that turned out to be two
thirds of a change. A signed bundle reporting a measurement root while withholding that
three of its four events are the literal `0xAB` is a misleading artefact, and
`v1-limitations.md` recording the fact does not fix it: a limitations row is read by
someone auditing the project, a bundle by a fleet manager or by code.

So R2 (install) and R3/R4 (disclose) are **one change**. If the disclosure turns out
larger than expected, **escalate rather than landing the install alone** — see trap 1.

## 0.2 The honest-claim boundary, which is R3's whole difficulty

`measuredd` cannot look at 32 bytes and know whether they are a hash or a literal. So it
**must not** report "how many events are real" — that is a claim it cannot support, and a
field whose name implies it would be the defect rather than the fix.

What `measuredd` does know is **which append path an event arrived by**:
`APPEND_IMAGE_SET`, where the kernel computed the digest and `measuredd` fixed the
classification, versus `APPEND_EVENT`. Report that, and name the field for what it
actually measures. Getting this name right matters more than the plumbing.

`GET_HEAD_STATS` has an unused fourth reply word, so the protocol change should be small.
Say what you chose; if a new op is cleaner than the spare word, say that instead.

## 1. Order

1. **R1**, including confirming RFC-0.39-001 has landed. If it has not, stop.
2. **R5's tier, demonstrated failing**, before anything else — it asserts
   `measurement_head_available=false` today and must be seen failing against the
   pre-change tree by design.
3. **R3**, measuredd's counts, and the protocol to carry them.
4. **R2**, the slot-4 install as a `CALL` capability, and the `capability-topology` waiver
   **removed** rather than reworded.
5. **R4**, the bundle fields, encoder-zeroed, `DIAG_BUNDLE_VERSION` 3 → 4, frozen schema
   **regenerated from the encoder**.
6. **Re-run R5 green**, with the head checked against an independent derivation.
7. **R6**, the limitations rows 13/14/15, and **R7**, the gates.

R3 before R2 so that the moment the capability exists, the disclosure exists with it.

## 2. Traps specific to this line

1. **Landing the install without the disclosure.** This is the one that matters. A release
   whose signed bundle misleads is the outcome the line exists to prevent, and a
   half-finished line produces exactly it. If R3/R4 grow, escalate.
2. **A tier that asserts `true` and a non-zero head.** That would pass against a bundle
   reporting a placeholder chain as a measured one. R5's independent derivation and the
   counts are the assertion. This is the third milestone running where this distinction
   has decided a requirement; apply it rather than restating it.
3. **The frozen chain-digest stream must not move.** The counts belong in the head and the
   bundle, **not** in `compute_chain_digest`'s preimage. `chain-digest-v1.frozen` and
   `the_chain_digest_stream_is_untouched()` must still pass — if either moves, stop and
   escalate.
4. **The frozen bundle schema is regenerated, not edited.** The encoder is the
   description. Hand-editing `bundle-v1.frozen` to match what you think you wrote is how
   a frozen file comes to describe a layout that does not exist (it has happened twice;
   the file's own header notes say so).
5. **Two sentences nobody may write** (R6): that attestation is now real (it is constants
   — E-094), or that the chain is now fully measured (three of four are placeholders —
   E-095). Both will be tempting in a release note. Both are false.
6. **Do not install an attestd route while you are in there.** `last_attestation_available`
   stays `false`. E-094 is the next line, not a bonus.
7. **`capability-topology`'s figures move** — one fewer waived, one more installed. State
   the new numbers in the review request so the next cut's handoff can pre-state them.
8. **Rebuild before re-recording the repro baseline**, commit the `.bin`s with it, and
   regenerate the ABI snapshot in this line. One `test-all` at a time.

## 3. What to put in the review request

- **Confirmation that RFC-0.39-001 landed first**, with its commit.
- **R3's field names and what they measure**, in your own words, plus why the name cannot
  be read as a semantic claim about the digests.
- **What you chose for the protocol** — the spare `GET_HEAD_STATS` word or a new op — and
  why.
- **Both R5 transcripts**, pre-change red and post-change green, with the head's value
  shown equal to an independent derivation and the counts visible.
- **The `capability-topology` figures before and after**, and confirmation the waiver text
  was removed rather than reworded.
- **That `chain-digest-v1.frozen` is unchanged** and the layout test passed.
- **Exact commands and exit statuses** for every gate in R7, and the new ABI snapshot
  count.
- **Any place my RFC is wrong**, especially R3 — it is the requirement I am least certain
  I have specified at the right altitude.
