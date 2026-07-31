# Developer Handoff — RFC-v0.23-001

**Governing RFC:** [RFC-v0.23-001](../../proposed/RFC-v0.23-001-abdd-live-path.md)
**Milestone:** v0.23
**Status:** inherited from the governing RFC (Proposed — accepted for implementation 2026-07-31)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. What this job actually is

Not "add a proxy service." **Move rendering across a boundary it is currently on
the wrong side of, and remove the old path.**

`fjell-init` today constructs `StateNode`/`EventNode` values and renders them
itself, linking `fjell-proxy-text` as a library — **13 call sites, in init's own
address space.** The `proxy-text` *service* prints one line and exits.

So Fjell's serial output looks like an ABDD demonstration and is an ABDD
violation: emitter and renderer are the same process, which is precisely what
the architecture forbids.

**The job is only done when `init` no longer links `fjell-proxy-text` at all.**
Standing a proxy service up beside init's inline rendering would satisfy every
marker while changing nothing architecturally. That is the failure mode this
handoff most wants to prevent.

## 0.1 Design decisions already made — do not re-open

Settled by the architect so they do not land on you mid-slice:

1. **Emitter is `sample-service`** — already a live IPC participant and the SDK
   reference. The reference service emitting intent demonstrates the authoring
   pattern; a synthetic emitter demonstrates nothing a service author would copy.
2. **Endpoints: `SEMANTIC_STREAM => 7`, `PROXY_TEXT => 8`.** Verified free —
   1–6 are in use (storaged, measuredd, attestd, recoveryd, cap-broker,
   sample-service).
3. **The return leg is proven by refusal**, not by success alone. A capability
   boundary is demonstrated by it saying no.
4. **`proxy-text` holds `ProxyState` loop-local.** `static mut` is forbidden in
   services (BSS-write page faults in `no_std` RISC-V). Measured at ~5.5 KiB
   against a 64 KiB service stack, `no_std`, alloc-free, fixed-capacity — it
   fits. **Checked, not a risk**; do not re-derive it.

---

## 1. Change scope

**In scope:** `crates/services/fjell-semantic-stream/`,
`crates/services/fjell-proxy-text/`, `crates/services/fjell-sample-service/`,
`crates/services/fjell-init/` (including its `Cargo.toml`),
`crates/fjell-kernel/src/task/spawn.rs` (the `ep_obj` match **only**),
`tests/qemu/profiles/`, and the `SERVICES`/profile plumbing needed to run them.

**Explicitly NOT in scope:**

- **No new syscalls.** The 9 declared-but-undispatched ones stay undecided and
  untouched. Gate 12's `syscall-surface` must stay green at 35/26/9 — if your
  change moves those numbers, you have left scope.
- No kernel behaviour change beyond the two `ep_obj` match arms.
- No change to the frozen v1 intent catalog (ADR-v0.5-004).
- No console input. There is no input path at any layer; adding one is kernel
  work and a different direction entirely.
- No wiring of the other 15 non-participating services.
- No richer proxies, no Personal Proxy.

---

## 2. Slice 1 — Bring both services live

1. Add `ep_obj` arms in `spawn.rs`: `SEMANTIC_STREAM => 7`, `PROXY_TEXT => 8`.
2. Have `init` spawn both images (`ImageId::SEMANTIC_STREAM` = 6,
   `ImageId::PROXY_TEXT` = 7) and wait for ready before using them.
3. Give each a service loop on the **`recoveryd` pattern** —
   `send_ready()`, then `loop { recv_call(); match tag { … reply(…) } }`.
   `crates/services/fjell-recoveryd/src/main.rs` is the reference; it is ~35
   lines and it works.

**Ordering matters and is easy to get wrong.** Slice 2 makes init emit rather
than render, so the semantic services must be spawned and ready *before* init
reaches its first emission point. Establish that ordering here, in Slice 1,
while the only observable is "they start and stay alive."

**Evidence:** both appear in the serial log and neither reaches `sys_exit` on
the success path.

## 3. Slice 2 — Forward path, and remove the violation

**This is the slice that matters.** Two halves, both required:

**(a) Stand up the path.** `sample-service` emits an intent node →
`semantic-stream` routes it → the `proxy-text` service renders it to the serial
console.

**Corrected 2026-07-31 — this originally named `renderer::ingest`, which was
wrong.** See `review-record-design-conflict.md` (`.git-exclude/reviewed/`).
There are two non-interoperating semantic subsystems in this codebase:

| | **Subsystem A — use this** | Subsystem B — out of scope |
|---|---|---|
| Types | `fjell-semantic-format`: `StateNode`, `EventNode`, `IntentNode` | `fjell-semantic-v1`: tag-keyed catalog + codec |
| Entry points | `render_state` / `render_event` / `render_intent` (`lib.rs`) | `ProxyState::ingest(bytes, …)` (`renderer.rs`) |
| `semantic-stream` speaks it | **yes** — `publish`, `validate_and_publish`, `dispatch_action` already exist | no |
| Action concept | **yes** — `IntentNode.actions`, and `render_intent` returns `Option<ActionId>` | none at all |

Build on **A**. It is what init's 13 call sites use, what `semantic-stream`
already speaks, and the only one with an action concept for Slice 3. B is a
second unwired subsystem and is not this RFC's problem.

For Slice 3, use `ActionSpec.required_capability: Option<CapabilityRequirement>`
as the declared source of truth for the capability check rather than hardcoding
it — the intent states which capability its action needs.

**Transfer:** these nodes are kilobyte-scale (`TextToken` ≈ 136 B; `StateNode`
carries two plus `FixedVec<StateFact, 16>`) and do not fit a 4-word message. Use
chunked byte transfer on the `storaged` `WRITE_BEGIN`/`WRITE_CHUNK`/
`WRITE_COMMIT` precedent; the types are `Copy` with fixed-size arrays and no
pointers, so raw chunking and reassembly is sound.

**Record the divergence.** `docs/src/external-design/ipc.md:76` says bulk
transfer "uses a shared region granted by capability." That is `DmaShare` (111)
— one of the nine undispatched syscalls — so the documented mechanism is
unavailable. Chunking is correct here, but add a note to `ipc.md` stating the
shared-region path is not currently available. Do **not** leave this as a silent
workaround.

Report the round-trip count per node in the review request. Dozens of messages
per node is acceptable for a demonstration; it should not be mistaken later for
a designed transport.

**(b) Remove the old path.** Convert init's 13 `render_*` call sites into
emissions over IPC, then **delete `fjell-proxy-text` from
`crates/services/fjell-init/Cargo.toml`** and from its imports.

If (b) is not done, (a) is theatre. The acceptance criterion is not "the proxy
renders" but "**init cannot render**."

Expect init's output to change shape — it will now be produced by a different
task. That is the point, and the QEMU profiles that assert on init's current
output may need their expectations updated. Update them honestly; do not
preserve old markers by keeping old behaviour.

**Evidence:** rendered output in the serial log attributable to the `proxy-text`
task; `init`'s `Cargo.toml` no longer lists `fjell-proxy-text`; `cargo tree` or
equivalent confirms the dependency is gone.

## 4. Slice 3 — Return leg

`proxy-text` issues an `ActionRequest` (the type already exists in
`fjell-semantic-format`, alongside `ActionSpec`, `ActionKind`, `ActionResult`).
It is capability-checked, and **the refusal case is demonstrated** — a proxy
lacking the right is refused.

Both cases are required. Success alone does not show a boundary exists.

## 5. Slice 4 — Gate it

New `tests/qemu/profiles/semantic.toml`, fail-closed, with markers for at
minimum:

- both services reaching their loops,
- an intent node rendered by the proxy task,
- an `ActionRequest` accepted under a valid capability,
- an `ActionRequest` **refused** without it.

Wire it into `test-all`. An ungated demonstration rots — this project has ample
evidence of that.

---

## 6. If the path exposes an ABDD design gap

**Plausible.** The renderer's unit tests have never exercised it across a
process boundary, and the boundary may turn out to need something the design did
not anticipate.

**Report it. Do not redesign the boundary inside a slice.** A design gap becomes
its own RFC. Escalate rather than deciding.

## 7. Prohibited shortcuts

- Do not leave `init` linking `fjell-proxy-text`. That is the whole job.
- Do not add a syscall, or change the 35/26/9 syscall surface.
- Do not preserve a marker by preserving the behaviour it was asserting.
- Do not write the demonstration to satisfy its own markers — the emitter is a
  real service and the refusal must be genuine.
- Do not touch the frozen intent catalog.
- Do not mark unexecuted commands as passed.

## 8. Required evidence

1. `cargo xtask build` — result and warning count
2. `cargo xtask test-all --no-qemu` — tier table
3. `cargo xtask test-all` — full tier table, including the new `semantic` profile
4. `cargo xtask release-rehearsal` — twelve-gate table
5. **Gate 12 `syscall-surface` still 35/26/9** — proof you stayed in scope
6. Proof `init` no longer depends on `fjell-proxy-text`
7. Serial-log excerpt showing rendered output attributable to the proxy task
8. Serial-log excerpt showing the `ActionRequest` refusal

## 9. Optional, judgement yours

`init/src/main.rs` contains two NUL bytes in a padded byte literal
(`*b"release-m8-dev\0\0"`), which makes GNU grep treat the file as binary and
silently skip it. It blinded the architect's own analysis of this very RFC. If
removing the literal NULs is clean given the format, do it; if it complicates
the release-id encoding, leave it and say so.

## 10. Review request

Standard format, in `.git-exclude/review-request/` per the owner's direction;
the architect copies it into `rfcs/handoffs/` during review.

Flag for focused review: the init conversion in Slice 2(b), any QEMU profile
expectations you had to change, and anything the boundary surfaced.
