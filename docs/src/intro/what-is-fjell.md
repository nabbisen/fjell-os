# What is Fjell?

> **Fjell OS is a verifiable, capability-based operating system for
> high-assurance edge and fleet nodes where every authority, update,
> recovery action, and runtime state must be explainable.**

In practice that means four things you can rely on:

**Authority is a handle, never an identity.** A service can only do what a
capability in its capability space allows. There is no ambient authority: no
root user, no implicit file access, no default network. Every grant is
explicit, typed, and traceable to the signed authority that issued it.

**Every action leaves evidence.** Authority grants, updates, boot decisions,
and recovery steps each emit a signed, machine-readable semantic record. An
auditor can reconstruct what a node did, and why, from the records alone —
this is what "explainable" means here, and it is verified at release time as
the [Trust Report](../release/trust-report.md).

**The security core is small and checked three ways.** The kernel runs on
RISC-V (Sv39, single-hart at v1.0) with services isolated in user mode.
Beyond the conventional test tiers (host tests, property tests, fuzzing,
QEMU smoke and negative tests), the release-critical invariants — capability
rights can never be amplified, and a revoked lease can never be reused — are
formally proved in Verus and machine-checked as a release gate.

**Updates and recovery are first-class.** Signed bundles, anti-rollback
metadata, A/B boot-control with proven mirror selection, attested fleet
state, and an operator recovery playbook are part of the system, not
afterthoughts.

Fjell is written in Rust (`no_std` kernel and services) and targets QEMU
`virt` as the validated v1.0 profile, with real-hardware deployment tracked
as a post-v1.0 milestone.

For who this is for, read [Why Fjell?](why-fjell.md). For the architecture,
start at the [Overview](../architecture/overview.md). For what Fjell
deliberately does not do, see [v1.0 Non-Goals](../release/v1-non-goals.md).
