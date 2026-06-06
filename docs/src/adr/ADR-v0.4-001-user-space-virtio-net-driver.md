# ADR-v0.4-001 — User-space virtio-net Driver with Capability-gated Net Access

**Status:** Accepted  
**Date:** 2026-05-19 (v0.4.0, RFC v0.4-001)

---

## Context

Fjell OS needs network connectivity for update metadata fetch, diagnostics
push, and attestation.  ADR-0012 established that general network access must
not precede security closure.  v0.4.0 is the first release where the security
posture is considered sufficient for minimal, operator-gated network use.

The question is: where does the virtio-net driver live, and how is access to
it constrained?

## Decision

The virtio-net driver runs entirely in user-space as a service process,
not in the kernel.  Access to the network device is mediated by two new
capability kinds: `CapKind::Interrupt` (0x0014) for IRQ binding and
`CapKind::NetDevice` (0x0015) for packet send/receive authority.

cap-broker enforces the policy: only `VIRTIO_NET` (ImageId 22) may
receive `CapKind::NetDevice`, and only `netd` (ImageId 21) may acquire a
`NetDevice` capability via cap-broker brokerage.  No other service may
access the device registers or DMA buffers directly.

The virtio ring abstraction (`Ring`, `RING_SIZE=16`, `NET_RING_DESCRIPTORS=16`)
is implemented in a host-testable core library (`fjell_driver_virtio_net`),
separate from the driver binary, so that the ring logic can be unit-tested
without MMIO.

## Consequences

- Network packet flow: `virtio-net driver → netd → secure-transportd → services`.
- No service can bypass this path; the kernel mediates all capability grants.
- The host-testable core library approach (features, ring, state, MMIO
  register map) allows CI to verify correctness without QEMU.
- DMA region grants are scoped to the driver and revoked on driver fault.
- IRQ handling uses the new `sys_irq_bind` / `sys_irq_wait` / `sys_irq_ack`
  syscalls; kernel handles IRQ demultiplexing.
