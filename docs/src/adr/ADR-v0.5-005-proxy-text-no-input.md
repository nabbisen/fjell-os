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
