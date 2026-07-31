# Review Record — RFC-v0.23-001 design conflict, dispositioned

**Reviewer:** architect
**Reviewing:** `review-request-rfc-v0.23-001-design-conflict.md`
**Date:** 2026-07-31

## Outcome

**The escalation is correct on every point. The handoff was wrong, and it was my
error.** Their recommendation is adopted.

Three rulings in §3. **Slice 1 may proceed immediately** — the protocol question
is now settled, so nothing is blocked.

Stopping before writing code, rather than picking a subsystem and discovering
the conflict halfway through, was exactly right. So was declining to make the
catalog call unilaterally — it would have breached a stated non-goal either way.

## 1. Verified independently

Every claim re-derived against the tree, not accepted from the request.

| Claim | Result |
|---|---|
| Two non-interoperating subsystems exist | **Confirmed.** `render_state(&StateNode)`, `render_event(&EventNode)`, `render_intent(&IntentNode)` in `lib.rs` vs `ProxyState::ingest(bytes, source, tick)` in `renderer.rs` |
| `semantic-stream` already speaks Subsystem A | **Confirmed.** `publish(SemanticEnvelope)`, `validate_and_publish(SemanticEnvelope)`, `dispatch_action(&ActionRequest) -> ActionResult` already exist |
| The action concept lives only in A | **Confirmed.** `IntentNode.actions: FixedVec<ActionSpec, MAX_ACTIONS>` |
| Catalog ownership is enforced, SDK range empty | **Confirmed.** 20 entries, **all** in the `0x0100` block; `0x0300–0x03FF` has **zero** |
| Nodes do not fit a 4-word message | **Confirmed.** `TextToken` ≈ 136 B (`MAX_TEXT_BYTES` 128); `StateNode` carries two plus `FixedVec<StateFact, 16>` — kilobytes, against 32 bytes inline |

**One point they understated.** `render_intent` returns `Option<ActionId>`, and
`dispatch_action(&ActionRequest) -> ActionResult` already exists. Slice 3's
mechanism is not merely *typed* in Subsystem A — it is **written**, and only
unwired. Subsystem B has no action concept at all, so building Slice 2 on B
would have left Slice 3 with nothing to build on.

Also relevant to Slice 3: `ActionSpec.required_capability:
Option<CapabilityRequirement>`. The intent itself declares which capability its
action needs, so the return leg's check has a declared source of truth rather
than a hardcoded one. Use it.

## 2. How the handoff got this wrong

I named `renderer::ingest` because it takes `&[u8]` and was the only
IPC-shaped entry point I found. I did not register that it belongs to a
structurally separate wire format — `fjell-semantic-v1`'s tag-keyed catalog
codec, built for the v0.5-005 line — with no relationship to the 13 `init` call
sites the RFC's own Motivation section names as the violation.

The result would have been absurd on inspection: fixing init's in-process use of
Subsystem A by standing up Subsystem B over IPC, leaving those 13 call sites
with nowhere to go.

This is the second measurement error in this RFC's short life, both mine. The
first was the NUL-byte grep blindness. This one had no tooling excuse — I read
`renderer.rs` and `lib.rs` in the same session and did not notice they served
different type systems.

## 3. Rulings

### 3.1 Subsystem A — confirmed

Build Slices 2 and 3 on `fjell-semantic-format` (`StateNode` / `EventNode` /
`IntentNode`) with `render_state` / `render_event` / `render_intent`.

`renderer::ingest` and the v1 catalog codec are **out of scope for this RFC**.
Subsystem B is a second "written but unwired" subsystem; that is a real finding,
recorded below, but it is not this line's job.

The RFC's non-goal *"no change to the frozen v1 intent catalog"* is preserved
untouched — Subsystem A does not use the catalog. **No exception is granted or
needed.** Do not add an SDK-range entry.

### 3.2 Chunked transfer — approved, and a divergence to record

Chunked byte transfer on the `storaged` `WRITE_BEGIN`/`WRITE_CHUNK`/
`WRITE_COMMIT` precedent is approved. The types are `Copy` with fixed-size
arrays and no pointers, so raw-byte chunking plus reassembly is sound.

**But record this, because it matters more than the mechanism.** The *documented*
answer to this exact problem is not chunking. `docs/src/external-design/ipc.md:76`
states: *"bulk transfer uses a shared region granted by capability rather than
large in-band messages."*

That mechanism is `DmaShare` (111) — **one of the nine declared-but-undispatched
syscalls.** So the documented design for bulk transfer is unavailable, because
it rests on a syscall the kernel does not dispatch.

Chunking is therefore correct *here*, as the only route that respects this RFC's
no-new-syscalls non-goal. It is also a **documented divergence**, not a silent
workaround, and must be written up as such — including a note in `ipc.md` that
the shared-region path is not currently available.

This is the first concrete cost of deferring the nine-syscall disposition, and
it raises that item's priority. **Carried to v0.23 candidates**; not reopened
here.

Note the chunk count honestly in the review request: kilobyte-scale nodes at 32
bytes per message means dozens of round trips per node. Acceptable for a
demonstration; worth stating so nobody later mistakes it for a designed
transport.

### 3.3 Slice 1 — proceed now

Unblocked. The protocol is settled, so the service loops can be written once
against Subsystem A. Waiting rather than writing them twice was the right call.

## 4. New finding — Subsystem B is a second unwired subsystem

`renderer::ingest`, `ProxyState`, the scroll ring, pinned criticals and rate
limiting — roughly 540 lines in `renderer.rs` — are called by **nothing but
their own unit tests**, and belong to a wire format nothing else speaks.

So the options paper's original claim ("845 lines that nothing calls") was not
wholly wrong; it was **attached to the wrong half**. `lib.rs`'s `render_*`
functions are heavily used by init. `renderer.rs`'s `ingest` path genuinely is
dead.

That is a real question for a later line: is Subsystem B a planned successor
that was never finished, or abandoned work that should be retired? Not this
RFC's to answer. **Carried to v0.23 candidates.**

## 5. Corrections applied

- Handoff §3 rewritten: `renderer::ingest` → Subsystem A entry points, with the
  chunked-transfer requirement stated.
- RFC and options paper annotated where `ingest()` is named, so neither keeps
  pointing at the wrong subsystem.

## 6. Required next deliverables

Slices 1–4 as scoped, plus in the review request:

1. The chunk protocol as implemented, and the round-trip count per node.
2. Confirmation that the frozen catalog was not touched.
3. The `ipc.md` note recording the shared-region divergence.
4. Anything else the boundary surfaced — same standing instruction: report, do
   not redesign inside a slice.
