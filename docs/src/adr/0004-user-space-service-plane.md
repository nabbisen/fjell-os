# ADR-0004 — User-Space Service Plane

**Status:** Accepted  
**Date:** 2026-05-17 (v0.1.4, RFC 045) — captures decisions made in M3–M5

---

## Context

Fjell OS is a microkernel. All business logic lives in user space. But how
are user-space services organised? What is the "service plane" and what are
its contracts?

---

## Decision

Every service is a separate task communicating exclusively through IPC.
Service identity is expressed through `ServiceId` (16-byte tag); authority
through capability handles.

The **service plane** consists of:

- `fjell-service-manager` — lifecycle orchestration (spawn, health, restart policy).
- `fjell-cap-broker` — capability delegation and policy (default deny in v0.2).
- `fjell-configd` — configuration distribution.
- `fjell-auditd` — kernel audit drain.
- `fjell-semantic-stream` — semantic state/event/intent bus.
- `fjell-proxy-text` — text rendering of semantic state.
- Device services: `fjell-devmgr`, `fjell-driver-virtio-blk`.
- Durable services: `fjell-storaged`, `fjell-bootctl`, `fjell-upgraded`, `fjell-powerd`.
- Trust services: `fjell-verifyd`, `fjell-rootfsd`, `fjell-snapshotd`.
- Evidence services: `fjell-measuredd`, `fjell-attestd`, `fjell-recoveryd`.

At v0.1.x all these are embedded in the kernel image as prebuilt ELFs and
spawned by `fjell-init` using the inline-init workaround (ADR-0010 /
RFC 038).

---

## Consequences

- No service has ambient authority; each must receive an explicit capability
  from `cap-broker` or `init`.
- Fault isolation: a crashing service cannot crash the kernel or other
  services.
- Service restart is possible without a reboot (policy-dependent).
- The inline-init workaround at v0.1.x means all 20 services are in the
  TCB. v0.2 service-plane separation (RFC 038) begins shrinking this.

---

## Security Boundary Impact

The service plane is the boundary between trusted and untrusted code.
A service holding the wrong capability can exceed its authority only if
the kernel fails to check it — which is the v0.1.x enforcement gap (see
RFC 029 audit and v0.2 RFC 031 closure).

---

## Deferred Work

- Service-plane separation from inline-init: v0.2, RFC 038.
- Cooperative-service ready protocol + timer fail-safe: v0.2, RFC 037.
- `cap-broker` default deny: v0.2, RFC 040.
- Service auto-restart policy: v0.3.

---

## Related RFCs

- RFC 011 (Service Separation M3), RFC 019 (IpcTryRecv M4)
- RFC 037, RFC 038 (v0.2 service-plane separation)
- RFC 040 (cap-broker bootstrap and default deny, v0.2)
