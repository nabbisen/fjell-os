# Fjell OS Roadmap

Development proceeds as a series of focused milestones.  Each milestone
produces a named release archive.  No milestone stretches into the territory
of the next; scope discipline is a first-class constraint.

---

## v0.1.0 — Initial Release

### M0 · Repository Foundation ✅
- Cargo workspace with all crate skeletons
- `no_std` kernel crate, panic handler
- Documentation skeleton, ADR template
- CI pipeline skeleton
- `LICENSE`, `NOTICE`, `TERMS_OF_USE.md`

### M1 · Bootable Kernel
- Linker script (`link.ld`) for QEMU `virt` RAM at `0x8000_0000`
- `_start` assembly: hart selection, BSS clear, stack pointer
- UART 16550A driver (MMIO `0x1000_0000`)
- `kmain()` prints boot banner
- `cargo xtask qemu` runner

### M2 · Memory and Task Isolation
- M-mode shim → S-mode kernel handoff
- DTB-based physical memory discovery
- `BootAllocator` + bitmap `FrameAllocator`
- Sv39 page tables; shared kernel map + per-task user maps
- `TrapFrame`, `KernelContext`, `Task`, `TaskTable`
- Fixed-priority round-robin scheduler, idle task
- `sys_yield`, `sys_exit`
- User page-fault containment → `TaskState::Faulted`
- QEMU smoke test: `TEST:M2:PASS`

### M3 · IPC and Capability
- Synchronous rendezvous `Endpoint`
- `Capability`, `CapRights`, generation-tagged `CapHandle`
- Derivation tree, `cap_copy / cap_mint / cap_delete / cap_revoke`
- `ipc_send / ipc_recv / ipc_call / ipc_reply`
- One-shot reply edge
- Audit hooks for cap / IPC events
- QEMU smoke test: `TEST:M3:PASS`

### M4 · init / service-manager
- `fjell-init` user-space service
- `fjell-service-manager` with TOML service manifest
- Sample service lifecycle (start / exit / fault)

### M5 · Audit and State Export
- `AuditEvent` ring flush to `fjell-auditd`
- JSON Lines export
- `previous_hash` chain for tamper evidence

### M6 · Declarative Configuration
- TOML config schema + validation
- Dry-run, apply, rollback metadata

### M7 · Semantic Stream and Text Proxy
- `IntentNode` full schema
- `fjell-proxy-text` renderer
- `fjell-sample-service` emits intent

### M8 · v0.1.0 Hardening
- Property tests (`proptest`) for cap / IPC / scheduler
- Full unsafe audit with SAFETY comments
- Documentation review
- `CHANGELOG.md` entry, release tag

---

## Post v0.1.0

### v0.1.x — Stabilization / Audit / CI Foundation (in progress)

The v0.1.x release line freezes the v0.1.0 prototype, documents its
limitations, and adds the audit + CI foundation needed before
v0.2 modifies security boundaries. It adds no new OS functionality.

See [`docs/src/roadmap/v0.1.x-stabilization.md`](docs/src/roadmap/v0.1.x-stabilization.md)
and RFCs 024–030, 044–047 (`rfcs/`).

| Version  | Theme                                       | RFCs landed       |
|----------|---------------------------------------------|-------------------|
| v0.1.1   | Release freeze + CI foundation              | 024, 025          |
| v0.1.2   | Negative tests + threat model + ABI         | 026, 027, 028     |
| v0.1.3   | Capability / Lease / MMIO / DMA / Evidence  | 029, 030, 044     |
| v0.1.4   | ADR sync + release checklist                | 045, 046          |
| v0.1.5   | v0.2 preparation backlog                    | 047               |

### v0.2.0 — Security Boundary Closure (in progress: v0.2.9 hardening, post-review)

The first post-v0.1.x hardening milestone. Turns Fjell OS from a
local verified prototype into a system whose core security
boundaries are uniformly enforced. See the v0.2 RFC set (RFCs
031–043) and [`docs/src/security/v0.1.0-threat-model.md`](docs/src/security/v0.1.0-threat-model.md) §14.

| Phase | Name                                        | RFC      | Status |
|-------|---------------------------------------------|----------|--------|
| 1     | Capability Enforcement Core                 | 031, 032 | ✓ |
| 2     | Lease Revocation Semantics                  | 033, 034 | ✓ |
| 3     | MMIO Boundary Closure                       | 035      | ✓ |
| 4     | DMA Boundary Closure                        | 036      | ✓ |
| 5     | Cooperative Service Separation              | 037, 038 | ✓ |
| 6     | User Copy and Audit Drain                   | 039      | ✓ |
| 7     | cap-broker Bootstrap and Policy Enforcement | 040      | ✓ |
| 8     | Persistent Evidence Hardening               | 041      | ✓ |
| 9     | Negative Test Completion + Release Gate     | 042, 043 | ✓ |

**v0.2.9-v0.2.14 hardening releases** (COMPLETE):

| Release | Scope |
|---------|-------|
| v0.2.9 | ABI / test-harness correction (this release) |
| v0.2.10 | Capability/syscall enforcement closure |
| v0.2.11 | MMIO/DMA/audit hardening |
| v0.2.12 | Service separation + release-gate close |

**`TEST:V02:PASS` earned at v0.2.14 close.**

### Beyond v0.2 — executed

All post-v0.2 release lines are complete through **v0.21.2** (the current
release). Each line delivered a coherent theme:

| Line | Theme | Status |
|---|---|---|
| v0.3.0 | Hardware Trust Abstraction | ✅ |
| v0.4.0 | Minimal Secure Networking | ✅ |
| v0.5.0 | Multi-Platform Foundation + Semantic API Stabilization | ✅ |
| v0.6.0 | Verification / Property Testing (original M10) | ✅ |
| v0.7.0 | Distributed Snapshot Sync Foundation | ✅ |
| v0.8.0 | Fleet / Edge Operations Plane | ✅ |
| v0.9.0 | Developer Service Platform (original M11) | ✅ |
| v0.10.0 | Release Maturity (reproducible build, ABI, gates) | ✅ |
| v0.11.0 | Trust Spine Hardening | ✅ |
| v0.12.0 | Deployment Profile Hardening | ✅ |
| v0.13.0 | Fleet Reliability and Recovery Depth | ✅ |
| v0.14.0 | Developer Ecosystem Trial | ✅ |
| v0.15.0 | v1.0 Freeze Candidate | ✅ |
| v0.16.0 | Ed25519 Interoperability Closure | ✅ |
| v0.17.0 | Trust Anchor Provisioning and Manufacturing Flow | ✅ |
| v0.18.0 | Verus Promotion to Release-Required | ✅ |
| v0.19.x | Negative-test conversion (found six latent kernel bugs) | ✅ |
| v0.20.x | v1-readiness: fail-closed gate, IPC ABI fix (E-010) | ✅ |
| v0.21.x | Crate reorganization, audits, handoff + design docs | ✅ |
| v0.21.3 | Build restoration, as-built reconciliation, v0 release cycle | ✅ released |

### v0.22 — Gate Integrity (planned; owner-approved 2026-07-30)

v0.21.3 found **four** separate instances of a mechanical gate reporting
green while a documented rule went unmet. That is one class of defect, not
four bugs. Every completion claim in this project is settled by the eleven
gates, so v0.22 makes them mean what they claim before further function is
built on top of them.

Governing principle for the line: **every gate added or strengthened must be
demonstrated failing on a deliberately broken input before it is accepted.**

| # | Item |
|---|---|
| 1 | Gate 11 from substring matching to a real function-body scan (architect review H-03) |
| 2 | Gate 4 ABI signature normalisation — today a whole-tree `cargo fmt` invalidates the baseline wholesale |
| 3 | Mechanical syscall-count check, to stop documented-surface drift recurring |
| 4 | Bind documented rules to gates where cheap (ACCEPTED errata ↔ limitations; RFC folder ↔ Status; handoff status inheritance) |

Governed by `RFC-v0.22-001`. Out of scope: negative-coverage completion, the
9 undispatched syscalls, build determinism, DMA unmap, and anything touching
kernel/ABI/crypto behaviour.

### v0.23 — ABDD Live Path (planned; owner-approved 2026-07-31)

The first line in several to add runtime behaviour rather than documentation or
tooling. Fjell's distinguishing claim — applications emit meaning, a proxy
renders it — is currently demonstrated only by unit tests: `proxy-text` holds
845 lines of working renderer behind an entry point that prints one line and
exits, and `semantic-stream` is the same shape.

v0.23 connects them. A real service emits an intent node, `semantic-stream`
routes it, `proxy-text` renders it, and the proxy's return leg issues a
capability-checked `ActionRequest` — proven by refusal, not only by success.
The path is gated by a fail-closed QEMU profile so it cannot rot.

Adds no kernel surface and no syscalls. Governed by `RFC-v0.23-001`.

Chosen from four measured directions (`docs/src/roadmap/v0.23-direction-options.md`)
because it is roughly an order of magnitude smaller than any alternative,
depends on nothing, and is the only one producing a claim the project cannot
currently make.

### 0.24 — Instrument Audit and Repairs (**shipped 2026-08-03**)

Do the checks check what they claim? Eleven instruments have been caught
reporting success without having checked — and **every one was found
incidentally**, while doing something else. There are roughly **55**: 12
release-rehearsal gates, 19 `test-all` tiers, 16 CI jobs, and 8 committed
artifacts that assert repository state. They have never been audited as a set.

v0.23 made the project more *capable* — the ABDD path runs. It also revealed
that four smoke profiles had been attesting the wrong thing and that a whole
test tier never executed. Both by luck. 0.24 makes the project more
*trustworthy*, on the reasoning that every future capability line is declared
complete by these same instruments.

Audit-only: findings are reported and dispositioned individually, never fixed
in-pass. Governed by `RFC-0.24-001`, which carries the taxonomy — scope
blindness, proxy attestation, fail-open on absence, weak predicate, stale
assertion — derived from the eleven known instances.

**Outcome.** 58 instruments — **22 sound, 33 findings, 3 `UNAUDITED`.** Before
this line, all of them were reporting green. The audit was followed by two
repair lines it did not originally anticipate: `RFC-0.24-002` (seven
instruments that could not be trusted through a cut, including a first
mechanical gate that passed on a non-compiling workspace and a CI job named
"Property tests" that ran zero) and `RFC-0.24-003` (the ABI gate could not
identify 45 of its own 423 items, and had never seen two functions in the crate
carrying the syscall ABI).

**The honest summary is that this milestone made the instruments more honest,
not honest.** 33 findings remain open under errata E-013 through E-017; the 22
`sound` verdicts are themselves provisional, because two were found violating
the audit's own demonstration rule and the re-derivation of the rest is
incomplete (**E-017**). Records: `docs/verification/instrument-audit.md`,
`docs/verification/instrument-audit-closeout.md`,
`docs/release/records/0.24.0.md`.

### 0.25 — Functional advancement: the external interrupt plane (**shipped 2026-08-16**)

**Release stability is deprioritised.** The owner has directed functional
advancement first — service plane and human operability — with the instrument
audit's 0.25 candidates deferred behind it. Errata E-013 through E-017 stay
open and disclosed; new functional work will be certified by instruments
carrying 33 open findings, which is a stated and accepted cost.

Scoping the two chosen themes found them **coupled, and both blocked on the
same wall**: of the nine declared-but-undispatched syscalls, three are
`IrqBind` / `IrqAck` / `IrqWait`. Measured state of the tree:

- `scause` cause 9 (`SupervisorExternal`) is **not decoded** — it falls to
  `Other(scause)`, logged-and-ignored in user mode and **panicking in kernel
  mode**.
- There is **no PLIC driver** anywhere in the kernel.
- `IRQ_BIND` / `IRQ_UNBIND` / `IRQ_ACK` exist **only inside a doc comment**;
  no constants are defined.
- The UART driver is **TX-only** — `THR`/`LCR`/`FCR`, no `RBR`/`IER`/`LSR` —
  and `console.rs` has no read path of any kind.
- **`fjell-driver-virtio-net` calls all three syscalls**, gets
  `UnknownSyscall` from `sys_irq_bind`, prints "IRQ bind failed" and
  `sys_exit(1)`. **It has never once got past its own initialisation**, so the
  RX queue drain and `netd` notifications written under RFC-v0.7.3-001 have
  never executed.

So the project has been carrying a designed, documented interrupt architecture
with nothing underneath it — E-011's shape, one layer wider. `RFC-0.25-001`
builds the floor: PLIC, trap decode, the rights constants, the three dispatch
arms, UART RX, and a `fjell-driver-uart` service. Gate 12's `syscall-surface`
moves **35/26/9 → 35/29/6**.

**Correction to the service-plane figure below.** This roadmap has said "17 of
29 services never receive IPC." Measured by recv-loop presence: **15 of 29**.
Neither number is trustworthy enough to scope against — the grep only catches
direct `ipc_recv`/`try_recv` — and an actual count is a prerequisite for taking
the service-plane theme.

**Outcome.** Shipped as `0.25.0`. `RFC-0.25-001` built the floor — PLIC driver,
`SupervisorExternal` decode, the three rights constants, three dispatch arms,
UART RX, and `fjell-driver-uart` — and `syscall-surface` moved **35/26/9 →
35/29/6**. `RFC-0.25-002` adopted the 5-folder RFC lifecycle and gave RFC 000 a
folder-as-source-of-truth rule it had been cited for without containing.

**A byte typed at the console now reaches a userspace service over a
capability-checked, interrupt-driven path.** Not usable — there is no shell, no
command set, no line editing. Interactive.

Two things the line turned up that outlive it. `fjell-driver-virtio-net` gets
past `sys_irq_bind` for the first time since v0.4 and now fails one layer
further along for an honest reason (nothing ever populated its `CAP_IRQ` slot),
so the network receive path has still never executed. And **E-018**: the
scheduler's `PRIORITY_USER` has three disconnected copies with two values, so
`init` preempts every other spawned task — invisible until now because every
prior `init` path used a blocking recv that removed it from ready-queue
contention. Correcting the constant hung the M6 boot sequence and was reverted.

Records: `docs/release/records/0.25.0.md`.

### 0.26 — the scheduler priority defect, and what it was holding up (**shipped 2026-08-27**)

**E-018 becomes its own RFC.** Three copies of one constant with two values, a
narrow `image_id`-keyed stopgap shipped in 0.25.0, and a proper fix that hangs
the M6 boot sequence — meaning something already shipped depends on the broken
ordering in a way nobody yet understands. That investigation, not the cleanup,
is the line.

**Outcome.** Shipped as `0.26.0`. `RFC-0.26-001` unified the constant, but the
deliverable was the investigation: correcting it alone hung M6 boot, and the
cause was `svc-timeout` — RFC 042's negative-test service, an infinite
`sys_yield()` loop *by design*, occupying the top ready-queue bucket
permanently once its two enqueue paths disagreed.

Removing the asymmetry then stopped the **ABDD live path** running at all, and
the first attempt to fix that **could not be built** — which is how the real
defect surfaced. `semantic-stream` and `proxy-text` announced readiness *into
their own endpoints* while `init` held receive-capable capabilities to the same
objects: two receivers on one queue, readiness sharing a channel with protocol
traffic, and `wait_ready_exact` discarding what it did not recognise **without
replying**. `RFC-0.26-004` established the invariant — *a service's endpoint has
exactly one receiver* — and `RFC-0.26-002` was superseded, its premise having
been false.

**The pattern is the result.** E-019, E-020 and E-021 are one defect at three
depths: a test assuming a peer has blocked, a service assuming its peers are
ready, and the mechanism those were to be fixed *with* assuming exclusive use of
a shared channel. Each was invisible while a scheduler bug happened to make it
true.

**Still open.** **E-022** — `sys_ipc_send` blocks the sender on a queued one-way
send, against its own documented contract; found because removing the accidental
co-receiver removed its cover. **E-019** — the `ipc` profile is green again and
**nothing guarantees it**, which makes `RFC-0.26-003` more necessary than when
it was red, not less.

Records: `docs/release/records/0.26.0.md`.

### 0.27 — undecided

Candidates carried in: **E-022** (kernel IPC contract), **E-019** /
`RFC-0.26-003` (the green-by-accident test), the capability rights-narrowing
follow-up from RFC-0.26-004's review, and the 0.24 audit's still-open families
(E-013 through E-017).

### Beyond 0.26 — under discussion, not yet decided

**v1.0 is explicitly not in view** (owner, 2026-07-30); v0 development
continues. The owner has directed that functional advancement, not only
stabilization, must precede any v1.0 consideration — the current state is
far from production readiness or demonstrable appeal.

Two directions have now been chosen from the options paper — v0.23 (semantic
plane) and 0.24 (instrument audit, above). **0.24 has closed**, so the
remaining three are re-opened and undecided, alongside the 0.25 candidates the
audit's close-out produced (§6 there: a `release-rehearsal` proptest gate, the
literal-predicate design answer for E-014, a link-and-count integrity
instrument for E-016, CI list reconciliation for E-015, E-013's fix, and
completing E-017's re-derivation):

- Make the service plane real — 17 of 29 services never receive IPC
- Make it operable by a human — **kernel work first**: no console input path
  exists at any layer, so this needs UART RX, an interrupt path, and a read
  syscall before any userland command set
- Make it run on metal — hardware bring-up, currently placed at v2+

Two v0.23 candidates added by the RFC-v0.23-001 design-conflict review
(2026-07-31): the **nine-syscall disposition rises in priority**, because
`DmaShare` blocks the documented bulk-transfer path and forced a divergence in
v0.23; and **`renderer.rs`'s `ingest` subsystem** (~540 lines, a tag-keyed
catalog codec called by nothing but its own tests) needs a decision — unfinished
successor, or abandoned work to retire.

Three more added by the RFC-v0.23-001 Slices 1-4 review (2026-07-31):
**services cannot write to any static** (`spawn.rs` maps a service image
`R | X | U` with no `W`; applies to `static UnsafeCell`, not just `static mut`)
— a constraint every service author needs and the SDK docs do not state; the
**shared TOML array parser** closing an array on a `]` inside a string, which
silently loaded 2 of 4 markers — a fail-open in the harness itself; and the
**M8 slot/endpoint-ID confusion** in init's waits, folded into RFC-v0.23-002 as
the marker defect's first victim.

**The marker defect itself (RFC-v0.23-002, to be written).** `dispatch.rs`
emits the milestone markers from the kernel keyed on **hardcoded task-table
indices**, with the spawn order recorded in a code comment
(`exited_ok(10/14/19/21/1)`). So `TEST:V0.7-SYNC:PASS` does not mean "syncd
succeeded" — it means "whatever task occupies index 19 exited cleanly."

All four QEMU smoke profiles key on markers emitted this way (`smoke.rs`):
`m8`←14, `v0.4-net`←21, `v0.5-platform`←10, `v0.7-sync`←19. **None verifies
that the service it names succeeded.**

This has already caused harm, not merely risked it: init's M8 section passes
endpoint IDs where CSpace slots are expected, so that section never runs — and
the `m8` profile stayed green throughout, because its marker attests a
different task's exit. RFC-v0.23-001's added latency shifted task allocation
and re-pointed index 19, turning a silent wrong-reason pass into a visible
failure.

Same class as the four defects v0.22 addressed: a check reporting green for a
reason other than the thing it claims to check.

**Link rot in tracked documentation.** A sweep of every relative markdown link
in tracked files (2026-07-31) found **13 broken**, all pre-existing: superseded
ADR cross-references, `rfcs/proposed/v0.10/` paths from before that set moved to
`done/`, `../release/trust-report.md` where the file is `.txt`, and others.
Gate 12's R4 says bind only rules already violated in practice — this one is
violated 13 times, so a link-integrity subcheck now qualifies.

**`fjell-kernel` has no `[lib]` target, so `test-all` tier 1 skips it entirely**
(found 2026-08-01, RFC-v0.23-002 review; ERRATA E-013). `cargo test --workspace
--lib` silently omits a crate with only a `[[bin]]`, so five `#[cfg(test)]`
modules have never run under the tier that claims to run them — including
`lease/mod.rs`, the kernel-side half of a Verus release-required target. Fourth
instance of the class in this line and the most serious: a test tier reporting
green over a crate it never loads. Its own RFC, after `0.23.0`; the fix is
architectural (a `[lib]` target, or splitting a host-testable subset out).

**Grep blindness, three occurrences in one line.** A NUL-padded byte literal, a
decimal-only regex over hex constants, and UART interleaving tripping the binary
heuristic — each made a tool report nothing where there was something. Prefer
`grep -a` on build and serial output, and treat an empty result over a file
known to have content as suspect rather than conclusive.

Full analysis, with measurements and the dependency map, in
`docs/src/roadmap/v0.23-direction-options.md`.

- Make the semantic plane real (the ABDD live path and beyond)
- Make the service plane real (17 of 29 services currently never receive IPC)
- Make the system operable by a human (base userland, FR-SVC-006)
- Make it run on metal (hardware bring-up)

A measured finding that bears on that discussion: `proxy-text` contains 845
lines of working renderer that nothing calls — its service entry point
prints one line and exits. The same is true of `semantic-stream`. Fjell's
distinguishing demonstration is largely built and entirely unwired, so the
gap may be smaller than the current state suggests.

**v1.0.0 — First Supported Profile** is architect-conditionally-approved.
At v0.21.2, the workspace manifest was broken (`cargo metadata` failed to
parse), so the eleven mechanical gates could not actually run — Gate 9
(manual limitations sign-off by the owner) was not the only blocker at that
point, even though it was described as such. `rfcs/done/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md`
restores the build and re-verifies the mechanical gates; see
`rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/`
for current gate status. Gate 9 remains the only blocker that requires the
owner rather than a mechanical check. v1.0.0 must not be tagged, published,
or announced without explicit owner confirmation.

---

For the full roadmap — the original M0–M11 MVP plan, the complete execution
record, and the forward roadmap (v1.0, v1.1, v2+) — see
[`docs/src/roadmap/roadmap.md`](docs/src/roadmap/roadmap.md).
