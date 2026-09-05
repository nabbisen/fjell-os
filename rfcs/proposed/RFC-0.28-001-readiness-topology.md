# RFC-0.28-001: The readiness protocol is split in two, and its completion signal cannot fire

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.28
**Tracks.** **E-024**, and the root cause underneath it: `SERVICE_READY` is
delivered to two different destinations depending on an unrelated table, and
RFC 058's readiness tracking has never completed once.
**Touches.** `crates/services/fjell-init`, the four-plus services that announce
readiness, `crates/fjell-service-manager`, `crates/fjell-kernel/src/task/spawn.rs`
(the `ep_obj` table), `tests/qemu/artifacts/svc/expected-markers.txt`,
`rfcs/done/RFC-0.26-004-readiness-channel.md` (its invariant text).
**Relates to:** **E-021** (the same missing-`else`, closed for two objects and
live on more); **E-023** (an RFC reading `Implemented` with unbuilt behaviour);
**RFC-0.27-002 §4** (which this line probably answers — see §5); RFC 058.

## Summary

E-024 was filed as *"`init` co-receives on four services' own endpoints."* That
is true and it is the symptom. The cause is one line in a table nobody
associated with readiness.

**Every service announces readiness to capability slot 0.** What slot 0 *means*
is decided by `crates/fjell-kernel/src/task/spawn.rs:204`:

```rust
let ep_obj: u32 = match image_id {
    ImageId::STORAGED  => 1,   ImageId::MEASUREDD => 2,
    ImageId::ATTESTD   => 3,   ImageId::RECOVERYD => 4,
    ImageId::CAP_BROKER => 5,  ImageId::SAMPLE_SERVICE => 6,
    ImageId::SEMANTIC_STREAM => 7, ImageId::PROXY_TEXT => 8,
    ImageId::DRIVER_UART => 9,
    _ => 0,
};
cs.install_raw(0, Capability { object_id: ep_obj, … });   // ← slot 0
```

`fjell-service-manager` receives `SERVICE_READY` on **object 0**
(`main.rs:20,49`). So:

| Images | Slot 0 resolves to | Their `send_ready()` reaches |
|---|---|---|
| the **9** with a dedicated object | **their own endpoint** | nobody — themselves |
| the **5** falling to `_ => 0` | object 0 | **service-manager** |

**The slot number never changed. Its meaning changed underneath the services**
the moment each was given a dedicated endpoint — for reasons recorded in that
table as being about test routing and ABDD paths, with no mention of readiness.

## What that costs, verified rather than inferred

**1. Nine services announce into an endpoint only they receive on.** Under
rendezvous IPC (RFC-0.27-002) that is a permanent block. It does not deadlock
today **only because `init` is a second receiver** on those objects — the exact
arrangement RFC-0.26-004 removed for objects 7 and 8 and did not know existed on
the rest. That is E-024.

**2. RFC-0.26-004's invariant is false.** *"A service's endpoint has exactly one
receiver"* is written in a `done/` RFC and is untrue for objects 1–4 today.
Nothing checks it.

**3. `init`'s waits carry E-021's defect, still.**
`fjell-init/src/main.rs:122-139`:

```rust
loop {
    // blocking recv on ep
    if t == MREADY || t == AREADY || t == RREADY { break; }
}   // no else — any other tag is consumed and discarded
```

A mismatched `call` is swallowed and its sender never replied to. The code
comment at `main.rs:153-158` **already flags this**, deferring it as
"not yet observed to bite in practice." This RFC is that deferral coming due.

**4. RFC 058's completion signal cannot fire, by arithmetic.**
`fjell-service-manager/src/main.rs:85` emits `NEG:SVC:READY_ACCEPTED:PASS` at
`n_ready >= 10`. **At most 5 of the 14 images can ever report.** The threshold is
unreachable by construction, and the marker appears in no log this project has
ever produced.

**5. The test was narrowed to match.** `fjell-service-api` defines **four** SVC
markers; `tests/qemu/artifacts/svc/expected-markers.txt` expects **two**. The
missing pair — `READY_ACCEPTED` and `UNAUTHORIZED_READY` — are precisely the two
that require service-manager to receive a READY message.

**6. The recorded cause is wrong.** `docs/release/v1-limitations.md:328` says
*"svc 2/4 — READY pair pending a startup-timing fix."* It is not timing. It is
topology, and a wrong diagnosis on the record is why nobody looked again.

**RFC 058 reads `Implemented`.**

## The settled part — decisions not to be re-opened

**D1 — The target is RFC 058's design, finished; not a new protocol.** 058
already specifies `SERVICE_READY` going to service-manager, which validates the
kernel-attested sender and records ready state. The design is right and was
half-built. Do not design a replacement.

**D2 — The fix is not another entry in the `ep_obj` table.** Adding the nine
images to a second slot, or pointing slot 0 back at object 0, would make the
symptom go away and leave the cause: **a slot whose meaning depends on an
unrelated table, with nothing naming the dependency.** Readiness must address
service-manager explicitly — a named slot with a named constant — so that a
future dedicated endpoint cannot silently redirect it again.

**D3 — The svc profile ends this line expecting all four markers.** The
expectation was narrowed to fit broken behaviour; it is not narrowed further,
and `READY_ACCEPTED` is not "fixed" by lowering `n_ready >= 10`. If the
threshold is wrong it is wrong for a stated reason, in writing.

**D4 — RFC-0.26-004's invariant text is corrected in this line**, either to be
true or to state its real scope. A `done/` RFC asserting something false about
the tree is the defect class this project spent 0.27 on.

**D5 — Evidence is committed.** `tests/evidence/` exists now (RFC-0.27-004). A
claim that `READY_ACCEPTED` fires is demonstrated by a promoted log with
provenance, not by an assertion that the run was green.

## The open question — §5

**Should `init` wait for readiness at all?**

If service-manager is the readiness tracker, `init` waiting separately on each
service is a second, redundant tracker — and it is the redundancy that makes
`init` a co-receiver, which is what breaks the invariant.

1. **`init` stops waiting per-service and waits for one signal from
   service-manager.** Each service endpoint gets exactly one receiver, the
   invariant becomes *true* rather than merely restated, and E-021's missing-
   `else` disappears with the loop that contains it. Cost: `init`'s boot
   sequence currently interleaves per-service waits with work, and this changes
   its shape.
2. **`init` keeps per-service waits but on service-manager's endpoint**, with
   an exact-tag match and an `else` that does not discard.
3. **`init` keeps waiting on service endpoints**, and the invariant is rewritten
   to permit a readiness co-receiver. Honest, and it keeps the hazard.

**Answer in writing before implementing.** The architect's inclination is 1, and
it is only an inclination — three lines this milestone have overturned one.

### A probable consequence, to confirm rather than assume

RFC-0.27-002 §4 left open whether a genuinely non-blocking one-way send should
exist, on the evidence that six services wanted to announce-and-continue. **If
readiness goes to service-manager — a task sitting in a receive loop — the send
rendezvouses immediately and the motivating use case evaporates.** That would
answer §4 with "no, and here is why the need was an artifact of the topology."

Check it; do not assert it. It is the most satisfying possible outcome and that
is exactly why it should be tested rather than believed.

## Scope

The services' `send_ready()`; `fjell-init`'s wait functions; `spawn.rs`'s slot
installation; `fjell-service-manager`; the svc profile's expected markers;
RFC-0.26-004's invariant text; `docs/release/v1-limitations.md:328`'s wrong
cause; **E-024** → `CLOSED`; **E-031**.

### Non-goals

- **Redesigning the service plane.** This finishes RFC 058; it does not replace
  it.
- **Timer-based timeouts.** 058's cooperative-tick timeout is out of scope; the
  `START_TIMEOUT` marker already passes.
- Changing rendezvous IPC semantics (RFC-0.27-002 D1 still stands).
- E-013, E-019/RFC-0.26-003, E-025, E-027, E-028, E-029, E-030.
- Any ABI or syscall addition. **If §5 concludes one is needed, escalate.**

## Risks

**Boot ordering is load-bearing and undocumented.** `init` reaching each wait in
a fixed sequence is what keeps nine blocked senders alive today. Changing who
waits changes what unblocks them, and the failure mode is a hang, not an error —
the same shape as the M6 hang RFC-0.26-001 spent its line explaining. **Expect
to reproduce a hang and explain it before fixing it.**

**Something else may be relying on `init` receiving on those endpoints.** The
missing-`else` loop silently consumes non-READY traffic; anything that has been
quietly absorbed by it for releases will start arriving somewhere new. That is a
finding to record, not to absorb.
