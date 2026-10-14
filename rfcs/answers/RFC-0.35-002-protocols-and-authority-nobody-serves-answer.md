# RFC-0.35-002 R1–R8, the hang demonstrated and gone, and E-074 closed alongside

**Governing RFC:** [../accepted/RFC-0.35-002-protocols-and-authority-nobody-serves.md](../accepted/RFC-0.35-002-protocols-and-authority-nobody-serves.md)
**Handoff:** [../handoffs/RFC-0.35-002-protocols-and-authority-nobody-serves/implementation-handoff.md](../handoffs/RFC-0.35-002-protocols-and-authority-nobody-serves/implementation-handoff.md)

Written in the order the handoff requires: R1 and the reachability question
first, R6's first half (the hang) before R2 lands, everything after R5
ordinary. The handoff's own checklist predates RFC-0.35-001's first review,
which added **E-074** to this line's scope (the governing RFC's header, R7
and Non-goals all name it; the handoff does not, because it was written
before that addition landed) — not a conflict with the handoff, a scope
growth in the RFC it says is authoritative. Closed here, alongside E-068
and E-069, for the same reason RFC-0.35-001's own review argued against:
a closed entry should not carry a residual it could have resolved.

---

## R1 — Re-derivation at the current tip, with controls, including the reachability question

| Finding | RFC's claim | What the tree says |
|---|---|---|
| E-068 declaration | `fjell-service-api::diagnosticsd` declares `AUDIT_QUERY`/`AUDIT_RECORD`/`AUDIT_STREAM_END`; `diagnosticsd` sends and loops | **Confirmed.** Word-boundary search, both files: 12 exact occurrences total (4 per constant — 1 in `fjell-service-api`, 3 in `diagnosticsd`), zero elsewhere in the live tree. |
| `auditd` implements nothing | `fjell-auditd` matches `AUDIT_QUERY` zero times | **Confirmed**, word-boundary. Control: `diagnosticsd` matches **three** times (`:51` doc comment, `:97` wire-protocol doc, `:108` the real call), not four as the RFC's own derivation said — re-derived, not assumed; the discrepancy does not change the finding. |
| Reachability — does anything send `BUILD_BUNDLE`/`PUSH`? | No — two sites each (declaration, `diagnosticsd`'s own comparison) | **Confirmed**, tree-wide word-boundary search, both constants: exactly two sites each, neither a caller. No escalation needed. |
| E-069 declaration | `transportd` declares four endpoints, `recv_msg` has one call site | **Confirmed.** `CAP_SMGR_EP`(1)/`CAP_UPGRAD_EP`(2)/`CAP_DIAG_EP`(3)/`CAP_ATTEST_EP`(4); `recv_msg(` called once, on `CAP_UPGRAD_EP`. |
| "Three client endpoints" | `:131`'s comment names three polled in round-robin | **Confirmed as a comment claim; re-derived as to what is actually unserved.** Of the three "client" endpoints the comment names (`upgraded`/`diagnosticsd`/`attestd` — `CAP_SMGR_EP` is readiness, not a client endpoint, and was never part of this claim), **two** are unserved (`CAP_DIAG_EP`, `CAP_ATTEST_EP`); `CAP_UPGRAD_EP` is the one real receive. R4's own wording ("the three unserved endpoint constants go") overstates by one — two constants are deleted, not three; `CAP_SMGR_EP` and `CAP_UPGRAD_EP` both stay, both used. |

Both of R1's required checks held: no in-tree sender reaches `BUILD_BUNDLE`
or `PUSH`, so the severity does not change and this did not need escalation.

## R6 (first half) — the hang, demonstrated, before any fix

**Not an absence — a real send, never answered.** Scratch worktree,
pre-fix tip: `spawn.rs` temporarily installs a shared endpoint object
`neg-test` can send to and `diagnosticsd` can receive on at its
otherwise-uninstalled `CAP_TOOLS_EP` (slot 1) — exactly the capability
`RFC-0.35-001`'s R5 and this line's own non-goals forbid installing for
real, installed only in a worktree never committed. `neg-test`, after every
other scenario completes (so `diagnosticsd` has long since booted), sends
`BUILD_BUNDLE` and blocks on a reply.

Result: `SCRATCH: calling diagnosticsd BUILD_BUNDLE` prints and nothing
after it ever does. The run's own 30-second timeout kills the QEMU
process — not a crash, not an early exit, the harness's own wrapper
(`qemu-system-riscv64: terminating on signal 15 ... (timeout)`). The rest of
the machine keeps running (`TEST:M8:PASS` and the other smoke markers print
normally) — one task stuck, not a frozen machine, the same shape E-072
demonstrated for the busy-spin. Full transcript (not committed, scratch,
cited by path):
`target/scratch/rfc-0.35-002-evidence/hang-before-fix.log`.

## R2 — The audit-query sub-protocol is retracted

`AUDIT_QUERY`/`AUDIT_RECORD`/`AUDIT_STREAM_END` are deleted from
`fjell-service-api::diagnosticsd` (`crates/fjell-service-api/src/lib.rs`).
`diagnosticsd`'s `collect_audit_events` — the function that sent the query
and looped until a reply tag `auditd` never sends — is deleted entirely,
along with its declared `CAP_AUDITD_EP` (slot 3): a slot for a retracted
protocol is standing authority nobody exercises, the same least-privilege
argument the governing RFC's ruling makes (reason 3).

**The distinction a caller needs, stated, not inferred from a count.**
`fjell-diag-format::bundle::DiagnosticBundle` gains `audit_available: bool`
(`crates/formats/fjell-diag-format/src/bundle.rs:35`), defaulting `false` in
`zeroed()` (`:98`) and encoded in `write_canonical` (`:120`, `c.u8` — this
crate has no `bool` encoder; `self.audit_available as u8` matches the
precedent other format crates already use for a boolean field). `diagnosticsd`
never sets it `true` — nothing implements the query, so nothing earns the
`true` state — and `build_bundle` no longer calls the deleted function at
all. A caller reading `audit_available == false` now knows "not
implemented," not "queried, zero records." `fjell-diag-format`'s own
digest-covered field list and buffer-size comment are updated
(`crates/formats/fjell-diag-format/src/builder.rs`); the frozen schema is
regenerated (`cargo xtask schema dump`; `fjell_consistency_check::frozen`
and `fjell-diag-format`'s own 15 unit tests pass).

**Caught by `test-all` itself, not a reviewer: a new field is a wire-format
change, and this crate has a gate for exactly that.**
`crates/fjell-schema/tests/golden.rs` stores a digest of each sample's
canonical byte stream and refuses, by its own header's instruction, to
have that digest "regenerated to make a change pass" — a changed digest
must come with a version bump and a note, or the test is checking nothing.
Tier 2 (`cargo test -p fjell-schema --test golden`) failed exactly this
way on first `test-all` run. Fixed properly, not by silently updating the
stored hash: `DIAG_BUNDLE_VERSION` bumped `1 -> 2`
(`crates/formats/fjell-diag-format/src/bundle.rs`), `golden.rs`'s
`diag_bundle` test carries the new digest and a note citing the bump — no
new domain separator, unlike `fjell-attestation-format`'s V1/V2 split,
since this bundle's digest is never cross-verified against a differently
versioned one the way a signature is.

## R3 — The retraction leaves a record, not a hole

`rfcs/ERRATA.md`'s E-068 entry carries a **CLOSED** blockquote naming what
was retracted, what the bundle now states instead, and — explicitly —
**what designing the query for real requires**: a capability decision (who
may read the audit log, which right, what badge, revocable how, observed by
which tier) that has not been made, quoting the governing RFC's own ruling
§1. `docs/src/releasing/v1-limitations.md` carries the same record in the
same commit, including the `audit_available` mechanism a reader of the
limitations page — not only the errata register — will meet. A reader who
finds the deleted constants finds why, and finds the open question, not a
silent absence.

## R4 — `transportd` declares only what it uses

`CAP_DIAG_EP` and `CAP_ATTEST_EP` are deleted
(`crates/services/fjell-secure-transportd/src/main.rs`); `CAP_SMGR_EP` and
`CAP_UPGRAD_EP` stay, both used. The CSpace-layout comment above the
declarations and the top-of-file module doc are corrected to match. The
`:165`–`168` loop comment (was `:131` at the RFC's own tip; line numbers
moved as the file grew) no longer claims a round-robin:

```
// RFC-0.35-002 (closes E-069): this used to say "we poll the three
// client endpoints in round-robin." There was no round-robin — one
// endpoint, `CAP_UPGRAD_EP`, was ever served; `diagnosticsd`'s and
// `attestd`'s are gone (above). This loop receives on the one endpoint
// this service actually speaks to.
```

Nothing here installs a capability — `spawn.rs` is untouched by this
section of the line, matching RFC-0.35-001 R5's exclusion and this line's
own non-goal.

## R5 — No loop spins on absence, and a recv error is distinguishable from an unexpected tag

**Both `recv_msg` helpers now return `Result<_, ()>`**, not a tuple that
collapses an error into a sentinel a real message could coincidentally
produce (`crates/services/fjell-diagnosticsd/src/main.rs`,
`crates/services/fjell-secure-transportd/src/main.rs`). `sys_yield()` stays
in the shared `Err` arm, unconditionally, for every call site, not only the
service's own main loop.

**`diagnosticsd`'s chronic case — `CAP_TOOLS_EP` errors on every iteration,
by design, for the life of every boot** (RFC-0.35-001 R5 waived installing
it; this line's own non-goals forbid installing it too). Printing that on
every iteration would flood the console and tell an operator nothing the
first line did not already say, so it is disclosed **once**
(`main.rs:179`–`189`, a loop-local `let mut recv_error_disclosed`, not a
`static mut` — a service's image is mapped `R | X | U`, never `W`
[`fjell-semantic-stream`'s own module doc], so a `static mut` write would
page-fault; a stack-resident loop local does not). An unexpected tag — a
real message this loop does not recognise, not an unchanging condition —
is disclosed **every** time (`main.rs:220`). **Chosen behaviour: yield and
keep listening, not exit.** A future line installing this slot should not
require the service to be respawned to notice, and there is nothing to
block on since the capability does not exist.

**`secure-transportd`'s now-rare case.** `CAP_UPGRAD_EP` is installed
(RFC-0.35-001 R5), so a recv error here is an edge case, not a chronic
condition — disclosed every time it fires (`main.rs:106`), no flood risk
since it is not expected to recur. The loop's own `_ => {}` catch-all for a
tag `SxtTag` does not act on (outside the enum entirely, or one of its own
reply-only variants echoed back) is also disclosed every time
(`main.rs:252`), where it previously discarded the event with no trace.

## R6 (second half) — the hang, gone

Same scratch mechanism, now against the fixed tree: `diagnosticsd` receives
`BUILD_BUNDLE`, and `collect_audit_events` is gone, so `build_bundle`
returns. Full round trip, this time with a plain send/recv on the client
side (not `sys_ipc_call` — `diagnosticsd` replies with a plain `send_tag`,
not `sys_ipc_reply`, so a call's reply edge is the wrong tool and does not
complete; matching the service's own send/recv shape does):

```
SCRATCH: sending diagnosticsd BUILD_BUNDLE
diagnosticsd: bundle built
SCRATCH: diagnosticsd BUILD_BUNDLE replied Ok
```

No timeout kill before the reply; the run's own eventual timeout, after
every scenario including this one completed, is the normal idle-park every
profile without an injected trigger ends in. Full transcript:
`target/scratch/rfc-0.35-002-evidence/hang-gone-after-fix.log`.

## R7 — E-068, E-069 and E-074 CLOSED, with E-068's consequence corrected

**E-068's consequence corrected in the register**, not just reworded:
`rfcs/ERRATA.md`'s entry used to say the query *"returns nothing,
silently."* It now says what R1's re-derivation and R6's demonstration
together show: `collect_audit_events` could not terminate, so
**the first operator-triggered diagnostic bundle hung the service** — a
hang nothing had ever observed only because nothing in the tree could ever
send the trigger.

**E-068 and E-069 CLOSED**, each with a dated blockquote in `rfcs/ERRATA.md`
and a dedicated paragraph in `v1-limitations.md`, in the same commit as the
code.

**E-074 CLOSED alongside, not left ACCEPTED inside this line's own commit
history.** `attestd`'s entire "Remote attestation push" section
(`push_attestation`, `CAP_SXT_EP`, the `sxt_send`/`sxt_recv` helpers only it
called) is deleted — not just the function, everything that existed solely
to support it. `netd`'s `CAP_DRV_EP` is deleted the same way. **Found in
passing, worth recording rather than sidestepped:** `push_attestation`'s own
write to `CACHED_NONCE`, a `static mut`, would have page-faulted the moment
it ran, for the same `R | X | U`, never `W` reason R5's design above relies
on — a second, previously-latent defect this deletion removes rather than
needing to fix, since the dead function this bug lived in had no caller to
ever trigger it.

**Guest bytes, checked not assumed.** A full rebuild after these two
deletions left `fjell-attestd.bin` and `fjell-netd.bin` **byte-for-byte
identical** to the committed prebuilts (`sha256sum` against `git show
HEAD:...`, both files, both match) — the dead code the compiler already
eliminated moved no bytes when the source stopped declaring it, which is
itself evidence the functions really were unreachable, not merely unused
by the measure a grep can take. `diagnosticsd`'s and `secure-transportd`'s
prebuilts did change (their source changed), and the repro baseline is
re-recorded in the same commit as the rebuild (R8, below).

`capability-topology`'s `WAIVERS` list is trimmed to match: the five slots
this line retired (`SECURE_TRANSPORTD` 3/4, `DIAGNOSTICSD` 3, `ATTESTD` 5,
`NETD` 1) are removed from it, not left as stale entries describing
constants that no longer exist. The gate now reports
`PASS (2 installed, 4 waived to a named line, 6 checked)`, not eleven; a
unit test reads the real tree and asserts the new count
(`discovery_on_the_current_tree_matches_the_known_six`).

## R8 — The gates, each by its own exit status

| | |
|---|---|
| `cargo fmt --all --check` | **exit 0** |
| `abi-snapshot --verify` | **PASS**, exit 0 — 453/453 (regenerated after the breaking removal of three protocol constants: 456 → 453, 3 removed, 0 added, 0 changed) |
| `release-rehearsal` | **exit 0 — ALL MECHANICAL GATES PASS**; Gate 1, Gate 4, Gate 12 all `[PASS]` |
| `consistency-check --all` | **exit 0 — PASS, 19 subchecks**; `capability-topology: PASS (2 installed, 4 waived to a named line, 6 checked)`; `errata-limitations: PASS (9 ACCEPTED errata)` |
| `fjell-diag-format`, `fjell-schema` host tests | **15 + 16 + 18 passed, 0 failed** (builder/bundle tests; `frozen.rs`'s `every_committed_file_is_what_its_encoder_generates`; `golden.rs`'s 18, `diag_bundle` among them, after the version bump) |
| `fjell-consistency-check` host tests | **10/10 in `capability_topology`**, including the new six-slot assertion |
| `test-all` | *(below, filled after the background run completes)* |
| repro-check | `--skip-build` **PASS, 32 artefacts identical** after re-recording; exactly 2 digests changed (`fjell-diagnosticsd`, `fjell-secure-transportd`), matching `git status`'s own list; `fjell-attestd`/`fjell-netd` unchanged, confirmed above |
| prebuilt digests, `fjell-attestd`/`fjell-netd` | **unchanged**, `sha256sum` against `git show HEAD:...` — both match exactly |
| CI run id | none — nothing pushed |

## Decisions I am making, so the record states them as decisions

1. **No setter for `audit_available: true`.** Nothing today has a reason to
   set it — the query is not implemented, by this line's own ruling. Adding
   one now would be exactly the "build the architecture around a claim"
   the governing RFC's ruling §1 refuses, aimed at a field instead of a
   protocol. The default (`false`) is honest for everything that exists
   today; a future line implementing the query adds the call that sets it,
   when there is a real reason to.
2. **`diagnosticsd`'s recv-error disclosure is a loop-local `bool`, not a
   richer backoff (exponential delay, a retry counter).** R5 asks for a
   chosen behaviour and a reason; "yield forever, disclose once" is the
   smallest behaviour that satisfies "fail closed and say so" without
   either flooding the console or adding state that would need to survive
   across a respawn to still mean something. A real backoff schedule is
   more machinery than this unchanging, already-disclosed condition needs.
3. **`CAP_TOOLS_EP`, `CAP_SMGR_EP`'s readiness-send wrongness (handoff trap
   5), and `diagnosticsd`'s own slot-0 readiness send remain untouched.**
   All three are RFC-0.35-001's and this line's own non-goals name them
   out of scope; confirmed still true at this tip and left alone.

## Afterword — what survived contact

The handoff's own checklist not mentioning E-074 was the first real
surprise — not a conflict to escalate, since the governing RFC is
unambiguous that E-074 is in scope (added at RFC-0.35-001's review, named
in the header, R7 and the Non-goals), but a reminder that *"the handoff
does not redefine the RFC"* cuts the other way too: a handoff's checklist
being incomplete does not shrink the RFC's own scope.

The scratch demonstration's own debugging was the second: three dead ends
before a working harness (`SERVICE_READY_SEND_SLOT` collided with the first
slot number tried; an unallocated object id produced `InvalidCap` from the
`EndpointTable`, not from the capability itself; `sys_ipc_call`'s reply
edge does not complete against a plain `send_tag` reply) — each resolved by
reading the kernel's own dispatch code rather than guessing at the IPC
model from the service side. None of this is new work *in* the committed
line; all of it stayed in the scratch worktree, reverted and confirmed
empty before the real demonstration ran.

The `fjell-attestd`/`fjell-netd` byte-identical prebuilts were the
pleasant surprise — not assumed, checked, and worth citing as independent
confirmation that E-074's "no caller anywhere" finding from RFC-0.35-001's
own re-derivation was exactly right.

**`test-all` itself caught the fourth, before any review did.** Adding
`audit_available` to `DiagnosticBundle` is a wire-format change, and this
tree already has a gate for exactly that shape of change
(`tests/golden.rs`), one that refuses by its own header's words to have
its stored digest "regenerated to make a change pass." First `test-all`
run failed tier 2 on it. The honest fix was the one the file's own
instruction names — a version bump and a note — not a quieter one; this is
what the gate is for, and it worked.
