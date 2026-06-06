# ADR-v0.4-004 — Operator-Initiated Remote Update Metadata Fetch

**Status:** Accepted  
**Date:** 2026-05-19 (v0.4.0, RFC v0.4-004)

---

## Context

`upgraded` must be able to fetch update index metadata from a remote server
to decide whether an update is available, without exposing the device to
unsolicited inbound traffic or autonomous background polling.

## Decision

`upgraded` gains a `fetch_update_index()` function that:
1. Requests an `UpdateMetadata` SXT channel from `secure-transportd`.
2. Issues `SXT_UPDATE_METADATA_FETCH` over the established channel.
3. Receives `SXT_UPDATE_METADATA_REPLY` and reads the payload.
4. Closes the channel.

All steps are initiated by `upgraded`; there is no background polling
or push from the server.  The fetch is gated by the existing anti-rollback
policy checks already wired in `upgraded`.

cap-broker grants `upgraded` only `RIGHT_SEND | RIGHT_RECV` on
`ResourceClass::SxtSession`; it cannot mint new session capabilities.

## Consequences

- The "no autonomous network" rule from ADR-0012 is preserved: fetch
  happens only when the upgrade policy engine decides to check.
- Server-side push attacks are structurally impossible; upgraded initiates
  all TCP connections.
- The SXT channel is short-lived (open → fetch → close); no persistent
  connection is maintained.
- In v0.4.0 the HTTP/1.1 request body and response parsing are stubbed;
  v0.5.0 will add strict HTTP parsing and response validation.
