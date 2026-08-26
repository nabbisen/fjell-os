# RFC-0.26-004: Readiness needs a channel of its own

**Status:** Accepted — by the owner (nabbisen), 2026-08-27; implementation may begin (RFC 000)
**Milestone:** 0.26 — **blocks every release** (Gate 7 red on E-020 until this lands)
**Tracks.** How a service learns another service is ready, and who may receive
on a service's endpoint.
**Touches.** `crates/services/fjell-init`, `crates/services/fjell-sample-service`,
and — depending on the answer to the open question — `fjell-semantic-stream`,
`fjell-proxy-text`, `fjell-service-manager`, or `crates/fjell-kernel/src/task/spawn.rs`
(capability installation). **Deliberately wider than its predecessor's, which is
why that one could not be completed.**
**Relates to:** **supersedes RFC-0.26-002**; closes **E-020** (OPEN); expected to
close **E-021**; RFC 058 (the readiness protocol), RFC-0.26-001 (which exposed
this), **E-019 / RFC-0.26-003** (the same family, separate line).

## Summary

RFC-0.26-002 asked `sample-service` to wait for its peers using a signal it
claimed already existed. **That premise was false**, and the implementer stopped
rather than approximating it — which is what its D1 required.

`semantic-stream` and `proxy-text` each announce readiness by sending
`proto::READY` **into their own endpoint** — object 7 and object 8, slot 0.
Those are the same objects that carry ordinary protocol traffic, and `init`
holds *receive-capable* capabilities to both (slots 6 and 7) so it can wait for
those announcements.

**So two tasks receive on one queue, with nothing arbitrating between them, and
readiness shares that queue with real work.**

`sample-service` cannot observe readiness there without probing an inbox whose
messages `init` may consume instead — and `init`'s `wait_ready_exact` discards
what it does not recognise **without replying**, blocking the caller forever
(**E-021**, observed live: `PUBLISH_BEGIN` swallowed).

**Readiness needs a channel that is not also the work channel.** That is this
RFC.

## Motivation

### What is actually wrong

| | Today |
|---|---|
| Who announces | the service, into **its own** endpoint |
| Who listens | `init`, on **the same** endpoint |
| What else uses that endpoint | every protocol message the service exists to serve |
| Who arbitrates | **nothing** — whoever dequeues first takes the next message |
| What happens to a mismatched blocking call | consumed, dropped, **never replied to** |

`sys_ipc_send` returns `SendResult::Queued` when no receiver waits, so nothing
is lost to a drop — the race is over **who dequeues first**, and it is
unarbitrated in both directions. If `semantic-stream` reaches its own `recv`
before `init` does, it consumes its own handshake and `init` waits forever.

### Why this is the third instance of one pattern

E-019, E-020 and this are the same defect at three depths:

- **E-019** — a test assumes a peer has blocked.
- **E-020** — a service assumes its peers are ready.
- **This** — the mechanism those were meant to be fixed *with* assumes exclusive
  use of a shared channel.

Fixing E-020 on top of this would be building a synchronisation on an
unsynchronised primitive.

### The architect's error, recorded

RFC-0.26-002 stated *"the protocol is not missing — its use is."* I confirmed
`send_ready()` existed and inferred it was usable, without checking who could
observe it. Confirming a thing exists and inferring it is fit for purpose is the
failure mode this project has spent four milestones naming; this instance is in
an RFC premise rather than a gate predicate.

## The open question — the deliverable

**Where does readiness go, and who may receive on a service's endpoint?**

Shapes, none pre-selected:

1. **A dedicated readiness endpoint per service**, receive-capable only to
   `init` (or service-manager). Work traffic and handshakes stop sharing a
   queue. Costs an object and a capability slot per service.
2. **Route readiness through `service-manager`**, which already tracks it for
   services using the shared endpoint. `SM_STATUS_QUERY` / `SM_STATUS_REPLY`
   are **declared in `fjell-service-api::tags` with no handler anywhere** —
   building one is a real protocol addition, and it makes readiness a question
   any service can ask rather than a message it must catch.
3. **Remove `init` as a receiver** — have it learn readiness some other way
   entirely, so a service's endpoint has exactly one receiver, which is the
   invariant whose absence causes this.

**Whichever is chosen, state the invariant it establishes**, in the form *"a
service's endpoint has exactly one receiver"* or an explicit, justified
exception.

## Design decisions

### D1 — Decide before implementing

Same rule as RFC-0.26-001 and RFC-0.26-003. **The written answer is the
deliverable**; the code follows it.

### D2 — No re-timing, again

Reordering `init`'s M4/M5 spawns would very likely make the `semantic` profile
pass today. It is forbidden, for the third RFC running: it re-encodes an
ordering assumption to paper over one.

### D3 — E-021 is expected to close here, but is not the goal

`wait_ready_exact`'s missing `else` is a real defect independently. Patching it
alone is **not** an acceptable outcome for this RFC — it stops the lost reply
without making the channel arrangement correct.

If the chosen design leaves `init` receiving on any endpoint another task can
call into, **the `else` must still be fixed and the residual hazard stated.**

### D4 — Scope is wider on purpose

Its predecessor's `Touches` line named one file and that is why it could not be
completed. This one names the files the answer may need. **That is not licence
to wander:** touching a file not needed by the chosen design is still scope
creep, and the review will ask which decision required it.

## Scope

| # | Requirement |
|---|---|
| **R1** | Answer the open question in writing, committed — which shape, why the others were rejected, and the invariant it establishes |
| **R2** | Implement it |
| **R3** | `sample-service` waits on the new arrangement; the stale comment asserting peer readiness is gone |
| **R4** | `semantic` profile green, with **evidence the wait executed** — not merely that markers appeared |
| **R5** | **E-020 → `CLOSED`**; **E-021 closed or its residue stated** (D3); `ERRATA.md` and `v1-limitations.md` edited in the same commit |

### Non-goals

- **E-019 / RFC-0.26-003.** Separate line. If this RFC's answer happens to give
  it a mechanism, **escalate** — do not absorb it.
- Reworking RFC 058's readiness protocol for services that are working today.
- Any kernel scheduler change. **RFC-0.26-001 is correct and is not to be
  softened.**
- Gate 12 `syscall-surface` must stay **35/29/6** unless a new syscall is
  proposed, escalated, and accepted.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | A re-timing ships as a fix | **High** | **High** | D2, and the review asks what the wait waits on |
| R2 | `wait_ready_exact` is patched and called done | Medium | **High** | D3 — patching the `else` alone is explicitly not an acceptable outcome |
| R3 | The wider `Touches` invites unrelated changes | Medium | Medium | D4 — each file touched must be required by the chosen design |
| R4 | The answer needs a kernel capability change | Medium | High | Possible and legitimate; **escalate before writing it** |
| R5 | Not host-testable (E-013) | **Certain** | Medium | QEMU evidence, cited by log |

## Acceptance criteria

- [ ] **The open question answered in writing and committed**, naming the
      invariant established.
- [ ] `cargo xtask qemu-run --profile semantic` **PASS**, all four markers.
- [ ] **Evidence the wait executed.** Green markers alone do not satisfy this.
- [ ] No reordering of spawns, no yield counts, no retries, no raised bounds.
- [ ] **E-020 `CLOSED`**; **E-021 closed, or its residue stated explicitly**.
- [ ] Gate 7 back to `0 OPEN errata`; `release-rehearsal` green.
- [ ] `cargo xtask test-all` — **20/21**, `ipc` still failing under E-019 /
      RFC-0.26-003, stated rather than absorbed.
- [ ] `cargo fmt --all --check` clean — run, not predicted.

## Why its predecessor is superseded rather than continued

RFC-0.26-002 was not incomplete. **Its premise was wrong**: it asserted a
usable signal existed. Continuing it would carry that claim forward in a
document the next reader would take as settled design.
