# RFC-0.32-001: A fuzz harness that has never run, aimed mostly at formats with nothing to parse

**Status:** Accepted — by the owner (nabbisen), 2026-09-15; implementation may begin (RFC 000)
**Milestone:** 0.32
**Tracks.** **E-043** — the fuzz harness has never run, and the release process
cannot see the job that would show it.
**Touches.** `fuzz/`, the root `Cargo.toml` (workspace membership),
`.github/workflows/ci.yml` (the fuzz job and its triggers),
`docs/src/release/v0-release-cycle.md` and `docs/src/release/release-handoff.md`
(exit criterion 9), `docs/src/adr/ADR-v0.6-003-format-fuzzing.md`,
`docs/src/release/v1-readiness.md`. **Does not touch kernel, ABI or service
source.**
**Relates to:** E-046 (the envelope receive path — not fuzzed here, see D7);
E-045 (the same originating RFC's other mechanism, tracked 0.33); E-041 (whose
final census counted this job's "skipped" runs as passing); RFC-v0.6-003 (which specified
the harness and was marked Implemented); RFC-0.31-002 (the rule that a CI claim
needs a run id).

## Summary

Everything below was observed with a run id, a scratch-clone probe, or `git
log`, not read from `ci.yml` or the ADR.

### Finding 1 — three stacked defects, and the harness never compiled

Every scheduled run of `ci-fuzz-nightly` since the harness was added
(`e63d19f`, 2026-06-06) has failed, on all eight targets. The latest,
`34829212731` on 2026-09-14, ran against the released 0.31.0 tree and turned
the README badge red. Each defect hid the next:

| # | Defect | Since |
|---|---|---|
| 1 | `fuzz/` is neither a workspace member nor excluded; cargo refuses to build it | `e63d19f`, 2026-06-06 |
| 2 | its dependency paths point at `../crates/<name>`; the format crates moved to `crates/formats/` | `a5b5167`, 2026-07-23 |
| 3 | six of eight targets do not compile; five call functions that **never existed** | the harness's creation |

*Corrected by the implementation (R1), recorded at review: **five** of eight
did not compile — `board_profile_parse` builds once defects 1 and 2 are
repaired, and the five failures are exactly the five functions that never
existed. And not every scheduled run failed on these three defects: the runs
of 2026-07-27 and 2026-08-24 (both at `891a1ec`) failed earlier, on a root
`Cargo.toml` with unterminated strings, fixed in `f3519dc`.*

Defects 2 and 3 were found by repairing 1, then 2, in a scratch clone. At
`e63d19f` none of `v2::parse_record`, `release_metadata::parse`, the module
`upgrade_format::rollback`, `diag_format::parse_bundle` or
`keyring::snapshot::parse` existed. **This harness was never a working harness
that rotted.** It is `fjell-identityd`'s shape (E-042): code written against an
API that was never there, recorded as done.

### Finding 2 — of eight targets, one fuzzes a decoder

| Target | What it actually does |
|---|---|
| `semantic_record_parse` | **fuzzes `fjell_semantic_v1::decode`** — the only real target |
| `update_index_parse` | `let _ = data.len();` — compiles because it tests nothing |
| `board_profile_parse` | decodes no input; checks that a digest is deterministic |
| `attestation_v2_parse`, `release_metadata_parse`, `rollback_record_parse`, `keyring_snapshot_parse`, `diagnostic_bundle_parse` | call functions that never existed |

And the formats most targets were named after mostly **have no byte decoder at
all** — attestation v2, release metadata, rollback records and the diagnostic
bundle expose constructors, digests and `sign`/`verify`, nothing that turns
bytes into structure. The `update-index` format RFC-v0.6-003 listed never
existed as a crate. Repairing the eight targets as written would fuzz almost
nothing.

### Finding 3 — the decoders that do exist are not fuzzed

Public functions that decode a `&[u8]` into structure, from a first inventory:

| Decoder | Input comes from | Fuzzed |
|---|---|---|
| `fjell_semantic_v1::decode` | encoded intent envelopes | **yes** |
| `fjell_keyring::revocation::…::from_bytes` | revocation records | no |
| `fjell_audit_format::AuditRecordBin::from_bytes` | the kernel's audit ring, via `fjell-auditd` | no |
| `fjell_dtb_derive::parse_header` | a device tree — firmware-supplied on real hardware; no dependent crate today | no |
| `fjell-proxy-text`'s `ingest` | encoded envelopes from other services | no |
| `fjell_service_api::chunked::reassemble` | IPC chunks from another service | no — **unsound as written, E-046** |

**The inventory is incomplete by construction**: it matched public functions
with a single-line signature. It found no decoder for the boot-control block or
the store's records, because those paths reinterpret struct memory rather than
parse bytes (E-046). R1 re-derives it properly.

### Finding 4 — the job could not have worked even if it compiled

- The seed corpora live in `fuzz/corpora/<name>/`; `cargo fuzz run` is given no
  corpus argument, so every run would start empty.
- It installs a floating `nightly` and `cargo install cargo-fuzz` unpinned on
  every run.
- It runs **only on schedule** (`if: github.event_name == 'schedule'`), never on
  push or pull request. ADR-v0.6-003's *"format regressions … are caught before
  merge"* could not have held.

### Finding 5 — three instruments, one blind spot

The instrument audit recorded the job **UNAUDITED** because it is
schedule-only. E-041 closed on push runs in which it was **skipped**, reading
skipped as passing. Exit criterion 9 reads the release commit's **push** run,
so a schedule-only job is structurally outside every cut — 0.31.0 shipped with
it red. And the README badge shows the most recent run of **any** event:
after the 2026-09-14 scheduled run it read `failing`; after the next push
(`eb71cbe`, run `34951074576`) it read `passing` again. **It is red from each
Monday's scheduled run until the next push, and green the rest of the week** —
while every cut reads a push run. A reader who looks on any other day sees
green.

## The settled part

**D1 — The harness builds.** Out of the workspace (`exclude`, or its own
`[workspace]` table), with correct dependency paths.

**D2 — A target must exercise a real decoder of bytes that can reach it from
outside the decoding component.** `update_index_parse` and
`board_profile_parse` are retired, not kept as placeholders. The five targets
written against functions that never existed are retired unless a real decoder
for that format exists. **No target is written for a format that has no
decoder, and no decoder is written so that a target can exist.** The measure of
this line is decoders exercised, not target files.

**D3 — The seed corpora are used.** Passed to the run, and a grown corpus is
either kept or its loss is a stated choice.

**D4 — Green is observed.** A real fuzz run, per target, with a run id. A green
build is not a green fuzz run.

**D5 — The job can be triggered on demand.** `workflow_dispatch`, admitted by
the job's `if:`, so a fix is proven the day it lands rather than the following
Monday. The schedule stays.

**D6 — The release cycle reads scheduled runs.** Exit criterion 9 extends to
record the latest scheduled run's per-job conclusion alongside the release
commit's push run. A red scheduled job blocks the tag or takes an accepted-risk
statement, under the existing rule.

**D7 — `reassemble` is not fuzzed in this line.** Fuzzing a path that is
undefined behaviour by construction would only rediscover E-046. Its target
belongs to E-046's line, written against the fixed decoder.

**D8 — Demonstrated failing** (RFC-v0.22-001): a target that crashes on a
crafted input turns the job red on a real run, and the on-demand trigger is
shown working. Otherwise a green fuzz job is indistinguishable from one that
cannot fail.

**D9 — The README badge is not filtered to push events.** That would hide the
job, not fix it.

## The open questions — §7 and §8

**§7 — Should fuzzing also run on push, and how much?** The harness rotted for
three months because nothing ever compiled it on the path where changes land.

1. **Schedule and dispatch only** (D5 plus the status quo). Cheapest. The next
   API change that breaks a target is again invisible until Monday — and D6
   makes that a cut-time surprise instead of a merge-time one.
2. **A build-only job on push** — `cargo fuzz build` for every target, running
   none — alongside scheduled runs. Catches all three of Finding 1's defects the
   moment they are introduced, for the cost of one sanitizer build.
3. **A short fuzz run on push** (tens of seconds per target) as well as the
   scheduled long runs. Exercises the decoders on every change; costs runner
   time on every push, and a short run finds little.

**I lean to 2**, and name its hazard myself: a green *build* job is the easiest
thing in this project to read as "fuzzing works" — E-041 read a *skipped* job
that way. If shape 2 is chosen, its name and its record entry must say build,
not fuzz.

**§8 — What toolchain does the fuzz job use?** Fuzzing needs nightly for its
sanitizers, and a floating nightly can break the job with no change to the tree
— E-037's drift, on the one job nobody reads. The alternative is a dated nightly
declared in one place, which then needs its own currency step like exit
criterion 10, and `toolchain-declarations` currently excludes `cargo +<name>`
jobs by design. Argue whether the pin is worth that machinery for a job that
gates only through D6.

**Answer both in writing before implementing.**

## Requirements

**R1 — Re-derive before changing anything.** Reproduce each stacked defect on a
clean tree. Re-derive the decoder inventory completely — private decoders,
`TryFrom` implementations, multi-line signatures, and raw reinterpretations —
and classify each by where its input comes from and whether a fuzz crate can
reach it. Report every disagreement with this RFC's tables, in either
direction.

**R2 — D1.**

**R3 — The target set**, decided under D2 from R1's inventory: a table of kept,
retired and added targets, each with its reason. D3 wired.

**R4 — The CI job:** D5, and §7 and §8 built as answered.

**R5 — D8's demonstrations**, on real runs, with run ids.

**R6 — D4.** An observed green run per kept target, with the run id. **This is
E-043's closure condition.**

**R7 — D6**, in both the cycle document and the standing handoff, with the
record shape.

**R8 — Every claim E-043 names made true or corrected.** ADR-v0.6-003's
consequences, and `v1-readiness.md`'s row. That row's criterion — "Fuzz targets
(≥ 4)" — is itself a weak predicate: it was satisfied by eight target files,
five of which never compiled. Replace the count with a statement about decoders
exercised, or say why a count is right.

**R9 — E-043 → CLOSED**, or its survivors named; register and
`v1-limitations.md` in the same commit.

### Non-goals

- **E-046**: making `reassemble` or the checksum paths sound, or fuzzing them
  (D7).
- **E-045**: the frozen schemas (tracked 0.33).
- Restructuring a service crate so the fuzz crate can reach its decoder. If a
  decoder is unreachable, record it.
- Adding byte decoders to formats that have none.
- The advisory gate.

## Risks

**Retiring most of the targets will look like losing coverage.** It removes the
appearance of coverage. The number of decoders actually exercised goes from one
to however many R1 finds — which is the only number that was ever meaningful.

**A real fuzz run may find real crashes** in decoders nobody has fuzzed before.
That is the point. A crash is a finding: record it as an erratum; fix it here
only if the fix is inside the decoder and small, and stop and escalate
otherwise.

**A build-only job read as fuzzing** (§7, shape 2) is exactly how this erratum
happened one level up. Name it for what it proves.
