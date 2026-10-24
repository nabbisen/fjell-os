# RFC-0.39-002: The head is readable, and says how much of it is real

**Status:** Accepted (0.39) — accepted 2026-10-24
**Milestone:** 0.39
**Tracks.** Nothing in the register. This is the work RFC-0.38-001 deliberately deferred
at the owner's scoping — installing `DIAGNOSTICSD` slot 4 and letting
`measurement_head_available` read `true` — **plus the disclosure that makes the flag
honest**, which is this line's own addition and the reason it is not a one-day change.
**Depends on RFC-0.39-001**, which must land first.
**Touches** *(indicative)*: `crates/fjell-kernel/src/task/spawn.rs`,
`crates/services/fjell-diagnosticsd/src/main.rs`,
`crates/services/fjell-measuredd/src/main.rs`,
`crates/formats/fjell-diag-format/` *(format + frozen schema)*,
`crates/fjell-service-api/src/lib.rs`, `tests/qemu/profiles/remote-diag.toml`,
`tools/fjell-consistency-check/src/capability_topology.rs` *(the waiver goes away)*,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **E-095** and **E-094**, both of which this line must disclose and
neither of which it closes; **RFC-0.36-001**, whose availability-flag pattern this
extends; **RFC-0.35-004**, whose frozen chain-digest stream this must not move.

## What this line is for

`DIAGNOSTICSD` slot 4 is declared and uninstalled. `query_measurement_head` therefore
fails, `measurement_head_available` reads `false`, and `remote-diag` asserts that it
does. 0.38 made the head worth reading; this line lets it be read.

**It is not "turn the flag on."** A signed bundle saying `measurement_head_available: 1`
beside a 32-byte head reads as *"this node's measurement state is attested, here is its
root."* After RFC-0.39-001 the provenance of every event is attested, which removes the
worse of the two reasons that reading is false. The other remains: **three of the chain's
four events are the literal `0xAB`** (**E-095**), and nothing in the bundle says so.

So this line installs the capability **and** makes the bundle state how much of the chain
is a real measurement. Without that, shipping the install creates a release whose signed
artefact misleads — which is why the owner authorised both halves together and why B
without C was ruled out.

### Why a field and not a documentation row

`v1-limitations.md` is read by someone auditing the project. A bundle is read by a fleet
manager, or by code. The standard this project is held to is that an API must not *permit*
the misunderstanding, not that the misunderstanding be recorded somewhere findable. A
signed artefact reporting a measurement root while withholding that three quarters of it
is a placeholder is a misleading API with a truthful appendix.

The pattern already exists and `bundle-v1.frozen` states it: *a field that may be
unavailable carries its own flag, adjacent to it, zeroed by the encoder.* This extends it
from **absent** to **partly placeholder**.

## Requirements

**R1 — Re-derive, with controls.** That slot 4 is declared and uninstalled and the
`capability-topology` waiver names this line; that `measurement_head_available` is derived
from `query_measurement_head` succeeding rather than hardcoded; `remote-diag`'s current
assertions; and the three availability flags' encoder-zeroing pattern in
`fjell-diag-format`. Confirm **RFC-0.39-001 has landed** before starting — if it has not,
stop.

**R2 — Install slot 4 as a `CALL` capability.** `query_measurement_head` is a call now,
not a send and a recv, so a `SEND` capability would not serve it. The
`capability-topology` waiver for `DIAGNOSTICSD` slot 4 **stops being a waiver** and
becomes an installed slot; its text goes rather than being reworded.

**R3 — The chain reports its own composition, and only what it can honestly know.**
`measuredd` must report **two counts** beside the head: how many events the chain holds,
and how many of those carry a digest **computed by something that could compute it** —
i.e. appended through `APPEND_IMAGE_SET`, where the kernel produced the digest and
`measuredd` fixed the classification.

**This is deliberately a structural claim, not a semantic one.** `measuredd` cannot
inspect 32 bytes and know whether they are a hash or a literal; asking it to report
"how many are real" would be asking it to assert something it cannot determine. What it
*can* know is which append path an event arrived by, and that is exactly the distinction
that matters. Report that, and name the field for what it measures.

`GET_HEAD_STATS` already carries `latest_seq`, `dropped` and `last_event_kind` with its
fourth reply word unused, so the protocol change should be small. **State what you chose
and why**, and if you conclude a new op is cleaner than the spare word, say so.

**R4 — The bundle carries the counts, written by the encoder.** Adjacent to
`measurement_head`, following the established pattern: fixed width, written whether or not
available, **zeroed by the encoder** when unavailable rather than trusting the caller's
raw value. Bumps `DIAG_BUNDLE_VERSION` 3 → 4 and regenerates `bundle-v1.frozen`.

The encoder is the description: the frozen file must be regenerated from the encoder, not
hand-edited to match.

**R5 — `remote-diag` asserts the new truth, demonstrated failing.** It must assert
`measurement_head_available=true`, the head's value **against an independent derivation**
(not merely non-zero — the standard RFC-0.38-001's R6 set), and the composition counts.
**Shown red against the pre-change tree** — where it asserts `false` and will fail by
design — and green after. Both transcripts in the review request.

A tier that asserts `true` and a non-zero head would pass against a bundle reporting a
placeholder chain as a measured one. The counts are the point.

**R6 — No retro-claim, and two specific sentences nobody may write.** Nothing in this
line, its commits, its limitations rows or its release note may imply:

1. **that attestation is now real** — the signed attestation record is independent
   constants (**E-094**), and this line does not touch it; or
2. **that the chain is now fully measured** — three of four events remain placeholders
   (**E-095**), which is precisely what R3 and R4 exist to disclose.

`v1-limitations.md` rows 13, 14 and 15 are the places this is already written; update them
to say what changed and what did not, in the same commit.

**R7 — Gates by exit status.** `release-rehearsal`, `consistency-check --all`,
`readiness-check`, `abi-snapshot --verify`, `fjell-repro-check`, `test-all` to
completion, `callsite-audit`, `cargo fmt --check`. A bundle-format change and new protocol
constants move the snapshot: regenerate it in this line. Rebuild **before** re-recording
the repro baseline and commit the binaries with it. One `test-all` at a time.

**Expect `capability-topology`'s figures to move** — "4 installed, 2 waived" becomes one
fewer waived and one more installed. That is this line working; state the new figures in
the review request so the next cut's handoff can pre-state them.

### Non-goals

- **E-095 — making the three events real.** It needs real release, rootfs and policy
  verification, and `APPEND_EVENT` widened from 16 to 32 bytes of subject digest. A
  milestone, not a line. This line **discloses** it; it does not close it.
- **E-094 — the attestation record.** Needs a route from the chain to `attestd` and a
  decision about three `*_verified: true` flags. The natural next line, and once this one
  has wired a measurement consumer, that route is the same class of work.
- **`last_attestation_available`.** It stays `false`. Do not install an attestd route
  "while you are in there".
- **Moving `compute_chain_digest`'s byte stream** (RFC-0.35-004). The counts live in the
  head and the bundle, **not** in the chain-digest preimage, so the frozen layout must not
  move. `chain-digest-v1.frozen` and `the_chain_digest_stream_is_untouched()` must still
  pass; if either moves, **stop and escalate**.
- **Any addition to the kernel's ABI surface.** Escalate. Service-API and bundle-format
  items are not that; regenerate the snapshot here.

## Risks

- **Shipping R2 without R4.** The two are one change for a reason: the install is what
  makes the disclosure necessary. If R3/R4 turn out larger than expected, **escalate
  rather than landing the install alone** — a release whose signed bundle misleads is the
  outcome this line exists to prevent, and a half-done line would produce exactly it.
- **A tier that asserts `true` and a non-zero head.** R5's independent derivation and the
  counts are what stop it passing for the wrong reason. This is the third milestone in a
  row where that distinction decided a requirement.
- **Reporting a semantic claim `measuredd` cannot support.** "How many events are real" is
  not knowable from the bytes. R3 asks for the append path, which is. If the field's name
  implies more than the path supports, the field is the defect.
- **Rewording the waiver instead of removing it.** A waiver that has been satisfied is not
  a waiver with new text.
