# Fjell OS — Project Summary (PM Handoff)

*Compact project-level handoff. Version: v0.21.2.*

## 1. Project goal

Fjell OS is a Rust-first, capability-based microkernel operating system for
high-assurance edge and fleet nodes. The problem it solves is operational
trust: for any node in a fleet, an operator must be able to answer what is
running (every binary is content-addressed and signed), who authorised it
(every capability grant has a leased, traceable provenance), and how to
recover (every documented failure mode has a tested playbook). The target
users are operators of industrial gateways (archetype A1), sensor/edge fleet
nodes (A2), and regulated field devices (A3) — not general-purpose servers or
desktops. The expected deliverable for the v1.0 milestone is the first
*supported QEMU profile*: a kernel plus user-space service plane that boots on
the QEMU `virt` RISC-V machine, enforces capability and lease semantics, emits
fail-closed negative-test evidence for the covered security boundaries, and
carries selective Verus machine-checked invariants. Success for v1.0 means all
eleven mechanical release gates pass and the owner signs the manual limitations
gate (Gate 9). Explicitly out of scope for v1.0: real-hardware validation,
multi-hart/SMP, a POSIX surface, full store/upgrade negative coverage, and a
production trust-anchor lifecycle.

## 2. Requirements snapshot

**Functional (API-visible behaviour).** A capability-gated syscall surface
(26 dispatched syscalls across IPC, capability management, lease lifecycle,
task management, hardware access, platform, audit, and scheduler groups; a
further 9 are declared in the ABI but not dispatched — see
[Syscall Reference](../../api/syscalls.md)); synchronous
rendezvous IPC with kernel-attested sender identity; lease-bounded authority
where revocation atomically cancels in-flight IPC; signed-bundle verification;
append-only audit evidence drainable to user space.

**Non-functional.** `#![forbid(unsafe_code)]` except at audited, classified
boundaries (zero missing SAFETY annotations is a hard gate); reproducible build
(two-build SHA-256 comparison over the artefact set); selective formal
verification (Verus machine-checked predicates for capability non-amplification
and lease revocation); ABI stability (snapshot gate forbids removals);
fail-closed negative tests (an absent expected marker or a present forbidden
marker fails the run).

**Compatibility constraints.** Rust 2024 edition, toolchain 1.91; target
`riscv64gc-unknown-none-elf`, QEMU `virt` profile; mdBook documentation;
Apache-2.0, author nabbisen. Full requirements live in
`Fjell-OS-要件定義書_v1` (requirements definition) and the RFC register under
`rfcs/`.

## 3. Current status

| Area | Status | Evidence | Owner | Notes |
|---|---|---|---|---|
| Requirements | Done | `rfcs/done/` (154 resolved RFCs), `docs/release/v1-limitations.md` | nabbisen | v1.0 scope frozen |
| External design | Done | `rfcs/`, `docs/src/adr/` (10 current ADRs) | nabbisen | Crate boundaries reorganized v0.21.0 |
| Implementation | Done | `cargo xtask build` (zero warnings at v0.21.2) | — | 80 crates; kernel + 29 service programs |
| Tests / proofs | Done | `cargo xtask release-rehearsal` (Gates 1–8,10,11 pass); Verus capability 8/8, lease 5/5 | — | Gate 9 manual, pending |
| Release readiness | Partial | `docs/release/v1.0-release-notes.md` | nabbisen | Gate 9 sign-off is the only blocker |

## 4. Important decisions

See `decision-log.md` for the full register. The decisions a future PM must
preserve or consciously revisit:

| ID | Decision | Why | Consequence | Source |
|---|---|---|---|---|
| DEC-001 | v1.0 is a narrow QEMU profile, not a production OS | Honest scoping; avoids over-claiming | Release notes must state every non-claim | `docs/release/v1.0-release-notes.md` |
| DEC-002 | v1.0.0 cannot be tagged/published without owner confirmation | Single human authority over the release | No CI or agent may tag v1.0.0 | Architect review v0.20.0 |
| DEC-003 | Selective Verus boundary (capability + lease only) | Verifying everything is infeasible; verify the security-critical predicates | IPC/service-manager are tested, not proven | RFC-v0.18-001 |
| DEC-004 | Crate subdirectory grouping (arch/drivers/formats/services) | 80 flat crates were unnavigable | Path deps relative to subdir; names unchanged | CHANGELOG v0.21.0 |
| DEC-005 | Store/upgrade negative profiles deferred from v1 gate | Late-stage scope control | Must be documented as non-gated; required by v1.1 | Architect review v0.20.0 §4.2 |

## 5. Risks and open questions

| ID | Risk / Question | Impact | Current mitigation | Owner | Due |
|---|---|---|---|---|---|
| RISK-01 | Gate 9 not yet signed | Blocks v1.0.0 tag | Limitations doc is in signable state | nabbisen | — |
| RISK-02 | DMA user-VA unmap bypassed in `revoke_by_pa` | Stale PTE after DMA revoke (mitigated by zeroize-before-reuse) | Frame zeroized before allocator reuse; full unmap deferred | — | post-v1.0 |
| RISK-03 | Store/upgrade negative emitters absent | No runtime evidence for two security-relevant paths | Documented as non-gated; specs exist | — | v1.1 |
| RISK-04 | svc READY negative pair is timing-sensitive | Partial service-lifecycle coverage (2/4 markers) | Documented as partial | — | v1.1 |

## 6. Next-step recommendation

1. **Owner (nabbisen):** read `docs/release/v1-limitations.md` and
   `docs/release/v1.0-release-notes.md`; sign Gate 9. *Blocking dependency for
   everything below.*
2. **Owner:** after Gate 9, confirm the v1.0.0 tag explicitly (publication
   control). Expected artifact: the v1.0.0 git tag and release archive.
3. **Implementer (post-v1.0):** add store/upgrade negative emitters
   (`NEG:STORE:*`, `NEG:UPGRADE:*`). Evidence: new passing profiles in
   `cargo xtask qemu-negative`.
4. **Implementer (post-v1.0):** isolate the DMA page-table corruption root cause
   and re-enable `unmap_user_va_for` in `revoke_by_pa`.
5. **QA (post-v1.0):** stabilise the svc READY negative pair to reach 4/4.
6. **Architect (post-v1.0):** strengthen Gate 11 callsite-audit from heuristic
   to function-body scan (architect review H-03).
