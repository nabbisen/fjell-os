# Security policy

Fjell OS is a capability-based microkernel: every authority a task holds is an
explicit, revocable capability, and the project's release evidence exists to
make that claim checkable. A report that shows authority being obtained,
retained, or exercised outside that model is the most valuable thing anyone can
send us, and it gets a real response.

## Reporting a vulnerability

If you believe you have found a security issue in Fjell OS:

1. **Do not file a public GitHub issue.** Public issues become indexable
   immediately and put other operators at risk.
2. **Open a [private security advisory](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)**
   on the repository: https://github.com/nabbisen/fjell-os/security/advisories/new
3. Include enough information to reproduce: the release tag or commit, the
   QEMU profile or board, the configuration, and a minimal example.

## What you can expect

- An acknowledgement within a small number of days.
- A discussion of severity and timeline before any public disclosure.
- Credit in the changelog and security advisory unless you ask for anonymity.

## Supported versions

Fjell OS is **pre-1.0 and single-maintainer**. In practice only the most recent
release receives fixes; there is no long-term-support line, no backport
commitment, and no stated end-of-life policy yet. This paragraph describes
current practice rather than a guarantee, and is stated plainly because an
implied commitment is worse than a small explicit one.

## What's in scope

The authoritative boundary is
[`docs/security/threat-model-v1.md`](../docs/security/threat-model-v1.md),
which enumerates 20 in-scope threats (T1–T20) with the mechanism defending
each. Reports against any of them are in scope. In summary, that means:

- **Capability-model defeats** — acquiring authority never delegated, bypassing
  cap-broker policy, replaying expired lease authority, escalating a service's
  initial authority (T1, T2, T3, T18).
- **Isolation defeats** — MMIO ownership confusion, DMA-based memory aliasing,
  trap-frame corruption, IPC sender forgery (T4, T5, T7, T17).
- **Update and trust-chain defeats** — rollback to a superseded image, bundle
  tampering, signature forgery, signing-key compromise, attestation replay,
  stale trust anchors (T8–T12, T14).
- **Evidence defeats** — audit-ring gaps, persistent-store corruption, and
  substituting a build that a reproducibility check would not catch (T15, T16,
  T20).
- **Unsafe-code regressions** — any `unsafe` block reachable in a way its
  `SAFETY` comment does not justify (T6).

## Out of scope

The threat model records **8 threats v1.0 explicitly does not defend against**
(OS1–OS8), each with its rationale — among them side channels (timing, power,
EM), covert channels via audit-ring timing, a persistent physical adversary
with anti-tamper bypass, compromise of the build environment before signing,
and cryptographic break of Ed25519. These are documented non-claims, not
oversights; please read the rationale before reporting one.

Also out of scope:

- Findings that require already holding `TrustAnchorRoot` or host root (OS1,
  OS7).
- Findings that depend on the operator misusing an option documented as
  dangerous — see also T19, which covers operator mistake *within* authorised
  scope and **is** in scope.
- Best-practice nits on otherwise-safe code (please file a normal issue or PR).
- Vulnerabilities in upstream Rust crates — report those upstream, though we
  appreciate a heads-up. Fjell's direct external dependency surface is nine
  crates, and the kernel itself has none.

## Known limitations

Before reporting, it is worth reading
[`docs/release/v1-limitations.md`](../docs/release/v1-limitations.md) and the
errata register [`docs/rfcs/ERRATA.md`](../docs/rfcs/ERRATA.md). The project
records what it knows is wrong, including gaps in its own verification
instruments. A limitation already recorded there is not a vulnerability report,
but a demonstration that one of them is *exploitable* very much is.
