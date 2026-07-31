# Review Request — RFC-v0.23-001: design conflict found before Slice 2/3 implementation

**Governing RFC:** [rfcs/proposed/RFC-v0.23-001-abdd-live-path.md](../../rfcs/proposed/RFC-v0.23-001-abdd-live-path.md)
**Handoff:** [rfcs/handoffs/RFC-v0.23-001-abdd-live-path/implementation-handoff.md](../../rfcs/handoffs/RFC-v0.23-001-abdd-live-path/implementation-handoff.md)
**Submitted by:** implementation model
**Status:** **No code written yet.** Investigation only. Stopping per handoff §0
("If you find a design conflict, stop and escalate — do not resolve it in
code") and handoff §6 ("Report it. Do not redesign the boundary inside a
slice.").

Branch: `docs/v0.21.3-rfc-and-design-baseline` (unchanged so far)

---

## 1. What I was doing

Investigating Slice 1 (bring `semantic-stream`/`proxy-text` live) and Slice 2
(forward path) together, since the handoff's design decisions for both are
interdependent. Slice 1's mechanics (`ep_obj` arms, spawn ordering, service
loops) are straightforward and unaffected by what follows — I have not
touched them yet, pending direction on the issue below, since Slice 2's
wire-protocol choice determines what `semantic-stream`'s and `proxy-text`'s
service loops actually need to speak, and rewriting them twice seemed worse
than asking first.

## 2. The finding

The handoff (§3, Slice 2) says: *"`sample-service` emits an intent node →
`semantic-stream` routes it → `proxy-text` receives it and calls
`renderer::ingest`."*

**This doesn't fit together, for two independent reasons, once you look at
what `renderer::ingest` and the frozen catalog actually are.**

### 2a. `renderer::ingest` belongs to a disconnected subsystem

There are **two separate, non-interoperating semantic mechanisms** already
in this codebase, both live in the `fjell-proxy-text` crate:

| | Subsystem A (what init calls today) | Subsystem B (what the handoff points at) |
|---|---|---|
| Types | `fjell-semantic-format`: `StateNode`, `EventNode`, `IntentNode` (with `ActionSpec`/`ActionKind`) | `fjell-semantic-v1`: a frozen, tag-keyed catalog + byte codec |
| Entry point | `fjell_proxy_text::{render_state, render_event, render_intent, ...}` — `lib.rs`, called **13 times** by `init` today (exactly the violation the RFC describes) | `ProxyState::ingest(bytes, source, tick)` — `renderer.rs`, currently called by **nothing** except its own unit tests |
| Used by `semantic-stream` today | Yes — `publish()`/`validate_and_publish()`/`dispatch_action()` in `crates/services/fjell-semantic-stream/src/main.rs` operate on exactly these types | No |
| Has an action/capability concept | Yes — `IntentNode.actions: FixedVec<ActionSpec, 8>`, feeding directly into Slice 3's `ActionRequest`/`ActionResult` | No — no action concept exists in the v1 catalog schema at all |

Subsystem A is what `fjell-init`'s 13 render call sites, and
`semantic-stream`'s existing (dead-code-but-present) `publish`/
`dispatch_action` functions, already operate on. Subsystem B
(`renderer::ingest`) was built for a different RFC line (v0.5-005) and has
never been wired to anything outside its own tests — it is a second,
parallel "845 lines that nothing calls," not the boundary the RFC's own
Motivation section describes `init` as violating.

Building Slice 2 on B, as the handoff's prose literally says, would mean:
`init`'s violation (calling A in-process) gets fixed by standing up a
*different* subsystem (B) over IPC — the specific 13 call sites the RFC
names still have to move to *something*, and B has no type that maps to
`StateNode`/`EventNode`/`IntentNode` at all. I don't think this is what was
intended; I think it's an oversight — `ingest()` was probably the only
IPC-shaped entry point found when the handoff was written, without
registering that it belongs to a structurally separate wire format from the
one `init` and `semantic-stream` already share.

### 2b. Independently: the frozen catalog has hard ownership, and `sample-service` owns none of it

Even setting aside 2a, using the v1 catalog (`fjell-semantic-v1`) for
`sample-service`'s emission runs into a second, independent wall:
`crates/fjell-semantic-v1/src/catalog.rs` enforces per-range ownership
(`CATALOG_RANGES`, RFC-v0.7.5-001 / W-M-02), not just documentation:

```rust
pub const CATALOG_RANGES: &[CatalogRangeOwner] = &[
    CatalogRangeOwner::new(0x0100, 0x011F, "UPDATE",   "fjell-upgraded", None),
    CatalogRangeOwner::new(0x0120, 0x012F, "ATTEST",   "fjell-attestd",  None),
    CatalogRangeOwner::new(0x0130, 0x013F, "SECURITY", "fjell-kernel",   None),
    CatalogRangeOwner::new(0x0140, 0x014F, "NET",      "fjell-netd",     None),
    CatalogRangeOwner::new(0x0150, 0x015F, "RECOVERY", "fjell-recoveryd",None),
    CatalogRangeOwner::new(0x0160, 0x016F, "PLATFORM", "fjell-devmgr",   None),
    CatalogRangeOwner::new(0x0170, 0x017F, "HEALTH",   "fjell-measuredd",None),
    CatalogRangeOwner::new(0x0180, 0x01FF, "SUMMARY",  "fjell-syncd",    None),
    CatalogRangeOwner::new(0x0200, 0x02FF, "FLEET",    "fjell-syncd",    Some("v0.8")),
    CatalogRangeOwner::new(0x0300, 0x03FF, "SDK",      "fjell-sdk",      Some("v0.9")),
    ...
];
```

Every populated entry in `CATALOG_V1` is owned by one specific crate.
`sample-service` owns none of them. The one range that exists *for exactly
this purpose* — `SDK` (0x0300–0x03FF), owned by `fjell-sdk` — has **zero
entries populated**. So using the v1 catalog for this RFC's emitter means
one of:

- have `sample-service` emit another crate's tag (e.g. `HEALTH.TARGET_REACHED`,
  owned by `fjell-measuredd`) — violates the ownership design the catalog
  itself enforces via `CatalogRangeOwner`/`range_owner_for`, or
- add a new `SDK`-range entry to `CATALOG_V1` — directly contradicts the
  RFC's explicit non-goal: *"No change to the frozen v1 intent catalog
  (ADR-v0.5-004)."*

Neither is a call I should make unilaterally.

## 3. My recommendation

**Build Slice 2/3 on Subsystem A** (`fjell-semantic-format` +
`render_state`/`render_event`/`render_intent`), not B:

- It's the literal subject of the RFC's own Motivation section (the 13
  call sites `init` makes today).
- `semantic-stream`'s existing `publish`/`validate_and_publish`/
  `dispatch_action` functions already operate on these exact types —
  no new subsystem needs to be stood up, only moved across a process
  boundary.
- `IntentNode.actions: FixedVec<ActionSpec, MAX_ACTIONS>` gives Slice 3's
  `ActionRequest`/`ActionResult` return leg a natural home, so Slices 2 and
  3 become one coherent flow instead of two unrelated protocols bolted
  together.
- Touches nothing in the frozen v1 catalog — fully respects that non-goal.

**The cost:** `StateNode`/`EventNode`/`IntentNode` are multi-KB (dominated
by `FixedVec<_, N>` of `TextToken`-bearing fields — e.g. `IntentNode` is
several KiB), so they don't fit in a 4-word IPC message. I'd need a chunked
byte-copy transfer — the same shape as `storaged`'s existing
`WRITE_BEGIN`/`WRITE_CHUNK`/`WRITE_COMMIT` protocol (already precedented in
this codebase for 512-byte sectors), just longer, since every type
involved is `Copy` with no pointers (fixed-size arrays only, verified by
reading `fjell-semantic-format/src/lib.rs` in full), so a raw-byte
chunked copy + reassembly is sound and needs no new serialization logic.

This is more new IPC code than the v1-codec path would have been (which
already has a compact `encode`/`decode`), but it avoids two conflicts with
explicit RFC non-goals instead of hitting one of them.

## 4. What I need from you

1. **Confirm Subsystem A is the right target**, or direct me otherwise —
   including, if you'd rather, an explicit exception to add one `SDK`-range
   catalog entry (I'd want that stated explicitly rather than assumed,
   since the RFC lists catalog changes as a non-goal).
2. Let me know whether to proceed with **Slice 1 now** (unaffected by this
   question — `ep_obj` arms, spawn ordering, and the `recoveryd`-pattern
   service loop skeletons don't depend on which forward-path subsystem is
   chosen) while this is dispositioned, or hold everything until this is
   resolved.

I have not modified any files. Task tracking (5 tasks: Slices 1–4 plus
final evidence/release) is in place locally but nothing is marked complete.
