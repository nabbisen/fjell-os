# ADR-0012 — No General Network Before Security Closure

**Status:** Accepted  
**Date:** 2026-05-17 (v0.1.4, RFC 045) — captures decision made in scope
declaration (RFC 024)

---

## Context

Fjell OS is a security-oriented embedded OS. Networking would be valuable
for attestation, remote update, diagnostics, and remote management. But
networking introduces a large new attack surface. The design question is:
when is it safe to add networking?

---

## Decision

No network transport is added until the local security boundary is closed.

v0.2.0 (*Security Boundary Closure*) is the mandatory gate. v0.4.0
(*Minimal Secure Networking*) is the earliest version that adds any
networking.

The networking in v0.4 is constrained to:
- Attestation protocol (one specific endpoint, limited surface).
- Update metadata transport (signed, verified bundle delivery).
- Diagnostics (read-only, capability-controlled).

General-purpose sockets, TCP server ports, general application networking —
these have no current milestone.

---

## Consequences

- v0.1.x and v0.2.x have zero external network attack surface.
- The first network protocol Fjell OS speaks will be designed with the
  full benefit of the v0.2 capability enforcement model.
- `virtio-net` is not shipped and is excluded from the v0.1.x build.
- Any RFC proposing to add a network listener before v0.4.0 is rejected
  by default; it must argue against this ADR.

---

## Security Boundary Impact

This ADR *eliminates* the network attack surface for v0.1.x and v0.2.x.
It is the highest-leverage security decision in the project: every
vulnerability class that requires network reachability is simply absent.

---

## Deferred Work

- Attestation transport: v0.4.
- Signed update transport: v0.4.
- Diagnostics network: v0.4.
- General-purpose sockets: no current milestone.
- TCP / UDP stack: no current milestone.
- TLS / DTLS / noise: no current milestone.

---

## Related RFCs

- RFC 024 (Release Freeze and Scope Declaration, v0.1.1)
- RFC 027 (Threat Model §4, v0.1.2) — out-of-scope attacker: remote
