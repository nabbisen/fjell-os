# Fjell OS — Standards Mapping (CRA Annex I / IEC 62443-4-1 / IEC 62443-4-2)

**Governed by:** [RFC-0.27-003](../../rfcs/accepted/RFC-0.27-003-standards-mapping.md)
**Milestone:** 0.27
**Re-verified at:** every release cut, per
[the v0 release cycle](../src/release/v0-release-cycle.md) (R5).

---

## What this document is not (D1)

This document does not claim, and must not be read as claiming, that Fjell
OS is **compliant**, **certified**, or **conformant** with the Cyber
Resilience Act or with IEC 62443. Only a conformity assessment body can
establish conformity; nothing here is that assessment. This document maps
Fjell mechanisms and evidence artifacts to external requirements, and states
honestly, row by row, where a mechanism exists, where it partially exists,
and where it does not exist at all.

Fjell OS is pre-1.0, single-maintainer, and has never been placed on the EU
market. The Cyber Resilience Act's obligations bind manufacturers placing
products on the market; none of them currently bind this project. This
mapping exists so that a manufacturer evaluating Fjell as a component can see
what is and is not already true, not to assert a legal status Fjell does not
have. See [RFC-0.27-003](../../rfcs/accepted/RFC-0.27-003-standards-mapping.md)'s
Motivation section for the full reasoning, including why 11 September 2026 is
not a deadline that binds this project.

## Sourcing (D2/D3)

**CRA (Annex I).** Regulation (EU) 2024/2847 of the European Parliament and
of the Council of 23 October 2024 (the Cyber Resilience Act), OJ L,
20.11.2024. Retrieved from the official EUR-Lex PDF
(`https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=OJ:L_202402847`) on
**2026-08-31**. Annex I appears at pages 67-68 of 81 in that PDF. The
verbatim clause text is quoted below each Part's heading, before its table,
so every row's Requirement cell can carry a short label rather than
repeating the full clause.

**IEC 62443-4-1 and 62443-4-2.** Both are sold by the IEC and have not been
purchased for this project. Per D3, **no clause text is transcribed or
paraphrased from either standard.** The tables below map only to the
publicly documented *structure* — 62443-4-1's eight named practices, and the
seven Foundational Requirements (FR1-FR7) that 62443-4-2 organises
component-level requirements under — by identifier and title. **This half of
the mapping is structural, not clause-level, and cannot be more precise than
that until the owner decides to purchase the standards.**

## Status vocabulary (D4)

`met` / `partial` / `not-met` / `not-applicable` / `roadmap` / `unassessed`.
A row with no cited artifact is `not-met`, never `partial`. A row whose
evidence is "the architecture makes this true" with nothing to point at is
`not-met`.

**`unassessed`** — the criterion for this row lives in a source the project
has not read. The mechanism cell records **candidate evidence only** and
asserts no verdict. A row may not leave `unassessed` until the criterion
has actually been read. (Added in review of commit `fb05a1a`, 2026-08-31 —
see the amendment note in RFC-0.27-003 §D4. Every IEC 62443 row in this
document is `unassessed`: neither 62443-4-1 nor 62443-4-2 has been
purchased, so the criterion each row would be checked against has never
been read, and D3 forbids reading it from any other source. A `met` or
`partial` verdict against unread text is a guess wearing a status column's
authority, not a status.)

## §4 — Does a `not-met` row block a release?

**Answer: shape 3 — a release fails only when a `met` or `partial` row's
cited evidence no longer exists.** `not-met` and `roadmap` rows never block;
they are the expected, honest state of a first draft (D5).

Argued independently, not adopted on the architect's inclination alone
(handoff §2):

- **Shape 3 costs nothing extra to build.** R3's four checks — closed
  vocabulary, `met`/`partial` rows cite a path, every cited path exists,
  no row silently skipped — already *are* shape 3. There is no separate
  design to justify; the subcheck this RFC requires anyway implements it.
- **Shape 3 targets this project's actual, repeated failure mode.**
  Every prior instrument defect this project has found (E-013, E-016,
  E-025, E-026) was a citation or a scan that quietly stopped pointing at
  what it once did, with nothing noticing. A cited artifact disappearing
  out from under a `met` row is exactly that failure shape, applied to a
  document read by outsiders instead of by two people.
- **Shape 1 (never blocks) is not enough here specifically.** D5 already
  says the reviewer will read a high `met` count as a defect signal — but
  that is a human doing the reading, at review time, by hand. A document
  written for assessors and buyers, per the RFC's own framing, needs at
  least the mechanical floor that nobody has to remember to look.
- **Shape 2 (no silent regression, `met` → `not-met` must be recorded) is
  the more complete answer and is deliberately not built now.** It needs a
  stored baseline of the mapping's prior state to diff against, and this
  project's own errata register (E-025 especially) is full of instruments
  that went stale because nobody answered "who verifies the baseline
  itself doesn't drift" before building it. Building shape 2 today would
  relocate this document's own risk into a second document, without a
  design for keeping *that* one honest. Recorded here as **unscheduled**
  follow-on work, not implemented.

## What this instrument does and does not verify (R3 disclosure)

The `standards-mapping` subcheck (`tools/fjell-consistency-check/src/standards_mapping.rs`)
verifies that every status cell is from the closed vocabulary, that every
`met`/`partial` row cites at least one path, and that every cited path
exists in the tree. **It does not verify that a cited artifact still says
what its row claims.** A row can cite a real file that no longer supports
the sentence next to it, and the gate stays green. This is a weak predicate,
it is deliberate (checking semantic support mechanically is not buildable),
and it is why R2 — the human re-opening of every cited artifact — remains
part of every reconciliation pass, not a one-time step this subcheck
replaces.

---

## Part I — CRA Annex I, Part I: cybersecurity requirements relating to the properties of products with digital elements

> (1) Products with digital elements shall be designed, developed and
> produced in such a way that they ensure an appropriate level of
> cybersecurity based on the risks.
>
> (2) On the basis of the cybersecurity risk assessment referred to in
> Article 13(2) and where applicable, products with digital elements shall:
>
> (a) be made available on the market without known exploitable
> vulnerabilities;
>
> (b) be made available on the market with a secure by default
> configuration, unless otherwise agreed between manufacturer and business
> user in relation to a tailor-made product with digital elements,
> including the possibility to reset the product to its original state;
>
> (c) ensure that vulnerabilities can be addressed through security
> updates, including, where applicable, through automatic security updates
> that are installed within an appropriate timeframe enabled as a default
> setting, with a clear and easy-to-use opt-out mechanism, through the
> notification of available updates to users, and the option to
> temporarily postpone them;
>
> (d) ensure protection from unauthorised access by appropriate control
> mechanisms, including but not limited to authentication, identity or
> access management systems, and report on possible unauthorised access;
>
> (e) protect the confidentiality of stored, transmitted or otherwise
> processed data, personal or other, such as by encrypting relevant data at
> rest or in transit by state of the art mechanisms, and by using other
> technical means;
>
> (f) protect the integrity of stored, transmitted or otherwise processed
> data, personal or other, commands, programs and configuration against any
> manipulation or modification not authorised by the user, and report on
> corruptions;
>
> (g) process only data, personal or other, that are adequate, relevant and
> limited to what is necessary in relation to the intended purpose of the
> product with digital elements (data minimisation);
>
> (h) protect the availability of essential and basic functions, also after
> an incident, including through resilience and mitigation measures against
> denial-of-service attacks;
>
> (i) minimise the negative impact by the products themselves or connected
> devices on the availability of services provided by other devices or
> networks;
>
> (j) be designed, developed and produced to limit attack surfaces,
> including external interfaces;
>
> (k) be designed, developed and produced to reduce the impact of an
> incident using appropriate exploitation mitigation mechanisms and
> techniques;
>
> (l) provide security related information by recording and monitoring
> relevant internal activity, including the access to or modification of
> data, services or functions, with an opt-out mechanism for the user;
>
> (m) provide the possibility for users to securely and easily remove on a
> permanent basis all data and settings and, where such data can be
> transferred to other products or systems, ensure that this is done in a
> secure manner.

| ID | Requirement | Status | Mechanism | Evidence |
|----|-------------|--------|-----------|----------|
| CRA-I-1 | (1) Appropriate cybersecurity based on risk | met | RFC-governed threat model, adversary-capability grid, 20 in-scope threats each mapped to a defending RFC with stated residual risk | [threat-model-v1.md](../security/threat-model-v1.md) |
| CRA-I-2a | (2)(a) No known exploitable vulnerabilities when made available | not-applicable | Point-in-time state claim about a product placed on the market. Fjell OS is pre-1.0 and has never been placed on the market; re-evaluate at first release intended for market placement | — |
| CRA-I-2b | (2)(b) Secure by default, incl. reset to original state | partial | TOFU trust-anchor provisioning behind an explicit `--allow-tofu-provision` flag (no silent default trust); reset-to-original-state and factory/hardware-anchored provisioning tiers are deferred to v1.1/v2+ | [v1-limitations.md](../release/v1-limitations.md); [RFC-v0.17-001](../../rfcs/done/RFC-v0.17-001-trust-anchor-provisioning.md) |
| CRA-I-2c | (2)(c) Security updates, incl. automatic/opt-out/notification | partial | Signed, content-addressed bundle updates with an anti-rollback counter exist; there is no automatic-update, opt-out, or notification UX — updates are operator-driven | [RFC-v0.9-004](../../rfcs/done/RFC-v0.9-004-bundle-builder-and-signed-service-package.md); [RFC-v0.11-003](../../rfcs/done/RFC-v0.11-003-bundle-signing-pipeline-and-key-material-management.md); [RFC-v0.3-003](../../rfcs/done/RFC-v0.3-003-anti-rollback-metadata-and-upgraded-local-confirmation-hardening.md) |
| CRA-I-2d | (2)(d) Protection from unauthorised access, incl. reporting | met | Capability system: every syscall accessing a resource requires a handle with matching kind and rights (`require_cap`); cap-broker enforces manifests with operator approval for grants | [capability-system.md](../src/architecture/capability-system.md); [threat-model-v1.md](../security/threat-model-v1.md) (T1, T2) |
| CRA-I-2e | (2)(e) Confidentiality of stored/transmitted data | partial | Signing-key material is encrypted at rest (RFC-v0.16-006). Bulk data-in-transit confidentiality is not production-ready: `fjell-sxt-crypto`, the only crate implementing it, states in its own doc-comment that it must not be used in production (unvetted AES with a documented cache-timing leak) and gates compilation behind an explicit acknowledgement feature; its consumer `secure-transportd` is currently a smoke-test stub | [RFC-v0.16-006](../../rfcs/done/RFC-v0.16-006-key-handling-encryption-patch.md); [fjell-sxt-crypto/src/lib.rs](../../crates/fjell-sxt-crypto/src/lib.rs); [v1-limitations.md](../release/v1-limitations.md) |
| CRA-I-2f | (2)(f) Integrity of stored/transmitted data, commands, config | met | Kernel-attested IPC sender identity (no forgery); Ed25519-signed, content-addressed bundles; persistent-store integrity; audit ring records corruption-relevant events | [threat-model-v1.md](../security/threat-model-v1.md) (T9, T15, T16, T17) |
| CRA-I-2g | (2)(g) Data minimisation | not-applicable | Fjell OS is a kernel/component with no data-collection surface of its own; re-evaluate against a specific deployed service that processes personal data | — |
| CRA-I-2h | (2)(h) Availability of essential functions incl. after incident, DoS resilience | partial | Fleet partition FSM keeps a coordinator-less fleet visibly degraded rather than fabricating authority; no dedicated in-kernel resource-exhaustion or DoS mitigation instrument was found in this project's mechanism inventory | [threat-model-v1.md](../security/threat-model-v1.md) (T13); [RFC-v0.13-002](../../rfcs/done/RFC-v0.13-002-fleet-split-reconnect-and-reconciliation.md) |
| CRA-I-2i | (2)(i) Minimise negative impact on other devices'/networks' availability | not-applicable | Fjell's network-facing services (`netd`, `virtio-net`, `secure-transportd`) are smoke-test stubs at v1 with no live network dataplane; nothing exists yet through which Fjell could impact another device's or network's availability | [v1-limitations.md](../release/v1-limitations.md) |
| CRA-I-2j | (2)(j) Limit attack surface incl. external interfaces | met | `syscall-surface` subcheck mechanically holds declared, dispatched, and committed-expectation syscall sets equal; the capability system bounds every external interface to an explicit grant | [syscall_surface.rs](../../tools/fjell-consistency-check/src/syscall_surface.rs); [capability-system.md](../src/architecture/capability-system.md) |
| CRA-I-2k | (2)(k) Reduce incident impact via exploitation mitigation | met | `forbid(unsafe_code)` except at an audited, annotated boundary; unsafe-audit and MMIO-audit release gates mechanically enforce a checked justification comment at every unsafe/MMIO site | [release_rehearsal.rs](../../crates/fjell-tools/src/release_rehearsal.rs) (Gates 2-3); [threat-model-v1.md](../security/threat-model-v1.md) (T4, T6) |
| CRA-I-2l | (2)(l) Security-related recording/monitoring, opt-out | partial | Kernel-managed audit ring records security-relevant events, drained via a capability-gated syscall; there is no user-facing opt-out because there is no end-user surface at the component level — audit is currently an operator/service concern | [ring.rs](../../crates/fjell-kernel/src/audit/ring.rs); [threat-model-v1.md](../security/threat-model-v1.md) (T16) |
| CRA-I-2m | (2)(m) Secure, permanent removal of data and settings | not-met | No factory-reset or secure-erase user-facing mechanism was found. A narrower and more load-bearing gap is already recorded: even in-memory key erasure (`ZeroizeOnDrop`) is unverified | [v1-non-goals.md](../release/v1-non-goals.md) |

## Part II — CRA Annex I, Part II: vulnerability handling requirements

> Manufacturers of products with digital elements shall:
>
> (1) identify and document vulnerabilities and components contained in
> products with digital elements, including by drawing up a software bill
> of materials in a commonly used and machine-readable format covering at
> the very least the top-level dependencies of the products;
>
> (2) in relation to the risks posed to products with digital elements,
> address and remediate vulnerabilities without delay, including by
> providing security updates; where technically feasible, new security
> updates shall be provided separately from functionality updates;
>
> (3) apply effective and regular tests and reviews of the security of the
> product with digital elements;
>
> (4) once a security update has been made available, share and publicly
> disclose information about fixed vulnerabilities, including a
> description of the vulnerabilities, information allowing users to
> identify the product with digital elements affected, the impacts of the
> vulnerabilities, their severity and clear and accessible information
> helping users to remediate the vulnerabilities; in duly justified cases,
> where manufacturers consider the security risks of publication to
> outweigh the security benefits, they may delay making public information
> regarding a fixed vulnerability until after users have been given the
> possibility to apply the relevant patch;
>
> (5) put in place and enforce a policy on coordinated vulnerability
> disclosure;
>
> (6) take measures to facilitate the sharing of information about
> potential vulnerabilities in their product with digital elements as well
> as in third-party components contained in that product, including by
> providing a contact address for the reporting of the vulnerabilities
> discovered in the product with digital elements;
>
> (7) provide for mechanisms to securely distribute updates for products
> with digital elements to ensure that vulnerabilities are fixed or
> mitigated in a timely manner and, where applicable for security updates,
> in an automatic manner;
>
> (8) ensure that, where security updates are available to address
> identified security issues, they are disseminated without delay and,
> unless otherwise agreed between a manufacturer and a business user in
> relation to a tailor-made product with digital elements, free of charge,
> accompanied by advisory messages providing users with the relevant
> information, including on potential action to be taken.

| ID | Requirement | Status | Mechanism | Evidence |
|----|-------------|--------|-----------|----------|
| CRA-II-1 | (1) Identify/document vulnerabilities and components, incl. SBOM | not-met | SBOM emission is BIZ-01 and unbuilt | — |
| CRA-II-2 | (2) Address/remediate without delay, security updates separable | roadmap | SECURITY.md commits to an acknowledgement and a severity/timeline discussion; the bundle system can ship one service's update independently of others, but shipping a security update separately from a functionality update has never been exercised — no vulnerability has been reported against a released version (project is pre-1.0) | [SECURITY.md](../../.github/SECURITY.md) |
| CRA-II-3 | (3) Effective and regular tests and reviews of security | met | A 12-gate release-rehearsal run at every cut; a 27-entry errata register recording every found defect; a dedicated adversarial review pass against the threat model and non-goals | [release_rehearsal.rs](../../crates/fjell-tools/src/release_rehearsal.rs); [adversarial-review-v0.16.md](../security/adversarial-review-v0.16.md); [ERRATA.md](../rfcs/ERRATA.md) |
| CRA-II-4 | (4) Share/publicly disclose fixed vulnerabilities | roadmap | Process is committed in SECURITY.md (credit in changelog/advisory, discussion before disclosure) but not yet exercised — no vulnerability has been reported and fixed pre-1.0 | [SECURITY.md](../../.github/SECURITY.md) |
| CRA-II-5 | (5) Coordinated vulnerability disclosure policy | met | Private GitHub security-advisory intake, explicit no-public-issue instruction, acknowledgement and severity/timeline commitment | [SECURITY.md](../../.github/SECURITY.md) |
| CRA-II-6 | (6) Facilitate sharing of vulnerability info, contact address | met | SECURITY.md publishes the reporting channel as the single point of contact | [SECURITY.md](../../.github/SECURITY.md) |
| CRA-II-7 | (7) Mechanisms to securely distribute updates, automatic where applicable | partial | Signed, content-addressed bundle distribution with anti-rollback exists; automatic distribution/application is not implemented — fleet updates are deliberately operator-driven | [RFC-v0.9-004](../../rfcs/done/RFC-v0.9-004-bundle-builder-and-signed-service-package.md); [v1-non-goals.md](../release/v1-non-goals.md) (N20) |
| CRA-II-8 | (8) Disseminate without delay, free of charge, with advisory messages | roadmap | No update has ever been disseminated in response to a security issue (pre-1.0); the free-of-charge/advisory-message commitment is not yet separately documented | — |

---

## IEC 62443-4-1 — structural mapping (secure product development lifecycle practices)

**Structural only (D3): identifier and title, not clause text.** IEC
62443-4-1 organises its requirements into eight named practices. **Every row
below is `unassessed` (D4): the criterion each practice actually requires
lives in the paywalled text, which has not been read.** The Mechanism
column names candidate Fjell evidence that *corresponds by topic* to the
practice's name — it does not assert that Fjell satisfies the practice, and
must not be read as doing so until the standard is purchased and each
practice's real requirement is checked against it.

| ID | Requirement (practice, public structure only) | Status | Mechanism | Evidence |
|----|------------------------------------------------|--------|-----------|----------|
| IEC-4-1-SM | Security Management | unassessed | Candidate evidence: an RFC-governed change process and a 28-entry errata register, as a security-relevant governance structure. Whether this is what SM requires is unknown | [000-rfc-lifecycle-policy.md](../../rfcs/done/000-rfc-lifecycle-policy.md); [ERRATA.md](../rfcs/ERRATA.md) |
| IEC-4-1-SR | Specification of Security Requirements | unassessed | Candidate evidence: the threat model and the non-goals register, as a security-requirements specification, RFC-governed and versioned. Whether this is what SR requires is unknown | [threat-model-v1.md](../security/threat-model-v1.md); [v1-non-goals.md](../release/v1-non-goals.md) |
| IEC-4-1-SD | Secure by Design | unassessed | Candidate evidence: capability-based authority as the project's stated architectural invariant (I1-I6). Whether this is what SD requires is unknown | [capability-system.md](../src/architecture/capability-system.md) |
| IEC-4-1-SI | Secure Implementation | unassessed | Candidate evidence: `forbid(unsafe_code)` except at an audited boundary, enforced by the unsafe-audit and MMIO-audit release gates. Whether this is what SI requires is unknown | [release_rehearsal.rs](../../crates/fjell-tools/src/release_rehearsal.rs) (Gates 2-3) |
| IEC-4-1-SVV | Security Verification and Validation Testing | unassessed | Candidate evidence: 21-tier `test-all`, 12-gate `release-rehearsal`, fail-closed QEMU negative-test categories, and Verus proofs. Whether this is what SVV requires is unknown | [test_all.rs](../../crates/fjell-tools/src/test_all.rs); [release_rehearsal.rs](../../crates/fjell-tools/src/release_rehearsal.rs) |
| IEC-4-1-DM | Management of security-related issues (defect management) | unassessed | Candidate evidence: the errata register tracks every known defect with a disposition (OPEN/CLOSED/ACCEPTED); project-internal, not unified with SECURITY.md's advisory intake. Whether this is what DM requires is unknown | [ERRATA.md](../rfcs/ERRATA.md); [SECURITY.md](../../.github/SECURITY.md) |
| IEC-4-1-SUM | Security Update Management | unassessed | Candidate evidence: signed bundle distribution with anti-rollback; no separate update-management policy document beyond SECURITY.md's pre-1.0 disclaimer. Whether this is what SUM requires is unknown | [RFC-v0.9-004](../../rfcs/done/RFC-v0.9-004-bundle-builder-and-signed-service-package.md); [SECURITY.md](../../.github/SECURITY.md) |
| IEC-4-1-SG | Security Guidelines | unassessed | Candidate evidence: the threat model's Operator Obligations section and SECURITY.md's pointer to known limitations, as operator-facing security guidance. Whether this is what SG requires is unknown | [threat-model-v1.md](../security/threat-model-v1.md); [SECURITY.md](../../.github/SECURITY.md) |

## IEC 62443-4-2 — structural mapping (foundational requirements, component level)

**Structural only (D3): identifier and title, not clause text.** IEC
62443-4-2 organises component-level technical requirements under the same
seven Foundational Requirements (FR1-FR7) used across the 62443 series.
**Every row below is `unassessed` (D4)** for the same reason as the 4-1
table above: the criterion each FR actually requires is in the paywalled
text. The Mechanism column names candidate Fjell evidence by topic
correspondence only.

| ID | Requirement (foundational requirement, public structure only) | Status | Mechanism | Evidence |
|----|------------------------------------------------------------------|--------|-----------|----------|
| IEC-4-2-FR1 | FR1 — Identification and Authentication Control | unassessed | Candidate evidence: capability handles as the sole means of identifying authorised access; `require_cap` enforces kind and rights matching. Whether this is what FR1 requires is unknown | [capability-system.md](../src/architecture/capability-system.md); [threat-model-v1.md](../security/threat-model-v1.md) (T1) |
| IEC-4-2-FR2 | FR2 — Use Control | unassessed | Candidate evidence: cap-broker enforces manifests with operator approval for grants; rights checked on every capability use. Whether this is what FR2 requires is unknown | [threat-model-v1.md](../security/threat-model-v1.md) (T2) |
| IEC-4-2-FR3 | FR3 — System Integrity | unassessed | Candidate evidence: kernel-only trap frames; signed, content-addressed bundles; epoch-bound persistent-store integrity. Whether this is what FR3 requires is unknown | [threat-model-v1.md](../security/threat-model-v1.md) (T7, T9, T15) |
| IEC-4-2-FR4 | FR4 — Data Confidentiality | unassessed | Candidate evidence, same finding as CRA-I-2e: key-at-rest encryption is real; the only bulk-confidentiality crate is explicitly non-production. Whether this is what FR4 requires is unknown | [RFC-v0.16-006](../../rfcs/done/RFC-v0.16-006-key-handling-encryption-patch.md); [fjell-sxt-crypto/src/lib.rs](../../crates/fjell-sxt-crypto/src/lib.rs) |
| IEC-4-2-FR5 | FR5 — Restricted Data Flow | unassessed | Candidate evidence: capability-gated IPC with no ambient channel; default-on networking for arbitrary services explicitly rejected. Whether this is what FR5 requires is unknown | [capability-system.md](../src/architecture/capability-system.md); [v1-non-goals.md](../release/v1-non-goals.md) (N2) |
| IEC-4-2-FR6 | FR6 — Timely Response to Events | unassessed | Candidate evidence: the kernel audit ring records security-relevant events, drained via a capability-gated syscall. Whether this is what FR6 requires is unknown | [ring.rs](../../crates/fjell-kernel/src/audit/ring.rs); [threat-model-v1.md](../security/threat-model-v1.md) (T16) |
| IEC-4-2-FR7 | FR7 — Resource Availability | unassessed | Candidate evidence, same finding as CRA-I-2h: graceful fleet-partition degradation exists; no verified in-kernel DoS/resource-exhaustion mitigation found. Whether this is what FR7 requires is unknown | [threat-model-v1.md](../security/threat-model-v1.md) (T13) |

---

## Findings surfaced while building this mapping

- **`fjell-sxt-crypto` is a documented non-production dependency for the
  confidentiality rows (CRA-I-2e, IEC-4-2-FR4).** Its own doc-comment states
  the AES implementation has a data-dependent-indexing cache-timing leak and
  requires an explicit `crypto-profile-development` feature acknowledgement
  to compile. This is not new information — RFC-v0.7.3-002 documented it in
  v0.7.1 — but it had never before been read against an external
  requirement, and confirms D5's expectation directly: the clause that
  "sounds like" the strongest `met` candidate on first read is the one that
  turned out `partial`.
- **RFC-v0.7.3-002's own specified deliverable docs do not exist.** That RFC
  (Status: Implemented, v0.7.1) specifies creating
  `docs/src/security/crypto-profile.md` and
  `docs/src/security/crypto-roadmap.md`, and lists their existence as its
  own acceptance criteria. Neither file exists anywhere in the tree today,
  while `fjell-sxt-crypto`'s live doc-comment still points at both. This is
  the same defect shape as E-023 (a release tool RFC marked done while most
  of its specified behaviour was never built) recurring in a different RFC.
  Filed as **E-028** (`docs/rfcs/ERRATA.md`), ACCEPTED, `unscheduled` — not
  fixed here, per this RFC's non-goal on building mechanisms.
- **No clause in Annex I resisted the mapping outright.** Every clause
  produced at least one honest row; none required inventing a Fjell
  mechanism that does not exist to force a fit. The clauses that came
  closest to "does not apply" (data minimisation, impact on other
  devices/networks) are marked `not-applicable` with a stated reason to
  re-evaluate, rather than silently omitted.
