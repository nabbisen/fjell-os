# ADR-0004 — Semantic Stream (ABDD)

**Status:** Superseded — see ADR-0005 Semantic Stream First (RFC 045)  
**Date:** 2026-05-04

## Context

How should Fjell OS implement ABDD (Accessible by Default and by Design)
without hard-coding presentation patterns for every disability category?

## Decision

Applications emit **structured intent** (`IntentNode`) rather than pixel
coordinates or terminal escape sequences.  A **Presentation Proxy** outside
the kernel translates that intent into whatever output modality the user
requires: text, speech, braille, machine JSON, or anything else.

The OS guarantees: structured output is available.  The OS does *not*
dictate: how that output looks.

## Rationale

Hard-coding accessibility modes ("high-contrast mode", "screen-reader mode")
requires enumerating all possible user needs in advance.  That enumeration
is always incomplete and becomes a maintenance burden.

If the OS only emits *meaning* — "this is a confirmation dialog with two
choices: Proceed or Cancel, danger level: Critical" — then any proxy can
render it optimally for its audience, including audiences not yet imagined.

This is the Unix pipeline principle applied to UI: separate the *production*
of information from its *presentation*.

## Consequences

- `fjell-semantic-format` defines `IntentNode`, `StateNode`, `EventNode`
  schemas — implemented in M7.
- `fjell-proxy-text` is a reference Presentation Proxy that renders
  `IntentNode` as plain text — implemented in M7.
- The kernel itself emits no pixel data and has no graphics subsystem.
- A crashed proxy does not affect kernel, audit, config, or service-manager.
