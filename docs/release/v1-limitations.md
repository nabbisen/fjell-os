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
  target, or a host-testable subset split out). Deferred to its own RFC after
  `0.23.0`. Found during RFC-v0.23-002 Slice 1; scope widened by
  RFC-0.24-001 Pass 1.

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
  design answer instead. Recorded, not fixed; 0.25 candidate.

- **Instrument scopes are hand-enumerated and have drifted from reality**
  (Errata **E-015**, ACCEPTED). **19 of 89 workspace crates are never named in
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
  report success without checking. Recorded, not fixed; 0.25 candidate.

- **No instrument verifies any document link, index, or count** (Errata
  **E-016**, ACCEPTED). `rfcs/README.md` — the repository's RFC index under
  RFC 000 — has zero instrument coverage; the only trace of it in the entire
  instrument set is a doc comment that mentions the file without opening it.
  Thirteen relative links in tracked documentation are broken. The
  instrument audit's own totals table stated a population of 56 while summing
  to 54, unnoticed across four passes and three RFCs because nothing checks a
  count. The index's
  "Shipped" column names a release for roughly 150 rows as `v0.3.0`, `v0.22.0`
  and so on — tags that do not exist under those names, since release tags
  have never carried a `v` prefix. The 0.24 series was renamed to match on
  2026-08-03; historical rows were left, because renaming ~150 files to apply
  a convention retroactively would break the links that commits and release
  records point at. One link-and-count instrument closes all three, and adding
  an instrument was RFC-0.24-001's explicit non-goal. Recorded, not fixed;
  0.25 candidate.

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
  `Implemented-with-Errata`. Recorded, not fixed; 0.25 candidate.

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
  scope. Recorded, not fixed; needs its own line.

- **The ABDD live path no longer runs** (Errata **E-020**, **OPEN**).
  RFC-v0.23-001 shipped this project's distinguishing architectural bet in
  `0.23.0` — `sample-service` emits an intent, `semantic-stream` routes it, and
  a *separate* `proxy-text` task renders it, with the capability-checked
  refusal demonstrated — and created `tests/qemu/profiles/semantic.toml` in the
  same RFC as a fail-closed guard so the path could not rot. Since RFC-0.26-001
  removed the scheduler priority asymmetry, **the path does not execute at
  all**: measured zero occurrences of `sample-service demo intent` or
  `proxy-text: action` in the profile's serial log.
  `crates/services/fjell-sample-service` calls `emit_sample_intent()` once from
  `service_main()` under a comment asserting its downstream peers are *"already
  spawned and ready by this point"* — an assertion about scheduling order
  rather than a synchronisation, which the priority asymmetry had been silently
  satisfying. **This is a service, not a test harness, and the consequence is a
  shipped feature that no longer runs**, which is why it is OPEN rather than
  ACCEPTED. Gate 7 (`ERRATA register (0 OPEN)`) therefore fails and no release
  can be cut until it is fixed — deliberately. The fix is `sample-service`
  waiting for its peers rather than asserting them; it already sends
  `SERVICE_READY` to service-manager, so the protocol exists. Its own line,
  next.

- **`init` can silently consume and drop another task's IPC** (Errata
  **E-021**, ACCEPTED). `fjell-init`'s `wait_ready_exact` loops on a blocking
  receive and discards any message whose tag does not match the one it wants —
  no reply, no re-queue, no log. `init` holds receive-capable capabilities to
  endpoint objects 7 and 8, which are also `semantic-stream`'s and
  `proxy-text`'s own endpoints and carry ordinary protocol traffic, so **two
  tasks receive on one queue with nothing arbitrating between them**. A blocking
  `sys_ipc_call` consumed by `init` leaves its caller blocked forever. Observed
  live during RFC-0.26-002: `init` swallowed `semantic_stream::PUBLISH_BEGIN`.
  Unsafe on any endpoint another task can call into; RFC-0.26-001's scheduling
  change only altered where it lands. Recorded, not fixed — the missing `else`
  and the shared-channel arrangement should be decided together, in
  **RFC-0.26-004**.

- **v1.0 release-checklist Step 9 references a build output that does not
  exist** (Errata **E-012**, ACCEPTED). Step 9 signs
  `target/release-bundles/*.bundle`; nothing in `crates/` or `tools/` writes
  that path, and `package-release` produces a single tarball. Steps 9–10 are
  the signing steps, so the v1.0 checklist cannot currently be executed to
  completion. Deliberately not investigated in v0.22 (owner decision,
  2026-07-30 — v1.0 is not in view); must be resolved before v1.0 preparation
  begins.

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
