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
  RFC-v0.24-001 Pass 1.

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
