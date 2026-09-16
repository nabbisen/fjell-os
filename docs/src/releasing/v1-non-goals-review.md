# v1.0 Non-Goals — Adversarial Review

*Required by RFC-v0.15-005 §3. The following non-goals were challenged
and the challenges were reviewed before landing v0.15.0.*

---

## Challenge 1 — N8 (multi-fleet federation)

**Challenger position:** "Operators running Fjell across multiple sites will need federation. Calling it a non-goal will block adoption."

**Response:**
- The v1.0 threat model (RFC-v0.15-002) is bounded to a single fleet; federation changes the threat surface.
- The reconsideration path is explicit: an identity-level RFC analogous to RFC 061 is the gate.
- v1.0 with a clear upgrade path is better than v1.0 with half-specified federation.

**Decision:** N8 held. Reconsideration path documented.

---

## Challenge 2 — N9 (automatic leader election)

**Challenger position:** "Manual coordinator promotion is operationally dangerous — if the operator is unavailable, the fleet is stuck."

**Response:**
- Automatic leader election requires distributed consensus which requires a fundamentally different failure model.
- The alternative — "first surviving node promotes itself" — creates split-brain risk with no current mitigation.
- The TrustAnchorRoot signature requirement makes the manual path auditable; automatic promotion would not be.
- MTTR for coordinator loss is documented at 30 min (§3.4 of recovery guide).

**Decision:** N9 held. MTTR added to failure modes table.

---

## Challenge 3 — N20 (self-healing without operator approval)

**Challenger position:** "Requiring operator confirmation for every recovery action defeats the purpose of having an automated system."

**Response:**
- N20 prohibits authority-changing self-healing only. Non-authority-changing retries (measurement upload retries, reconnect attempts) are permitted.
- The boundary is: if a recovery action changes who can do what, it requires operator sign-off. If it only retries an operation within existing authority, it proceeds automatically.
- This is consistent with RFC 061's identity: every authority grant is explainable.

**Decision:** N20 held; §what-fjell-does-provide clarified to make the boundary explicit.

---

*Review conducted during v0.15.0 development cycle.*
*Three challenges evaluated; all three non-goals held.*
*RFC-v0.15-005 acceptance criterion §3 satisfied.*
