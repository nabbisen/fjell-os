# Developer Handoff — RFC-0.26-004

**Governing RFC:** [RFC-0.26-004](../../done/RFC-0.26-004-readiness-channel.md)
**Milestone:** 0.26 — **blocks every release**
**Status:** inherited from the governing RFC (Implemented, 0.26.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. You already did the hard part

You found this. RFC-0.26-002 told you a usable readiness signal existed, you
checked instead of believing it, and you stopped when the check came back
negative. **That escalation is why this RFC exists and why its predecessor is in
`archive/`.**

So: nothing here asks you to re-derive §1–§3 of your own escalation. The
mechanism is settled and written into the RFC. What is open is the **design
answer**, and that is the deliverable.

## 0.1 One correction to your mechanism, in your favour

`sys_ipc_send` returns `SendResult::Queued` when no receiver is waiting
(`cap/syscall.rs:500`), so the READY message is **not dropped** — it queues.

The dependency is therefore **who dequeues first**, not who was blocked first.
Slightly wider than you described, and it cuts both ways: if `semantic-stream`
reaches its own `recv` before `init` does, it consumes its own handshake and
`init` waits forever.

## 0.2 Design decisions settled — do not re-open

1. **Decide before implementing.** The written answer is the deliverable.
2. **No re-timing.** Reordering `init`'s M4/M5 spawns would probably make the
   profile pass today. Forbidden — third RFC running.
3. **Patching `wait_ready_exact`'s missing `else` alone is not an acceptable
   outcome.** It stops the lost reply without making the channel arrangement
   correct. If your design leaves `init` receiving on an endpoint another task
   can call into, fix the `else` **and state the residual hazard**.
4. **Do not soften RFC-0.26-001.** The scheduler fix is correct.

---

## 1. The question, and what a good answer looks like

**Where does readiness go, and who may receive on a service's endpoint?**

State the **invariant** your answer establishes, ideally in this form:

> *A service's endpoint has exactly one receiver.*

— or an explicit, justified exception to it. An answer that makes the profile
green without naming an invariant has not answered the question; it has moved
the race.

Three shapes are in the RFC. Notes on each:

- **Dedicated readiness endpoint per service.** Costs an object and a slot per
  service, in `spawn.rs`'s capability installation. Cleanest invariant.
- **Route through `service-manager`.** `SM_STATUS_QUERY`/`SM_STATUS_REPLY` are
  declared in `fjell-service-api::tags` **with no handler anywhere** — confirm
  that before relying on it. Makes readiness a question any service can ask.
- **Remove `init` as a receiver.** Most direct route to the invariant; requires
  `init` to learn readiness another way.

## 2. Scope is wider — that is deliberate, and not licence

The predecessor named one file and that is precisely why it could not be
completed. This one names the files an answer *may* need.

**Every file you touch must be required by the design you chose.** The review
will ask which decision required it. "It was in `Touches`" is not an answer.

## 3. The trap that will cost you an hour

`crates/services/fjell-init/src/main.rs` **contains NUL bytes**. `file` reports
it as `data`, and plain `grep` therefore returns **nothing** on it — silently.
Use `grep -a`.

This project has been bitten by exactly this before, on this exact file, and the
architect nearly concluded `wait_ready_exact` was dead code while verifying your
escalation.

## 4. Required evidence

1. **The written answer**, committed — which shape, why the others were
   rejected, and the invariant established.
2. `cargo xtask qemu-run --profile semantic` **PASS**, all four markers.
3. **Evidence the wait executed** — a log line, an ordering, something showing
   `sample-service` synchronised rather than was scheduled late enough.
   **Green markers alone will be sent back.**
4. The stale comment in `sample-service::service_main()` gone or corrected.
5. **E-020 → `CLOSED`**; **E-021 closed, or its residue stated**. `ERRATA.md`
   and `v1-limitations.md` in the **same commit**.
6. Gate 7 back to `0 OPEN errata`; `release-rehearsal` green.
7. `cargo xtask test-all` — **20/21 expected**, `ipc` still failing under E-019
   / RFC-0.26-003. State it; do not absorb it.
8. `cargo fmt --all --check` — run it.

## 5. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The invariant**, in one sentence, and whether anything in the tree still
  violates it.
- Any file you touched that a reader might not see the necessity of.
- Whether E-021 closed cleanly or left residue — and if residue, exactly what.
- Anything you found while reading that is not in this RFC. Your last submission
  found the defect that made this RFC necessary by doing that.
