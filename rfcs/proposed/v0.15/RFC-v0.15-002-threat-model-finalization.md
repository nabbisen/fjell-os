# RFC-v0.15-002 — Threat Model Finalization

**Status:** Proposed
**Target version:** v0.15.0
**Parent:** v0.15-001.
**Cross-refs:** RFC 027 (v0.1 threat model), RFC 042 (v0.2 boundary
    expansion), RFC 061 (identity).

## 1. Problem

RFC 027 established a v0.1 threat model. RFC 042 expanded it for v0.2.
Subsequent milestones added pieces but never re-baselined. v1.0 cannot
ship without a single authoritative document that says, in plain
language, what Fjell defends against, what it does not, and what the
operator must defend instead.

## 2. The threat model document

`docs/security/threat-model-v1.md` is the deliverable. Its structure:

### 2.1 Adversary model (by capability, not by archetype)

Every threat is keyed to a capability:

- **C-NET-PASSIVE** — read fleet network traffic.
- **C-NET-ACTIVE** — read + modify + inject fleet network traffic.
- **C-NODE-EXEC** — execute arbitrary code on one node.
- **C-NODE-PHYS** — physical access to one node (one-time).
- **C-NODE-PHYS-PERS** — physical access on demand.
- **C-OPER-MISTAKE** — operator error within authorised scope.
- **C-OPER-MAL** — malicious operator within authorised scope.
- **C-SIGN-COMPR** — possession of a current signing key.
- **C-ANCHOR-COMPR** — possession of the TrustAnchorRoot key.
- **C-SUPPLY** — modify the build supply chain.

For each capability:

1. What the adversary can do.
2. Which Fjell invariant (RFC 061 §4) defends against it, and how.
3. Which RFC(s) implement the defence.
4. Where the defence is *partial* — and what the residual risk is.
5. What the operator must do to close the gap.

### 2.2 In-scope threats

The set Fjell commits to mitigating at v1.0:

- **T1.** Unauthorised capability acquisition. Defended by I1–I3.
- **T2.** Cap-broker policy bypass. Defended by RFC 031, 040.
- **T3.** Lease-expired authority replay. Defended by I3 + v0.11-005.
- **T4.** MMIO ownership confusion. Defended by RFC 016, 035, 051.
- **T5.** DMA-based memory aliasing. Defended by RFC 017, 036, 052.
- **T6.** Unsafe-code regression. Defended by v0.6-004 + RFC 060.
- **T7.** Trap-frame corruption. Defended by RFC 001, 022.
- **T8.** Update rollback. Defended by RFC v0.3-003, RFC 002.
- **T9.** Bundle tampering. Defended by RFC v0.9-004, v0.11-003.
- **T10.** Signature forgery (algorithmic). Defended by Ed25519
    (v0.11-002).
- **T11.** Signature key compromise. Defended by v0.11-004 +
    v0.13-003.
- **T12.** Attestation replay. Defended by v0.11-005.
- **T13.** Fleet partition exploitation. Defended by v0.13-002.
- **T14.** Stale trust anchors. Defended by v0.11-004 §6.
- **T15.** Persistent-store corruption. Defended by RFC 053.
- **T16.** Audit-ring evidence gap. Defended by RFC 053, 054.
- **T17.** IPC sender forgery. Defended by RFC 055.
- **T18.** Service init authority escalation. Defended by RFC 057, 058.
- **T19.** Operator mistake within authorised scope. Defended by
    v0.13-003 confirmations + bounded blast radius.
- **T20.** Reproducibility-failure-as-substitution. Defended by
    RFC-v0.10-003.

### 2.3 Out-of-scope threats (explicit)

The set Fjell **does not** defend against at v1.0:

- **OS1.** Compromise of `TrustAnchorRoot` key combined with
  filesystem write access to deployed nodes (irrecoverable; operator
  must re-provision physically).
- **OS2.** Compromise of the build environment used to produce the
  release tarball before signing.
- **OS3.** Hardware side channels (timing, power, EM). Mitigations
  exist in chosen primitives but no defence is claimed.
- **OS4.** Coordinated compromise of more than `f` nodes where `f`
  is the fault-tolerance bound (currently 0 — Fjell does not assume
  byzantine fault tolerance at v1.0).
- **OS5.** Adversary with sustained physical access and intent to
  destroy/replace hardware (anti-tamper is hardware-side).
- **OS6.** Adversaries with cryptographic capability exceeding the
  Ed25519 security assumption (e.g. CRQC for the asymmetric layer);
  post-quantum hybrid is research track.
- **OS7.** Insider with legitimate `TrustAnchorRoot` access intending
  harm (organisational control, not technical).
- **OS8.** Side-channel attacks via the audit ring (covert channel
  through audit-record timing; possible but unmitigated).

Each is named, scoped, and accompanied by the rationale for
non-coverage.

### 2.4 What the operator must do

The threat model ends with explicit operator obligations:

- Key custody and rotation policy.
- Physical security for the `TrustAnchorRoot`.
- Build environment hygiene.
- Monitoring of the audit ring.
- Periodic re-attestation cadence (v0.13-004).
- Incident response per v0.13-003 / 005.

## 3. Audit gate

The threat model gains a small CI assertion: every T<n> in the
in-scope list must reference at least one merged RFC in `done/`.
Failure means a claimed mitigation is undocumented; the gate refuses.

The audit also produces a small reverse-index: every defence-bearing
RFC has a `Threats addressed: T<n>, T<m>` line added to its frontmatter
during v0.15 landing.

## 4. Trust Report integration

The Trust Report (RFC 061 §6) gains a "Threats addressed" subsection
showing the live count of in-scope threats and the most recent audit
gate verdict.

## 5. Acceptance criteria

1. `docs/security/threat-model-v1.md` exists and covers §2.
2. Every T<n> in-scope item references an existing merged RFC.
3. Every OS<n> out-of-scope item has explicit rationale.
4. Capability table from §2.1 is complete.
5. Operator-obligations section §2.4 is committed.
6. CI gate `ci-threat-model-check` exists and verifies the
   RFC-cross-reference index.
7. Trust Report shows "Threats addressed" subsection.
8. One adversarial review pass is performed at landing — a reviewer
   tries to find a missing scenario or an over-claim; findings are
   either incorporated or explicitly rejected with rationale.

## 6. Out of scope

- Quantitative risk scoring. The doc is qualitative.
- A standalone whitepaper. The committed Markdown is authoritative.
- Compliance mapping (FIPS, CC, ISO). Done as separate documents
  post-v1.0 if pursued.
- Empirical pentest coverage. Worth doing but separate from threat
  model documentation.
- Coverage of v2.0 / post-v1.0 features that have not yet shipped.
