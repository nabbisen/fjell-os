# RFC-v0.23-001: ABDD Live Path

**Status:** Proposed — **accepted for implementation by the owner (nabbisen), 2026-07-31**
**Milestone:** v0.23
**Tracks.** Runtime realization of the semantic plane. The project's
distinguishing architectural bet, currently unexercised at runtime.
**Touches.** `crates/services/fjell-semantic-stream/`,
`crates/services/fjell-proxy-text/`, `crates/services/fjell-sample-service/`,
`crates/fjell-kernel/src/task/spawn.rs`, `crates/services/fjell-init/`,
`tests/qemu/profiles/`.
**Relates to:** `docs/src/roadmap/v0.23-direction-options.md` (the measurement
this RFC acts on), original milestones M7/M8, FR-SEM-001…005.

## Summary

Fjell's stated reason for existing is that applications emit **meaning**, not
drawing commands, and a **separate** proxy renders it.

That separation does not exist at runtime. `fjell-init` constructs
`StateNode`/`EventNode` values and renders them **itself**, linking
`fjell-proxy-text` as a library — 13 call sites, all in init's own address space.
Meanwhile `proxy-text` as a *service* prints one line and exits, and
`semantic-stream` does the same.

So the serial output that resembles an ABDD demonstration is an **ABDD
violation**: the emitter and the renderer are the same process, which is exactly
what the architecture forbids.

This RFC moves rendering across the boundary. A service emits an intent node,
`semantic-stream` routes it, the `proxy-text` **service** renders it, and the
proxy's return leg issues a capability-checked `ActionRequest`. `init` stops
rendering. It adds no kernel surface and no syscalls.

## Motivation

### What already exists

Measured at `0.22.0`:

| Component | State |
|---|---|
| `proxy-text` renderer | **845 lines**. Note (2026-07-31): these split across *two* subsystems — `lib.rs`'s `render_*` (used by init, **the target**) and `renderer.rs`'s `ingest()` (a separate catalog-codec path, unwired, out of scope). See the design-conflict review record. |
| Round-trip data model | `ActionId`, `ActionSpec`, `ActionKind`, `ActionRequest`, `ActionResult` in `fjell-semantic-format` |
| Intent catalog | frozen v1 (ADR-v0.5-004), auto-published |
| Service binaries | both built and embedded in the kernel image |
| `ImageId` constants | `SEMANTIC_STREAM = 6`, `PROXY_TEXT = 7` |
| Debug-name entries | `8 => "sem-stream"`, `9 => "proxy-text"` |

### What is missing

| Gap | Evidence |
|---|---|
| Endpoint assignment | both fall through to `_ => 0` in `spawn.rs`'s `ep_obj` match |
| `init` spawns neither | no spawn call for either image |
| `semantic-stream` service loop | `main.rs` ends in `sys_exit(0)` |
| `proxy-text` service loop | `main.rs` is 10 lines, ends in `sys_exit(0)` |
| An emitter over IPC | nothing emits an intent node *across a process boundary* |
| Separation | `init` links `fjell-proxy-text` and renders inline — 13 call sites |

Nine release lines built the infrastructure around this and never closed it.
The measured gap is roughly **200–300 lines**, including converting init's 13
render call sites into emissions.

### A note on how this was nearly missed

The options paper originally recorded that the renderer was "845 lines that
nothing calls." That was wrong, and the recommendation was accepted while it
stood. `init/src/main.rs` contains two NUL bytes inside a legitimate NUL-padded
byte literal (`*b"release-m8-dev\0\0"`), so GNU grep classifies the file as
binary and **silently suppresses every match** — reporting a clean result that
looked like evidence of absence.

Recorded because it is the same failure class this project keeps finding in its
gates: a tool reporting nothing while not looking. The `.rs` file is valid Rust
and the Rust-based gate tooling (`fs::read_to_string`) is unaffected; only
grep-based inspection is blinded.

### Why now

The owner directed (2026-07-31) that functional advancement must precede v1.0
consideration, and that the current state is far from production readiness or
demonstrable appeal. Of the four candidate directions measured in the options
paper, this is the smallest by an order of magnitude, depends on nothing, and is
the only one producing a claim the project cannot currently make.

## Goals

1. Both semantic services run as live IPC participants rather than exiting.
2. An intent node emitted by a real service is rendered by the proxy, visible in
   the serial log.
3. The proxy's return leg issues an `ActionRequest` that the kernel
   capability-checks — and **refuses** when the right is absent.
4. The whole path is **gated**, not merely observed once.

Goal 4 is not decoration. An ungated demonstration rots; this project has ample
evidence of that.

## Non-goals

- **No console input.** There is no input path at any layer, and adding one is
  kernel work (options paper §4). A human typing at the proxy is out of scope.
- **No new syscalls and no kernel behaviour change.** The 9 declared-but-
  undispatched syscalls remain undecided and untouched.
- No richer proxies (audio, braille) and no Personal Proxy.
- No wiring of the other 15 non-participating services — that is Direction B.
- No change to the frozen v1 intent catalog.

## Design decisions settled here

**The emitter is `sample-service`.** It is already a live IPC participant and it
is the SDK reference service. Having the reference service emit intent
demonstrates the authoring pattern end-to-end rather than inventing a synthetic
emitter that no service author would copy. This also strengthens the developer
surface at no extra cost.

**The return leg is proven by refusal, not by success alone.** The convincing
evidence that a capability boundary exists is that it says no. The negative case
— a proxy lacking the action right is refused — is a required marker, not an
optional extra.

**`proxy-text` holds its state loop-local.** `static mut` is forbidden in
services (BSS-write page faults in `no_std` RISC-V). Measured: `ProxyState` is
~5.5 KiB (`ScrollRing` 32 × ~112 B, 8 pinned, 64 rate-table entries) against a
64 KiB service stack, `no_std` and alloc-free. It fits comfortably as a
loop-local. **Checked, not a risk** — recorded so nobody re-derives it.

## Scope

| Slice | Content | Evidence |
|---|---|---|
| 1 | Bring both services live: `ep_obj` arms, `init` spawn, service loops on the `recoveryd` pattern | Both appear in the serial log and do **not** exit |
| 2 | Forward path: `sample-service` emits an intent node → `semantic-stream` routes → `proxy-text` renders. **`init` stops linking `fjell-proxy-text`**; its 13 render call sites become emissions | Rendered output visible in the serial log, produced by the proxy **task**, not by init |
| 3 | Return leg: proxy issues `ActionRequest`; capability-checked; refused without the right | Both accept and refuse observed |
| 4 | Gate it: a `semantic` QEMU profile with fail-closed markers | Profile passes in `test-all`; absent marker fails the run |

Sequencing is strict — each slice depends on the previous.

## Testing and verification requirements

A new `tests/qemu/profiles/semantic.toml` with fail-closed markers covering, at
minimum:

- both services reaching their loops,
- an intent node rendered,
- an `ActionRequest` accepted under a valid capability,
- an `ActionRequest` **refused** without it.

Fail-closed semantics are inherited from the existing negative harness: an
absent expected marker or a present forbidden marker fails the run.

Plus the standing release-cycle exit criteria (RFC-v0.21.3-002) and the
twelve-gate table.

## Acceptance criteria

- [ ] `semantic-stream` and `proxy-text` run as live IPC participants; neither
      calls `sys_exit` on the success path.
- [ ] An intent node emitted by `sample-service` is rendered by `proxy-text`,
      observable in the serial log.
- [ ] An `ActionRequest` from the proxy is capability-checked; the refusal case
      is demonstrated.
- [ ] `tests/qemu/profiles/semantic.toml` exists, is fail-closed, and passes.
- [ ] `cargo xtask test-all` includes the new profile; all tiers pass.
- [ ] Twelve mechanical gates pass; Gate 12 stays green (the syscall surface is
      unchanged — 35 declared, 26 dispatched — and this RFC must not alter it).
- [ ] **`init` no longer depends on `fjell-proxy-text`** — removed from its
      `Cargo.toml` and its imports. Without this the demonstration coexists with
      the violation it exists to remove, and the separation is cosmetic.
- [ ] Rendered output in the serial log is attributable to the `proxy-text`
      task, not to `init`.
- [ ] Consider removing the literal NUL bytes from `init`'s release-id byte
      literal so grep-based inspection stops silently skipping the file. Small,
      in-scope since this RFC edits init anyway; not required if it complicates
      the format.
- [ ] No new syscall; no kernel behaviour change.
- [ ] Release record for `0.23.0` per RFC-v0.21.3-002.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Running the path exposes ABDD boundary design gaps the renderer's unit tests never exercised | **Medium** | Medium | Expected and useful. **Report findings; do not redesign the boundary inside a slice.** A design gap becomes its own RFC. |
| R2 | Scope creep into Direction B — "while we're here, wire these too" | Medium | Medium | Explicit non-goal. The other 15 services are out of scope regardless of how cheap they look mid-slice. |
| R3 | The demonstration is written to pass its own markers rather than to be real | **Medium** | **High** | Sharpened by the corrected finding: it is now possible to "demonstrate" ABDD by standing a proxy service up beside init's inline rendering and leaving both. The acceptance criteria therefore require init to *stop* rendering, not merely require the proxy to *start*. |
| R4 | Endpoint-number collision with the existing assignments (1–6 in use) | Low | Low | Assign 7 and 8; verify against `spawn.rs` before use. |

## Alternatives considered

| Option | Assessment |
|---|---|
| **Wire the two services and gate the path** *(chosen)* | Smallest measured path to the project's headline claim; uses infrastructure already built. |
| Forward path only, defer the return leg | Cheaper, but "the proxy can return an operation request, capability-checked" is in the original M7/M8 criteria. Stopping at output-only would leave the claim half-made and the ActionRequest model still unexercised. |
| Build a richer proxy first | The text proxy already exists and works. Building a second renderer before the first one has ever run inverts the order. |
| Wire all 17 non-participating services | Direction B. Higher effort, more diffuse claim, and it does not make the ABDD path arrive sooner. |

## Open question

Not blocking: whether `semantic-stream` should fan out to multiple proxies now
or stay single-consumer until a second proxy exists. Recommend single-consumer —
fan-out without a second consumer is speculative generality, and the boundary is
designed to allow it later.
