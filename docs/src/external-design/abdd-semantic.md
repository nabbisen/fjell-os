# External Design — ABDD / Semantic Streams

*Subsystem 7 of 9. Anchored to FR-SEM-001…005, NFR-ACC-001…004 and
`fjell-semantic-format`, `fjell-semantic-v1`, `fjell-semantic-stream`,
`fjell-proxy-text` at v0.21.2.*

## 1. Responsibility

This subsystem is Fjell's distinguishing architectural bet: applications and
services emit **meaning, state, and operational intent** as structured data —
not drawing commands. Rendering (visual, audio, braille, simplified,
machine-processing) is the job of an external or user-space **Presentation
Proxy** / **Personal Proxy**, never the OS core. This is what makes ABDD an
architecture rather than a feature (design principle 2.6, "will not do" 4.5/4.6).

The design intent, from the requirements analysis, is to reject "enumerate every
accessibility pattern and branch on it" in favour of three shifts: intent/render
separation, proxy-side dynamic translation, and continuous state measurement
instead of fixed categories. The OS provides the semantic stream and the safe
boundary; the proxy does the adaptation.

## 2. External surface

### Intent Stream (FR-SEM-001, as-built `fjell-semantic-format`)

A structured, typed stream. Core value types: `Severity` (Low/Normal/Important/
Critical), `Status` (Unknown/Ok/Degraded/Warning/Failed), `EventResult`
(Ok/Denied/Failed/TimedOut/NotApplicable), `Importance` (Low/Normal/High/
Critical), plus `NodeId`, `CorrelationId`, `ActionId`, `ResourceName`, and
`BoundedText`. A node carries not just a display string but the meaning,
operability, importance, and danger level the requirement demands (NFR-ACC-001).

### Intent catalog (FR-DEV-002, as-built `fjell-semantic-v1`)

A frozen, versioned catalog of `IntentSchema` entries — e.g.
`UPDATE_STAGING_STARTED`, `UPDATE_ROLLBACK_TO_PREVIOUS_SLOT`,
`ATTEST_RECORD_SIGNED`, `SECURITY_PROVIDER_FAULTED`, `NET_LINK_UP`. Each schema
defines typed fields. The catalog is auto-published to
[Intent Catalog v1](../api/semantic-catalog.md).

### Presentation Proxy boundary (FR-SEM-002, as-built `fjell-proxy-text`)

`proxy-text` is the reference Presentation Proxy: it receives semantic nodes and
renders them to text (severity/status labels, scroll ring, pinned criticals,
rate limiting). It demonstrates that the OS emits intent and the proxy chooses
presentation — the OS prescribes no display method.

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-SEM-001 Intent Stream | Typed semantic nodes carrying meaning/options/danger | `fjell-semantic-format`, `semantic-stream` |
| FR-SEM-002 Presentation Proxy boundary | Stream consumed by a proxy that owns presentation | `fjell-proxy-text` (reference) |
| FR-SEM-003 Personal Proxy support | Structured state suffices for per-user adaptation on the proxy side | schema is presentation-agnostic |
| FR-SEM-004 Stream auth & integrity | See [Security & Trust](./security-trust.md) — signed, sequenced, per-proxy scope | `secure-transportd`, capability scope |
| FR-SEM-005 UI-independent operation | Same operation via console, API, audit tooling, proxy | **Output only**: text via `proxy-text`. No input path is built (ADR-v0.5-005; [what does not exist yet](../releasing/v1-limitations.md#accessibility-and-inclusion--what-does-not-exist-yet)), so the same *operation* cannot yet be performed through a console or a proxy |
| NFR-ACC-001 No loss of meaning | Nodes carry severity/importance/operability, not just strings | `Severity`/`Importance`/`Status` |
| NFR-ACC-002 Display-independence | No visual GUI dependency anywhere in core operation | proxy-side rendering only |
| NFR-ACC-003 Extensibility | No fixed user categories in the OS; proxy extends rules | catalog is data, proxy is external |
| NFR-ACC-004 Human readability | Text proxy output is directly readable | `proxy-text` renderer |

## 4. The architectural contract

The load-bearing property is **complete decoupling of display from processing**.
The OS core carries only "state and intent"; how it is expressed is outside the
OS's responsibility entirely. This is what simultaneously serves the
verifiability goal (no GUI stack to verify — "will not do" 4.5), the
sustainability goal (no rendering power in the base system), and the inclusion
goal (unknown needs handled on the proxy side). A display-less industrial robot
and an assistive personal device run the *same* core; only the proxy differs.
That is the architectural claim; it has been exercised with **two** proxies, on one decoder, one of them rendering braille cells (§5).

## 5. As-built scope limits & gaps

- **Continuous state measurement** (the analysis's third shift — adapting to
  input latency, mistap frequency, etc.) is a *design direction* for the proxy
  layer, not implemented in v1.0. The OS provides the semantic substrate that
  would make it possible; the adaptive proxy is future work.
- **Two presentations exist, and both are text on a console.** `proxy-text`
  renders the stream as text; `proxy-braille` (RFC-0.34-001) renders the same
  envelopes as lines of **uncontracted braille cells** — the stream a braille
  display driver would consume — through the same decoder, asserted by content
  from a committed vector. It is a small documented rule set, **not a braille
  standard**, read by no braille reader and driven on no display: QEMU `virt` has
  neither an audio device nor a braille display, so a proxy can only emit the
  stream a synthesiser or display driver would consume. It shows the producer's
  fallback text (a `TextToken`'s id is ignored — no catalogue exists), which
  means the claim above holds for the structure around the text and not for the
  text; and it omits an action's capability, reversibility and confirmation. See
  [what does not exist yet](../releasing/v1-limitations.md#accessibility-and-inclusion--what-does-not-exist-yet).
- **An unavailable proxy no longer stalls the publisher, with survivors.**
  `semantic-stream` never initiates a blocking call to a presentation: each
  presentation asks the stream and is answered by reply (an 8 KiB queue each,
  dropping the newest when full, reported on the node's own output); `proxy-text`
  asks like the others. Errata E-058 is closed. What survives — a
  presentation that is alive but silent is not detected as dead, a short window
  in which a faulting presentation could still hold the stream, gaps that are
  reported on the console and not through the presentation, and nothing that
  restarts a stuck one — is listed at
  [what does not exist yet](../releasing/v1-limitations.md#accessibility-and-inclusion--what-does-not-exist-yet).
- **There is no input path.** The proxy is output-only by decision
  (ADR-v0.5-005); the `fjell-tools` route that ADR names is not built.
- **Personal Proxy** (FR-SEM-003) is supported by the schema being
  presentation-agnostic, but no Personal Proxy implementation ships at v1.0.

## 6. Related subsystems

Stream integrity depends on [Security & Trust](./security-trust.md) (FR-SEM-004:
session capability, message signing, sequence numbers, replay prevention). The
developer-facing schema is covered in [Developer Surface](./developer-surface.md).
