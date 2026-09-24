# ADR-v0.6-003 — Format Fuzzing and Frozen Schema Registry

**Status:** Accepted  
**Date:** 2026-05-19 (v0.6.0, RFC v0.6-003)

## Context

Binary format parsers are historically the richest source of exploitable bugs.
Fjell OS has 8 critical formats that cross service boundaries and survive across
reboots.

## Decision

A `fuzz/` directory using `cargo +nightly fuzz` contains 8 targets. Each target
verifies that the parser never panics on arbitrary input and that parse → serialize
→ parse produces an identical result.

Frozen schema files lock field layouts. Any layout change must be accompanied by
a BREAKING-SCHEMA commit, a schema version bump, and an ADR — enforced by CI.

Fuzzing runs nightly with the seeded corpora as starting points.

> **Correction, 2026-09-15 (E-043).** The harness described here has never
> run. The CI job is weekly, not nightly, and runs only on its schedule —
> never on push or pull request — so it could not catch a regression "before
> merge" even when working. It has failed every week since it was added:
> `fuzz/` is misconfigured as a workspace member, its dependency paths broke in
> the July 2026 crate reorganisation, and five of the eight targets call parser
> functions that never existed *(this said six, and "no longer exist", until
> RFC-0.32-001 re-derived both)*. The decision stands; the consequences below
> have not held.

> **Resolved, 2026-09-15 (RFC-0.32-001; E-043 CLOSED).** The decision now
> holds, with three differences stated rather than smoothed over.
>
> - **The harness targets decoders, not formats.** Of the eight formats this
>   ADR counted, most have no byte decoder at all, so a target per format
>   would fuzz nothing — five of the original targets called functions that
>   never existed. There are now six targets, one per real byte decoder a host
>   fuzz crate can reach.
> - **Only `revocation_record_parse` checks parse → serialize → parse**,
>   because only revocation records have an encoder to round-trip through.
>   The other five check that decoding never panics on any input.
> - **Fuzzing runs weekly and on demand, not nightly** — the `fuzz-run` job,
>   each target for 300 seconds from committed, verified seeds. Run
>   `34976532420` fuzzed all six on the fixed, unreleased tree (`058c586`) *(corrected at review: this said "the released decoders")*:
>
>   | Target | libFuzzer |
>   |---|---|
>   | `semantic_record_parse` | `Done 164962007 runs in 301 second(s)` |
>   | `revocation_record_parse` | `Done 204905946 runs in 301 second(s)` |
>   | `audit_record_parse` | `Done 294673929 runs in 301 second(s)` |
>   | `dtb_derive_board_profile` | `Done 10721172 runs in 301 second(s)` |
>   | `dtb_validate` | `Done 20054742 runs in 301 second(s)` |
>   | `cap_manifest_parse` | `Done 9974999 runs in 301 second(s)` |
>
> The first run of the device-tree target found a real crash in
> `fjell-dtb-derive` within 30 seconds (E-047, fixed); its input is now a
> permanent regression seed.
>
> *(Updated 2026-09-25, RFC-0.33-005: `fjell-dtb-derive`, its target
> `dtb_derive_board_profile` — the row above — and that seed were deleted; see E-047
> and E-048. This table is the record of what run `34976532420` fuzzed.)*

## Consequences

- Format regressions that cause parser panics are caught before merge.
- Schema drift (accidental field reorder, size change) is caught per-PR.
- The frozen schema files serve as authoritative wire-format documentation.

> **Correction, 2026-09-15 (RFC-0.32-001).** The first consequence — parser panics caught before merge — is now true **for inputs already
> in the corpus, and not for new ones.** On every push and pull request,
> `fuzz-build` builds every target and replays every committed seed, so a
> target that stops compiling, or a decoder that starts panicking on a known
> input, is caught before merge. A panic on an input nobody has seen yet is
> found only by the weekly or dispatched `fuzz-run`, after merge.

> **Correction, 2026-09-15 (E-045).** Neither consequence about schemas has
> held. `ci-schema-gate` checks only that the frozen files exist and are not
> empty; the `fjell-tools schema dump` generator and the comparison test that
> RFC-v0.6-003 specified were never built; and in both formats checked, the
> frozen description no longer matches the code, with no schema version bumped.

> **Correction, 2026-09-24 (RFC-0.33-003).** The two schema consequences now hold,
> in a narrower form than they were first written. The frozen files are
> **generated** by `cargo xtask schema dump` from the functions that produce each
> format's bytes — not by a generator that knows the fields, which would have been
> a second description — and a test in Gate 1 fails, naming the field, when a
> committed file differs from what its encoder writes; `ci-schema-gate` was
> retired in its favour. So an accidental field reorder, re-widthing or rename is
> caught per-PR **for the seventeen formats that have a file**. It does not enforce
> the BREAKING-SCHEMA commit, the version bump or the ADR: the regenerated file's
> diff makes the change visible to the reviewer, and the process consequence stays
> a review obligation. Five format crates that produce bytes have no generated
> description yet (Errata E-065).
