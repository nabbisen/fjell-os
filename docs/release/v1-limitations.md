# v1.0 Limitations — Gate 9 Reference

*The single authoritative list for release-rehearsal Gate 9 ("confirm the
v1.0 limitations section"). Each item links to its governing record. Changes
require updating the governing record first, then this page.*

| # | Limitation | Governing record |
|---|------------|------------------|
| 1 | **Hardware** — no validated real-hardware deployment; the VisionFive 2 profile is provisional and was never booted on silicon | Errata **E-004** (ACCEPTED); `docs/deployment/starfive-visionfive2.md` TODOs |
| 2 | **Multi-hart** — the kernel runs single-hart; SMP scheduling, per-hart locking (e.g. the console spinlock), and IPIs are deferred to the multi-hart milestone | v1.0 design decision; `crates/fjell-kernel/src/console.rs` invariant note |
| 3 | **POSIX** — no POSIX compatibility surface (descriptors, fork, signals, ttys) | Non-goal **N1** |
| 4 | **Kernel-IPC for the SDK reference service** — the SDK reference service does not operate over live kernel-mediated IPC | Non-goal **N21** |
| 5 | **ZeroizeOnDrop** — no independently verified byte-level key-erasure guarantee | Non-goal **N23** |
| 6 | **Trust-anchor provisioning** — TOFU with `--allow-tofu-provision` flag (dev/QEMU), factory station (v1.1), hardware-anchored (v2+). Flag implemented (`cargo xtask provision-dev --allow-tofu-provision`) in v0.20.0. | **RFC-v0.17-001** (Accepted, 2026-06-04) |
| 7 | **`cap_install` rights validation does not execute** — `sys_cap_install`'s and `sys_cap_install_with_rights`'s doc-comments claim the kernel validates `rights ⊆ installer authority`; no such check runs, because the `CapInstall` syscall has no dispatch arm at all. The path fails closed (`UnknownSyscall`) rather than granting excess rights — not a live security hole — but the documented behaviour is not shipped. Disposition of `CapInstall` and the other 8 declared-but-undispatched syscalls deferred to v0.22. | Errata **E-011** (ACCEPTED); **RFC-v0.21.3-001** §M2 |

Additional operational notes (not Gate 9 items, listed for completeness):

- **`test-all` tier 1 never runs the tests of any package without a library
  target — including the verification tooling's own** (Errata **E-013**,
  ACCEPTED). Tier 1 ("Host library tests") runs
  `cargo test --workspace --lib`, which silently skips any package with no
  library target. Measured across the workspace: **40 of 89 manifests have no
  lib target, and 10 of those carry 166 `#[test]` functions that `--lib` never
  reaches.**

  **Eight of those ten are the gate tools themselves** — `fjell-tools` (68,
  including `callsite_audit`'s, which are Gate 11's own demonstrations),
  `fjell-consistency-check` (26 — Gate 12's), `fjell-unsafe-audit` (10 —
  Gate 2's), `fjell-abi-snapshot` (8 — Gate 4's), `fjell-mmio-audit` (7 —
  Gate 3's), `fjell-readiness-check` (5 — Gate 5's), plus `fjell-repro-check`
  (6), `fjell-ci-coverage` (4), `fjell-summary-check` (2), and `fjell-kernel`
  (30). So the demonstrations that establish several gates as sound are
  themselves never run by the tier that claims to run the test suite.

  For `fjell-kernel` specifically, the real target is bare-metal with no
  libtest harness, so no alternate invocation reaches its modules either —
  including the kernel-side lease table (`lease/mod.rs`, one half of a Verus
  release-required target) and the RFC-v0.23-002 milestone-marker tests.

  The follow-up RFC has two separable halves: the **nine host binaries**,
  ordinary `std` crates where the gap is the bare `--lib` flag and the fix is
  trivial; and **`fjell-kernel`**, where it is architectural (a `[lib]`
  target, or a host-testable subset split out). Found during RFC-v0.23-002
  Slice 1; scope widened by RFC-0.24-001 Pass 1. Tracking: **unscheduled**
  (re-dispositioned from *"RFC after 0.23.0"* by RFC-0.27-001 — three
  releases have shipped since without one being written).

  **Second confirmation (RFC-0.24-001 Pass 4).** The six gate-tool crates —
  `fjell-abi-snapshot`, `fjell-consistency-check`, `fjell-mmio-audit`,
  `fjell-readiness-check`, `fjell-repro-check`, `fjell-summary-check` — are
  **also never named in any job in `.github/workflows/ci.yml`**, which lists
  its packages explicitly by name. So nothing runs their tests anywhere, by
  any mechanism, in ordinary operation: not tier 1, and not CI. The three
  crates backing Gate 8's validation drills are a separate matter and are
  recorded under E-015, not here.

- **Several verification instruments decide by matching a fixed string**
  (Errata **E-014**, ACCEPTED). Gate 5 counts rows containing `**OPEN**`, so a
  row marked `**BLOCKED**` is counted in none of its four buckets — absent, not
  miscounted. Gate 6 counts the literals `§1`..`§6` and discards its
  regeneration's exit status. Gate 7 counts `OPEN` in the errata register. The
  negative harness's `FORBIDDEN` list matches `"TEST:FAIL"`, which is not a
  substring of the real message `TEST:M7:FAIL (init did not exit cleanly)`.
  `errata-limitations` requires only that an erratum's *ID* appear in this
  file, and passed over a live case where the content diverged while the ID
  matched. `fjell-unsafe-audit`'s category extractor splits on whitespace and
  commas, so `category=csr-asm; <explanation>` silently reads as `Unknown`.
  The shared TOML array parser closes an array at a `]` inside a string
  literal, loading 2 of 4 markers silently. None is a live false-green today;
  each individual patch would be a better string, and the family needs one
  design answer instead. Recorded, not fixed; **unscheduled** — carried
  through the 0.25 and 0.26 lines with no line taken up, per RFC-0.27-001's
  re-disposition rather than writing a milestone nobody intends to keep.

- **Instrument scopes are hand-enumerated and have drifted from reality**
  (Errata **E-015**, ACCEPTED). **21 of 91 workspace crates are never named in
  any `ci.yml` job** — six are the gate tools above, and three back Gate 8's
  validation drills (`fjell-sig-ed25519`, `fjell-fleet-sync`,
  `fjell-config-sync`), whose markers therefore run only at
  `release-rehearsal` time and never on a push or PR. `ci-qemu-negative`'s
  matrix lists nine categories while `test-all` runs ten: the `semantic`
  category added by RFC-v0.23-001 has never run in ordinary CI. The
  `KNOWN_V01X_CATEGORIES` / `KNOWN_V02_CATEGORIES` lists no longer describe the
  profiles on disk, and `smoke.rs`'s `v0.6-verification` milestone is defined
  in code and invoked by nothing anywhere. These are checks that **do not
  run**, or run over an incomplete set — distinct from E-014's checks that
  report success without checking. Recorded, not fixed; **unscheduled** — same
  re-disposition as E-014, for the same reason.

- **No instrument verifies any document link, index, or count** (Errata
  **E-016**, **CLOSED** by RFC-0.27-001). `rfcs/README.md` — the repository's
  RFC index under RFC 000 — had zero instrument coverage; thirteen (measured
  again for this RFC: fourteen) relative links in tracked documentation were
  broken; the instrument audit's own totals table had stated a population of
  56 while summing to 54 (already corrected under RFC-0.24-003); and the
  index's "Shipped" column names a release for roughly 150 historical rows as
  `v0.3.0`, `v0.22.0` and so on — tags that never carried a `v` prefix,
  left as historical rows rather than retroactively renamed. RFC-0.27-001
  built the missing coverage as three new `fjell-consistency-check`
  subchecks: `errata-tracking` (the tracking-column defect this same RFC
  found, below), `doc-links` (every relative link in a tracked `.md` file
  must resolve; 12 of 14 broken links fixed mechanically, 2 recorded in
  `tests/doc-links/known-broken.txt` pending an ADR-renumbering decision
  outside this line's scope), and `doc-counts` (`rfcs/README.md`'s five
  folder-count assertions checked against the tree). The index's stale
  release-tag rows are unchanged — renaming ~150 historical files was out of
  scope here as it was in 2026-08-03, for the same reason (it would break the
  links commits and release records point at).

- **The instrument audit's `sound` verdicts are not all demonstration-backed**
  (Errata **E-017**, ACCEPTED). RFC-0.24-001 requires every instrument claimed
  `sound` to carry a committed demonstration of it failing, and records
  `UNAUDITED` otherwise. Two rows were found violating that in review —
  `ci-proptest`, certified on the completeness of its crate list while running
  zero tests, and `Gate 4`, certified by the architect because the tool's own
  unit suite passed, which is proxy attestation rather than the gate observed
  failing. Both were repaired. The re-derivation of the remaining `sound` rows
  against the same question is **incomplete**, and Gate 4 — the first
  re-derived — fell immediately, so the base rate is not known to be low. **The
  22 `sound` verdicts are provisional.** This is why RFC-0.24-001 ships
  `Implemented-with-Errata`. Recorded, not fixed; **unscheduled** — the
  re-derivation of the remaining rows has not been picked up by any line
  since 0.24; RFC-0.27-001 re-dispositions rather than performs it.

- **Two QEMU negative profiles (`ipc`, `semantic`) assume an unsynchronised
  scheduling order and fail after RFC-0.26-001's scheduler fix** (Errata
  **E-019**, ACCEPTED). `fjell-neg-test`'s IPC blocked-recv/blocked-call
  scenarios documented, in their own source comments, an assumption about
  *relative* task-scheduling order that RFC-0.26-001's fairness fix no longer
  guarantees. **`fjell-sample-service`'s startup intent emission was
  originally recorded here too; it is a service rather than a harness and has
  been split out as E-020, OPEN** — see below. Same root cause as the M6 hang RFC-0.26-001 investigated and
  fixed — code assuming ordering instead of synchronising on it — in a
  silently-skipped-assertion shape rather than a hang. Fixing either needs
  the affected service to synchronise explicitly; out of RFC-0.26-001's
  scope. Recorded, not fixed. Its own line now exists and is accepted:
  **RFC-0.26-003** (*"The blocked-recv test needs a rendezvous it cannot
  currently have"*), which found that `ipc` has since gone green again by
  the same accident this entry describes — RFC-0.26-004 fixed a defect
  upstream that let `sample-service` reach its main loop, and `neg-test`'s
  own scheduling assumption is unchanged and unsynchronised underneath the
  now-passing profile. Not yet implemented.

- **The ABDD live path runs again** (Errata **E-020**, **CLOSED** by
  RFC-0.26-004). RFC-v0.23-001 shipped this project's distinguishing
  architectural bet in `0.23.0` — `sample-service` emits an intent,
  `semantic-stream` routes it, and a *separate* `proxy-text` task renders it,
  with the capability-checked refusal demonstrated — and created
  `tests/qemu/profiles/semantic.toml` in the same RFC as a fail-closed guard.
  RFC-0.26-001's scheduler fix removed the priority asymmetry the path had
  been silently relying on, and `sample-service` called `emit_sample_intent()`
  on the bare assertion that its peers were *"already spawned and ready by
  this point"* rather than synchronising on it.
  RFC-0.26-004 replaced the assertion with a real wait —
  `emit_sample_intent()`'s transport is a blocking `sys_ipc_call`, which
  queues and blocks the caller until `semantic-stream` actually reaches its
  receive loop and replies — made safe by establishing that
  `semantic-stream`'s and `proxy-text`'s endpoints each have exactly one
  receiver (see E-021 below), so the call can never be delivered to, and
  dropped by, anyone else. All four `semantic.toml` markers pass, with a
  causal ordering in the serial log (not just marker presence) confirming the
  wait actually executed. See
  `docs/rfcs/RFC-0.26-004-readiness-channel-answer.md`.

- **`init` no longer receives on another task's endpoint** (Errata **E-021**,
  **CLOSED** by RFC-0.26-004, cleanly, no residual hazard). `fjell-init`'s
  `wait_ready_exact` used to loop on a blocking receive and discard any
  message whose tag did not match the one it wanted — no reply, no re-queue,
  no log — while holding receive-capable capabilities to endpoint objects 7
  and 8, which are also `semantic-stream`'s and `proxy-text`'s own endpoints
  and carry ordinary protocol traffic: two tasks received on one queue with
  nothing arbitrating between them. RFC-0.26-004 removed `wait_ready_exact`
  entirely rather than patching its missing `else` — there is no code path
  left in `init` that can receive on either endpoint. `init`'s capability to
  object 7 is narrowed to `CALL` only (kernel-enforced: a future `sys_ipc_recv`
  there would fail the rights check); its capability to object 8 is removed
  outright. **Invariant established: a service's endpoint has exactly one
  receiver — the service itself.** See
  `docs/rfcs/RFC-0.26-004-readiness-channel-answer.md`.

- **The one-way send wrapper's name and doc-comment described a primitive
  the kernel never implemented** (Errata **E-022**, **CLOSED** by
  RFC-0.27-002). `sys_ipc_try_send` was named as though it tries and
  documented as a non-blocking, fire-and-forget contract; the kernel
  implements one-way send as coherent **rendezvous** IPC, symmetric with
  two-way call/reply — `sendq`/`recvq` are waiter queues, not message
  buffers, and the kernel correctly blocks the caller when no receiver is
  waiting. **The kernel was correct; the wrapper's name and documentation
  were not**, which RFC-0.27-002 fixed by renaming it to `sys_ipc_send` and
  rewriting its doc-comment, plus the three normative docs describing the
  old contract. First found live while implementing RFC-0.26-004: once
  `init` was correctly removed as `semantic-stream`'s/`proxy-text`'s
  accidental co-receiver (E-021), each service's pre-existing
  `send_ready()` call (announcing into its own endpoint, before reaching
  its own receive loop) blocked itself permanently under the corrected
  understanding of the contract — those two calls were dead code under the
  new invariant regardless, and were deleted. **RFC-0.27-002's required
  audit found this shape recurs in five more services**
  (`fjell-measuredd`, `fjell-attestd`, `fjell-recoveryd`, `fjell-storaged`,
  each via raw inline `asm!`, not the wrapper). None **self-deadlocks**:
  each sender genuinely **blocks** when it finds no receiver waiting (the
  kernel's own audit ring confirms this live — `fjell-attestd`'s own
  `send_ready()` recorded `Queued`, not `Delivered`) and is **woken** once
  `init`'s `wait_service_ready`/`wait_storaged_ready` reaches that
  endpoint — the exact masking arrangement already removed for
  `semantic-stream`/`proxy-text`, intact here only because nothing has
  asked `init` to stop. Filed as **E-024**, below. Whether a genuinely
  non-blocking one-way send
  should exist is answered as **a real, recurring need, not decided by
  this line** — an ABI addition requires escalation this RFC's scope does
  not authorise. See
  `docs/rfcs/RFC-0.27-002-one-way-send-contract-answer.md` for the full
  audit and reasoning.

- **The release tool's `RELEASE.md` generation and consistency checks were never
  built** (Errata **E-023**, **CLOSED** by RFC-0.27-001). RFC-v0.7.1-001,
  marked `Implemented (v0.7.1)`, specified five behaviours for the release
  tool; one shipped. `package_release.rs` read the workspace version and
  tarred the repository root — it did not generate a `RELEASE.md`, did not
  produce a digest manifest of `crates/fjell-kernel/prebuilt/`, did not exit
  non-zero on inconsistency, and **did not grep for stale version mentions
  outside `CHANGELOG.md`** — the second row, and the one that mattered: it is
  exactly what would have caught `README.md` sitting at `0.21.3` with five
  wrong counts through five releases, found only when the owner asked. The
  root `RELEASE.md` — a ten-line signpost carrying none of the specified
  contents — was removed 2026-08-27. RFC-0.27-001 built the specified
  check, scoped as `version-currency` (a new `fjell-consistency-check`
  subcheck, not a `package-release` change): `README.md` — the one document
  whose purpose is "what is Fjell OS right now" — must not assert a version
  other than the current workspace version. A tree-wide sweep was tried and
  rejected as unbuildable (over 800 legitimate historical version mentions
  across RFC files and `ROADMAP.md`); see the subcheck's own design note for
  why `README.md` is the right scope. `RELEASE.md`-file generation and a
  prebuilt-artefact digest manifest remain unbuilt — out of this line's
  scope, and not what caused the incident this erratum records.

- **v1.0 release-checklist Step 9 references a build output that does not
  exist** (Errata **E-012**, ACCEPTED). Step 9 signs
  `target/release-bundles/*.bundle`; nothing in `crates/` or `tools/` writes
  that path, and `package-release` produces a single tarball. Steps 9–10 are
  the signing steps, so the v1.0 checklist cannot currently be executed to
  completion. Deliberately not investigated in v0.22 (owner decision,
  2026-07-30 — v1.0 is not in view); must be resolved before v1.0 preparation
  begins.

- **`init` receives on four services' own endpoints, and RFC-0.26-004's
  one-receiver invariant is narrower in the tree than in its text** (Errata
  **E-024**, ACCEPTED). `storaged`, `measuredd`, `attestd` and `recoveryd` each
  announce readiness into the same endpoint object they later receive protocol
  traffic on; that announcement does not deadlock only because `init` still
  holds receive rights on those four objects and reaches every wait in a fixed
  boot sequence. `init`'s `wait_service_ready`/`wait_storaged_ready` also retain
  the missing-`else` defect recorded as E-021 and closed only for the two
  objects RFC-0.26-004 touched: a message with an unexpected tag is consumed and
  discarded. Load-bearing as it stands; reworking RFC 058's readiness protocol
  was a non-goal of both RFC-0.26-004 and RFC-0.27-002.

- **`trust-report`'s capability inventory depends on the developer's untracked
  working tree** (Errata **E-025**, ACCEPTED, scheduled 0.27). The cap-manifest
  scan skips a hand-listed `["target", ".git", "tests/runs"]` and not
  `.git-exclude/`, so any scratch checkout under `.git-exclude/tmp/` inflates
  the reported manifest count — and doubles the reported unsafe-site inventory
  (`311/311` becomes `622/622`, internally consistent and wrong). The committed
  report is correct; the tool that produces it is not bounded by what git
  tracks, and nothing in its output signals when it has strayed.

- **No QEMU serial log has ever been committed alongside the document that
  cites it** (Errata **E-026**, ACCEPTED). E-013 leaves nothing kernel-side
  host-testable, so QEMU logs are the only evidence for kernel behaviour, and
  `.gitignore:28` (`*.log`) keeps all of them out of the tree.
  `tests/qemu/artifacts/` is additionally overwritten by the next run, and
  `test-all`'s per-tier logs under `tests/runs/` capture build stdout rather
  than the serial transcript. Every QEMU-evidenced claim in this project is
  therefore **unverifiable from a clone** — not shown to be wrong, but not
  checkable either. Evidence cited by RFC-0.27-002 is resolvable by run id on
  the machine that produced it only.

- **Nothing checks that the threat model's threats cite RFCs** (Errata
  **E-027**, ACCEPTED). The v0.9–v0.15 handoff asserted a "threat-model gate"
  enforcing it; no such gate exists in any commit on any branch. The property
  holds as of 2026-08-31 — all 20 `Tn` sections in
  `docs/security/threat-model-v1.md` cite an RFC, and the 20 in-scope / 8
  out-of-scope counts are correct — but it is held by hand, and a regression
  would be reported by nothing.

- **QEMU negative-test coverage status (v0.19/v0.20).** The nine main
  negative categories now run real QEMU profiles with fail-closed marker
  checking (a wrong error, an unexpected success, or a panic in the serial
  log fails the run). Seven categories have all markers confirmed
  (capability 8, mmio 3, dma 3, audit 1, user-copy 2, policy 4, harness 1);
  one is partially confirmed (svc 2/4 — READY pair pending a startup-timing
  fix); the ipc profile is restored to 3/3 in v0.20.0 after fixing the IPC
  words ABI and the reply-edge cancellation path. The `store` and
  `upgrade` negative profiles exist as marker specifications but have **no
  emitting scenarios yet** and are explicitly **not v1 release-gated**;
  running them manually fails honestly rather than placeholder-passing.

- Several services in the QEMU image are **smoke-test stubs** that signal
  ready and exit by design (`fjell-netd`, `fjell-secure-transportd`,
  `fjell-driver-virtio-net`, `fjell-proxy-text`, `fjell-driver-virtio-blk`,
  `fjell-powerd`, among others). Their full implementations are tracked on
  the post-v1.0 roadmap; their early-exit pattern is intentional.
- The repro-check baseline (`tests/repro/baseline-digests.txt`) tracks the
  committed `prebuilt/*.bin` artefacts and must be re-recorded whenever the
  prebuilt service binaries are rebuilt — see `tools/fjell-repro-check`.
