# RFC-v0.5-005: Text Proxy Hardening and Critical-Intent Rendering

**Status.** Implemented (v0.5.0)

## Status

Draft (revised, supersedes pack v0.5-005 draft)

## Target Version

`v0.5.0`.

## Phase

Platform Surface and Semantic Stabilization — Epic E (Text Proxy).

## Related Work

- v0.2 `proxy-text` service.
- v0.5 RFC 004 — semantic catalog v1 (the rendering source).
- v0.4 RFC 005 — DiagnosticBundle redaction (analogous rules).
- v0.6 RFC 003 — semantic schema fuzzing (renderer is one of the fuzz
  targets).

---

## 1. Summary

Harden the `proxy-text` service so that:

- it renders **only catalog-v1** intents;
- it renders unknown intents as `«unknown(0x...)»` rather than dropping them
  silently;
- it has explicit **rate-limiting** per intent class to avoid flooding the
  console under attack or fault loop;
- it surfaces **critical intents** in a distinguishable region with a sticky
  banner;
- it has a deterministic rendering width so terminals at any column width
  produce identical output (post-truncation).

The renderer remains read-only — no input is accepted from the console.

---

## 2. Motivation

`proxy-text` is the operator's window into the system. Two failure modes
identified during v0.4 bring-up:

- a panic loop in a driver flooded the console with `NetDriverFaulted`
  events, hiding everything else;
- an unrecognised intent (an in-development tag not yet in catalog) was
  silently dropped, leaving the operator unaware of an actual problem.

Both are addressed by structural changes (rate-limit + unknown-rendering),
not by editing service code.

---

## 3. Goals

```text
- Render every intent that reaches the proxy.
- Render unknown intents in a parseable "unknown" form.
- Rate-limit per (intent_tag, source_service) at a per-second budget.
- Identify critical intents (a fixed subset) and pin them at the top.
- Width-deterministic output for ≥ 80-column terminals; truncation
  rules documented and tested.
- No console input accepted.
```

## 4. Non-Goals

```text
- No interactive operator commands. proxy-text is one-way.
- No formatted color output (terminal capability detection is out of
  scope; render mono).
- No multi-language. ASCII-only.
- No persistence; proxy-text's state is in-memory only.
```

---

## 5. External Design

### 5.1 Rendering layout

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ Fjell OS — phase: enforcing — boot_id: 0x.....  measurement: sha256:abcd │  fixed banner
├──────────────────────────────────────────────────────────────────────────┤
│ ! 0x0102 UPDATE.STAGING_FAILED  cand=R000058 err=0x0007                  │  critical pin
│ ! 0x0151 RECOVERY.EXITED        outcome=0x01                              │  critical pin
├──────────────────────────────────────────────────────────────────────────┤
│  0x0100 UPDATE.STAGING_STARTED  cand=R000063 channel=stable-- ctr=63     │  scrolling
│  0x0142 NET.SXT_CHANNEL_OPENED  ch=7 kind=UpdateMetadata server=update.. │
│ «unknown(0x0177)» raw_len=24                                              │  unknown-form
└──────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Critical intents (pinned)

```text
0x0102 UPDATE.STAGING_FAILED
0x0110 UPDATE.ROLLBACK_BLOCKED
0x0111 UPDATE.ROLLBACK_TO_PREVIOUS_SLOT
0x0121 ATTEST.RECORD_VERIFY_FAILED
0x0131 SECURITY.PROVIDER_FAULTED
0x0141 NET.LINK_DOWN
0x0150 RECOVERY.ENTERED
0x0151 RECOVERY.EXITED
0x0171 HEALTH.TARGET_FAILED
```

Pinned intents stay visible until either:

- a corresponding "clear" intent arrives (e.g., `RECOVERY.EXITED` clears
  `RECOVERY.ENTERED`), or
- the operator presses a *side-channel* clear (out-of-scope mechanism, NOT
  via console input — typically a small physical button or `fjell-tools
  proxy clear`).

### 5.3 Rate-limit policy

```text
Per (intent_tag, source_service):
  budget: 8 events per second
  burst:  16 events
  on overflow: render once "(... <N> suppressed in 1s ...)" then drop until
  budget refills.
```

---

## 6. Data Model

### 6.1 Renderer state

```rust
pub const MAX_PINNED:      usize = 8;
pub const SCROLL_BUFFER:   usize = 32;

pub struct ProxyState {
    pub banner:        BannerInfo,
    pub pinned:        [Option<RenderedEntry>; MAX_PINNED],
    pub scroll:        ScrollRing<RenderedEntry>,
    pub rate_table:    [RateLimitEntry; RATE_TABLE_SIZE],
    pub width:         u16,
}

pub struct RenderedEntry {
    pub tag:       u16,
    pub at_tick:   u64,
    pub source:    ServiceId,
    pub body:      [u8; 96],     // ASCII, NUL-padded
    pub body_len:  u8,
    pub critical:  bool,
    pub unknown:   bool,
}

pub struct RateLimitEntry {
    pub key:        (u16, ServiceId),
    pub tokens:     u8,
    pub last_tick:  u64,
    pub suppressed: u16,
}
```

### 6.2 Truncation rules

```text
- Field-by-field rendering with explicit max widths per FieldKind.
- AsciiStr64 fields truncate to 12 chars + ".."
- Digest32 fields render first 8 hex chars + "..".
- u64 fields render in decimal, no thousands separators.
- Total entry width capped at terminal width − 2 (for borders).
```

### 6.3 Unknown form

```text
«unknown(0x<tag>)» raw_len=<N>
```

`raw_len` is the encoded byte count (post-header); contents are not
displayed.

---

## 7. Internal Design

### 7.1 Render pipeline

```text
on receive(record):
  if !catalog_v1.knows(record.tag):
      entry = render_unknown(record)
      try_emit(entry)
      return
  schema = catalog_v1.schema(record.tag)
  parsed = decode(record.bytes, schema)?     // RFC v0.5-004 decoder
  entry = format(record.tag, parsed, schema)
  if rate_limit.allow(record.tag, record.source):
      try_emit(entry)
  else:
      rate_limit.suppress(record.tag, record.source)
```

### 7.2 Emit semantics

```text
try_emit:
  if entry.critical:
      pinned.insert(entry)  (LRU eviction within pinned region)
  else:
      scroll.push(entry)    (ring buffer)
  render_frame()
```

### 7.3 Width-determinism

The renderer ignores actual terminal width at runtime; it accepts a
configured `WIDTH` from `configd` (default 80). All entries are produced at
or below WIDTH-2 columns. A separate ADR locks this to 80 for the v1 era.

### 7.4 Banner content

```text
"Fjell OS" || phase || " — boot_id: 0x" + hex(boot_id) ||
"  measurement: " + truncated(chain_digest)
```

Banner refreshes when any of the following intents arrives:

- `SECURITY.REGISTRY_ENFORCING`
- `PLATFORM.PROFILES_READY`
- a measurement-chain advance (subscribed via measuredd).

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-150: Faulty service emits an intent flood that hides critical
              events.
Mitigation:  per-(tag, source) rate-limit; critical region survives
              flooding because pinned entries are persistent.

Threat T-151: A new tag is accidentally introduced and silently dropped.
Mitigation:  unknown-tag rendering makes the issue visible.

Threat T-152: Operator-facing rendering is used for input.
Mitigation:  proxy-text is one-way; no input capability provisioned.
```

### 8.2 Audit emission

```text
ProxyTextSuppressed     { intent_tag, source, suppressed_count }
ProxyTextUnknownTag     { observed_tag, source }
ProxyTextBannerRefresh  { reason_code }
```

---

## 9. Memory / Resource Design

- Pinned region: 8 × 128 B ≈ 1 KiB.
- Scroll ring: 32 × 128 B ≈ 4 KiB.
- Rate table: 64 × 24 B ≈ 1.5 KiB.

Total fixed footprint ≈ 6.5 KiB.

---

## 10. Compatibility and Migration

- proxy-text continues to subscribe to the same semantic stream endpoint.
- New behavior is purely additive from the operator's perspective; existing
  scripts that screen-scrape proxy-text need re-validation against the new
  truncation rules.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- render_known_tag_within_width
- render_unknown_tag_emits_marker
- rate_limit_suppresses_after_budget
- rate_limit_emits_suppression_summary
- pinned_lru_eviction
- pinned_cleared_on_clear_intent
- truncation_digest_8_hex
- truncation_string_12_chars_plus_dotdot
- banner_refreshes_on_enforcing
- frame_renders_within_configured_width
```

### 11.2 Property tests (deferred to v0.6)

```text
- For any sequence of intents, total characters emitted per second is
  bounded by WIDTH * (1 + MAX_PINNED + SCROLL_BUFFER) * frames_per_second.
```

### 11.3 Negative

| Marker                                                  | Profile     |
|---------------------------------------------------------|-------------|
| `NEG:PROXY:FLOOD_DOES_NOT_HIDE_PINNED`                  | proxy-text  |
| `NEG:PROXY:UNKNOWN_TAG_MARKER_EMITTED`                  | proxy-text  |
| `NEG:PROXY:RATE_LIMIT_SUPPRESSION_LOGGED`               | proxy-text  |
| `NEG:PROXY:WIDTH_EXCEEDED_TRUNCATED`                    | proxy-text  |

---

## 12. Acceptance Criteria

```text
- proxy-text renders catalog-v1 with new layout.
- Pinned/critical region works.
- Rate-limit enforced.
- 4 NEG markers green.
- ADR-v0.5-005 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.5-005-proxy-text.md
docs/src/operator/proxy-text-layout.md
docs/src/adr/v0.5-005-proxy-text-no-input.md
docs/src/adr/v0.5-005-proxy-text-fixed-width.md
```

---

## 14. Open Questions

1. **Real terminal width** — operators with wider terminals see padding to
   the right. Acceptable for v1. v0.9 SDK may add a programmatic
   width-setter for tools that paint into proxy-text.
2. **Pinned region clear** — out-of-band clear mechanism is per-platform
   (button on a real board, `fjell-tools` command in QEMU). Tracked in a
   v0.5.x board profile.

---

## 15. Release Gate (RFC-local)

```text
- proxy-text refactor merged.
- All host tests green.
- 4 NEG markers green.
- ADRs Accepted.
```
