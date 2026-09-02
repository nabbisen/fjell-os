# RFC-0.26-004 — The readiness channel, decided

**Governing RFC:** [rfcs/done/RFC-0.26-004-readiness-channel.md](../../rfcs/done/RFC-0.26-004-readiness-channel.md)
**Deliverable:** R1 — the open question answered in writing, committed, naming
the invariant established, before the code that implements it. Written from
direct observation (QEMU serial logs, kernel source), not inference; every
claim below names the log line or source line it comes from.

## The question

> Where does readiness go, and who may receive on a service's endpoint?

## The answer: Shape 3 — remove `init` as a receiver

**Invariant established: a service's endpoint has exactly one receiver — the
service itself.**

`init` no longer holds any receive-capable capability to `semantic-stream`'s
endpoint (object 7) or `proxy-text`'s endpoint (object 8):

- Its capability to object 7 (CSpace slot 6) is narrowed from
  `CapRights::ALL_NON_META` to `CapRights::CALL` only
  (`crates/fjell-kernel/src/main.rs`, init-CSpace bootstrap section). Its one
  remaining use is `emit_envelope`'s outbound `ipc_call` (used throughout
  M7/M8), which `sys_ipc_call` checks against `CapRights::CALL` specifically
  — not `SEND`/`RECV` (`crates/fjell-kernel/src/cap/syscall.rs:629`). Even a
  future `sys_ipc_recv` added to `init` against this slot would fail the
  rights check at the kernel boundary, not silently reintroduce the hazard.
- Its capability to object 8 (the old slot 7) is **removed entirely**, not
  narrowed: `init` never sent to `proxy-text` directly — the only thing that
  slot was ever used for was `wait_ready_exact`, which is also removed.
- `wait_ready_exact(ep, expected)` itself — the function with the missing
  `else` branch (E-021) — is deleted from `crates/services/fjell-init/
  src/main.rs`, not patched. There is no code path left in `init` that can
  receive on either endpoint, so the missing `else` cannot fire because the
  function that had it no longer exists.

`sample-service` no longer assumes its peers are ready (E-020); it waits, for
real. `emit_sample_intent()`'s transport,
`fjell_service_api::chunked::send`, is a blocking `sys_ipc_call`
(`SyscallNumber::IpcCall`). If `semantic-stream` has not yet reached its own
`recv_call()`, the call **queues** (`SendResult::Queued`,
`crates/fjell-kernel/src/cap/syscall.rs:667-673`) and `sample-service` blocks
until `semantic-stream` actually processes it and replies — a real wait, not
a timing assumption. This is safe under the invariant above precisely
because `semantic-stream`'s endpoint has exactly one receiver: the call can
never be delivered to, and dropped by, anyone else.

## Why the other two shapes were rejected

**Shape 1 — a dedicated readiness endpoint per service, receive-capable only
to `init`.** Architecturally the shape the RFC itself calls "cleanest
invariant," and it does not (per E-022 below) hit the self-deadlock Shape 3's
sibling defect exposed. Rejected for cost, not correctness: it requires a new
kernel object and a new capability slot per service (`spawn.rs`'s capability
installation, touched once per service rather than once total), for
information `init` does not currently act on beyond blocking until it
arrives. Shape 3 reaches the identical invariant — one receiver per
endpoint — by removing a reader rather than by adding a channel, which is
strictly less surface for the same guarantee, given nothing downstream of
`init`'s old `wait_ready_exact` calls needed the two prints they gated to be
timed against anything (confirmed: those two `sys_debug_writeln` calls in
`init`'s M5 section are now unconditional and gate nothing).

**Shape 2 — route readiness through `service-manager`.** Rejected as
underbuilt for what it would need to become correct: `SM_STATUS_QUERY` /
`SM_STATUS_REPLY` are declared in `fjell_service_api::tags` with **no handler
anywhere** (confirmed by reading `fjell-service-manager`'s source — no match
arm for either tag exists). Building one is a real protocol addition, which
RFC-0.26-004's D4 ("scope is wider on purpose... that is not licence to
wander") counsels against absorbing into this RFC when Shape 3 answers the
question with files already in scope. It would also not, on its own, fix
anything for `sample-service`: `sample-service` waiting on `semantic-stream`
is a peer-to-peer dependency, not a status query `init` or `service-manager`
would ever be positioned to answer for it.

## A defect this design exposed, not one it created (E-022)

Implementing Shape 3 and validating R4 surfaced a previously-latent kernel
defect: `sys_ipc_send`'s one-way path blocks the **sender** when the message
queues with no receiver yet waiting
(`crates/fjell-kernel/src/cap/syscall.rs:540-544`,
`Ok(SendResult::Queued) => { block(tasks, sched, cur_id); ... }`) — contrary
to `sys_ipc_try_send`'s own doc-comment (`crates/fjell-syscall/src/lib.rs:
278-279`), which describes a non-blocking, fire-and-forget contract, and
contrary to this RFC's own handoff (§0.1), which read `SendResult::Queued`
the same way.

`semantic-stream` and `proxy-text` each carried a pre-existing `send_ready()`
call — a one-way send of `proto::READY` into their *own* endpoint, issued
before either task first reaches its own `recv_call()`. Under Shape 3's
invariant, nothing else ever holds a receive capability to that endpoint, so
that message can never have a waiting receiver at the moment it's sent. Per
the defect above, the send therefore blocks the sender — permanently, since
the only task that could ever call `recv()` and wake it is the task that is
now blocked. Confirmed live: with `send_ready()` in place, `semantic-stream`
never printed its own `"M5: semantic-stream started"` line (distinct from
`init`'s identically-worded spawn-label print — confirmed by instrumented
diagnostic, added and removed during investigation) despite the task being
demonstrably alive and scheduled.

This was not a pre-existing failure made visible by chance: under the design
E-021 replaces, `init` also held a receive capability to these same
endpoints and reached its own blocking receive almost immediately after
spawning each service, so a receiver was already waiting by the time
`send_ready()` ran (`SendResult::Delivered`, not `Queued`). Removing that
accidental, unsynchronised co-receiver — correctly, per E-021 — removed the
cover along with it.

**Fix applied, in scope:** `send_ready()` is dead code under the established
invariant regardless of the kernel defect — nothing has consumed
`semantic_stream::READY` or `proxy_text::READY` since `wait_ready_exact` was
removed (confirmed: no remaining reference to either constant anywhere in
the tree). Both calls, and the now-unused `send_ready()` functions, are
removed from `fjell-semantic-stream` and `fjell-proxy-text`. This is not a
workaround for the kernel defect — it is removing code the chosen design
already made meaningless, which happens to also be the only code in the
current tree that could trigger the defect. The kernel defect itself is
filed independently as **E-022** and is not fixed here: fixing
`sys_ipc_send` is a kernel syscall-semantics change outside this RFC's
`Touches`, and Risk R4 explicitly anticipates escalating rather than
resolving a finding of this shape unilaterally.

## Evidence the wait executed, not merely markers

> **Unresolvable historical citation (RFC-0.27-004 R6, 2026-09-03).** The
> path below is overwritten by every later run of the `semantic` profile —
> the exact failure mode RFC-0.27-004 exists to close — and the specific
> run quoted here no longer exists to promote: `tests/qemu/artifacts/`
> carries none of this project's history before this RFC's own R3 change,
> and by the time RFC-0.27-004 was implemented this profile had been rerun
> many times over (including as a QEMU negative-test category with the
> same name). **Not re-run to stand in for the original** (D4) — the
> quoted lines below are the citing document's own record of what a real
> run once showed, not something that can be checked against the tree
> today. The underlying architectural claim (semantic-stream validates
> before forwarding; the capability-checked refusal fires) rests on code
> paths unchanged since this RFC shipped; tracked for a fresh, properly
> -provenanced re-run and promotion under **Errata E-026** (`0.28`).

`tests/qemu/artifacts/semantic/serial.log`, current run:

```
31:M5: semantic-stream started          (init's spawn-label print)
32:M5: semantic-stream started          (semantic-stream's own — proves it ran)
35:M5: proxy-text started
36:M5: proxy-text started               (proxy-text's own)
37:M5: semantic policy loaded
39:M5: semantic policy loaded           (semantic-stream's own)
41:M5: semantic operations ready
83:[INTENT][Normal] sample-service demo intent   (semantic-stream validating the forwarded envelope)
92:sample-service: intent emitted                (sample-service's blocking call returns, AFTER 83)
93:proxy-text: action accepted
97:proxy-text: action DENIED (capability not held)
```

Lines 32/36/39 are each service's own body executing its first debug lines —
identical text to `init`'s spawn-time prints (a pre-existing coincidence,
not a double-spawn; the same phenomenon RFC-0.26-001's investigation
document records for `storaged`), but a second, distinct occurrence, proving
each task actually ran past `send_ready()`'s removal point and into its own
code.

The causal order at lines 83→92 is the wait executing, not luck: line 83 is
`semantic-stream` validating and forwarding the intent it received via
`PUBLISH_COMMIT`, which happens *inside* the handler that runs before
`semantic-stream` replies `PUBLISH_OK`. `sample-service`'s "intent emitted"
print (line 92) only fires after `chunked::send` receives that reply. If
`sample-service` had merely gotten lucky on scheduling order rather than
genuinely blocked-and-woken, there would be no way for `semantic-stream`'s
own internal processing (line 83) to necessarily precede `sample-service`'s
completion (line 92) — a race would show them interleaved unpredictably
across runs. QEMU TCG is deterministic given the same binary and inputs
(established in RFC-0.26-001's investigation and reused here), and this
ordering reproduced identically across repeated runs during investigation.

All four `semantic.toml` markers pass: `"M5: semantic operations ready"`,
`"sample-service demo intent"`, `"proxy-text: action accepted"`,
`"proxy-text: action DENIED (capability not held)"`.

## Non-goals held

- No re-timing: no spawn in `init`'s M4/M5 section was reordered.
- No yield counts, retries, or raised bounds were added anywhere in this
  change.
- `wait_ready_exact`'s missing `else` was not patched in isolation — the
  function is removed, closing E-021 with no residual receiver hazard.
- E-019 / RFC-0.26-003 (`ipc` profile) is untouched. **Correction, added at
  review:** the profile itself started passing again as a side effect of
  this RFC (`sample-service` reaching its main loop for the first time lets
  it serve `BIND_LEASE_FOR_IPC_TEST`) — `fjell-neg-test` was not touched,
  and the unsynchronised assumption E-019 records is unchanged underneath
  it. A green test that is green for a reason nobody guaranteed is not a
  smaller problem than a red one; RFC-0.26-003 remains warranted, re-framed
  to describe that.
- No kernel scheduler change; RFC-0.26-001 is unmodified by this RFC.
- Syscall surface unchanged — no new syscall proposed.

## Follow-up recorded at review, not fixed here

The review that accepted this RFC found the invariant enforced by rights on
only one of its four holders. `sample-service` (`spawn.rs:528`) and
`proxy-text` (`spawn.rs:582`) both hold `CapRights::ALL_NON_META` — which
includes `RECV` — on `semantic-stream`'s endpoint (object 7), and
`semantic-stream` (`spawn.rs:546`) holds the same on `proxy-text`'s endpoint
(object 8). None of the three ever calls `sys_ipc_recv` against these
capabilities — each only ever calls into the endpoint it holds — so this is
a **latent over-grant, not a live hazard**: nothing is currently misbehaving.

The fix `init`'s own capability received in this RFC — narrow to `CapRights::
CALL` — is exactly the fix these three need too, and would make "a service's
endpoint has exactly one receiver" hold **by rights**, not merely by every
current caller's behaviour, everywhere the invariant is claimed rather than
on one of four holders. Ruled at review as a follow-up, not a reason to
reopen this line: narrowing three more capabilities touches `spawn.rs` and
needs its own QEMU evidence that nothing depended on the wider rights — small,
but not free. Not filed as a new erratum, since nothing is currently
misbehaving; recorded here per the review's own instruction.
