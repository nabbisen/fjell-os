# Developer Handoff — RFC-0.26-002

**Governing RFC:** [RFC-0.26-002](../../accepted/RFC-0.26-002-abdd-path-synchronisation.md)
**Milestone:** 0.26 — **blocks every release** (Gate 7 red until this lands)
**Status:** inherited from the governing RFC (Accepted, 2026-08-27)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This is small, and the small part is not the risk

One service, one call site. `sample-service` must **wait** for
`semantic-stream` and `proxy-text` before `emit_sample_intent()`.

**The risk is that the fix is a timing assumption in disguise.** Yielding a few
more times, retrying, raising a bound, sleeping — any of these will make the
profile green today and break again at the next scheduling change, which is
precisely how this defect was created. That is rejected in advance (RFC D1).

**The signal already exists.** Both peers call `send_ready()` emitting
`proto::READY` — `fjell-semantic-stream/src/main.rs:107`,
`fjell-proxy-text/src/main.rs:112`. `sample-service` already participates in the
readiness protocol in the other direction, sending `SERVICE_READY` to
service-manager as its first act. **It announces its own readiness and assumes
everyone else's.** Your job is the missing half.

## 0.1 What the review will ask

**"What does it wait on?"**

A specific answer — a message, an endpoint, a state — passes. *"Long enough"*,
*"reliably"*, or *"in practice"* does not, however green the run.

## 0.2 Design decisions settled — do not re-open

1. **Establish, do not re-time.** No yield counts, retries, sleeps, or bounds.
   If the emission cannot be made to wait, **stop and escalate** — do not
   approximate it.
2. **Failure must be loud.** If the peers never become ready, the log must say
   so. Emitting into the void and continuing is what made this invisible.
3. **E-019 / the `ipc` profile is not yours.** It is RFC-0.26-003, and it is a
   genuinely different problem — no signal exists there. Do not fix it here,
   and do not let it delay this.
4. **Do not soften RFC-0.26-001.** The priority fix is correct; the assertion
   was the defect. Restoring the asymmetry would trade a known bug for a hidden
   one.

---

## 1. The stale comment is part of the deliverable

`crates/services/fjell-sample-service/src/main.rs`, in `service_main()`:

```
// RFC-v0.23-001: emit a demonstration intent to semantic-stream. Done
// once at startup, before the request loop — semantic-stream and
// proxy-text are already spawned and ready by this point (Slice 1).
```

That comment is the defect, written down. **It must not survive the fix.**
A comment asserting a property the code now establishes is how the next person
re-introduces this.

## 2. Required evidence

1. **`cargo xtask qemu-run --profile semantic` PASS**, all four markers.
2. **Proof the wait executed** — a log line, an ordering, something showing
   `sample-service` synchronised rather than happened to be scheduled late
   enough. **Green markers alone do not satisfy this** and will be sent back.
3. What appears in the log if a peer never becomes ready (decision 2) —
   demonstrated, or reasoned explicitly if it cannot be provoked.
4. The stale comment gone or corrected.
5. **E-020 → `CLOSED`**, `ERRATA.md` and `v1-limitations.md` in the **same
   commit**. Splitting them is what produced the divergence the 0.24 audit
   found, and `errata-limitations` matches only the ID, so it would not catch a
   second one.
6. Gate 7 back to `0 OPEN errata`; `cargo xtask release-rehearsal` green.
7. `cargo xtask test-all` — **20/21 expected**, with `ipc` still failing under
   E-019 / RFC-0.26-003. **State that; do not absorb it** and do not report
   21/21 by touching `ipc`.
8. `cargo fmt --all --check` — run it.

## 3. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **What the wait waits on**, in one sentence. This is the whole review.
- Anything that made you reach for a yield or a retry, even if you did not keep
  it — that is where the pressure is, and naming it is useful.
- Whether `semantic-stream`/`proxy-text`'s existing `send_ready()` was usable
  as-is, or needed anything. If it needed something, that is a second finding.
