# RFC-0.38-003: A non-blocking receive must not strand its sender

**Status:** Implemented (0.38.0) — accepted 2026-10-21
**Milestone:** 0.38
**Tracks.** **E-089** — `sys_ipc_try_recv` neither wakes a blocked one-way sender
nor installs a reply edge, both of which `sys_ipc_recv` does. `driver-uart` blocks
inside its send and so never reaches `sys_irq_ack`, which means UART RX delivers
exactly one byte per boot; every injecting QEMU profile sends a single character, so
no tier has ever been in a position to notice.
**Touches** *(indicative)*: `crates/fjell-kernel/src/cap/syscall.rs`,
`crates/fjell-kernel/src/trap/syscall.rs` *(dispatch signature)*,
`tests/qemu/profiles/`, `crates/drivers/fjell-driver-uart/` *(possibly)*,
`crates/services/fjell-init/` *(possibly)*, `ROADMAP.md`, `rfcs/ERRATA.md`.
**Relates to:** **RFC 019**, which introduced `sys_ipc_try_recv` to escape a
receiver-side deadlock and specified neither of the two actions missing here;
**RFC-0.28-001**, which removed `wait_service_ready`; **RFC 034** and
**RFC-v0.7.4-003**, which own the IPC wake hooks this must not duplicate.

## What is wrong

`sys_ipc_try_recv` (`crates/fjell-kernel/src/cap/syscall.rs:785`) takes
`table: &TaskTable` and throws it away:

```rust
let _ = table; // future: per-task stats
```

On a successful receive it calls `deliver(tf, &msg)` and stops. `sys_ipc_recv`
(`:593`) does two more things on the same event: for a call it installs the reply
edge, and for a one-way send it **wakes the sender** — its own comment reads
"One-way send: sender can proceed immediately."

`sys_ipc_send` blocks the caller when it queues with no receiver waiting
(`:540`):

```rust
Ok(SendResult::Queued) => { block(tasks, sched, cur_id); ok(tf); ... }
```

### The live consequence: one byte per boot

1. `fjell-driver-uart` calls `sys_ipc_send(CAP_INIT_EP.0, byte)`
   (`crates/drivers/fjell-driver-uart/src/main.rs:89`) and then `sys_irq_ack(CAP_IRQ)`
   on the **next line** (`:91`).
2. The only receive on that endpoint is `fjell-init`'s polling
   `sys_ipc_try_recv(UART_RX_EP)` (`crates/services/fjell-init/src/main.rs:1173`).
   There is no blocking `recv` on `UART_RX_EP` anywhere in the tree.
3. So the first byte queues. The driver **blocks inside the send**. `init`'s
   `try_recv` takes the message and never wakes the driver. The driver never reaches
   `sys_irq_ack`. The UART interrupt is never acknowledged. **No second byte is ever
   delivered.**

This contradicts ROADMAP 0.25's description of the console as interactive.

### Why nothing saw it, which is the part that matters

**Every QEMU profile that injects input injects exactly one byte** — nine profiles,
each `inject_bytes` a single character (`"D"`, `"F"` twice, `"L"`, `"P"`, `"Q"`,
`"R"` twice, `"Z"`). The tiers pass because no test has ever sent a second byte.

This defect is not hidden behind a weak assertion or a stale marker. It is hidden by
the **shape of the test inputs**, which no amount of reading the assertions would
have revealed. It was found by an outside reader on their first pass through the
tree, and that is the fact to keep in view while fixing it: the fix is cheap, the
reason it survived is not.

### The second half is latent, and you should know that before hunting for it

The missing reply edge would strand any caller whose `ipc_call` is dequeued by a
`try_recv` receiver. **No such caller exists today.** `try_recv` has exactly one
call site in the tree — `init:1173`, on `UART_RX_EP` — and the only thing sending to
that endpoint is `driver-uart`'s one-way `sys_ipc_send`. `cap-broker:546` records in
a comment that it uses a blocking `recv` because "try_recv not available".

So do not spend time looking for a failing call path: there is none to find. The
reply-edge gap is a latent defect in a kernel primitive, and it is in scope because
the next service to use `try_recv` for a request/response endpoint would hit it with
no warning — not because something is broken by it now. R4 says how to demonstrate
it.

### Design context, not an excuse

RFC 019 (`rfcs/done/019-ipc-try-recv-cooperative-services.md`, Implemented v0.1.0)
added `sys_ipc_try_recv` precisely to escape a deadlock: "spawned services cannot
signal readiness without blocking on recv (which deadlocks without preemption)". It
specified the syscall as `EndpointTable::try_recv()` "without blocking", returning
`Ok` / `WouldBlock` / `InvalidCap`. Waking a blocked sender and installing a reply
edge appear nowhere in that specification.

So this is a gap in the original design rather than a regression — and the
receiver-side deadlock RFC 019 was built to avoid is now reproduced on the **sender**
side, by the same absence of preemption. Worth stating in the commit message: the
primitive was introduced to fix exactly the shape of bug it went on to cause.

## Requirements

**R1 — Re-derive, with controls.** That `try_recv` discards its table argument; the
two actions `recv` performs that it does not; that `send` blocks on `Queued`; the
adjacency of the driver's send and `irq_ack`; that `init:1173` is the only `try_recv`
caller and there is no blocking recv on `UART_RX_EP`; and the nine single-character
`inject_bytes` values. Count the profiles yourself — `/usr/bin/grep -a` over
`tests/qemu/profiles/`, since plain `grep` skips NUL-containing files here.

**Then sweep:** any other endpoint served only by a non-blocking receive, and any
other place a sender can block with no one positioned to wake it. Report what you
find; **if a second live instance exists, stop and escalate** rather than widening
this line yourself.

**R2 — `try_recv` does, on delivery, what `recv` does.** Wake a one-way sender;
install the reply edge for a call. This means the handler needs `&mut TaskTable` and
the scheduler rather than `&TaskTable`, which changes its signature and its dispatch
call site in `trap/syscall.rs`.

**State the borrow shape you chose.** `sys_ipc_recv` already does both of these
things with the borrows it holds, so the shape exists in the tree to copy — prefer
reusing its path over writing a parallel one, and say so if the two cannot be shared.
Reuse the RFC 034 / RFC-v0.7.4-003 wake helpers rather than adding a third way to
wake a task.

**R3 — A tier that sends more than one byte.** This is the requirement that keeps
E-089 closed. Extend an existing profile or add one whose `inject_bytes` is at least
two characters, and assert that **both** arrive. Everything else in this line is a
code change that a future refactor could quietly undo; this is the part that would
notice.

Assert on the bytes, not on a count or a "console ready" marker. A tier that
tolerates one byte of two is this defect wearing a passing badge.

**R4 — The reply edge gets its own demonstration, constructed.** Since no caller
exists (above), build one: the smallest thing that issues an `ipc_call` to an
endpoint drained by `try_recv` and asserts the reply arrives. A host-level test
against the IPC path is acceptable if a QEMU tier would need a new service — **say
which you chose and why.** What is not acceptable is closing E-089 with the
reply-edge half asserted by nothing.

**R5 — Both tiers demonstrated failing.** R3's two-byte tier and R4's reply case must
be **shown red against the pre-fix binary** and green after, with both transcripts in
the review request. "A gate that cannot run is not a passing gate" is this project's
own standard; a tier that has only ever been green is the same thing in a different
costume.

**R6 — The interactive claim is made true or corrected.** ROADMAP 0.25 describes the
console as interactive. After R2 and R3, check whether it is — a console that
delivers every byte is not automatically a console with line editing or echo. If the
claim now holds, leave it; if it holds only partly, say precisely what works. Do not
leave a claim standing on the grounds that the specific defect behind it is gone.

**R7 — E-089 closes in this line's commit**, with `docs/src/releasing/v1-limitations.md`
updated if the gate requires it. The entry is **OPEN** and Gate 7 refuses until it
closes.

**R8 — Gates by exit status.** `cargo xtask release-rehearsal`,
`consistency-check --all`, `readiness-check`, `abi-snapshot --verify`,
`fjell-repro-check`, `test-all` to completion. A dispatch-signature change touches
the kernel, so rebuild **before** re-recording the repro baseline and commit the
binaries in the same commit. Only one `test-all` at a time — it shares
`fjell-disk.img`.

### Non-goals

- **Timer preemption.** `STIE` is never enabled and every trap re-queues the running
  task. That is the condition under which both this bug and RFC 019's original one
  exist, and changing it is not this line's work.
- **Redesigning the console.** Line editing, echo and an input protocol are separate
  questions. This line makes delivery correct.
- **`sys_ipc_try_recv_msg`** or any other new **kernel** ABI surface — a syscall, a
  syscall number, a `BootInfo` field. If you conclude the fix needs one, escalate.

  *Clarified 2026-10-22.* Changing an existing handler's internal signature is not
  that, and neither are tag or marker constants in `fjell-service-api`, even though
  `abi-snapshot` enumerates them: regenerate the snapshot in this line so the cut
  sees `Added: 0`.
- **The other `try_recv`-shaped question**, whether polling is the right pattern for
  `init` at all. Out of scope.

## Risks

- **Waking a sender from the wrong borrow.** The handler currently has no mutable
  access for a reason, and the dispatch site has to change with it. This is the one
  place a plausible-looking patch can introduce the aliasing class that
  RFC-0.38-002's own defect came from. If the borrows do not work out, escalate.
- **Fixing the kernel and leaving the tests one byte wide.** Then E-089 closes and the
  thing that let it live is untouched. R3 exists to prevent exactly that, and it is
  the requirement I will look at first in review.
- **A two-byte tier that passes for the wrong reason.** If the second byte arrives
  because of retry, polling luck or timing rather than a wake, the tier is green and
  the primitive is still wrong. Assert the mechanism where you can reach it.
