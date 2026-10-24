# Developer Handoff — RFC-0.39-001

**Governing RFC:** [RFC-0.39-001](../../accepted/RFC-0.39-001-the-chain-records-who-actually-said-it.md)
**Milestone:** 0.39
**Status:** inherited from the governing RFC (Accepted, 0.39)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does the chain record who actually appended each event?** Today it records whatever the
caller says, and for three events per boot what the caller says is false: `init` appends
them declaring `MeasurementSource::Verifyd`, and `verifyd` had no part in them. When the
source comes from the kernel-attested sender and a declared one is refused, this line is
done.

**Not the measure:** that the three events now read `Init`. That is the visible
consequence; the property is that a caller *cannot* declare a source at all.

## 0.1 This is the sibling of a thing 0.38 got right

RFC-0.38-001's `APPEND_IMAGE_SET` arm already does this, and its comment says why — *"the
classification is fixed here so a caller cannot relabel it."* Read that arm first. You are
applying its reasoning to `APPEND_EVENT`, not inventing a principle.

The attested sender is already in your hand: measuredd's receive helper returns it and the
call site throws it away with the comment *"also returns the sender identity, unused
here"*. The work is to look at what you already receive.

## 0.2 The two places this can go wrong quietly

1. **A fallback for an unknown `image_id`.** The mapping from the attested `image_id` to
   `MeasurementSource` will not cover everything, and a `_ =>` arm returning some
   plausible source would re-create this exact defect for the next service added. **Refuse
   instead.** R2 makes that the requirement, not a preference.

   The tree already contains the warning: `decode_source(0x01)` maps to `Measuredd` where
   the kernel's own enum means `Kernel`. It is unreachable now and was noted, unfiled, at
   the 0.38-004 review. Do not add a second one.

2. **Accepting the declared byte and ignoring it.** Smaller change, and it leaves every
   caller believing it declared something. R3 requires refusal because an API that
   silently discards what a caller set is one the caller misunderstands — which is the
   standard this project is held to, stated by the owner.

## 1. Order

1. **R1 completely**, including the class sweep for anywhere else a declared identity is
   recorded where an attested one is available. Escalate if you find a second instance;
   do not widen.
2. **R5's tier, demonstrated failing (the pre-change half), before R2.** Against today's
   tree a task can declare any source and the chain believes it. Capture that; it is the
   evidence E-096 was real and it cannot be reconstructed afterwards.
3. **R2**, the attested mapping, failing closed on an unknown `image_id`.
4. **R3**, the reserved byte and its error.
5. **R4** — the three events become `source: Init`, and whatever reads them follows.
6. **Re-run R5 green.** Re-take any moved chain-digest goldens (§2.1).
7. **R6**, closing E-096 and updating limitations row 15, and **R8**, the gates.

R7 (no retro-claim) governs the prose in steps 5–7, not a step of its own.

## 2. Traps specific to this line

1. **Chain digest values will move, and that is correct.** `source` is inside
   `compute_chain_digest`'s 130-byte preimage, so three events changing provenance changes
   the digests those boots produce. What must **not** move is the stream's *layout* —
   `chain-digest-v1.frozen` and 0.38's `the_chain_digest_stream_is_untouched()` guard it
   and must still pass. **A moved value with the layout test green is this line working; a
   moved layout is a finding — stop and escalate.** Say in the review request which
   goldens moved.
2. **`init` is the appender, so `Init` is the truth.** Do not route the three events
   through `verifyd` to keep the old label. `verifyd` verifies nothing (E-095); that would
   be the same false claim with more steps, and it would also be new wiring in a line that
   has none.
3. **Do not widen `APPEND_EVENT`'s digest to 32 bytes.** It is a real defect, recorded in
   E-096 as a dependency for E-095, and it is a non-goal here because the three events
   carry a literal. **If R3's wire change makes widening free in the same edit, say so in
   the review request rather than doing it** — I would rather rule than discover.
4. **`/usr/bin/grep -a`, not `grep`** — plain `grep` skips NUL-containing files here, and
   a blind "0 sites" has closed an erratum wrongly in this project before. Control every
   absence probe against a file you know contains the string, R1's sweep especially.
5. **Rebuild before re-recording the repro baseline**, and commit the `.bin`s with it. A
   protocol constant change also moves the ABI snapshot; regenerate it here.
6. **RFC-0.39-002 must not start before this lands.** If you are tempted to install slot 4
   "while the measurement code is open", don't — publishing the head before provenance is
   attested is the thing the milestone's order exists to prevent.

## 3. What to put in the review request

- **R1's sweep as a conclusion with its command and control** — other sites recording a
  declared identity where an attested one exists, including the negative case.
- **The `image_id` → `source` mapping you chose**, and what happens on an unknown id.
- **Which error R3 returns** for a non-zero source byte, and why that one.
- **Both R5 transcripts**, pre-change (the declaration accepted) and post-change
  (refused), plus the positive case that a known service records *that* service.
- **Which chain-digest goldens moved**, with the layout test's result beside them.
- **Exact commands and exit statuses** for every gate in R8.
- **Any place my RFC is wrong.** Every line in 0.38 found one, and both of us are better
  off for it. The tree wins.
