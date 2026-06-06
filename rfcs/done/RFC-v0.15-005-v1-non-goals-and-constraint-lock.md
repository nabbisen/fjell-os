# RFC-v0.15-005 — v1.0 Non-Goals and Constraint Lock

**Status:** Implemented (v0.15.0)
**Target version:** v0.15.0
**Parent:** v0.15-001.
**Cross-refs:** RFC 061 §3.4, §4, §7.

## 1. Problem

RFC 061 listed Fjell's identity, archetypes, invariants, and an
initial non-goals set. v0.10–v0.14 have surfaced both pressures to
add scope and clarifications that need to land in writing. v0.15
locks the constraint set for v1.0 — turning RFC 061's strategic
choices into committed, individually-justified prohibitions.

The deliverable is **the document an operator or external observer
can read** to know what Fjell *won't* do at v1.0, *why*, and what the
post-v1.0 reconsideration path looks like.

## 2. The non-goals document

`docs/release/v1-non-goals.md` — explicitly authoritative for v1.0.
Structure:

### 2.1 Form

Each non-goal is recorded as:

```text
## N<n>: <one-line non-goal>

What Fjell does not do:
  ...

Why this is rejected for v1.0:
  ...

Why it is tempting:
  ...

What an operator should do instead:
  ...

Reconsideration path (post-v1.0):
  - condition under which this could be revisited
  - the RFC that would have to be written first
```

The four-headed structure forces honesty: rejection without
acknowledging the temptation is a form of evasion; rejection without
an alternative path leaves operators stuck.

### 2.2 The committed v1.0 non-goals

Drawn from RFC 061, refined by v0.10–v0.14 experience:

- **N1.** POSIX compatibility surface.
- **N2.** Default-on networking for arbitrary services (networking
  per capability remains; v0.4 stays).
- **N3.** Desktop GUI or windowing.
- **N4.** Browser hosting or any user-agent.
- **N5.** Package manager with dependency resolution (file-based
  registry from v0.14-004 stays; dependency-resolved installation
  does not).
- **N6.** General-purpose remote shell or SSH-equivalent.
- **N7.** Container orchestration substrate (Kubernetes-style).
- **N8.** Hard real-time scheduling guarantees.
- **N9.** Multi-fleet federation (single fleet at v1.0).
- **N10.** Automatic leader election / coordinator failover (operator
  promotion remains; v0.13-005).
- **N11.** WASM workload hosting.
- **N12.** Heterogeneous accelerator support (GPU/NPU/DSP).
- **N13.** Self-service end-user provisioning workflows.
- **N14.** Multi-language SDK bindings (Rust only).
- **N15.** Public service marketplace.
- **N16.** AI agent autonomous authority (agents may exist as
  services within the capability model; "autonomous authority" is
  rejected).
- **N17.** Live code patching of running services.
- **N18.** Cryptographic algorithm hot-swap.
- **N19.** Distributed consensus across nodes (paxos / raft).
- **N20.** Hardware abstraction across multiple boards at v1.0
  (one supported target per RFC-v0.12-002).

### 2.3 Per-non-goal commitment

For each N<n>, the four-headed treatment is filled. Examples below
are abbreviated; the document carries the full text.

**N1. POSIX compatibility surface.**

- *What Fjell does not do:* implement `read(2)`, `write(2)`,
  `open(2)`, `fork(2)`, the file-descriptor abstraction, signals,
  process groups, ttys, or any POSIX-shaped library that requires
  shipping them.
- *Why rejected:* the identity in RFC 061 §2 commits Fjell to
  explainable authority. POSIX descriptors are ambient authority
  (a process holds an FD, not a capability). Shipping a POSIX shim
  defeats I1 by re-introducing ambient authority through
  compatibility.
- *Why tempting:* enormous existing software base; would make Fjell
  immediately useful for a wide set of operators.
- *Operator alternative:* services are authored against `fjell-sdk`.
  Existing POSIX software runs on Linux or BSD on adjacent hardware.
- *Reconsideration path:* a research-track RFC could explore a
  "POSIX-shaped service" that translates ambient-style requests into
  capability-bearing IPC while preserving I1. Outcome unknown; not
  scheduled.

**N9. Multi-fleet federation.**

- *What Fjell does not do:* allow nodes in fleet A to send signed
  authority records that bind fleet B; allow trust anchors to flow
  between independently-administered fleets.
- *Why rejected:* the threat model (v0.15-002) is built around a
  single-fleet boundary. Federation introduces cross-org trust
  policy that has no current operational consumer.
- *Why tempting:* multi-tenant fleet operators exist; would broaden
  applicability.
- *Operator alternative:* run independent Fjell fleets; exchange
  evidence externally if needed.
- *Reconsideration path:* requires a follow-up identity-level RFC
  (analogue to RFC 061 but for federation) plus a threat-model
  extension.

(Each remaining N<n> follows the same form.)

### 2.4 Constraint lock

After v0.15 lands, additions to N1–N20 may only be made by RFCs
explicitly authorising the change with reference to this RFC.
Removals (i.e. allowing a previously-rejected goal) require an
identity-level RFC.

This is a hard rule because non-goals erode silently: every
individual exception is reasonable; the cumulative effect is identity
loss.

## 3. Adversarial review

The document survives v0.15 only after an adversarial review pass:

- A reviewer (anyone, internal or external) attempts to negotiate at
  least three non-goals off the list.
- For each attempt, the document either:
  - Holds (the rejection is reaffirmed in the doc).
  - Is updated (the doc admits an unforeseen consideration; this is
    rare and itself an event worth noting in the next RFC).
- The review pass and its outcome are committed at
  `docs/release/v1-non-goals-review.md`.

Without this pass, the document is unproven.

## 4. Linkage to the readiness matrix

The v1.0 readiness matrix (RFC-v0.10-007) gains a "Non-goals" row
asserting that this document exists and has passed adversarial
review. Without that assertion's resolution to DONE, v1.0 cannot tag.

## 5. Acceptance criteria

1. `docs/release/v1-non-goals.md` exists and covers N1–N20.
2. Each N<n> follows the four-headed structure of §2.1.
3. Adversarial review attested at
   `docs/release/v1-non-goals-review.md`.
4. The readiness matrix row "Non-goals" is DONE.
5. The document references the identity statement in RFC 061 §2.
6. No N<n> contradicts an existing invariant from RFC 061 §4.

## 6. Out of scope

- Predicting which non-goals will be revisited post-v1.0.
- A roadmap for v2.0.
- Marketing language. The document is operational.
- Public consultation processes around the non-goals. The list is
  authorial; external input is welcome but not authoritative.
