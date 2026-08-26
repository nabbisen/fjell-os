# RFC-0.26-002: The ABDD path must synchronise, not assume

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.26 — **blocks every release** (Gate 7 is red until this lands)
**Tracks.** Service startup synchronisation on the ABDD live path.
**Touches.** `crates/services/fjell-sample-service`. No kernel, ABI, capability,
lease, IPC, or crypto change.
**Relates to:** ERRATA **E-020** (this RFC closes it), RFC-v0.23-001 (which
shipped the path and its guard), RFC-0.26-001 (which removed the accident the
path was relying on), **E-019** / **RFC-0.26-003** (a related assumption with no available signal — separate line).

## Summary

**The ABDD live path does not run.** Measured on the current tree: zero
occurrences of `sample-service demo intent` or `proxy-text: action` in
`tests/qemu/artifacts/semantic/serial.log`.

`crates/services/fjell-sample-service` calls `emit_sample_intent()` once from
`service_main()`, under this comment:

> *"semantic-stream and proxy-text are already spawned and ready by this point
> (Slice 1)."*

That is an **assertion about scheduling order, not a synchronisation.** It was
silently true only because of the priority asymmetry RFC-0.26-001 correctly
removed.

This RFC makes the path establish what it currently assumes.

## Motivation

### What is actually broken

RFC-v0.23-001 shipped this project's distinguishing architectural bet in
`0.23.0` — a service emits meaning, a *separate* task renders it, and the return
leg is capability-checked with the refusal demonstrated. It created
`tests/qemu/profiles/semantic.toml` **in the same RFC**, as a fail-closed guard,
so the path could not rot.

The guard is now permanently red. It is reporting exactly what it was built to
report, and because it always fails it can no longer detect anything else.

### The signal already exists; only the wait is missing

Both peers already announce themselves:

- `crates/services/fjell-semantic-stream/src/main.rs` — `send_ready()`, emitting
  `proto::READY`
- `crates/services/fjell-proxy-text/src/main.rs` — `send_ready()`, same shape,
  called at line 112

And `sample-service` already participates in the readiness protocol in the other
direction: it sends `SERVICE_READY` to service-manager as its own first act.

**So this is not a missing protocol. It is a missing use of one.** The service
announces its own readiness and then assumes everyone else's.

### Why this is OPEN and not an accepted limitation

`ACCEPTED` in this project's errata register means a documented, deliberate
limitation. This is live drift in a released capability — the register's own
definition of `OPEN`.

The consequence is deliberate: **Gate 7 (`ERRATA register (0 OPEN)`) fails, so
`release-rehearsal` is red and no release can be cut until this lands.** A
milestone that stopped a shipped feature from running should not be able to ship
while that fact sits on the books as an accepted limitation.

## Design decisions

### D1 — Establish readiness. Do not re-time the assumption.

The fix is that `sample-service` **waits until its peers are ready** before
emitting.

**Explicitly forbidden:** yielding a fixed number of times, retrying N times,
sleeping, or emitting speculatively and hoping. Every one of those re-encodes
the same assumption with a larger constant and will break again at the next
scheduling change — which is exactly how this defect was created.

If the emission cannot be made to wait, **stop and escalate.** Do not
approximate it.

### D2 — Failing to synchronise must be loud

If the peers never become ready, `sample-service` must **say so** — not emit
into the void and not hang silently. A visible refusal is worth more than a
silent success, and this project has now twice been bitten by a service
proceeding on an assumption nobody could see failing.

### D3 — E-019 is not in scope, and it is not the same fix

`fjell-neg-test`'s `ipc` assumption looks like the same defect and is **not**
fixed here, for a reason worth stating precisely:

| | Waits for | Signal available? |
|---|---|---|
| **E-020** (this RFC) | peers to be **ready** | **Yes** — both peers already call `send_ready()` |
| **E-019** | a peer to be **blocked in `sys_ipc_recv`** | **No** — and none exists to build |

Readiness can be announced *before* entering it. "I am blocked" cannot: a task
cannot atomically send that message and then block, so any announcement is
itself racy. E-019 needs a design answer — possibly restructuring the test
rather than synchronising it — and that is **RFC-0.26-003**.

Bundling them would put a fix with an available signal behind one without.

### D4 — The mechanism is the implementer's

This RFC states the property, not the implementation, on the same reasoning as
RFC-0.24-002 Slice 5 and RFC-0.24-003 R5: a stated property survives a better
idea, and a specified mechanism forecloses one.

## Scope

| # | Requirement |
|---|---|
| **R1** | `sample-service` establishes that `semantic-stream` and `proxy-text` are ready before `emit_sample_intent()` — by waiting, not by timing (D1) |
| **R2** | Failure to synchronise is visible in the serial log (D2) |
| **R3** | The stale comment asserting peer readiness is removed or corrected — it must not outlive the assumption |
| **R4** | `semantic` profile green; **E-020 → `CLOSED`**, with `ERRATA.md` and `v1-limitations.md` edited in the same commit |

### Non-goals

- **E-019 / the `ipc` profile** (D3).
- Any change to `semantic-stream` or `proxy-text` beyond what R1 requires of
  their existing `send_ready()`.
- Any kernel, scheduler, ABI, capability, lease, IPC, or crypto change.
  **RFC-0.26-001's fix is correct and is not to be softened to make this
  easier.**
- Gate 12 `syscall-surface` must stay **35/29/6**.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | The fix is a disguised timing assumption — a yield loop with a bigger bound | **High** | **High** | D1 forbids it explicitly. The review will ask what the service *waits on*, and "long enough" is not an answer |
| R2 | It passes by luck of ordering rather than by synchronising | **High** | High | **The evidence must show the wait executed** — see acceptance criteria. Green markers alone do not distinguish the two |
| R3 | Softening RFC-0.26-001 to make the path work again | Low | **Critical** | Explicit non-goal. The priority fix is correct; the assertion was the defect |
| R4 | Scope creep into E-019, or into a general service-readiness framework | Medium | Medium | D3. One service, one path |
| R5 | Not host-testable — E-013 | **Certain** | Medium | QEMU evidence, cited by log, as in RFC-0.25-001 and RFC-0.26-001 |

## Acceptance criteria

- [ ] `cargo xtask qemu-run --profile semantic` **PASS**, with all four markers.
- [ ] **Evidence that the wait actually executed** — a log line, ordering, or
      equivalent showing `sample-service` synchronised rather than happened to
      be scheduled late enough. *Green markers alone do not satisfy this.*
- [ ] The failure path demonstrated or reasoned explicitly: what appears in the
      log if a peer never becomes ready (D2).
- [ ] The stale comment gone or corrected (R3).
- [ ] **E-020 `CLOSED`**, `ERRATA.md` and `v1-limitations.md` in the same commit.
- [ ] Gate 7 back to `0 OPEN errata`; `release-rehearsal` green.
- [ ] `cargo xtask test-all` — 20/21, with **`ipc` still failing under E-019 /
      RFC-0.26-003** and that stated, not quietly absorbed.
- [ ] `cargo fmt --all --check` clean — run, not predicted.

## What this is really about

A service announced its own readiness and assumed everyone else's. That worked
for two releases because an unrelated scheduler bug happened to make it true.

The bug is fixed. The assumption is what remains, and it is the second time in
two lines that code asserting an ordering rather than establishing one has cost
this project a working feature.
