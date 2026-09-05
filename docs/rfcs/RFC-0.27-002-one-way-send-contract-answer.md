# RFC-0.27-002 — Should a genuinely non-blocking one-way send exist?

**Governing RFC:** [rfcs/done/RFC-0.27-002-one-way-send-contract.md](../../rfcs/done/RFC-0.27-002-one-way-send-contract.md)
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

- `measuredd`, `attestd`, `recoveryd` (M8): all three are spawned before
  `init` reaches any of the three matching `wait_service_ready` calls, so
  each one's `send_ready()` genuinely **blocks** the sender
  (`SendResult::Queued`, confirmed live via the audit ring — see
  "Evidence" below) rather than merely leaving a message parked
  unclaimed. Each is later **woken** once `init`'s boot sequence
  deterministically reaches the matching wait and dequeues it.
- `storaged` (M6): `init` calls `wait_storaged_ready` immediately after
  spawning it, before any other task can run, so `storaged`'s send finds
  `init` already waiting and is delivered without blocking at all — the
  same masking arrangement `semantic-stream`/`proxy-text` used to have,
  still intact here because `init` was never asked to stop receiving on
  this object.
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

**Revised after review.** The first submission inferred blocking from
log-interleaving timing (which tasks' prints appeared between a sender's
own pre/post lines). That inference was invalid: `trap_dispatch`
(`crates/fjell-kernel/src/trap/dispatch.rs:128`) calls `schedule_next` after
*every* trap, not only blocking ones — a task whose syscall completed
without blocking is still re-enqueued and `choose_next()` may pick a
different task next (`dispatch.rs:255-334`, the "Timer preempt or spurious"
arm re-enqueues *any* still-`Running` task). So other tasks running between
a sender's own prints is consistent with *either* outcome and proves
neither. Checked directly instead: the kernel's own audit ring records
`arg1 == 0` for `Queued` and `arg1 == receiver_tid` for `Delivered`
(`cap/syscall.rs:538,543`) — unambiguous, independent of scheduling
timing. The run is `tests/runs/20260827-233602/`, run id
`20260827-233602`. **One correction to the previous resubmission
instruction, found while following it**: `test-all`'s own per-tier log
(`04-qemu-smoke-m8.log`) captures only the build's own stdout (compiling,
`objcopy`, `bss-pad` lines) — not the QEMU serial transcript at all, for
either the smoke-test or negative-test tiers (checked both). `tests/runs/`
is genuinely not overwritten, but it does not by itself contain the
evidence a citation like this needs. The actual serial log
(`tests/qemu/artifacts/smoke-m8/serial.log`, which *is* overwritten by the
next run) was copied into the same run directory as
`tests/runs/20260827-233602/04-qemu-smoke-m8-serial-raw.log` so it survives
under the same run id — see "Persisting this evidence" below for why this
is a manual step today, not a `test-all` feature. **Reconciled by
RFC-0.27-004 (2026-09-03):** promoted to
[`tests/evidence/RFC-0.27-002/m8-attestd-storaged-audit-ring.log`](../../tests/evidence/RFC-0.27-002/m8-attestd-storaged-audit-ring.log),
with committed provenance recording that the build was instrumented and
cannot be reproduced from its commit sha alone — this is the same log,
given a permanent, resolvable home rather than a new one.

**A send with no receiver blocks (`Queued`), and is woken once a receiver
arrives.** `fjell-attestd`, `fjell-measuredd` and `fjell-recoveryd` are all
spawned before `init` reaches any of the three matching
`wait_service_ready` calls, so all three found no receiver yet. Temporarily
instrumented each to drain the kernel audit ring (via a temporary
`AuditDrain` capability grant, `spawn.rs`; removed before this submission)
and report its own `ipc.send` record. `attestd`'s own task index, printed
independently, is `27`:

```
DIAG:attestd:task_index:27
DIAG:attestd:ipc_send:arg0=26 arg1=0 (Queued: blocked)
DIAG:attestd:ipc_send:arg0=28 arg1=0 (Queued: blocked)
DIAG:attestd:ipc_send:arg0=27 arg1=0 (Queued: blocked)
...
attestd ready
```

`arg0=27` (this task) shows `arg1=0` — `Queued`, genuinely blocked, per
`block(tasks, sched, cur_id)` at `cap/syscall.rs:540`. `arg0=26` and
`arg0=28` (`measuredd` and `recoveryd`, spawned immediately before and
after) show the same. `"attestd ready"` printing afterward confirms each
was later woken and continued — the wake fires at `cap/syscall.rs:605`,
inside `sys_ipc_recv`'s handling of a message that was sitting in `sendq`,
which calls `wake(tasks, sched, msg.sender_tid)` for a one-way message
specifically once `init`'s `wait_service_ready` dequeues it.

**The predicate's one limit, recorded so the next user of this technique
knows it.** `arg1 == 0` means `Queued` only because no task at index 0
receives on these endpoints: the `Delivered` arm records `receiver_tid`
(`cap/syscall.rs:538`), so a delivery to task index 0 would be
indistinguishable from a block. What rules that out here is the
`storaged` record below — `arg1=2`, in the same boot (the log carries a
single `Fjell OS kernel started`), placing `init` at index 2, not 0. The
two halves of this evidence disambiguate each other; neither would be
airtight alone. (Architect, added in review.)

**A send with a receiver already waiting returns immediately
(`Delivered`), without blocking.** The same technique applied to
`storaged` (M6, where `init` calls `wait_storaged_ready` immediately after
spawning it, before anything else can run) found the opposite outcome:

```
DIAG:storaged:own_ipc_send:arg0=7 arg1=2 (Delivered: not blocked)
```

`arg1=2` (a real receiver task index, not `0`) means `init` was already
waiting when `storaged` sent — the interleaving observed in the withdrawn
first submission's trace was ordinary per-trap rescheduling, exactly as the
corrected understanding above predicts, not evidence of blocking. This is
reported as a correction, not hidden: the original citation was wrong, not
merely imprecise, and the mechanism it was offered as proof of is instead
established by the `attestd`/`measuredd`/`recoveryd` trace above.

**What this confirms about the receive side, reasoned rather than
re-inferred from timing.** `SendResult::Delivered` (`cap/syscall.rs:531`)
is only returned when `ep.send()` finds a receiver already parked in the
endpoint's `recvq` — mechanically, that receiver must have already called
`sys_ipc_recv`, found nothing queued (`storaged` had not sent yet), and
taken `RecvResult::Queued`'s blocking path (`cap/syscall.rs:610`,
`block(tasks, sched, cur_id)`) *before* `storaged`'s send executed. So
`storaged`'s own confirmed `Delivered` outcome is not just evidence about
the send side — it is a direct logical witness that `init`'s
`wait_storaged_ready` call had already blocked and was woken by
`storaged`'s `wake(tasks, sched, receiver_tid)` (`cap/syscall.rs:536`).
This replaces the withdrawn interleaving-based receive-side claim with one
derived from the same verified `arg1` fact, not from a second, equally
timing-dependent inference.

**Together**: a send blocks when it finds no receiver and is woken when one
arrives (positive case, `attestd`/`measuredd`/`recoveryd`, audit-confirmed
directly); a send with a receiver already present returns immediately
without blocking, and that outcome is only possible because the receiver
had already blocked waiting for it (`storaged`, audit-confirmed directly,
receive-side blocking derived from it). The negative case — blocks and is
*never* woken — remains the pre-RFC-0.26-004 `semantic-stream`/`proxy-text`
trace already on record
(`docs/rfcs/RFC-0.26-004-readiness-channel-answer.md`), cited as history,
not re-claimed as this RFC's own live demonstration.

## Persisting this evidence — a shape to decide, not invent silently

`tests/qemu/artifacts/` is overwritten by the next run and gitignored
(`.gitignore:28`, `*.log`). `tests/runs/<timestamp>/` is not overwritten —
but, checked directly rather than assumed, `test-all`'s own per-tier logs
under it capture only build stdout, never the QEMU serial transcript
(above), *and* everything under `tests/runs/` is also `*.log` and
therefore also gitignored. So neither the "not overwritten" property nor
the directory itself currently gets a citation like this to a committed,
resolvable artefact — no QEMU evidence this project has ever cited has
been committed alongside the document that cites it (a wider problem than
this RFC, per review). For *this* submission, the raw serial log was
copied by hand into `tests/runs/20260827-233602/
04-qemu-smoke-m8-serial-raw.log`, resolvable on this machine now under that
run id, but this is a one-off manual step, not something `test-all`
does or that survives a `git clone`. Two shapes for making a citation like
this resolvable from the commit itself, for whoever decides the general
question:

1. **A narrow `.gitignore` exception** for a specific evidence directory
   (e.g. `!tests/evidence/**/*.log`), with a convention that only a log
   explicitly copied there (never `tests/runs/` or `tests/qemu/artifacts/`
   wholesale) is committed — keeps routine run noise out of the tree.
2. **A checked-in transcript, not the raw log** — extract just the cited
   lines into a `.txt` alongside the document that cites them (this
   document already does this via fenced excerpts; the gap is that the
   *source* log behind the excerpt is not verifiable from the commit).

Not decided here — this RFC's own citation is resolvable per the run id
above, which is what was required.

**Decided by RFC-0.27-004 (2026-09-03): option 1**, with a promotion
command and mandatory D2/D3 provenance rather than a bare `.gitignore`
exception alone — see
[`tests/evidence/README.md`](../../tests/evidence/README.md). This RFC's
own citation, above, has been promoted under that mechanism.

