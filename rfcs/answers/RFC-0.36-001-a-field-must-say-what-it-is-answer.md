# RFC-0.36-001 R1–R10: a field must say what it is

**Governing RFC:** [../done/RFC-0.36-001-a-field-must-say-what-it-is.md](../done/RFC-0.36-001-a-field-must-say-what-it-is.md)
**Handoff:** [../handoffs/RFC-0.36-001-a-field-must-say-what-it-is/implementation-handoff.md](../handoffs/RFC-0.36-001-a-field-must-say-what-it-is/implementation-handoff.md)
**Escalation and ruling:** `.git-exclude/review-request/review-request-rfc-0.36-001-a-field-must-say-what-it-is.md`, ruled at `928afa9`

Written in the order the handoff requires, with R10 folded in where the
ruling placed it: alongside R2/R3/R5, since all four touch the same digest
and the same version bump.

---

## R1 — the sweep, complete, with a fifth found and ruled on

Every field of `DiagnosticBundle`, re-derived at `d110d74` before any edit:

| Field | Source | Can it fail or be absent? | Disposition |
|---|---|---|---|
| `schema_version` | compile-time constant (`DIAG_BUNDLE_VERSION`), written directly, not from `self.schema_version` | no | unchanged |
| `bundle_id` | literal `*b"diag0001"` at the one call site | no — an identifier, not a measurement; same class as `provider_id` | unchanged, named below |
| `created_tick` | the real IPC message word at call time | only via E-078 (no sender exists today) | unchanged |
| `provider_id` | compile-time constant (`TrustProviderId(1)`), same convention every service uses for its own identity | no | **now in the digest** (E-080) |
| `keyring_anchor_epoch` | **was** a bare literal `1`, never queried | always "fails" — no query exists | **closed** (E-079's third instance) |
| `measurement_head` | `query_measurement_head()`, a real IPC round trip | yes, genuinely | **closed** (E-076) |
| `last_attestation` | **was** a bare literal zero, never queried | always "fails" — no query exists | **closed** (E-079, first half) |
| `audit_available` | compile-time `false`, nothing ever sets it | no — honestly reflects "never queried" | unchanged (RFC-0.35-002's shape, confirmed still honest) |
| `audit_event_count` / `audit_events` | consistent with `audit_available: false` | n/a | unchanged |
| `semantic_intent_count` / `semantic_intents` | **was** one fabricated `add_intent` call | n/a — invented, not failed | **closed** (E-079, second half) — deleted |
| `bundle_digest` | computed from the above | reflects upstream honesty only | unchanged in mechanism |

**The four named findings confirmed, each with a control:**

- E-076: `query_measurement_head` returns `Digest32([0u8;32])` on a recv error
  or an unexpected tag — confirmed by reading the function in full before
  any edit.
- E-079 first half: `Digest32([0u8;32])` passed as `last_attestation`,
  literal, no call site queries anything for it — confirmed, zero other
  producers in the tree.
- E-079 second half: `add_intent(1, INTENT_UPDATE_STAGING_CONFIRMED, 0,
  tick - 2000)` — confirmed `add_intent` has no other caller anywhere
  outside `fjell-diag-format`'s own unit tests (`grep -rn "\.add_intent("`).
- E-075: `secure-transportd:160` sends on an uninstalled slot;
  `diagnosticsd:164` sends on its own identity endpoint — confirmed by
  reading `capability-topology`'s own prior output, which already named
  both as waived-but-undiscovered or self-addressed.

**The fifth, found by completing the sweep rather than stopping at four:**
`keyring_anchor_epoch` is the bare literal `1`, same function, same shape
as `last_attestation` — `fjell-diagnosticsd` has no `fjell-keyring`
dependency at all (checked its `Cargo.toml`), so unlike `fjell-attestd`
(`keyring.anchors_for(KeyPurpose::AttestationSigning).next().map(|a|
a.epoch.raw()).unwrap_or(1)` — a real query with a visible fallback) there
is no query and no fallback. Per R1's own instruction, this stopped
implementation entirely and escalated rather than being absorbed silently.
A second, differently-shaped thing was found in the same pass: the digest
doesn't cover `provider_id` or `keyring_anchor_epoch` at all, against
`RFC-v0.4-005` §6.4's own formula — filed as **E-080**.

**The architect's ruling** (`928afa9`): `keyring_anchor_epoch` folded into
E-079 (same defect, same remedy as `last_attestation`); `provider_id` and
`bundle_id`'s exclusion confirmed correct (identity constants, not state
standing in for state that should vary); E-080 tracked into **this line**
as **R10**, because the line already bumps `DIAG_BUNDLE_VERSION` and
re-takes the goldens — fixing coverage later would mean a second bump on
the same digest.

## R2 — goldens, from the unmodified encoder, first

Captured before any edit, at `928afa9`:

- `golden.rs`'s existing `diag_bundle` test, confirmed passing against the
  unmodified encoder (digest `f0d00a58…`) — the stored value already was
  the "before" golden; re-confirmed rather than assumed.
- The sample's full raw byte stream, extracted via a scratch
  `#[test]` (`BufSink::<2048>` + a forced `panic!` printing the hex,
  removed before committing): 162 bytes, decoded field-by-field —
  `domain(13)`, `schema_version(2)`, `bundle_id(8)`, `created_tick(8)`,
  `measurement_head(32)`, `last_attestation(32)`, `audit_available(1)`,
  `audit_event_count(1)`, then 65 bytes of events/intents. Confirms
  `provider_id`/`keyring_anchor_epoch` are absent from the stream
  end-to-end, independent of the `write_canonical` source reading.

**R10's own instruction on R2's order, followed exactly:** the goldens
above pin what the *present* encoder wrote, which is the evidence that the
only intended change is the one this line makes — not that the digest
stays fixed (inverted from RFC-0.35-004's rule, as R10 states explicitly).

## R3 — three availability flags, each adjacent to its field

`measurement_head_available`, `keyring_anchor_epoch_available`,
`last_attestation_available` — each declared next to the field it
qualifies in the struct, and written next to it in `write_canonical`
(flag, then value — the same order `audit_available`/`audit_event_count`
already used). Each carries a `c.note(...)` read by the generated file,
word for word: *"fixed width: N bytes written whether or not
`<flag>` is 1; zero when unavailable, not omitted and not the caller's raw
value."*

**The zeroing is enforced by the encoder, not trusted to the caller.**
`write_canonical` writes `0`/`[0u8; N]` for a field whenever its flag is
`false`, regardless of what the struct's raw field holds —
`unavailable_fields_are_zeroed_not_the_callers_raw_value` proves this
directly: two bundles differing only in the *raw* (never-hashed) value of
an unavailable field hash identically. This closes the risk the RFC itself
named: *"an availability flag can become its own lie."*

**Only `measurement_head_available` can ever be `true` today.** It is set
exclusively by `query_measurement_head`'s own return value — the code
that obtains the value is the only code that sets the flag (trap 4).
`keyring_anchor_epoch_available` and `last_attestation_available` are
permanently `false`: `BundleBuilder::new` has no parameter for either
underlying value any more (see R4), so there is no path by which either
flag could be set `true` without a constructor change a future line would
have to make deliberately.

## R4 — the fabricated intent deleted, and its absence is correct

`build_bundle` no longer calls `add_intent`. `semantic_intent_count` is
`0` on every bundle this crate can build today;
`audit_available: false` beside an empty audit list already establishes
the "none" convention this reuses. No flag, no placeholder — R4's own
instruction, followed literally.

**Resolved more structurally than a deletion alone:** `last_attestation`
and `keyring_anchor_epoch` are removed as `BundleBuilder::new` parameters
entirely, not merely zeroed by convention. Nothing in this crate or
`fjell-diagnosticsd` has ever obtained either value, so a constructor
that still accepted them would only ever receive a literal standing in for
state that should vary — the exact defect this line exists to close. A
future line that gives this service a real path to either value adds the
parameter back alongside the capability to fill it honestly.

## R5 — `DIAG_BUNDLE_VERSION` 2 → 3, propagated everywhere it needs to be

- `golden.rs`'s stored digest updated (`f0d00a58…` → `06332d60…`), with a
  note citing the bump, following this file's own header requirement.
- `bundle-v1.frozen` regenerated via `cargo xtask schema dump` — header
  reads `v3 wire` from the crate's own constant (RFC-0.35-002's mechanism),
  with no edit to the registry's `format_version` wiring needed.
- **The other eighteen generated files, confirmed unchanged** by the same
  `schema dump` run (`frozen.rs`: 16/16).
- `registry.rs` gained a third dated note on `bundle-v1`'s entry, same
  style as the two before it.

## R6 — both readiness sends, the observable, and the tier

Both services now send `fjell_service_api::tags::SERVICE_READY` on
`fjell_service_api::ready::SERVICE_READY_SEND_SLOT`, matching the nine
services that already did (`fjell-measuredd`'s `send_ready` is the
precedent copied). Their own `CAP_SMGR_EP` constants are deleted — not
renumbered: `capability-topology`'s waivers and E-076/E-078's own prose
cite the *remaining* slots (`diagnosticsd` 1/2/4) by number, so those stay
exactly as they were; only the dead slot (`diagnosticsd` 0,
`secure-transportd` 1) is removed, and slot 0 was never discovered or
waived in the first place (`capability_topology::discover_slots` excludes
every service's own identity slot 0 unconditionally — confirmed by
reading the function, not assumed from the prior waiver list's silence
about it).

**The observable, the part R6 calls out as the real cost:**
`NEG:SVC:FORMERLY_SILENT_READY_OBSERVED:PASS`, emitted by
`service-manager` only once its own endpoint has received
`SERVICE_READY` from **both** `ImageId::DIAGNOSTICSD` and
`ImageId::SECURE_TRANSPORTD` — a per-service fact `READY_ACCEPTED`'s
aggregate threshold cannot establish, since neither service was ever one
of the eight that threshold counted. Asserted in
`tests/qemu/profiles/svc.toml`'s `expected_markers`; confirmed live in the
serial log, in order:

```
secure-transportd: ready
diagnosticsd: ready
NEG:SVC:FORMERLY_SILENT_READY_OBSERVED:PASS
NEG:SVC:READY_ACCEPTED:PASS
```

**`READY_ACCEPTED_THRESHOLD` raised 8 → 10.** Its own comment named the
prior 8 as *"every image in this project that currently sends
`tags::SERVICE_READY` at all"* — once two more genuinely do, leaving the
constant and its comment at 8 would have been exactly the stale-claim
defect this register polices, introduced by this line if left uncorrected.
Confirmed reachable: `qemu-negative svc` still passes with the higher
threshold, since both newly-correct services send before the deadline.

## R7 — `capability-topology`'s waivers, shrunk and re-pointed

`secure-transportd`'s slot-1 waiver is removed outright: the declaration
it named no longer exists, so a waiver for it would be dead weight (the
exact "a waiver citing nothing" shape review found last milestone, in a
different form — here the *slot* is gone, not just the reason).
`diagnosticsd`'s slot-4 (measuredd) waiver now names **E-078**, not
E-076 — E-076 closes in this line, and a waiver citing a closed entry is
the defect R7 names explicitly.

Discovered-slot count: **6 → 5** (confirmed:
`discovery_on_the_current_tree_matches_the_known_five`, renamed from
`_known_six` — the function's own name would otherwise have become a
second stale claim). Live run:

```
capability-topology: note — DIAGNOSTICSD slot 1 (tools) ...: decided by RFC-0.35-002 (E-068 closed) ...
capability-topology: note — DIAGNOSTICSD slot 2 (secure-transportd) ...: E-069 closed ...
capability-topology: note — DIAGNOSTICSD slot 4 (measuredd) ...: E-078 — nothing can ask diagnosticsd for a bundle at all ...
capability-topology: PASS (2 installed, 3 waived to a named line, 5 checked)
```

## R8 — E-075, E-076, E-079, E-080 CLOSED; E-078 untouched and visibly open

All four dated CLOSED blockquotes name exactly what closed them, in
`rfcs/ERRATA.md`; summary-table rows moved ACCEPTED → CLOSED. E-078's own
entry is untouched in substance — still ACCEPTED, unscheduled, still
reads "every path is dead" for the parts that are still true. One clause
inside it (*"its readiness announcement reaches its own endpoint —
E-075"*) was now false the moment E-075 closed, and is struck through
with a dated correction rather than silently left wrong, naming which
symptoms are unchanged. `v1-limitations.md`'s four bullets rewritten in
the same commit, same shape as every prior closure this milestone;
`docs/src/releasing/v1-limitations.md`'s own `capability-topology`
paragraph (the RFC-0.35-001 historical bullet, not E-078's) gets the same
correction treatment for its now-stale slot counts and "two readiness
notifications... untouched by either line" sentence.

**The register does not let four closures read as a working diagnostics
path.** E-078's own text, unchanged: *"this does not make the service
work, and a reader of a release that closes them should not be able to
conclude it did."*

## R9 — the gates, the rebuild, the baseline

Guest code changed in exactly three services; exactly those three
prebuilt binaries moved (`fjell-diagnosticsd`, `fjell-secure-transportd`,
`fjell-service-manager`). Rebuilt before re-recording (trap 2):

```
cargo xtask build                                          # exit 0
rm tests/repro/baseline-digests.txt
cargo run -p fjell-repro-check -- --skip-build --record-baseline
cargo run -p fjell-repro-check -- --skip-build              # PASS, 32 identical
```

| | |
|---|---|
| `cargo fmt --all --check` | **exit 0** |
| `abi-snapshot --verify` | **PASS**, 454 → 455, one addition (`fjell-service-api::negative_markers::SVC_FORMERLY_SILENT_READY_OBSERVED`) — `BundleBuilder::new`'s signature change does not register: `fjell-diag-format` is not in the scanned stable-surface set |
| `release-rehearsal` | **exit 0 — ALL MECHANICAL GATES PASS** |
| `consistency-check --all` | **exit 0 — PASS, 20 subchecks**; `capability-topology: PASS (2 installed, 3 waived, 5 checked)` |
| `cargo test -p fjell-diag-format` | **19 passed, 0 failed** |
| `cargo test -p fjell-schema --test golden --test frozen` | **18 + 16 passed, 0 failed** |
| `cargo test -p fjell-consistency-check` | **207 passed, 0 failed** |
| `test-all` | **31 tiers, 30 PASS, 0 FAIL, 1 NOT-GATED** (`store`, pre-existing). Run `tests/runs/20261015-053337` |
| `repro-check` | `--skip-build` **PASS, 32 artefacts identical** |
| CI run id | none — nothing pushed |

## Decisions made along the way

1. **`keyring_anchor_epoch` and `last_attestation` dropped as constructor
   parameters, rather than kept and merely zeroed by convention.** A
   parameter nobody can fill honestly is worse than no parameter — this
   makes the fabrication structurally unrepeatable, not just inert.
2. **The zeroing is the encoder's own responsibility** (`write_canonical`
   checks each flag and substitutes `0`/zeroed bytes itself), not the
   struct's raw field value, so a future bug that sets a flag `false`
   without also zeroing the field cannot leak the stale value into the
   digest.
3. **`bundle_id` and `provider_id` are not given availability treatment.**
   Both are compile-time identity constants, the same convention every
   service in this project uses for its own name — confirmed by the
   architect's own ruling, not asserted unilaterally.
4. **The remaining CSpace slot numbers are preserved exactly**, even
   though removing `CAP_SMGR_EP` could have let them be renumbered from 0:
   every piece of documentation this milestone wrote cites them by number.

## Afterword

R1's own escalation path was exercised for real this line, not only
specified: a fifth finding was found, no code was written against it, and
the finding was handed up rather than resolved underfoot. The ruling that
came back split it correctly — one half absorbed into the line already in
flight, one half (E-080) that turned out to be the more serious finding of
the two, folded in for a reason specific to this line's own mechanics
(the version bump and goldens cycle already in motion) rather than general
convenience.
