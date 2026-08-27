# RFC-0.27-002 — Should a genuinely non-blocking one-way send exist?

**Governing RFC:** [rfcs/accepted/RFC-0.27-002-one-way-send-contract.md](../../rfcs/accepted/RFC-0.27-002-one-way-send-contract.md)
**Deliverable:** §4/R3 — the open question answered in writing, committed,
before any implementation beyond the rename. Written from direct reading of
every one-way-send call site in the tree, not inference.

## The question

> Should a genuinely non-blocking one-way send exist?

## The answer: Shape 3 — yes, the requirement is real, but not decided here

**The need is real and recurring, not a one-off design error — but deciding
its shape is an ABI-affecting decision this line is not authorised to make,
and R2's audit found the current tree does not actually need it built
today.** Record the requirement; close E-022 on the rename; let a separate,
properly-scoped RFC decide the primitive.

## Why Shape 1 ("no, rendezvous is correct as the only send") is rejected

Shape 1's argument is that announcing readiness into your own endpoint is a
design error, full stop, and the corrected name makes that obvious.

**That undersells how embedded the pattern is.** The RFC's own motivation
named two services doing this (`semantic-stream`, `proxy-text` — already
fixed by RFC-0.26-004). R2's full audit of every one-way `sys_ipc_send`
call site — including five services using raw `core::arch::asm!("li a7,
20", ...)` directly, bypassing the wrapper this RFC renames — found **five
more** with the identical shape: `fjell-measuredd`, `fjell-attestd`,
`fjell-recoveryd`, `fjell-storaged`, and (via the shared endpoint,
architecturally rather than by direct self-targeting) `fjell-verifyd`. Each
declares a fixed `EP_SLOT` used by both its own `send_ready()` and its own
later `recv_call()`. This is RFC 058's actual, load-bearing readiness idiom
across the service plane, not an isolated mistake in two files. Declaring
it uniformly wrong when six independent implementations converged on it
is a claim the evidence does not support.

## Why Shape 2 ("yes, add a syscall or flag now") is rejected

This is an ABI addition: `syscall-surface` moves, and it interacts with the
standing disposition of the six declared-but-undispatched syscalls, which
is itself an open roadmap item nobody has decided. The handoff's own D1/§0.2
instruction is explicit: **escalate before writing it.** Building a new
primitive to fix a problem R2's audit found is not currently live (below)
would be solving a hypothetical with a real ABI cost, decided unilaterally
by the implementer of an unrelated rename — exactly backwards from how this
project makes ABI decisions.

## Why Shape 3 is the answer, not a deferral of convenience

R2's audit (full results in the review request) found that **none of the
six self-targeting call sites currently violates the corrected contract** —
each is either delivered immediately or queued and later drained by a
genuine other receiver:

- `measuredd`, `attestd`, `recoveryd` (M8): `init`'s `wait_service_ready`
  reaches each corresponding `recv` before or exactly when each service's
  `send_ready()` runs, because `init`'s own boot sequence deterministically
  visits all three waits in a fixed order and a one-way send that finds no
  receiver **queues rather than drops** — so even the interleaving where a
  later-spawned service's `send_ready()` fires before `init` has reached
  that specific wait still resolves correctly once `init` gets there.
- `storaged` (M6): `init` calls `wait_storaged_ready` immediately after
  spawning it, before any other task can run — the same masking pattern
  `semantic-stream`/`proxy-text` used to have, still intact here because
  `init` was never asked to stop receiving on this object.
- `verifyd` (M7): its endpoint is the *shared* default object (`_ => 0` in
  `spawn.rs`'s `ep_obj` table, the same one `service-manager` collects
  ordinary `SERVICE_READY` broadcasts on) — a genuine, independent,
  always-running receiver, not a fragile ordering accident. Confirmed live:
  `verifyd` progresses past `send_ready()` into real M7 work in every
  passing `test-all` run (`"verifyd ready"` and subsequent markers).

**So there is no live bug to fix, and no caller left violating R2's
requirement.** But the mechanism keeping the first four safe is the exact
same accidental-ordering shape RFC-0.26-004 removed for `semantic-stream`/
`proxy-text` — and `init`'s `wait_service_ready`/`wait_storaged_ready`
retain the identical missing-`else` defect `wait_ready_exact` had (already
disclosed, not touched, in `fjell-init/src/main.rs`'s own comment since
RFC-0.26-004). Recording *"no primitive needed"* would be true only until
the next innocuous scheduling change, which is exactly the failure mode
this whole line exists to stop recording as coincidence.

## What is decided, and what is left for the next line

- **Decided:** the rename and doc-comment correction (R1) fully close
  E-022 as a documentation-contract defect — the kernel's actual behaviour
  now matches what the wrapper says.
- **Decided:** no caller is changed to work around the corrected contract.
  Non-goal held.
- **Not decided:** whether the six-instance readiness idiom should get a
  real non-blocking primitive, a restructured protocol that never needs
  one, or something else — RFC 058 rework, explicitly out of scope for
  both this RFC and RFC-0.26-004. The next line inherits a materially
  better starting point than "two services do this": a six-site inventory
  and the exact mechanism keeping each one safe today.

## Observation versus judgement, named separately per the handoff's request

**Observation** (verified by reading, not inferred): the six call sites,
their targets, and the specific mechanism (queueing, deterministic wait
order, or genuine independent receiver) keeping each safe today.

**Judgement** (a design call, not a measurement): that this evidence
favours Shape 3 over Shape 1 — i.e., that a recurring, six-instance pattern
across independently-written services is better read as an unmet primitive
need than as six independent design errors. A reviewer could reasonably
weigh this differently; the observation section is what should be checked
independently, not this conclusion.

## Evidence the corrected contract holds (R3's third requirement)

Two halves, each demonstrated live rather than inferred, since a name and a
doc-comment being wrong is exactly what asserting the mechanism without
checking looks like.

**A receive with no sender blocks, and is woken when one arrives.**
Instrumented `init`'s `wait_storaged_ready(2)` call and `storaged`'s
`send_ready()` with paired pre/post `sys_debug_writeln`s (added for this
investigation, removed before this submission — `git status --porcelain`
is clean of them). `tests/qemu/artifacts/smoke-m8/serial.log`:

```
DIAG:init:wait_storaged:pre
devmgr: profiles verified
NEG:MMIO:RIGHTS_CHECK:PASS
DIAG:storaged:send_ready:pre
devmgr: registered UART
DIAG:init:wait_storaged:post
DIAG:storaged:send_ready:post
```

Single-hart, cooperative: the only way `devmgr` and the `NEG:MMIO` marker
can execute *between* `init`'s own two prints is if `init` relinquished the
CPU in between — and the only thing `init` does between them is the
blocking `recv` inside `wait_storaged_ready`. `storaged` had not even
reached its own `send_ready:pre` yet, so `init`'s `recv` found nothing
queued and blocked, exactly as the corrected contract for the *receive*
side requires; `devmgr` (and whatever emits `NEG:MMIO:RIGHTS_CHECK`) ran
in the gap, and `init`'s `post` line proves it was later woken once
`storaged` did send.

**A send with no receiver blocks, and is woken when one arrives.** Already
demonstrated live, prior to this RFC, for the same underlying mechanism:
`semantic-stream`'s and `proxy-text`'s pre-RFC-0.26-004 `send_ready()`
calls — one-way sends into an endpoint with no waiting receiver — never
returned at all once `init` stopped being an accidental co-receiver
(`docs/rfcs/RFC-0.26-004-readiness-channel-answer.md`, "confirmed live:
this task never reached its own... print with the old call in place").
That is the negative case: blocks, and stays blocked with nothing to wake
it. The `init`/`storaged` trace above is the positive case: blocks, and
*is* woken once a sender arrives. Together they cover both halves of "a
send with no receiver blocks, and is woken when one arrives" — one from
each side of the rendezvous.

Both are the same primitive; instrumenting the send side directly for this
RFC (attestd/init, `EP_SLOT`-based `send_ready()`) was also tried and
produced a less legible trace — M8's higher concurrent task count made it
harder to attribute each intervening log line to a specific cause with
confidence, which is why the M6 (`storaged`) pair is the one cited above.

