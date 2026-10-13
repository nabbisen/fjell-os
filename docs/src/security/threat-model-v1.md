# Fjell OS — Threat Model v1.0

*Governed by RFC-v0.15-002. Authoritative for v1.0.0.*

---

## Adversary model

Every threat is keyed to an adversary capability, not a named actor.

| Code | Capability | Description |
|------|-----------|-------------|
| C-NET-PASSIVE | Read fleet network traffic | Passive on-path attacker |
| C-NET-ACTIVE | Read + modify + inject fleet traffic | Active on-path attacker |
| C-NODE-EXEC | Execute arbitrary code on one node | Full node compromise (post-exploit) |
| C-NODE-PHYS | Physical access to one node, once | Single-visit hardware adversary |
| C-NODE-PHYS-PERS | Physical access on demand | Persistent hardware adversary |
| C-OPER-MISTAKE | Operator error within authorised scope | Accidental misconfiguration |
| C-OPER-MAL | Malicious operator within authorised scope | Insider threat |
| C-SIGN-COMPR | Possession of a current signing key | Compromised release signing key |
| C-ANCHOR-COMPR | Possession of TrustAnchorRoot key | Highest-privilege compromise |
| C-SUPPLY | Modify the build supply chain | Software supply chain attack |

---

## In-scope threats (v1.0 commits to mitigating)

### T1 — Unauthorised capability acquisition

**Adversary:** C-NODE-EXEC  
**Defence:** Capability system (I1–I3); `require_cap` enforces handle + rights + kind matching. RFC 031, 040, RFC v0.9-002.  
**Residual:** A bug in `require_cap` itself. Mitigated by the unsafe-audit gate and the property test suite (RFC v0.6-001).

---

### T2 — Cap-broker policy bypass

**Adversary:** C-NODE-EXEC, C-OPER-MAL  
**Defence:** RFC 031 (cap-broker), RFC 040 (intent ledger). The broker enforces manifests; operator approval required for all grants.  
**Residual:** Typo in a manifest. Mitigated by `cargo xtask dev lint` (RFC v0.9-002) and the ABI snapshot gate (RFC-v0.10-002).

---

### T3 — Lease-expired authority replay

**Adversary:** C-NET-ACTIVE, C-NODE-EXEC  
**Defence:** I3 (lease epoch bound) + RFC-v0.11-005 (replay cache + nonce challenge). Expired leases are invalidated in the CSpace.  
**Residual:** Clock skew within configured tolerance window (24h default).

---

### T4 — MMIO ownership confusion

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 016 (MmioRegion), RFC 035, RFC 051. All MMIO access goes through the `MmioRegion` handle. RFC-v0.12-004 annotated all 23 MMIO sites.  
**Residual:** Unknown MMIO regions not yet imported into `devmgr`.

---

### T5 — DMA-based memory aliasing

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 017 (DMA capability), RFC 036, RFC 052. DMA grants are per-region and epoch-bound.  
**Residual:** Hardware IOMMU not present on v0.12 target; purely software enforcement.

---

### T6 — Unsafe-code regression

**Adversary:** C-SUPPLY  
**Defence:** I6 (`forbid(unsafe_code)` except at audited boundaries); unsafe-audit gate; RFC v0.6-004 + RFC 060.  
**Residual:** Unsoundness in the Rust standard library or LLVM — **and in this
project's own `unsafe`.** The unsafe-audit gate verifies that every site carries
a classified justification, not that the justification is true. E-046 was such a
site: a safe function reinterpreting another service's bytes as a struct, with a
SAFETY comment, passing the gate. RFC-0.32-002 removed it and added Miri over
the format crates on schedule and dispatch — a check of behaviour rather than of
comments, over three crates, not the tree. *(Corrected at the 0.32.0 cut review:
this residual named only the toolchain.)*

---

### T7 — Trap-frame corruption

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 001 (trap handling), RFC 022 (trap-frame layout). Trap frames are kernel-only memory.  
**Residual:** Microarchitectural fault-injection requiring C-NODE-PHYS-PERS.

---

### T8 — Update rollback

**Adversary:** C-NODE-EXEC, C-NODE-PHYS  
**Defence:** RFC v0.3-003 (anti-rollback counter), RFC 002 (boot control).  
**Residual:** Anti-rollback counter is software; hardware OTP rollback counter not present on v0.12 target.

---

### T9 — Bundle tampering

**Adversary:** C-NET-ACTIVE, C-SUPPLY  
**Defence:** RFC v0.9-004 (content-addressed bundle), RFC-v0.11-003 (Ed25519 signature).  
**Residual:** A tampered bundle that also has a valid signature requires C-SIGN-COMPR.

---

### T10 — Signature forgery (algorithmic)

**Adversary:** C-NET-ACTIVE  
**Defence:** Ed25519 (RFC 8032) via `fjell-sig-ed25519` (RFC-v0.11-002). Ed25519 provides 128-bit security level.  
**Residual:** Out-of-scope OS3 (CRQC); see below.

---

### T11 — Signing key compromise

**Adversary:** C-SIGN-COMPR  
**Defence:** RFC-v0.11-004 (key rotation + revocation records), RFC-v0.13-003 (compromise playbook).  
**Residual:** Time window between compromise and revocation distribution. Bounded by replay cache window (24h).

---

### T12 — Attestation replay

**Adversary:** C-NET-ACTIVE  
**Defence:** RFC-v0.11-005 (replay cache, nonce challenge). One nonce, one response; sliding window cache for async reports.  
**Residual:** Post-reboot cache emptiness. Mitigated by `verifier_boot_count` in challenge.

---

### T13 — Fleet partition exploitation

**Adversary:** C-NET-ACTIVE  
**Defence:** RFC-v0.13-002 (partition FSM). Partitioned nodes refuse authority-class operations; reconcile manifest required.  
**Residual:** Coordinator itself being the partitioned node. Operator promotion required (RFC-v0.13-005).

---

### T14 — Stale trust anchors

**Adversary:** C-NET-ACTIVE  
**Defence:** RFC-v0.11-004 §6 (stale-anchor degraded posture). Nodes refuse bundle installs if trust anchor update window exceeded.  
**Residual:** Operator must configure update window appropriate to connectivity profile.

---

### T15 — Persistent-store corruption

**Adversary:** C-NODE-EXEC, C-NODE-PHYS  
**Defence:** RFC 053 (storaged integrity). Content-addressed storage with epoch binding.  
**Residual:** Store corruption before first epoch write detected only on subsequent read.

---

### T16 — Audit-ring evidence gap

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 053, 054 (audit drain). Audit ring is kernel-managed; services emit records through the drain capability.  
**Residual:** A compromised kernel could suppress records. Mitigated by the measurement chain (T8 defences).

---

### T17 — IPC sender forgery

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 055 (kernel-attested sender identity). The kernel stamps sender `TaskId` on every IPC; services cannot forge it.  
**Residual:** None within the current architecture, for forgery. **Adjacent,
and not forgery:** a correctly identified sender whose *payload* is malformed —
by a bug or by compromise. The kernel attests who sent the bytes, never that
they are well-formed, so every receiver must decode rather than reinterpret.
E-046 was exactly this, on the semantic path; RFC-0.32-002 made that path
decode through a checked wire format and fuzzes the decoder. **No mechanism
requires every service to do the same.** *(Named at the 0.32.0 cut review: this
entry covered forgery and was silent on the neighbouring class.)* **T21 is a
second instance, on the presentation boundary** (RFC-0.34-005): a
presentation's return leg claimed an authority value as an ordinary payload
word; RFC-0.34-004 closed it the same way — the receiver decides from what
the kernel attests, never from what a correctly identified sender sent.

---

### T18 — Service init authority escalation

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 057, 058 (service lifecycle). Services are spawned from the init image with pre-bound caps; no escalation path.  
**Residual:** A vulnerability in `cap-broker`'s grant logic (covered by T2).

---

### T19 — Operator mistake within authorised scope

**Adversary:** C-OPER-MISTAKE  
**Defence:** Bounded blast-radius operations (RFC-v0.13-001 §3); confirmations for fleet-wide actions.  
**Residual:** Mistake in the TrustAnchorRoot ceremony (see OS7).

---

### T20 — Reproducibility-failure-as-substitution

**Adversary:** C-SUPPLY  
**Defence:** RFC-v0.10-003 (reproducible build gate), corrected by RFC-0.30-001. Two *genuinely independent* builds — a scoped `cargo clean` precedes each one, so the second cannot reuse the first's incremental output — compared by SHA-256 digest (hardened from FNV-1a in RFC-v0.16-005, H-04). Covers the kernel ELF and all 29 service prebuilts (30 artefacts). Runs in CI on every push (`ci-repro-check`), and every release record carries the run id for its own release commit (exit criterion 9), so whether it ran is answerable per release rather than asserted here. The toolchain that produced each artefact is recorded alongside it — `rustc -vV`'s release, commit hash, host and LLVM version, on the repro baseline, every evidence provenance file and the trust report (RFC-0.30-003) — and the channel is pinned to an exact patch version (RFC-0.31-003). This is **same-machine** reproducibility only: nothing compares two machines' digests against each other. The separate, unrelated `--skip-build` tier (test-all, CI-cheap) checks only that the *committed* prebuilts have not gone stale against their recorded baseline; it does not build anything and is not this defence.  
**Residual:** Malicious toolchain that produces identical output for different inputs; cross-machine reproducibility, unproven — no two machines' digests are ever compared. That gap is maintained in [`docs/src/releasing/v1-limitations.md`](../releasing/v1-limitations.md) and is deliberately not restated here.

*Rewritten 2026-09-13, at the 0.31.0 cut's review. **This row had been corrected under exit criterion 8 at two consecutive cuts** — at 0.30.0 for saying the toolchain was "not yet recorded" two days after RFC-0.30-003 recorded it, and at 0.31.0 for saying `ci-repro-check` "has never succeeded" after RFC-0.31-002 made it green, and for naming E-037's "two surviving instances" after E-037 closed. Both times the stale part was **live state restated here instead of cited**: which job is green, what is pinned, which errata are open. Checked across the document: of twenty rows, T20 was the only one carrying erratum numbers (E-037 ×2, E-041) or CI state, and the only one holding a criterion-8 correction. It now states the defence durably and points at the limitation set for anything that moves, so a third correction of the same kind should not be possible.*

---

### The presentation boundary (T21–T23, RFC-0.34-005)

*Added 2026-10-13. E-060: the component that receives every operator-facing
byte, and that RFC-0.33-002's archetype **A4** names as something a person
depends on, had never been analysed as a boundary. T17's own residual
already named the class it falls into; these three entries put it in the
model rather than leaving it implied.*

**What configuration this covers, and what it does not (§B).** Today, shipped,
a presentation (`proxy-text`, `proxy-braille`) is a task on the **same node**,
spawned from the same init image, holding a kernel-issued capability like any
other service. It is "in the TCB" in exactly the sense every other same-node
service already is: the kernel attests its identity, and nothing beyond that
identity is trusted — T17's own residual already establishes that a
correctly identified sender's payload is not. A compromised presentation is
not a new adversary capability; it is **C-NODE-EXEC**, the same class that
already covers compromising any other task, applied to this one. **What this
amendment does not cover: a presentation running on a separate or personal
device.** RFC-0.33-002 names that as a future direction and does not decide
it; if it is ever built, the channel between the node and that device —
authentication, replay, confidentiality, device identity — is a materially
different boundary from a same-node task, and is its own amendment, not a
thin extension of these three entries. **No assistive presentation on a
separate or personal device may be described as meeting this model until
that amendment exists** (E-060's own closing condition, carried forward
rather than met).

---

### T21 — Presentation payload forgery (the authority-claim instance)

**Adversary:** C-NODE-EXEC  
**Defence:** RFC-0.34-004 (closes E-059). A presentation's `DISPATCH_ACTION`
return leg used to carry the rights it claimed to hold as an ordinary IPC
payload word, and `semantic-stream` authorised against that word under a
comment asserting it was kernel-verified — it was, on the presentation's own
side, and not on the receiving one. The payload word is now never read: the
receiver's decision comes from the kernel-attested sender's own compiled-in
policy row (`CapRights::PROXY_TEXT_DEMO` for `proxy-text`, `0` for every
presentation that has never held a dispatch capability), the same value
`spawn.rs` installs, read from one declaration rather than two hand-kept
ones. Demonstrated: a forged claim (`0xFFFFFFFF`, every right there is) is
refused every run, in a committed tier (`semantic-liar`) a presentation
installed with no dispatch capability at all exercises against a dedicated
test action. **Instance of T17's adjacent class:** yes — a correctly
identified sender (the presentation) whose payload (the claimed rights) was
not what it was trusted to be.  
**Residual:** A dispatched action executes nothing yet (RFC-0.34-004 D6;
RFC-0.33-002 §C's input path is unbuilt). This defence has therefore not yet
been tested against a real stake — a capability-gated effect a forged claim
could actually obtain. It must be re-examined, not assumed to still hold
unchanged, the day an input path is built.

---

### T22 — An absent or crashed presentation stalls the node's narration

**Adversary:** C-NODE-EXEC  
**Defence:** RFC-0.34-001 (closes E-058). `semantic-stream` used to forward
each published envelope to a presentation with a blocking call before
replying to the publisher, so an absent or crashed presentation stalled
every publisher behind it — measured at 314 → 124 output lines with
`proxy-text` never started. The stream now never initiates a blocking IPC to
a presentation at all: each presentation **asks** for its next message and
is answered by **reply** — the one IPC that never blocks its issuer — and is
woken through an endpoint of its own when something new arrives. Demonstrated
in two committed tiers: `semantic-absent` (a presentation that never starts;
the node reaches its last phase, 270 lines, with the surviving presentation
still rendering) and `semantic-crash` (a test-only presentation that
registers, is woken, and faults on its first message; twelve subsequent
publishes are all answered). **Not an instance of T17's adjacent class** —
this is an availability property of the fan-out mechanism, not a decision
about a sender's payload.  
**Residual, named rather than left in `v1-limitations.md` alone:**
(a) a presentation that is alive but never asks again is not detected as
dead — there is no wall-clock timer, only the stream's own call activity to
count, and one with nothing queued is idle by design and never reported;
(b) a presentation that faults in the two instructions between being told
"parked" and reaching its next receive still blocks the stream on the
following wake — the one place the stream can wait on one, and it is code
this project wrote;
(c) envelopes queued past an 8 KiB bound are dropped, newest first, and the
gap is reported on the node's own console line, **not through the
presentation itself** — a person reading only one presentation's output is
not told something was skipped;
(d) nothing restarts a stuck presentation;
(e) a rendered line over the kernel's 160-byte console-line bound is split
— visibly, since E-062's close (`[cont]` marks each half) rather than
silently — but the bound itself is unchanged, and nothing stops a longer
title or description from being the first committed line to cross it (the
longest measured so far is 153 bytes of text, 135 of braille).

---

### T23 — A compromised presentation shows false state to the attending operator

**Adversary:** C-NODE-EXEC — reused, not a new class (§D). A presentation is
a task like any other; running malicious code as it is already inside the
capability "execute arbitrary code on one node", the same one that already
covers every other service. What is new is not the adversary — it is the
**victim**: every other entry in this model defends the node against a
sender, or the fleet against a node; this is the first entry whose receiver
is a person, with the archetype that makes this matter named explicitly
(**A4**, `v1-direction.md`: an operator who reads the node's state, warnings
and offered actions through a presentation instead of a screen).  
**Defence:** None.  
**Residual, named rather than asserted away:** nothing lets the attending
operator cross-check what a presentation shows against the node's own
authoritative record. The audit ring (T16) records security-relevant events,
but nothing surfaces it to an operator through a channel independent of the
presentation they are already reading — if that presentation is compromised,
it can render anything regardless of what it decoded, and the operator has
no second source. This is also why the excluded configuration above (a
presentation on a separate or personal device) cannot be called safe without
its own amendment: the same question — can what a person is shown be trusted
— only gets harder once the presentation is no longer even on the node whose
state it claims to report.

---

## Out-of-scope threats (v1.0 explicitly does NOT defend against)

| Code | Threat | Rationale |
|------|--------|-----------|
| OS1 | TrustAnchorRoot compromise + node filesystem write | Irrecoverable; re-provision physically |
| OS2 | Compromise of build environment before signing | Supply-chain hygiene is operator responsibility |
| OS3 | Cryptographic break of Ed25519 (CRQC) | Post-quantum hybrid is research track |
| OS4 | Byzantine fault from > 0 cooperating malicious nodes | BFT deferred post-v1.0 |
| OS5 | Persistent physical adversary with anti-tamper bypass | Anti-tamper is hardware-side |
| OS6 | Side-channel attacks (timing, power, EM) | Primitives have mitigations; no defence claimed |
| OS7 | Malicious insider with TrustAnchorRoot access | Organisational control, not technical |
| OS8 | Covert channel via audit-ring timing | Possible; unmitigated at v1.0 |

---

## Operator obligations

1. **Key custody:** TrustAnchorRoot on offline cold-storage; rotation procedure per RFC-v0.13-003.
2. **Physical security:** Nodes meeting A3 archetype require physical tamper evidence.
3. **Build environment hygiene:** Clean CI environments; reproducible builds verified.
4. **Audit monitoring:** Review Trust Report at each release cycle.
5. **Re-attestation cadence:** Schedule per RFC-v0.13-004 §2.4.
6. **Incident response:** Follow RFC-v0.13-003/005 playbooks.

---

*Gate: `ci-threat-model-check` verifies every T<n> references a merged RFC in `done/`.*
