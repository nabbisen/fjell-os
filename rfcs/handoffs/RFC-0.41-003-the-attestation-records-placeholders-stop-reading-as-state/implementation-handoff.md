# Developer Handoff — RFC-0.41-003

**Governing RFC:** [RFC-0.41-003](../../accepted/RFC-0.41-003-the-attestation-records-placeholders-stop-reading-as-state.md)
**Milestone:** 0.41 · **Independent** of 0.41-001, -002, -004 and -005 — no ordering constraint
**Status:** inherited from the governing RFC (Accepted, 0.41)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design conflict,
**stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can a reader of the signed record tell a computed field from a placeholder?** Today they cannot:
`"SN000001"` reads as a snapshot id and `1` reads as a keyring epoch, and nothing computes either.
When each remaining placeholder carries an availability disclosure beside it and the encoder writes
zero whenever that disclosure is false, this line is done.

**Second measure, and the one with teeth:** **the encoder, not the factory, does the zeroing.** A
caller that builds `RollbackClaims` by field assignment must not be able to emit a non-zero value
beside a `false` flag.

## 0.1 Traps

1. **`local()` is the only *honest* path, not the only path.** `RollbackClaims` is `pub` with `pub`
   fields. Any fix that lives in `AttestationRecordV2::local` leaves the hole open. The architect's
   own review comment had to be corrected from "the encoder writes flag 0 and value 0" to
   "`local()` writes flag 0 and value 0" for exactly this reason.
2. **This is not a `SCHEMA_VERSION` bump, and that was already escalated and ruled.** No byte of any
   record the tree can construct moves, because every construction sets the flag `true`. Precedent
   for regenerating `v2.frozen` without a bump is its own 2026-09-24 note (RFC-0.33-003 D3,
   *"Corrected toward the encoder; no byte moved"*). **Do not re-litigate it** — but see trap 3.
3. **If a byte does move, the premise is wrong.** Stop and escalate. Do not bump the version on your
   own judgement to make a golden test pass; that would be a format change this line was not scoped
   for.
4. **Do not add one flag per primitive.** Six new bytes for six fields would make the record harder
   to read than the placeholders did, failing the same bar this line serves. Group by *claim*.
5. **Row judgement in `v1-limitations.md` is the architect's.** Report what you found; do not edit
   a limitations row on your own judgement. This is the §2 reservation from the release procedure.

## 1. Order of work

**Step 1 — the encoder property first (R2).** Make `write_canonical` zero
`rollback.trust_provider_counter_value` when `trust_provider_counter_supported` is `false`. It is
written unconditionally today and is the one flag-and-value pair in the v3 record that is not
encoder-zeroed. Doing this first gives you the pattern for step 2 and closes the known hole even if
step 2 grows.

**Step 2 — the placeholders (R1).** Each field from the RFC's table gets an availability disclosure
adjacent to it, zero-or-absent when unavailable, zeroed **in `write_canonical`**. Group: the three
keyring anchor epochs are one claim; `snapshot_id` and its `reason` are one claim;
`health.status`, `rollback.last_advance_source` and `freshness.key_epoch` are each their own.
Follow the availability-flag pattern the digests use and `bundle-v1.frozen` documents.

**Step 3 — the test that can actually fail (R3).** Construct a record **by field assignment, not
through `local()`**; set each flag `false` beside a deliberately non-zero value; encode; assert the
encoded bytes are zero. One test covering every pair is better than one per field.

**This is the only test that fails if someone later moves a zeroing back into a factory.** Write it
before you trust steps 1 and 2, and record that you saw it red.

**Step 4 — the frozen schema and the registry (R4).** Regenerate `v2.frozen`; add a dated note in
the same shape as its 2026-09-24 precedent saying what changed and that no byte moved. Update
`crates/fjell-schema/src/registry.rs` and `crates/fjell-schema/tests/golden.rs`.

**Step 5 — `attestd`'s console output (R5).** `print_record_summary` prints the counter pair
already; print the new disclosures in the same `name=value` shape so an operator sees which claims
are unavailable rather than inferring it from zeros.

**Step 6 — read `v1-limitations.md` row 13 (R6)** against what the record now says, and **report**
whether it is still exact. Do not rewrite it.

## 2. What must be shown, not asserted

* **The encoder property, via the direct-construction test.** A test that goes through `local()`
  cannot distinguish the two designs and is not evidence for this line.
* **No byte moved.** The golden test is the instrument. State that it passed unchanged — and if it
  did not, stop (trap 3).
* **Seen red.** Each new assertion must have failed before the fix.

## 3. Gates, and the ones that will move

* `cargo xtask consistency-check --all` — 20 subchecks; the schema registry subchecks are the ones
  this can move. 21 schemas today.
* `cargo xtask test-all` — 33 tiers, including `host-lib` (**761 / 55 / 1 ignored** today) and the
  measurement and attestation profiles. The `measurement` probe has been flake-prone; re-run it at
  your final tip, not only mid-line.
* `cargo run -p fjell-repro-check` — real two-build mode. **Not** the xtask default, which passes
  `--skip-build` and only re-hashes existing files.

**Expected to move:** `attestd.bin` — this recompiles the format crate into it — and the repro
baseline, **re-recorded in the same commit as the rebuild**.

## 4. Escalate rather than decide

* If any byte of a constructible record moves (trap 3).
* If a field cannot be grouped into a claim without ambiguity.
* If making the property hold seems to require changing `RollbackClaims`' visibility. That is a
  larger API change and an explicit non-goal; the encoder fix holds regardless.
* If `v1-limitations.md` row 13 needs more than a report.

## 5. What to report

One review request with: the direct-construction test and the fact you saw it red; confirmation
that the golden digest did **not** move; the grouping you chose for each claim and why; the new
`attestd` summary output as text; your reading of limitations row 13 **as a report, not an edit**;
the moved prebuilt digests with the baseline re-recorded alongside; and the two-build repro result,
stated as not `--skip-build`.
