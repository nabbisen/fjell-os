# ADR-v0.5-005 — proxy-text Is Output-Only; No Remote Input Path

**Status:** Accepted  
**Date:** 2026-05-19 (v0.5.0, RFC v0.5-005)

## Context

`proxy-text` renders semantic state to a serial terminal.  A tempting feature
would be to accept typed commands (e.g. "confirm upgrade").  This would create
an uncontrolled input path that bypasses capability policy.

## Decision

`proxy-text` is a pure output renderer.  It never reads from the serial port.
Operator input is delivered through the `fjell-tools` CLI over a separate
capability-gated IPC path, never through the text proxy's output fd.

Rate limiting (`RateLimitEntry`) is enforced per `(tag, service_id)` key to
prevent flood denial-of-service from a faulted service.  Pinned critical entries
(`critical: bool`) bypass the rate limit and always appear in the pinned region.

## Consequences

- The serial terminal cannot be used as an attack vector against the service plane.
- Operator actions require explicit tool invocation, which is logged in the audit trail.
- Renderer correctness can be tested purely as a function of intent stream → bytes.

> **Correction, 2026-09-24 (RFC-0.33-002's review).** The alternative this ADR
> names — operator input delivered through the `fjell-tools` CLI over a separate
> capability-gated path — **does not exist.** No `fjell-tools` command delivers
> anything to a running node; the only input that reaches one is a UART byte
> consumed by `init` for a test trigger. The decision recorded here (the proxy
> does not read input) stands; the route it points at instead is unbuilt, so **a
> person operating a node through a presentation has no way in at all.** Recorded
> in `v1-limitations.md` (E-054's section, item 3); the shape of an input path is
> RFC-0.33-002 §C's open question, and E-059 constrains it.

