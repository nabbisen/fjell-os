# RFC-0.37-001 R1–R9: a bundle gets asked for and produced

**Governing RFC:** [../accepted/RFC-0.37-001-a-bundle-gets-asked-for-and-produced.md](../accepted/RFC-0.37-001-a-bundle-gets-asked-for-and-produced.md)
**Handoff:** [../handoffs/RFC-0.37-001-a-bundle-gets-asked-for-and-produced/implementation-handoff.md](../handoffs/RFC-0.37-001-a-bundle-gets-asked-for-and-produced/implementation-handoff.md)
**Escalation and ruling:** `.git-exclude/review-request/review-request-rfc-0.37-001-a-bundle-gets-asked-for-and-produced.md`, ruled at `4d82005` — R5 withdrawn, **E-085** filed, R2/R3/R4/R6 onward unaffected.

Written in the handoff's own numbered order, against tip `d3d7fa1`.

---

## R1 — the three findings re-derived at the tip, with controls, and the fourth-tag sweep

**`fjell-fleetd`'s missing loop, confirmed — and found to be worse than its own
erratum said.** `loop {` appears zero times in `main.rs`; it declares no
capability slots. Re-deriving this (rather than trusting E-081's own text)
surfaced a structural absence the erratum never named: `fjell-fleetd` had
**no `ImageId`**, was **absent from `qemu.rs`'s `SERVICES` build list and
the kernel's embedded image table**, and its own `src/rt.rs` held only a
panic handler — **no `_start` symbol**, the one thing every other service's
`rt.rs` provides and `link.ld`'s `ENTRY(_start)` requires. It had never been
cross-compiled and linked as a bootable image, confirmed directly: with the
panic handler removed as a control (before any real fix), `rustc` refused
outright — `error: #[panic_handler] function required, but not found`.
"`fleetd` becomes a service" (R3) turned out to be literal, not idiomatic —
anticipated by the governing RFC's own Risks section ("the task-table, stack
and endpoint limits… expect `init: spawn error`").

**`fjell-diagnosticsd`'s uninstalled receive slot and two-site reachability,
confirmed.** `CAP_TOOLS_EP` is slot 1, installed for no image (`spawn.rs`,
re-checked). `BUILD_BUNDLE` and `PUSH` each have exactly two occurrences
tree-wide — their declaration in `fjell_service_api::diagnosticsd` and
`diagnosticsd`'s own comparison arms.

**E-083's tags, confirmed on both sides.** `fjell-diagnosticsd`'s
`query_measurement_head` sent the local `MEASUREDD_HEAD_QUERY`/`_REPLY`
(`0x320`/`0x321`); `fjell-measuredd`'s real arm is
`measuredd::{GET_HEAD, HEAD_REPLY}` (`0x303`/`0x304`) — confirmed by reading
both files directly. `0x320`/`0x321` are, independently,
`recoveryd::READY`/`LIST_SNAPSHOTS` in `fjell_service_api` — confirmed by
reading that module. Five `SXT_*` constants in the same file hand-copied
`secure-transportd`'s own `SxtTag` enum values.

**The fourth-tag sweep: nothing else found.** Every other locally-declared
tag constant in `fleetd`'s and `diagnosticsd`'s own sources was checked
against `fjell_service_api`'s declared tags; no further collision or
duplication exists in either file beyond the five `SXT_*`/two `MEASUREDD_*`
instances E-083 already named.

## R2 — E-083 closed, demonstrated failing first

**The mismatch demonstrated failing, against the real `measuredd`, before
any fix was written.** `CAP_MEASUREDD_EP` (slot 4) was temporarily installed
in `spawn.rs` pointing at `measuredd`'s own real endpoint object, and a
scratch, unconditional call to `query_measurement_head()` was added to
`diagnosticsd`'s boot path, printing its result. Booted via `cargo xtask
qemu-test m8`: `SCRATCH-E083: measurement_head_available=0` — with a live
capability and a live `measuredd`, the old tags produced no usable reply.
Both the scratch call and the scratch install were reverted before the real
fix (confirmed: `grep -rn SCRATCH` on both files returns nothing in the
committed tree).

**The real fix:** `diagnosticsd` now reads the shared
`fjell_service_api::{measuredd::{GET_HEAD, HEAD_REPLY}, sxt::{OPEN_CHANNEL,
OPENED, DIAG_PUSH, DIAG_ACK, CLOSE}}`; the local `MEASUREDD_HEAD_QUERY`/
`_REPLY` and five `SXT_*` constants are deleted. `secure-transportd`'s own
`SxtTag` enum (`channel.rs`) now reads the same `sxt` constants for its five
matching discriminants and `from_u16` match arms, closing the duplication
from the other side — confirmed binary-identical afterward (`cargo xtask
build`: `fjell-secure-transportd.bin` did not move).

**What R2 does not fix, deliberately.** `query_measurement_head`'s own
payload decoding still reads `w0`/`w1` as the first 16 bytes of the chain
digest, which they are not — `measuredd`'s real reply layout is
`(latest_seq, cd_lo, dropped)`, only `cd_lo` carrying digest content. That
is **E-085** (below), not E-083, and is left exactly as found: moot in
practice, since slot 4 stays uninstalled in this line, so every call still
hits the `recv_msg` error path and returns `(zero digest, false)` regardless
of what the stale decoding logic would do with a real reply.

## R3 — `fleetd` becomes a service, literally

`src/rt.rs` now carries the `_start` global_asm block every other service's
does (the panic handler moves to `main.rs`, matching convention);
`build.rs` and `link.ld` are copied verbatim from `fjell-measuredd`
(identical boilerplate across every service that has them — confirmed by
diff). `fjell-abi::service::ImageId::FLEETD` (`0x22`) is new; `fjell-fleetd`
is added to `qemu.rs`'s `SERVICES` and the kernel's `image_bytes` table.

`service_main` is otherwise unchanged through its format self-checks, then
sends readiness on the uniform slot and calls `run_event_loop() -> !` — no
`sys_exit(0)` on the success path. The loop blocks on
`FLEETD_TRIGGER_RECV_SLOT` for `init`'s console-byte trigger and, on each
one, asks `diagnosticsd` for a bundle over `CAP_DIAGNOSTICSD_EP` (slot 1, a
SEND|RECV capability to `DIAGNOSTICSD_TOOLS_EP_OBJECT`, the same shared-
object shape as the update-path channel — both sides send a request and
block for a reply, not a one-way wake).

**The evidence it is alive after `ready`, not merely present:** booted via
`cargo xtask qemu-test m8` with no trigger injected, `fleetd` reaches
`fleetd: ready` and the boot completes normally (`TEST:M8:PASS`) — the loop
blocks without hanging the rest of the system. With the console byte `D`
injected (`remote-diag` tier, R6 below), the loop demonstrably executes:
`fleetd: remote-diag trigger received…` through `fleetd: bundle ready
(audit_event_count=0)` only appear if the blocking recv actually woke and
the round trip actually completed.

**MAX_TASKS (40) was not hit** — confirmed by the boot completing with no
`init: spawn error`, the symptom the governing RFC's own Risks section
named.

## R4 — `diagnosticsd` reachable; exactly the install the conversation needs

`DIAGNOSTICSD_TOOLS_EP_OBJECT` is installed as a SEND|RECV capability at
slot 1 in `diagnosticsd`'s own CSpace, mirroring `fleetd`'s side exactly.
**Nothing else is installed for either service**: `fleetd` gets no MMIO, no
`TaskCreate`, no second endpoint; `diagnosticsd`'s slot 2 (`sxt`, E-069) and
slot 4 (`measuredd`, now E-085) stay exactly as they were. The least-
privilege argument is the one RFC-0.35-001 R5 already made for the update-
path channel, applied here to the same shape of problem.

## R5 (replacement) — slot 4 stays uninstalled, honestly

Not implemented as originally written — see the escalation and ruling.
**What is implemented is the replacement the ruling specifies**: slot 4
(`CAP_MEASUREDD_EP`) is not installed anywhere in this line's `spawn.rs`
changes, and `measurement_head_available` reads `false` as a structural
consequence (`query_measurement_head`'s `recv_msg` call always hits its
error path with no capability to receive on). R6's tier (below) asserts
this directly.

**A stale text correction, found while implementing R6.** The governing
RFC's own R6 paragraph, and the matching line in the handoff's review-
request checklist, still said `measurement_head_available` should read
`true` — unedited when R5 was amended, even though the amended R5's own
text already states the opposite ("R6's tier asserts that it does [read
`false`]"). Both corrected with a dated note citing the ruling, propagating
a decision the architect had already made rather than making a new one.

## R6 — the tier, both reads, every flag honestly `false`

`tests/qemu/profiles/remote-diag.toml`: `D` injected after `driver-uart:
ready`. Run live (`cargo xtask qemu-negative remote-diag`), **9/9 expected
markers matched**, in order:

```
init: remote-diag trigger received; asking fleetd to run its scenario
fleetd: remote-diag trigger received; asking diagnosticsd for a bundle
diagnosticsd: bundle built
diagnosticsd: measurement_head_available=false
diagnosticsd: keyring_anchor_epoch_available=false
diagnosticsd: last_attestation_available=false
diagnosticsd: audit_available=false
fleetd: bundle ready (audit_event_count=0)
NEG:REMOTE_DIAG:BUNDLE_READY:PASS
```

**Both the instigator's own read and the deciding service's own output**,
the pattern `health-fail.toml`/`upgrade.toml` already established:
`fleetd`'s own line plus the literal marker is the instigator's read;
`diagnosticsd`'s four flag lines, printed by `diagnosticsd` itself inside
its `BUILD_BUNDLE` arm, are the deciding service's.

**Every flag reads `false` — asserted as correct, not as a shortfall.**
`audit_available`, `keyring_anchor_epoch_available`, and
`last_attestation_available` have never had a source (RFC-0.36-001);
`measurement_head_available` reads `false` because slot 4 stays uninstalled
(R5 replacement, E-085). A bundle that says "I could not obtain a
measurement head" is the true one. The tier asserts the flags themselves,
not merely that a bundle was built — a bundle of silent zeros with no flags
printed would not have satisfied this tier.

Committed as a gated profile: `tests/qemu/artifacts/remote-diag/{expected-markers,qemu-command,result-summary}.txt`
now exist, so `test-all` and CI's dynamically-derived negative-test matrix
pick it up automatically — no separate list to update.

## R7 — `capability-topology`, re-counted

`DIAGNOSTICSD` slot 1's waiver is retired (installed now, not waived).
`DIAGNOSTICSD` slot 4's waiver reason is rewritten to name **E-085** (this
line found and filed it) rather than **E-078** (this line closed it).
`FLEETD`'s own new `CAP_DIAGNOSTICSD_EP` (slot 1) is discovered and
genuinely installed, needing no waiver. Live run:

```
capability-topology: note — DIAGNOSTICSD slot 2 (secure-transportd) ...: E-069 closed ...
capability-topology: note — DIAGNOSTICSD slot 4 (measuredd) ...: RFC-0.37-001 (closes E-078): ... E-085, filed by that line ...
capability-topology: PASS (4 installed, 2 waived to a named line, 6 checked)
```

Discovered-slot count moved 5 → 6; unwaived (installed outright) moved 2 →
4. The bookkeeping test renamed
`discovery_on_the_current_tree_matches_the_known_five` →
`..._known_six`, matching RFC-0.36-001's own precedent for when a test's
name would otherwise become a stale claim.

## R8 — E-078, E-081, E-083 CLOSED; E-082 and E-084 untouched

All three dated CLOSED blockquotes in `rfcs/ERRATA.md` name exactly what
closed them; three summary-table rows move ACCEPTED → CLOSED (11 → 8
ACCEPTED remaining, confirmed by `consistency-check errata-limitations`).
`docs/src/releasing/v1-limitations.md`'s E-078/E-081/E-083 bullets rewritten
past-tense as closed, in the same commit, with cross-references elsewhere in
the file (E-079's own bullet; the E-081/E-084 bullet's "why E-078
understates itself" aside) corrected so none still reads either entry as
open.

**E-085, filed separately by the architect's own ruling, is untouched by
this line** — the governing RFC's own header says so explicitly ("Does not
track E-085"). **E-082 and E-084 (RFC-0.37-002's) are untouched** — confirmed
by their absence from any commit in this line's diff.

## R9 — the gates, the rebuild, the baseline

Guest code changed in exactly three services (`diagnosticsd`, `init`,
`fleetd` — new); all other 30 prebuilt binaries are bit-identical,
including `secure-transportd`'s despite its own source change (the `SxtTag`
discriminant values are unchanged, only how they're spelled). Rebuilt before
re-recording:

```
cargo xtask build                                          # exit 0
rm tests/repro/baseline-digests.txt
cargo run -p fjell-tools -- repro-check --record-baseline
cargo run -p fjell-tools -- repro-check                     # PASS, 33 identical
```

| Gate | Result |
|---|---|
| `cargo xtask build` | exit 0, 0 warning lines, `1 crate-level allow(s): fjell-storaged` |
| Forced rebuild, `riscv64gc-unknown-none-elf`: `fjell-diagnosticsd`, `fjell-fleetd`, `fjell-secure-transportd`, `fjell-init` | all 0 warnings |
| `cargo fmt --all --check` | exit 0 |
| `cargo test --workspace --lib --exclude fjell-proptest` | exit 0, 0 failed everywhere |
| `abi-snapshot --verify` | PASS, 455 → 467, 12 additive items (regenerated; the governing RFC's own new public surface), 0 removed |
| `consistency-check --all` | exit 0 — PASS, 20 subchecks; `errata-limitations: PASS (8 ACCEPTED, all referenced)`; `capability-topology: PASS (4 installed, 2 waived, 6 checked)` |
| `repro-check` | rebuilt first; re-recorded in the same commit; re-run: PASS, 33 artefacts identical |
| `cargo xtask qemu-negative remote-diag` | PASS, 9/9 markers matched |
| `test-all` | exit 0 — **32 tiers, 31 PASS, 0 FAIL, 1 NOT-GATED** (`store`, pre-existing, documented) — `remote-diag` now included among the 32, derived automatically. `tests/runs/20261018-025908` |
| `release-rehearsal` | **exit 0 — RELEASE-REHEARSAL: ALL MECHANICAL GATES PASS** — gates 1–8, 10, 11, 12 all PASS, including Gate 7 (ERRATA register, 0 OPEN) and Gate 12's `consistency-check` (20 subchecks) |

CI run id: none — nothing pushed.

## Decisions made along the way

1. **R1's "fourth finding" this time was a structural absence, not a tag
   collision — and it was handled the same way the handoff's own escalation
   discipline calls for: found, confirmed with a control, and fixed within
   R3's own already-authorized scope** ("`fleetd` becomes a service"),
   rather than escalated as a fifth erratum. The distinction from the
   `measuredd` payload finding (E-085, genuinely escalated): this one did
   not conflict with anything the RFC asked for — it was the RFC's own
   requirement, more literally true than its own prose suggested.
2. **The scratch E-083 demonstration used a temporary, unconditional call
   inside `diagnosticsd`'s own boot path, not the full `fleetd`
   conversation** — `fleetd` did not exist as a running service yet at the
   point R2 needed to be demonstrated, and R2 is ordered before R3 in the
   handoff. Isolating the demonstration to exactly what E-083 claims (the
   tag mismatch) avoided a dependency the order does not otherwise require.
3. **`query_measurement_head`'s stale payload-decoding logic is left
   exactly as found.** Fixing it would have been exactly the shape E-085
   rules against — there is nothing honest that logic could decode today,
   since slot 4 stays uninstalled; leaving it unreachable, rather than
   polishing dead code, keeps the diff to what this line actually changes.

## Afterword

This line's own risk section named two things and both held: "fleetd is the
first program this project gives an event loop to" was true in a more
literal sense than written (it was the first program this project gave a
*bootable image* to, full stop), and the task-table/spawn-limit risk it
predicted did not materialize (`MAX_TASKS` headroom was sufficient). The
escalation that paused this line for a day — R5's withdrawal — is the other
half of the same discipline working as designed: a defect found and
reported before a line of code was written against it, not absorbed or
guessed at.
