# RFC-0.40 fix-forward: the record stops claiming a call that was never made

**Handoff:** `.git-exclude/handoff/fix-forward-0.40.md` (F1–F5)
**Governing RFC:** [../accepted/RFC-0.40-001-the-attestation-record-stops-asserting-what-never-happened.md](../accepted/RFC-0.40-001-the-attestation-record-stops-asserting-what-never-happened.md)

Written against base `2220740`. All three 0.40 lines were already accepted; this is the work the
reviews required before the 0.40.0 cut.

## F1 — the counter pair

### R4, the pre-change transcript (taken first, before any change to the field)

The record summary gained one line (`attestd: record rollback counter_supported=… counter_value=…`),
and, as a scratch line beside it, `attestd`'s own `read_anti_rollback_counter()` result:

```
attestd: record rollback counter_supported=1 counter_value=0
attestd: SCRATCH provider counter read ok=1
```

The record says the call **succeeded** (`1`) and reports `0` — the genesis rollback record's minimum
(`RollbackRecord::genesis` is `min_counter 0`) — while the provider, asked, answers **1**
(`DevelopmentTrustProvider` starts its counter at 1). Nothing in `attestd` made that call. So the
claim and the value were each wrong, in different ways. (Control on the tier: with the new marker
`counter_supported=1 counter_value=1` applied to this transcript, `grep -c` finds **0**; the
`counter_supported=1` control in the same transcript finds 1.)

### R1 — the read

`sign_v2` calls `provider.read_anti_rollback_counter().ok()` once per record, through the provider it
already holds, and passes the `Option<u64>` to `AttestationRecordV2::local`, which sets the flag from
`is_some()` and the value from what was returned (`0` on `None`). Per record, never cached. The
scratch line was removed; `HardwareTrustProvider` (the trait that owns the method) lost its
`#[allow(unused_imports)]`, since it is now used. No shared reader, no `fjell-service-api` change.

### R2 — no second flag

None added. `RollbackClaims::trust_provider_counter_supported` *is* the availability disclosure; its
doc now also says the value is `0` when it is `false`. The canonical encoding is unchanged: the
`v2.frozen` schema and the attestation golden did not move, and the sample's two overrides became the
`local(...)` argument.

### R3 — asserted in the `measurement` profile

`print_record_summary` prints the pair; the profile expects
`attestd: record rollback counter_supported=1 counter_value=1` and the expected-markers artefact has
the line. The profile's header comment, which still said the tier "says nothing about the attestation
record, which is built from constants", was stale since 0.40-001 and is corrected.

### R4, post-change, and the failure case

Green: `attestd: record rollback counter_supported=1 counter_value=1`; `measurement` PASS (11 markers).

The failure case, not only the success: with the read replaced (scratch) by `None`, the record says
`counter_supported=0 counter_value=0`, and the tier is **red** — `missing marker … counter_supported=1
counter_value=1`. Reverted; `cmp` against the saved copy of `main.rs` identical; rebuilt; green again.

**What it carries on a normal boot:** the development provider's counter, `1`. Unit tests in
`tests_v2.rs` (30 pass in the crate) add the pair's two cases: a read that returned 7 beside a rollback
floor of 42 reports 7; a failed read reports `false`/`0` and differs from a read that returned 0.

### What did not move

Only `attestd.bin` among the prebuilts (baseline: one line). The ABI snapshot is unchanged — the marker
is a profile string, not a constant — `Added 0 / Removed 0 / Changed 0`.

## F2 — three errata filed, all ACCEPTED

| Entry | Status | Limitations row |
|---|---|---|
| E-101 residual placeholders | ACCEPTED, 0.41 | 13 (folded in, bold reference) |
| E-102 `auditd`'s trail stops at boot | ACCEPTED, 0.41 — the lead candidate | 11 |
| E-103 `reserve_range` | ACCEPTED, 0.41 | **16 (new; the number 0.40-003 freed)** |

E-102 records the direction as the owner's open question with the architect's recommendation (a
periodic drain rather than a wake) noted and **not taken**, and says in terms that E-068 does not decide
it (a retracted *query* sub-protocol, not a *trigger*). It also carries the shared-object-0 hazard (the
twenty-of-thirty-five figure is the architect's count; I did not re-derive it) and the `TaskSwitch`
volume question from the flake in 0.40-002. I checked the loop it describes in `fjell-auditd/src/main.rs`
and the dedicated-endpoint table in `spawn.rs`.

E-103 carries all three divergences. Divergence 1 and the doc/code disagreement I had; divergence 2 (the
reached head is consumed and relabelled) I confirmed **by reading the loop** — it sets the used bit and
the owner before it reaches the conflict — and did not probe myself; the entry cites the architect's
probes.

`errata-limitations`: PASS (14 ACCEPTED errata, all referenced); `errata-tracking`: PASS (103 entries).
*A slip the gate caught:* I first wrote E-102's tracking value as `0.41 (lead candidate)`;
`errata-tracking` accepts only an RFC id, a bare milestone or `unscheduled`, so the cell is `0.41` and
"lead candidate" lives in the entry text.

## F3 — E-094's closure

One paragraph appended to the closure note ("Scope of this closure"): it covers the enumerated fields,
names the counter pair (fixed here) and E-101's residuals, and says E-094 is not reopened. The original
text is untouched.

## F4 — two corrections

* Limitations row 13: the counter pair is named separately as a field of the same class that was missed
  and fixed in review; the rest points at E-101; boot slot, boot id and health target are said to be true
  of this deployment. Row 14 needed no change.
* The `#[ignore]` reason on `an_overlapping_reservation_does_not_leave_the_rest_of_its_range_free` now
  names **E-103** (and the comment above it says it covers divergence 1 only); E-103 names the test.
* Also corrected, in passing: `measurement-and-attestation.md` now describes the counter pair and E-101.

## F5 — the test figure

The figure the next cut's handoff carries is **759 / 55 binaries / 1 ignored at `2220740`** (the
architect's measurement). My requests' 753 was true at the 0.40-003 commit and omitted 0.40-001's six
`tests_v2.rs` tests. The fix-forward adds two more in the same file (`tests_v2.rs` 23 → 25), so the
figure at this commit is **761 passed / 55 binaries / 1 ignored**, summed from the `test result:` lines
of the full run's host-lib log (`tests/runs/20261029-043246/01-host-lib.log`); 759 + 2 = 761.

## Where the handoff is wrong

* Nothing that blocked the work. One precision: the handoff's pre-change claim that the record carries
  "the genesis minimum" is exactly right (`0`), and the provider's counter is `1`, so the two quantities
  differ on a normal boot — which is what makes the transcript readable.
