# RFC-0.41-003: The attestation record's remaining placeholders stop reading as state

**Status:** Accepted — accepted 2026-10-30
**Milestone:** 0.41
**Tracks.** **E-101** — the signed record still carries unqualified placeholders that read as this
node's state: keyring anchor epochs `1`/`1`/`0`, `snapshot_id = "SN000001"` with `reason 0`,
`health.status`, `rollback.last_advance_source = UpgradedConfirmation`, and
`freshness.key_epoch = 1`. Plus the **encoder-zeroing property** recorded in the same entry: the
`trust_provider_counter_*` pair is zeroed by the factory, not by the encoder.
**Touches** *(indicative)*: `crates/formats/fjell-attestation-format/src/v2.rs`,
`crates/formats/fjell-attestation-format/src/tests_v2.rs`,
`crates/formats/fjell-attestation-format/schema/v2.frozen`,
`crates/services/fjell-attestd/src/main.rs`, `crates/fjell-schema/src/registry.rs`,
`crates/fjell-schema/tests/golden.rs`, `rfcs/ERRATA.md`.
**Relates to:** **RFC-0.40-001**, which made the record truthful and whose field table enumerated
three flags and five digests — this line takes the members of the same class that table did not
list; **E-094**, closed, and **not** this shape: none of these asserts that an operation
*succeeded*; **E-095**, which will make verification real and re-read the whole record.

## What is wrong

`attestd` signs a record that describes this node, so its fields are read as that node's state.
`AttestationRecordV2::local` sets these from constants:

| Field | Constant | Reads as |
|---|---|---|
| keyring anchor epochs | `1` / `1` / `0` | a keyring state |
| `snapshot_id` | `"SN000001"` | a snapshot identifier |
| snapshot `reason` | `0` | a recorded reason |
| `health.status` | constant | this node's health |
| `rollback.last_advance_source` | `UpgradedConfirmation` | how the counter last advanced |
| `freshness.key_epoch` | `1` | the key epoch in force |

Nothing computes any of them. **This is not E-094's shape** — none claims an operation succeeded —
but `"SN000001"` is read as an id and an epoch is read as an epoch, and a reader cannot tell these
from the fields that are real. RFC-0.40-001 gave the five digests the right treatment: written as
zero behind an availability byte, with a `verified_flags` bit set only when the digest is
available. **These fields want that same treatment**, and the reason is the bar's second point
rather than its first: a field whose name implies more than it holds is a defect even when nothing
asserts success.

### The property the entry filed alongside it

**Every flag-and-value pair must be zeroed by the encoder when its flag is `false`, not by the
factory that happens to build it.** Today the five digests and the measurement section are
(`v2.rs`'s `zeroed` closure in `write_canonical`, and `measurement_written()`). The
`trust_provider_counter_*` pair is **not**: `write_canonical` writes the caller's value
unconditionally and only `AttestationRecordV2::local` maps `None` to `0`. `RollbackClaims` is
`pub` with `pub` fields, so `local()` is the only *honest* path, not the only path — a caller that
builds the struct directly can set `supported = false` beside a non-zero value and the encoder
will write it. `bundle-v1.frozen` states why the pattern exists: zeroed *"in the encoder itself
whenever unavailable rather than trusting the caller's raw value."*

**This is not a `SCHEMA_VERSION` bump.** No byte of any record the tree can construct moves, since
every construction sets the flag `true`. The precedent for regenerating `v2.frozen` without a bump
is its own 2026-09-24 note (RFC-0.33-003 D3, *"Corrected toward the encoder; no byte moved"*).
That was escalated and ruled at the 0.40.0 review; it is recorded here so the implementer does not
have to re-litigate it.

## Requirements

**R1 — availability beside each placeholder, encoder-zeroed.** Each field in the table gets the
treatment RFC-0.40-001 gave the digests: an availability disclosure adjacent to the field it
qualifies, the value written as zero (or absent) when unavailable, and **the zeroing done in
`write_canonical`** rather than trusted from the caller.

Group them where they already belong — the keyring epochs are one claim, the snapshot id and its
reason are one claim — rather than adding one flag per primitive. A reader should be able to see
*which claim* is unavailable, not reconstruct it from three bytes.

**R2 — the `trust_provider_counter_*` pair moves into the encoder.** `write_canonical` must zero
the value when `trust_provider_counter_supported` is `false`, so the property holds for every
construction and not only for `local()`. This closes the one exception in the v3 record.

**R3 — the property is asserted, not just implemented.** A test must construct a record **by
field assignment rather than through `local()`**, set each flag `false` beside a deliberately
non-zero value, encode it, and assert the encoded bytes are zero. That is the only test that can
fail if someone later moves a zeroing back into a factory. One test covering every pair is
preferable to one per field.

**R4 — `SCHEMA_VERSION` does not move, and the frozen schema is regenerated.** Regenerate
`v2.frozen` and add a dated note in the same shape as its 2026-09-24 precedent, saying what
changed and that no byte moved. Update `crates/fjell-schema/src/registry.rs` and the golden test
to match. **If any byte of a constructible record does move, stop and escalate** — that would mean
the premise of R4 is wrong and the line needs a version bump it was not scoped for.

**R5 — `attestd`'s summary output follows.** `print_record_summary` prints the counter pair today;
it must print the new availability disclosures too, in the same `name=value` shape, so an operator
reading the console sees which claims are unavailable rather than inferring it from zeros.

**R6 — the limitations row is re-read.** `docs/src/releasing/v1-limitations.md` row 13 describes
what the record does and does not say. When this line changes which fields are qualified, that row
must be re-read and corrected if it is no longer exact. **Row judgement is the architect's** — the
handoff says what to report rather than what to write.

### Non-goals

* **Making the placeholders real.** Computing a keyring epoch, a snapshot id or a health status is
  not this line. This line stops them *claiming* to be computed.
* **E-095.** Three of the measurement chain's four events are still `0xAB` because nothing verifies
  a release, a rootfs or a policy bundle. Separately scheduled, deliberately — see the 0.41
  schedule ruling.
* **A `SCHEMA_VERSION` bump.** See R4, including what to do if the premise fails.
* **Changing `RollbackClaims`' visibility.** Making the fields private would also close R2's hole,
  and it is a larger API change than this line is scoped for. The encoder fix is the one that
  holds regardless of visibility.

## Risks

**The record is signed.** A field that becomes an availability disclosure changes what the
signature covers. R4's premise is that no constructible record moves a byte; it is load-bearing
and must be *verified* rather than assumed — the golden test is the instrument.

**A per-primitive flag explosion** would make the record harder to read than the placeholders did,
which would fail the same bar this line serves. R1's grouping requirement exists for that reason.

**Prebuilts move.** This recompiles the format crate into `attestd.bin`. The repro baseline is
re-recorded in the same commit as the rebuild.
