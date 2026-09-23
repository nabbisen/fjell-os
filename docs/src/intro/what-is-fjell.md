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
the [Trust Report](https://github.com/nabbisen/fjell-os/blob/main/releases/trust-report.txt).

**The security core is small and checked three ways.** The kernel runs on
RISC-V (Sv39, single-hart at v1.0) with services isolated in user mode.
Beyond the conventional test tiers (host tests, property tests, QEMU smoke
and negative tests, and CI fuzzing of every reachable byte decoder), the release-critical invariants — capability
rights can never be amplified, and a revoked lease can never be reused — are
formally proved in Verus and machine-checked as a release gate.

**Updates and recovery are first-class.** Signed bundles, anti-rollback
metadata, A/B boot-control with proven mirror selection, attested fleet
state, and an operator recovery playbook are part of the system, not
afterthoughts.

Fjell is written in Rust (`no_std` kernel and services) and targets QEMU
`virt` as the validated v1.0 profile, with real-hardware deployment tracked
as a post-v1.0 milestone.

## Inclusion is a primary goal — and this is how far it goes

Alongside assurance, **inclusion is one of Fjell's primary goals.** The
mechanism is architectural: a service does not draw pixels, it emits *meaning* —
state, choices, warnings, the actions it offers — as a structured stream, and a
separate **presentation proxy** renders that stream. Because rendering is not the
OS core's job, a screen, speech, braille or a simplified summary are meant to be
different proxies on the same core, not different products. That is also what
keeps a GUI stack out of the core, and it is why the design does not try to
enumerate categories of user: it makes presentation something anyone can supply.

That is a **goal and a mechanism, not a delivery.** Today one presentation
exists, and it is text on a serial console; there is no speech or braille
presentation, no way for a person to answer the node through any presentation,
and nothing has been tested against an accessibility standard or with people.
The full list is on one page, beside the claim it qualifies:
[what does not exist yet](../releasing/v1-limitations.md#accessibility-and-inclusion--what-does-not-exist-yet).
A second presentation is proposed for the next milestone; the
[roadmap](https://github.com/nabbisen/fjell-os/blob/main/ROADMAP.md) records the
plan, and nothing here says it is done.

For who this is for, read [Why Fjell?](why-fjell.md). For the architecture,
start at the [Overview](../architecture/overview.md). For what Fjell
deliberately does not do, see [v1.0 Non-Goals](../releasing/v1-non-goals.md).
