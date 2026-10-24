# RFC-0.39-001: The chain records who actually said it

**Status:** Accepted (0.39) — accepted 2026-10-24
**Milestone:** 0.39
**Tracks.** **E-096** — the measurement chain's `source` is decoded from the caller's own
word while the kernel-attested sender is received and discarded, and three events per boot
declare a service that had no part in them.
**Touches** *(indicative)*: `crates/services/fjell-measuredd/src/main.rs`,
`crates/services/fjell-init/src/main.rs`, `crates/fjell-service-api/src/lib.rs`
*(protocol)*, `crates/formats/fjell-measure-format/` *(possibly)*,
`tests/qemu/profiles/`, `crates/services/fjell-neg-test/`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **RFC-0.38-001**, which established this line's principle on one append
path and left the other; **RFC-0.39-002**, which must not land before this one;
**E-095**, whose closure needs the end state this line creates.

## What is wrong

`MeasurementSource` means "which service produced the measurement event". `measuredd`'s
`APPEND_EVENT` arm decodes `kind`, **`source`** and `subject` from the caller's own word
`w0` and appends them verbatim. The **kernel-attested sender is received and thrown
away** — measuredd's receive helper returns it and the call site discards it with the
comment *"also returns the sender identity, unused here"*. The kernel attests sender
identity in `a6` (`tid | image_id << 16`); measuredd has it and does not look.

**And the declared source is not merely unattested, it is false.** `fjell-init` appends
the three verification events as `(kind, src, subj)` of `(2,2,2)`, `(3,2,3)`, `(4,2,4)`;
`src = 2` is `MeasurementSource::Verifyd`. `init` is the sender. `init` only *spawns*
`verifyd` and registers its name — it never consults it about any of these. So the chain
asserts, three times per boot, that **`verifyd` produced a verification result it had no
part in**.

RFC-0.38-001 got this right on the path it added. Its `APPEND_IMAGE_SET` arm fixes kind,
source and subject inside `measuredd`, with the comment *"the classification is fixed here
so a caller cannot relabel it."* That is the reasoning this line applies everywhere.

### Why this is 0.39's first line and not a later one

0.38 did not touch this because **E-086 was about the subject digest being a literal**,
not about provenance. A chain can record a true digest under a false source, and after
0.38 it does. E-096 is ACCEPTED only because **nothing reads the chain head** — slot 4 is
uninstalled and `measurement_head_available` reads `false`.

**RFC-0.39-002 removes that reason.** Installing the capability publishes the chain's
provenance to a signed bundle. So this line lands first, and the owner's own ordering
principle from 0.38 is the argument: *the chain is shown to be true before anything
reports it.*

## Requirements

**R1 — Re-derive, with controls.** The discarded sender and its comment; `APPEND_EVENT`
decoding `source` from `w0`; the three `(kind, src, subj)` tuples and that `src = 2` is
`Verifyd`; that `init` never consults `verifyd` about them; and that `APPEND_IMAGE_SET`
fixes its own classification. `/usr/bin/grep -a` — plain `grep` skips NUL-containing files
here.

**Then sweep for the class:** every other place the tree **records a declared identity
where an attested one is available**. The kernel attests the IPC sender; any service that
writes "who asked" from a payload rather than from `a6` is the same defect. **If you find
another, stop and escalate** — do not widen this line.

**R2 — `source` comes from the attested sender.** `measuredd` derives
`MeasurementSource` from the sender identity it already receives, and the mapping is
yours to state: the attested word carries `tid | image_id << 16`, and `ImageId` is the
stable half.

**State the mapping, and make it fail closed.** An `image_id` with no
`MeasurementSource` must be **refused**, not defaulted. A default would record a false
source for an unknown appender, which is this entry's own defect one level up — and
`decode_source`'s existing legacy behaviour is a warning here, not a pattern: it maps
`0x01` to `Measuredd` where the kernel's own enum means something else (noted at the
0.38-004 review, still unfiled because its path became unreachable).

**R3 — A declared source is refused, not ignored.** The `source` byte in `APPEND_EVENT`'s
word becomes reserved and must be zero; a non-zero value is an error, not a field quietly
overridden.

This is deliberate and it is the API half of this line. Accepting the byte and ignoring
it would leave every caller believing it had declared something, and a caller that
believes a false thing about an API is the failure the owner's standard names directly.
Refusing makes the contract legible at the call site. **State which error you return.**

**R4 — The three events say what is true.** After R2 they are `source: Init`, because
`init` appended them. That is a truthful record of a placeholder, which is the most this
release can say about them. Their serial output and any tier that reads them follow.

Do **not** make them say `Verifyd` by some other route, and do not have `init` ask
`verifyd` to append on its behalf as a way of preserving the old label — `verifyd`
verifies nothing (**E-095**), so that would be the same false claim with more steps.

**R5 — A negative tier: a task cannot declare a source it is not.** A task appends an
event with a non-zero `source` byte and is refused; and an event appended by a known
service records *that* service. **Demonstrated failing** against the pre-change tree,
where the declaration is accepted — both transcripts in the review request.

**R6 — E-096 closes in this line's commit**, with `docs/src/releasing/v1-limitations.md`
row 15 updated. The entry is ACCEPTED, so nothing is blocked; it closes because the defect
is gone, not because the milestone ended.

**R7 — No retro-claim.** Chains recorded before this line asserted `Verifyd` for three
events per boot. Do not write that the source was ever attested, and do not describe the
old behaviour as a relay — nothing was relayed.

**R8 — Gates by exit status.** `release-rehearsal`, `consistency-check --all`,
`readiness-check`, `abi-snapshot --verify`, `fjell-repro-check`, `test-all` to
completion, `callsite-audit`, `cargo fmt --check`. A protocol constant change moves the
snapshot: regenerate it in this line. Rebuild **before** re-recording the repro baseline
and commit the binaries with it. One `test-all` at a time.

### Non-goals

- **Widening `APPEND_EVENT`'s subject digest from 16 to 32 bytes.** Real, recorded in
  E-096 as a dependency for **E-095**, and deliberately not here: the three events carry a
  literal, so a wider field would carry more zeros. Whichever line makes them real widens
  it. If you find that R3's wire change makes widening free in the same edit, **say so in
  the review request** rather than doing it — I would rather rule on it than discover it.
- **Making the three events real** (E-095), **the attestation record** (E-094), and
  **installing slot 4** (RFC-0.39-002).
- **Any addition to the kernel's ABI surface.** Escalate. Service-API protocol constants
  are not that; regenerate the snapshot here.

### One trap, named because it will look alarming

`source` is **inside** `compute_chain_digest`'s 130-byte preimage (domain 20 + 2 + 8 +
1+1+1 + 32 + 1 + 32 + 32). So changing three events' source from `Verifyd` to `Init`
**changes the chain digest values those boots produce**. That is correct and expected: a
different provenance is a different measurement.

What must **not** change is the stream's *layout*, which RFC-0.35-004 froze and which
`chain-digest-v1.frozen` and 0.38's own `the_chain_digest_stream_is_untouched()` test
guard. **Those must still pass.** Any test or golden asserting a *literal* chain-digest
value will move; re-take it and say in the review request which ones moved and why. A
moved value with the layout test green is this line working. A moved layout is a finding —
**stop and escalate**.

## Risks

- **The `image_id` → `source` mapping failing open.** An unknown appender defaulting to
  any source re-creates this defect for the next service added. R2 makes refusal the
  requirement; a `_ =>` arm with a plausible-looking fallback is the thing to not write.
- **Silently ignoring the declared byte.** It is the smaller change and it leaves the API
  lying to its callers. R3 forbids it.
- **Preserving the old label out of tidiness.** `Verifyd` reads better than `Init` in a
  chain dump. It is also false. R4 exists because that temptation is real.
