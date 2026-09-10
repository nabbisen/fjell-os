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
| 7 | **`cap_install` rights validation does not execute** — `sys_cap_install`'s and `sys_cap_install_with_rights`'s doc-comments claim the kernel validates `rights ⊆ installer authority`; no such check runs, because the `CapInstall` syscall has no dispatch arm at all. The path fails closed (`UnknownSyscall`) rather than granting excess rights — not a live security hole — but the documented behaviour is not shipped. Disposition of `CapInstall` and the other **5** declared-but-undispatched syscalls (`PlatformReboot`, `TaskKill`, `MmioUnmap`, `DmaShare`, `Reboot`) was deferred to v0.22 and **did not happen**; it remains open. *Corrected at the 0.27.0 cut: this read "the other 8 … deferred to v0.22", a count that had moved (35 declared / 29 dispatched / 6 undispatched) and a deferral to a milestone that shipped without it. The current figure is printed by `syscall-surface` at every release rehearsal.* | Errata **E-011** (ACCEPTED); **RFC-v0.21.3-001** §M2 |

Additional operational notes (not Gate 9 items, listed for completeness):

- **`test-all` tier 1 used to never run the tests of any package without a
  library target — including the verification tooling's own** (Errata
  **E-013**, **CLOSED** by RFC-0.29-001). Tier 1 ("Host library tests") ran
  `cargo test --workspace --lib`, which silently skipped any package with no
  library target. Measured at the time: **40 of 89 manifests had no lib
  target, and 10 of those carried 166 `#[test]` functions `--lib` never
  reached** — the count grew steadily every release after (RFC-0.29-001's
  own re-derivation found 41/288 before its fix, and the two prior figures
  it corrected — 305 and 285/20 — had themselves already gone stale by the
  time they were used, since every instrument this project adds lands
  inside this count).

  **Fixed by a new tier**, not by widening `--lib`: `cargo test --workspace
  --bins --tests` reaches every crate above except `fjell-kernel`, but
  cannot be added blindly — `fjell-kernel` and every crate under
  `crates/services/`/`crates/drivers/` are `#![no_std]` binaries with their
  own `panic_impl`, and either flag tries to build all of them for the host,
  a compile error rather than a test failure. `crates/fjell-tools/src/
  cargo_metadata.rs` derives the exclude set from `cargo metadata` instead
  of a name list. Demonstrated failing first (RFC-v0.22-001): a register
  deliberately removed from a real `SYSCALL-CALLSITE-002`-guarded `asm!`
  block in `fjell-syscall` was caught by the new tier, reverted, and shown
  passing again.

  **`fjell-kernel`'s tests remain unreachable** — the real target is
  bare-metal with no libtest harness, so no host invocation reaches
  `lease/mod.rs` (one half of a Verus release-required target),
  `mm/frame_alloc.rs`, `mm/user_ptr.rs`, `task/scheduler.rs`, or
  `trap/dispatch.rs`'s tests (including the RFC-v0.23-002 milestone
  markers) either. Making the kernel host-testable was this line's
  explicit non-goal and, per this entry's own original framing, was never
  this erratum's core claim ("this is not 'kernel unit tests do not run'
  but 'the verification tooling's own tests do not run'") — tracked
  separately as a distinct, still-open architectural question.

  **The CI half is fixed too**, not just `test-all`'s: a new `ci-host-bins`
  job runs the identical derived command (`cargo xtask host-bin-tests`), so
  the gate tools' own tests — previously never named in any
  `.github/workflows/ci.yml` job at all — now run on every push and PR, not
  only locally.

- **Several verification instruments used to decide by matching a fixed
  string** (Errata **E-014**, ACCEPTED, tracked **RFC-0.29-002** — six of
  seven instances fixed, one survives). Gate 5 used to count rows containing
  `**OPEN**`, so a row marked `**BLOCKED**` was counted in none of its four
  buckets — fixed by reading the underlying tool's real exit status
  (`fjell-readiness-check` already computed the right answer; nothing
  needed re-deriving from its text). Gate 6 used to count the literals
  `§1`..`§6` and discard its regeneration's exit status — fixed, and
  demonstrated on a regeneration that fails while a stale six-section
  report remains on disk (it now fails, not passes). Gate 7 used to count
  the literal `"| OPEN |"`, missing an annotated `"| OPEN (blocked on X) |"`
  — the shape the register already uses for `ACCEPTED`; fixed by parsing
  the table's actual status cells (RFC-0.29-002 §7's shared parser). The
  negative harness's `FORBIDDEN` list used to match `"TEST:FAIL"`, not a
  substring of the real message `TEST:M7:FAIL (init did not exit cleanly)`
  — fixed with a structural `TEST:<token>:FAIL` scan. `errata-limitations`
  used to require only that an erratum's bare *ID string* appear anywhere
  in this file — fixed to require the file's own established `**E-NNN**`
  bold-reference convention, so an incidental, unformatted mention no
  longer counts as disclosure. `fjell-unsafe-audit`'s category extractor
  used to split on whitespace and commas only, so `category=csr-asm;
  <explanation>` silently read as `Unknown` — fixed by adding `;` to the
  delimiter set. **The shared TOML array parser survives, unfixed**: it
  still closes an array at the first line *containing* `]`, not the first
  unquoted one, so a marker string with a literal `]` (e.g. an
  `"[INTENT] ..."`-style marker) still truncates the array early —
  confirmed still live, not this line's scope (five specific instruments
  were named; this was not one of them).

- **Instrument scopes were hand-enumerated and had drifted from reality**
  (Errata **E-015**, **CLOSED** by **RFC-0.29-002**). Three of four
  historical instances were fixed by RFC-0.29-001: **21 of 91 workspace
  crates are never named in any `ci.yml` job** — the six gate-tool crates
  (E-013, above) and the three crates backing Gate 8's validation drills
  (`fjell-sig-ed25519`, `fjell-fleet-sync`, `fjell-config-sync`) now all run
  via the `ci-host-bins` job, closing this entry's own "possibly
  intentional; nothing in the workflow says so" — it was not intentional,
  and their tests run in ordinary CI now (Gate 8's drill *markers* stay
  rehearsal-only by design; that mechanism is separate and untouched).
  `ci-qemu-negative`'s matrix, `NEG_CATEGORIES`, and the
  `KNOWN_V01X_CATEGORIES`/`KNOWN_V02_CATEGORIES` lists were all **removed**
  — `qemu_run::discover_negative_categories`, derived from
  `tests/qemu/profiles/*.toml`, is the one answer every call site uses.
  **The fourth, surviving instance is now fixed too**: `smoke.rs`'s
  `v0.6-verification` milestone — mapped to a marker
  (`TEST:V0.6-VERIFY:PASS`) no kernel or service code has ever emitted, and
  invoked by nothing, anywhere — is deleted rather than wired up (there is
  no v0.6 functionality behind it to wire up to); `cargo xtask qemu-test
  v0.6-verification` now correctly reports `unknown milestone`.
  `fjell-driver-uart`, `fjell-svc-fault`, and `fjell-svc-timeout` remain
  absent from `ci-cross-check`'s crate list — a `cargo check` coverage gap,
  not a test gap (all three have zero `#[test]`s today), outside this
  erratum's own test-execution framing; disclosed, not part of its closure.

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

- **The instrument audit's `sound` verdicts were not all demonstration-backed**
  (Errata **E-017**, **CLOSED** by **RFC-0.29-002**). RFC-0.24-001 requires
  every instrument claimed `sound` to carry a committed demonstration of it
  failing, and records `UNAUDITED` otherwise. Two rows (`ci-proptest`, `Gate
  4`) were found violating that in the 0.24 review and repaired then; the
  re-derivation of the remaining rows was left incomplete, and the register's
  own recount (2026-09-09) itself undercounted — 21, not 22 — by missing
  `ci-verus`'s differently-bolded `**sound (by explicit design)**` heading.
  All 22 rows are now re-derived or corrected: 4 rows that cited only a
  tool's own unit suite (Gate 2, Gate 11, Gate 12, `syscall/expected.toml`)
  had their citations corrected to the real, specific demonstrations that
  already existed (a live category violation for Gate 2; the
  `SYSCALL-CALLSITE-001`/`-002` regression tests for Gate 11; one named
  failing-test per subcheck for Gate 12's now-ten subchecks); 4 rows with no
  demonstration at all (`fjell-abi-snapshot` ×2, `repro/baseline-digests.txt`,
  `ci-arm64-check`) each got one produced live against real committed data
  (a reverted-and-restored scanner regression, a corrupted digest byte, a
  broken cross-compile). **No instance survives.** This is why RFC-0.24-001
  shipped `Implemented-with-Errata` rather than `Implemented`.

- **The `ipc` negative profile's blocked-recv scenario assumed an
  unsynchronised scheduling order** (Errata **E-019**, **CLOSED** by
  **RFC-0.28-003**). `fjell-neg-test`'s `test_ipc_blocked_recv` documented,
  in its own source comment, an assumption about *relative* task-scheduling
  order — "sample-service immediately calls `sys_ipc_recv` and blocks before
  the scheduler returns to neg-test" — that RFC-0.26-001's fairness fix no
  longer guarantees. (`fjell-sample-service`'s startup intent emission was
  originally recorded here too; it is a service rather than a harness and
  was split out as **E-020**, closed separately by RFC-0.26-004 — see
  below.) A predecessor line, RFC-0.26-003, concluded no signal existed to
  wait on and none could be built — **false**: the kernel is the authority
  on a task's blocked state (`sys_task_status` already dispatched, already
  imported by `fjell-neg-test`) and can be polled; the real gap was
  *addressing* (`sample-service` isn't a task `neg-test` spawned itself, so
  it had no `TaskId` to poll). RFC-0.28-003 closed the addressing gap with
  a one-way, kernel-attested identity exchange on their already-dedicated
  endpoint (RFC 042's object 6, not the contested shared object 0) and
  replaced the single defensive `sys_yield()` with a bounded poll, failing
  closed with a new marker on exhaustion. Demonstrated live, both
  directions: with the wait removed, `sys_task_status` reads `Runnable`,
  not `Blocked`, and the profile now correctly reports FAIL; with the real
  poll, `sample-service` reaches `Blocked` after exactly 2 iterations,
  measured repeatedly — precisely characterising what the old single-yield
  comment was, in fact if not by contract, relying on.

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

- **`init` no longer receives on any service's own endpoint** (Errata
  **E-024**, **CLOSED** by RFC-0.28-001). `storaged`, `measuredd`, `attestd`
  and `recoveryd` used to announce readiness into the same endpoint object
  they later receive protocol traffic on, rescued from self-deadlock only by
  `init` also holding receive rights there and reaching each wait in a fixed
  boot sequence — the same missing-`else`-shaped hazard as E-021, on four
  more objects. RFC-0.28-001 gave service-manager its own dedicated
  readiness endpoint (distinct from the "shared" object 0, which `auditd`
  and `bootctl` independently default to and were found, live, to be racing
  service-manager for the same messages), narrowed `init`'s capability on
  all four remaining objects to `CALL` only, and replaced the direct
  per-service waits with a relay through service-manager. The invariant "a
  service's endpoint has exactly one receiver" now holds structurally, not
  by convention, on every object it names.

- **Two tools walked untracked scratch trees, with disagreeing exclusion
  lists** (Errata **E-025**, **CLOSED** by RFC-0.28-005). `trust-report`'s
  cap-manifest scan and `fjell-unsafe-audit`'s walk each hand-maintained a
  different set of skipped directories, and neither excluded
  `.git-exclude/` — the directory this project's own conventions use for
  scratch work. A checkout there took the cap-manifest count from 1 to 2
  and doubled the reported unsafe-site inventory (demonstrated live against
  a fresh clean-clone checkout: 284/284 to 568/568 — the RFC's own cited
  311/622 had already gone stale relative to `HEAD` by the time this was
  fixed, an unrelated drift checked and reported rather than assumed). Both
  readings were internally consistent, so nothing in the output said which
  repository it described. Fixed by deriving each tool's scope from `git`
  itself rather than an enumerated name list: `fjell-unsafe-audit` (which
  must still see a developer's uncommitted work) now uses
  `git ls-files --cached --others --exclude-standard`; `trust-report`'s
  cap-manifest scan (which reports on the repository as shipped) now uses
  `git ls-files` alone. Neither hand-lists `.git-exclude` or any other
  scratch-directory name; a scratch checkout is excluded because
  `.gitignore` already says so, not because a person remembered to add it.


- **No QEMU serial log had ever been committed alongside the document that
  cites it** (Errata **E-026**, **CLOSED** by RFC-0.27-004). E-013 leaves
  nothing kernel-side host-testable, so QEMU logs are the only evidence for
  kernel behaviour, and `.gitignore:28` (`*.log`) kept all of them out of the
  tree; `tests/qemu/artifacts/` was overwritten by the next run, and
  `test-all`'s per-tier logs under `tests/runs/` capture build stdout rather
  than the serial transcript. `tests/evidence/` (a narrow, deliberate
  `.gitignore` exception) plus `cargo xtask evidence promote` now let a
  citation be committed with mandatory provenance — including whether the
  build was instrumented, since a provenance block naming only a commit sha
  would let a reader assume a reproducibility the log may not have. Gate 12's
  `evidence` subcheck holds this both ways: every citation must resolve with
  valid, ancestor-checked provenance, and every promoted file must be cited
  by something. **Of the citations this project had already made, 1 of 3 was
  resolvable** and was promoted (RFC-0.27-002's); the other 2 were not — see
  **E-029**, below.

- **Nothing checks that the threat model's threats cite RFCs** (Errata
  **E-027**, ACCEPTED). The v0.9–v0.15 handoff asserted a "threat-model gate"
  enforcing it; no such gate exists in any commit on any branch. The property
  holds as of 2026-08-31 — all 20 `Tn` sections in
  `docs/security/threat-model-v1.md` cite an RFC, and the 20 in-scope / 8
  out-of-scope counts are correct — but it is held by hand, and a regression
  would be reported by nothing.

- **`fjell-sxt-crypto`'s doc-comment cites two documentation files that do
  not exist** (Errata **E-028**, ACCEPTED). RFC-v0.7.3-002 (Implemented,
  v0.7.1) specified `docs/src/security/crypto-profile.md` and
  `docs/src/security/crypto-roadmap.md` as deliverables — the latter is
  named in that RFC's own acceptance criteria — but neither exists anywhere
  in the tree, while the crate's live doc-comment still points at both.
  The underlying disclosure (development-only crypto, not for production
  use, documented cache-timing leak) is not lost — it is in the
  doc-comment itself — but the dedicated write-up is missing. Found while
  building the CRA/IEC standards mapping (RFC-0.27-003) and tracing its
  confidentiality-clause evidence into this crate.

- **Two historical QEMU-log citations were unresolvable** (Errata
  **E-029**, **CLOSED** by RFC-0.28-005 R3). RFC-0.27-004's R6
  reconciliation found the `tests/qemu/artifacts/semantic/serial.log`
  content cited by `RFC-0.26-004-readiness-channel-answer.md` and by the
  archived `RFC-0.26-002-abdd-path-synchronisation.md` no longer existed —
  overwritten by later runs before this RFC's per-run retention existed.
  Not re-run to stand in for the originals (D4); the first was annotated
  in place, the second (already `Superseded`) was left as the point-in-time
  record it already was. Fixed by re-running the `semantic` profile fresh
  (commit `6582d04`, clean tree) and promoting it with real provenance
  (`tests/evidence/RFC-0.28-005/semantic-fresh-2026-09-08.log`); the
  readiness-channel answer document now cites the fresh run **alongside**
  the historical annotation, which stays rather than being deleted once
  superseded. The archived RFC-0.26-002 citation is left as-is, per this
  project's practice of not rewriting archived records.

- **Nothing checked that the project's two version strings agree** (Errata
  **E-030**, **CLOSED** by RFC-0.28-005). `[workspace.package] version` and
  `crates/fjell-os/Cargo.toml`'s `fjell-abi` version pin must match at every
  release and are maintained by hand; `version-currency` checked `README.md`
  only, not this pair — the gap that cost the 0.27.0 cut its first command.
  `version-currency` now also compares the pair directly, demonstrated
  failing on the exact mismatch the 0.27.0 cut hit (`0.26.0` pin against a
  `0.27.0` workspace version). The check states its own limit in its
  failure message: the mismatch was already fail-closed (`cargo metadata`
  cannot resolve, so every gate, tier, and build fails with it regardless),
  so this buys the time of a named failure rather than adding correctness a
  passing run didn't already have.

- **RFC 058's readiness tracking now completes** (Errata **E-031**,
  **CLOSED** by RFC-0.28-001). `service-manager` used to emit
  `NEG:SVC:READY_ACCEPTED:PASS` only once 10 services reported ready — a
  threshold re-derivation found unreachable by an even wider margin than
  first thought (3 of 14 images could reliably reach service-manager under
  the old topology, not "at most 5": its own receiving object was also
  `auditd`'s and `bootctl`'s default, and live-verified to lose messages to
  both). RFC-0.28-001 gave service-manager a dedicated, uncontested
  endpoint and re-derived the threshold to **8** — the number of images
  that actually send `tags::SERVICE_READY` today, confirmed live and
  reproducible over repeated runs, not lowered to force the marker.

- **35 hand-rolled syscall `asm!` blocks across 14 crates carried three
  register-contract bugs** (Errata **E-032**, **CLOSED** by **RFC-0.28-002**).
  Twelve omitted the `a6` clobber (`IpcRecv`); eighteen declared `a0` as a
  plain input where the kernel writes status (`IpcRecv`/`IpcReply`); three
  `IpcCall` sites didn't declare the reply-word registers `sys_ipc_reply`
  overwrites unconditionally on completion. RFC-0.28-001 hit the first bug
  in two blocks it wrote — a non-deterministic permanent hang that debug
  prints made disappear — and fixed those two; RFC-0.28-002 deleted the
  other 28 hand-rolled blocks (every syscall they issued already had an
  audited wrapper) and fixed the register contract of the 7 kept, where no
  existing wrapper covers the shape needed (4-word `IpcCall`, worded
  `IpcReply`). Closed structurally: Gate 11's `SYSCALL-CALLSITE-001` refuses
  any raw syscall-issuing block outside `fjell-syscall` unless it is on an
  explicit, guard-owned allowlist naming exactly those 7 sites, each still
  checked for correct clobbers. A related gap found during the same audit —
  `fjell-syscall`'s own `sys_ipc_recv`/`sys_cap_inspect`/`sys_ipc_call_words`
  carried the identical defect class internally — is **E-033**, closed by
  RFC-0.28-004 (below).

- **Four `send` helpers accept a payload word that is never transmitted**
  (Errata **E-034**, ACCEPTED). `sxt_send`, two `send_tag`s and `send_sxt` take
  a data word; none packs a word count into the tag, so the kernel copies zero
  words and the payload has never reached a receiver. Nothing reads these words
  today; the hazard is the next caller who trusts the signature.

- **`fjell-syscall`'s own helpers carried the register-contract defect the
  crate exists to protect everyone else from** (Errata **E-033**, **CLOSED**
  by **RFC-0.28-004**). `sys_ipc_recv` lost the delivery's words and attested
  sender identity; `sys_ipc_call_words` never declared `a5` at all (a third
  instance, found extending the guard into this crate, not in the original
  scoping). `sys_cap_inspect`'s actual defect was more precise than first
  scoped: the second call it issued was `CapRevoke` (13), not `CapInspect`
  (14) — a stale literal, not a scheduling race. Demonstrated live: the one
  real caller (`fjell-proxy-text`) currently gets the *correct* rights value
  only because the inspected capability correctly lacks `REVOKE`, so the
  wrong-numbered call fails closed without touching the registers the first
  (correct) call had already written — undefined behaviour appearing correct
  by coincidence, not a guaranteed contract. All three now issue one
  correctly-declared syscall each; `SYSCALL-CALLSITE-002` (Gate 11) enforces
  it inside `fjell-syscall` going forward. `sys_ipc_recv`'s long-term future
  (repair vs. remove) remains an open escalation, not resolved by this fix.

- **The ABI baseline used to never be enforced current** (Errata **E-035**,
  **CLOSED** by **RFC-0.30-002**). `tests/abi/snapshot.json` used to hold 413
  items against a tree of 418; Gate 4 reported `Added: 5 (additive — OK)` and
  passed, correctly under the old rule. Removals and signature changes were
  still caught — but the baseline no longer described any shipped release, and
  a future regeneration would have absorbed every accumulated addition in one
  unreviewed step.

  **Fixed:** `fjell-abi-snapshot --verify` now fails whenever `Added != 0`, not
  only on `Removed`/`Changed sig` — the same gate, a stricter pass condition.
  Argued on measured cost, not assumed: `tests/abi/snapshot.json` has been
  touched in 8 commits across this project's entire 176-RFC history, so the
  gate is red only on the rare line that actually touches the stable surface,
  and only until that same line runs `--generate`. Demonstrated failing on a
  deliberately un-regenerated baseline (3 real current items held back via a
  scratch `--snapshot` copy, no tracked file touched): `Added: 3`, `Result:
  FAIL`, naming all three. `docs/src/release/v0-release-cycle.md`'s cut-time
  step is now a confirmation that the per-line discipline held, not a task.

- **The two-build reproducibility check used to never run, and could not fail
  if it were** (Errata **E-036**, **CLOSED** by **RFC-0.30-001**).
  `fjell-repro-check`'s two-build mode used to run `cargo xtask build` twice
  with no clean and no separate target directory, so the second build was an
  incremental no-op and the comparison was between a file and itself —
  measured at 0.41s and 0.40s per "build". Every call in the tree passed
  `--skip-build`, a staleness check on committed artefacts, not a
  reproducibility check, and neither mode named the kernel binary anywhere
  even though the two-build path already collected it.

  **Fixed:** a scoped `cargo clean --release --target
  riscv64gc-unknown-none-elf` now precedes each of the two builds, so the
  second cannot reuse the first's output. Measured, not assumed: a genuine
  two-build run costs **4–7 seconds total** (this project's crates are small
  — one kernel, 29 services, `opt-level = "s"`), not the "doubles CI build
  time" the RFC worried about before anyone had timed it. It now runs in CI
  on every push (`ci-repro-check`, `cargo xtask two-build-check`), not
  nowhere.

  **The build is, as measured, reproducible.** Two independent runs (each
  with its own clean) produced bit-for-bit identical output across all 30
  artefacts — the kernel ELF plus all 29 service prebuilts, both counts
  re-derived and confirmed — checked twice on 2026-09-09. Building the
  identical commit from a different absolute checkout path also reproduced
  identically, ruling out the classic embedded-build-path hazard for this
  toolchain/profile. This is **same-machine** reproducibility only —
  cross-machine remains untested and unclaimed while the toolchain is
  unrecorded (**E-037**, open).

  **The check's sensitivity was demonstrated, not assumed to exist:** forcing
  a differing `-C metadata` value scoped to the `riscv64gc-unknown-none-elf`
  target only (no kernel/service source touched) reproduced this project's
  own prior incident — `-C metadata` moving digests on a version bump
  (RFC-v0.16-005 H-04) — and the actual fixed comparison correctly reported
  `FAIL` on 14 of 29 service prebuilts.

  **Kernel coverage was already there, just unstated:** the two-build path's
  `DEFAULT_TARGETS` always included the kernel ELF alongside `prebuilt/` (30
  artefacts total); this is now said out loud (T20, `ERRATA.md`, here).
  `--skip-build`'s committed baseline stays 29, services only, by design —
  the kernel is not a committed artefact, so there is no baseline digest of
  it to record. Two checks, two honestly different scopes.


- **The toolchain is declared in five places, and recorded nowhere** (Errata
  **E-037**, ACCEPTED). `rust-toolchain.toml` (channel, `rust-src`, the RISC-V
  target), `.github/workflows/ci.yml` (`apt-get install rustc-1.91`, in several
  jobs), `docs/release/release-checklist.md`, `docs/src/internals/local-
  development.md` and `Cargo.toml`'s `rust-version` all state `1.91`
  independently; two of them are checks that would keep asserting `1.91` after
  a bump. The channel still floats within `1.91.x`, and a patch bump moves
  codegen and therefore digests. Nothing records which toolchain produced the
  repro baseline, so a cross-machine digest mismatch is indistinguishable from
  a real reproducibility failure.

  *Corrected 2026-09-10. This bullet previously said `rust-toolchain.toml` had
  been removed and that no `rust-version` field existed. Both were true for one
  day: the file was removed on 2026-09-09 (`4cebbc4`), restored the same day
  after the removal was found to have silently moved local builds from 1.91.1
  to 1.98.1 and changed all 24 committed prebuilts, and `rust-version = "1.91"`
  was added to `[workspace.package]` at the same time. The bullet was not
  updated then.*

- **Four subchecks used to fail without a result line naming themselves**
  (Errata **E-038**, **CLOSED** by **RFC-0.30-002**). With an RFC lifecycle
  folder absent, `rfc-status-folder`, `handoff-status`, `errata-tracking` and
  `doc-counts` all used to fail with a bare `consistency-check: cannot read …`
  line — no subject, so the run ended `consistency-check: FAIL` with no way to
  tell which of ten checks broke. (`handoff-status` was not actually silent, a
  correction from this line's own diagnosis: depending on which RFCs currently
  had handoffs, it either produced the same unnamed message or — worse —
  **passed incorrectly**, blind to the missing folder because it only ever
  touched lifecycle folders incidentally, through whichever RFC a handoff
  happened to cite.)

  **Fixed:** `read_file`/`read_dir_named` (`tools/fjell-consistency-check`) now
  take the calling subcheck's own name and print `<name>: FAIL — <reason>` on
  any I/O failure — applied everywhere that shape appeared in the crate, not
  only these four — the sweep reaches all ten subchecks, the other six being
  `doc-links`, `version-currency`, `syscall-surface`, `evidence`,
  `standards-mapping` and `errata-limitations`.
  `handoff-status` additionally now enumerates `rfcs/{proposed,accepted,done}`
  directly before touching any handoff, closing the blind-pass gap. Keeper
  files still exist in all four lifecycle folders, unrelated and unchanged —
  this fix is for when one goes missing anyway.


- **The release cut is the only work in this project nobody reviews** (Errata
  **E-039**, ACCEPTED). The cycle's Roles table makes the implementer
  Responsible for verifying exit criteria and producing the release record, with
  the architect Accountable and Consulted. In practice the architect has done
  all of it for five consecutive releases, so those changes land unreviewed —
  and they have included a four-commit staging failure, a `rfcs/accepted/`
  directory that vanished from fresh clones, and document corrections nobody
  checked.

- **QEMU negative-test coverage status (v0.19/v0.20).** The nine main
  negative categories now run real QEMU profiles with fail-closed marker
  checking (a wrong error, an unexpected success, or a panic in the serial
  log fails the run). All nine now have every specified marker confirmed
  (capability 8, mmio 3, dma 3, audit 1, user-copy 2, policy 4, harness 1,
  **svc 4/4 — Errata E-024/E-031, closed by RFC-0.28-001**, evidence:
  [`tests/evidence/RFC-0.28-001/svc-ready-accepted-unauthorized-rejected.log`](../../tests/evidence/RFC-0.28-001/svc-ready-accepted-unauthorized-rejected.log));
  the ipc profile is restored to 3/3 in v0.20.0 after fixing the IPC words ABI and the
  reply-edge cancellation path. The `store` and `upgrade` negative profiles
  exist as marker specifications but have **no emitting scenarios yet** and
  are explicitly **not v1 release-gated**; running them manually fails
  honestly rather than placeholder-passing.

- Several services in the QEMU image are **smoke-test stubs** that signal
  ready and exit by design (`fjell-netd`, `fjell-secure-transportd`,
  `fjell-driver-virtio-net`, `fjell-proxy-text`, `fjell-driver-virtio-blk`,
  `fjell-powerd`, among others). Their full implementations are tracked on
  the post-v1.0 roadmap; their early-exit pattern is intentional.
- The repro-check baseline (`tests/repro/baseline-digests.txt`) tracks the
  committed `prebuilt/*.bin` artefacts and must be re-recorded whenever the
  prebuilt service binaries are rebuilt — see `tools/fjell-repro-check`.
