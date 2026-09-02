# `tests/evidence/`

RFC-0.27-004. The one place in this project's tree where a QEMU serial log
is deliberately committed — because a document cited it, not because a run
happened to produce it.

## Why this exists

`fjell-kernel` has no `[lib]` target (Errata E-013), so nothing kernel-side
is host-testable. Every kernel-side claim this project makes rests on a
QEMU serial log — and until this RFC, none of those logs ever survived past
the run that produced them: `tests/qemu/artifacts/<profile>/serial.log` is
overwritten by the next run of that profile, and `tests/runs/<timestamp>/`
(also gitignored) captures build stdout, not the serial transcript. Every
QEMU-evidenced claim in this project was, by construction, unverifiable
from a clone (Errata E-026).

## How a log gets here

Never automatically. `cargo xtask evidence promote` is a deliberate act
with required arguments — there is no default source, no default
`--instrumented` answer, and no code path from `test-all` or
`release-rehearsal` that populates this directory as a side effect. If you
find a way running the ordinary test suite lands a file here, that is a
bug in this design, not a feature of it.

## Layout

```
tests/evidence/<rfc-id>/<name>.log               the promoted serial log, byte-for-byte
tests/evidence/<rfc-id>/<name>.provenance.txt    run_id, profile, commit_sha, command, instrumented
```

## What committing a log here does and does not prove

**It proves a run produced that output.** Nothing more.

**It does not prove the citing document's reading of the log is right.**
RFC-0.27-002's first submission read its own trace backwards — the log
itself would not have caught that; only re-examining the *mechanism* did.
A committed log looks authoritative the way a green gate looks
authoritative, and is owed the same scepticism: check what the citing
document actually derives from it, not just that the file exists.

**It does not prove the log can be reproduced from the commit it cites.**
Read the `instrumented` field in the provenance sidecar before trusting
`commit_sha` to mean "checkout this commit and re-run to get this exact
output." A log produced by a build carrying temporary diagnostics that were
later reverted is real evidence and cannot be regenerated from the commit
alone — the provenance says so explicitly rather than leaving a reader to
discover it by trying and failing.

**The `evidence` consistency-check subcheck verifies that a citation
resolves to a file with well-formed provenance whose commit sha is a real
ancestor of `HEAD`.** It does not and cannot verify that the provenance is
*honest* — a hand-edited sidecar claiming a real ancestor commit but a
fabricated `instrumented` field would pass. This is the same weak predicate
RFC-0.27-003's `standards-mapping` subcheck discloses about itself: the
tooling checks structure, not truth. See
[`docs/src/release/v0-release-cycle.md`](../../docs/src/release/v0-release-cycle.md)
for what a human must still re-check by hand at each release cut.

## Historical citations that predate this directory

RFC-0.27-004 R6 reconciled the existing `.log` citations this project had
already made. Some were promotable (real logs, still on disk); others were
not — their originating logs are gone, overwritten by later runs, and
**were not re-produced to stand in for them** (D4: re-manufacturing a lost
log is a forgery, not a fix). The unresolvable ones are annotated in place
in their citing documents and tracked to resolution in
[`docs/rfcs/ERRATA.md`](../../docs/rfcs/ERRATA.md) — see RFC-0.27-004's own
review request for the count and the reasoning behind each disposition.
