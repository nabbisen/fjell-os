# RFC-0.38-003 R1–R8: a non-blocking receive must not strand its sender

**Governing RFC:** [../accepted/RFC-0.38-003-a-non-blocking-receive-must-not-strand-its-sender.md](../accepted/RFC-0.38-003-a-non-blocking-receive-must-not-strand-its-sender.md)
**Handoff:** [../handoffs/RFC-0.38-003-a-non-blocking-receive-must-not-strand-its-sender/implementation-handoff.md](../handoffs/RFC-0.38-003-a-non-blocking-receive-must-not-strand-its-sender/implementation-handoff.md)

Written against base `58b6404` (RFC-0.38-002's tip), in the handoff's order: R1, then
R3 and R4 failing first, then R2, then both green, R6, R7, R8.

---

## Where the RFC's chain is incomplete, because the fix depends on it

The RFC's chain ends: *"`init`'s `try_recv` takes the message and never wakes the
driver … No second byte is ever delivered."* That is true of the unfixed tree and the
kernel defect is real. It is **not the only reason** a second byte is never delivered.

Run with the kernel fixed and the two-byte profile otherwise unchanged
(`r3-kernelfix-only`), `driver-uart` printed `driver-uart: byte received` **twice** —
the first send returned, the interrupt was acknowledged, the second interrupt was
taken — and the second byte still never reached `init`. `fjell-init`'s poll loop
`break`s after the first byte it receives and the process then exits, so there was no
receiver for a second. The RFC lists `fjell-init` as "possibly" touched; this is why.
A correct `try_recv` with a one-byte-wide `init` would have closed E-089 and left the
console delivering one byte.

## R1 — re-derived, with controls (`/usr/bin/grep -a` throughout)

| Claim | Found |
|---|---|
| `try_recv` discards its table | `let _ = table; // future: per-task stats`, handler took `table: &TaskTable` and `ct: &CapTable` |
| The two actions `recv` performs and it did not | `ct.set_reply[_with_lease]` for a call; `wake(tasks, sched, msg.sender_tid)` for a one-way send |
| `send` blocks on `Queued` | `Ok(SendResult::Queued) => { block(tasks, sched, cur_id); ok(tf); … }` |
| Driver: send then ack | `driver-uart/src/main.rs:89` `let _ = sys_ipc_send(CAP_INIT_EP.0, byte as usize);`, `:91` `sys_irq_ack(CAP_IRQ)` |
| One `try_recv` caller | `fjell-init/src/main.rs:1173`, on `UART_RX_EP` (`CapHandle(8)`). Every `try_recv` match in `crates/` and `tools/` was listed; the others are the wrapper, `fjell-ipc`, the SDK re-export, the kernel, and `cap-broker:546`'s comment (*"try_recv not available"*). Control: the same search finds the wrapper |
| No blocking recv on `UART_RX_EP` | its only two mentions are `init:1138` (the constant) and `:1173`. Control: the same search over `init` finds the `sys_ipc_recv_msg` calls on `INIT_RELAY_RECV_SLOT` |
| Nine single-character profiles | counted by hand, one `inject_bytes` per file: `health-fail` F, `upgrade` F, `reboot` R, `reboot-again` R, `remote-diag` D, `semantic-crash` P, `semantic-liar` L, `uart-rx` Q, `uart-rx-unbound` Z. **Nine; the RFC's list is exact.** (`grep -c inject_bytes` prints 2 for `uart-rx`: one is a comment) |

**The sweep, as a conclusion.** *Other endpoint served only by a non-blocking receive:*
none — `sys_ipc_try_recv` has the one caller. *Other places a sender can block with no
one positioned to wake it:* every one-way `sys_ipc_send` / `sys_ipc_send_words` site in
`crates/` (about 30 call sites, comments excluded) was listed and its target classified. Nearly all go to a service
whose loop is a blocking receive (service-manager, bootctl, neg-test, fleetd,
sample-service, diagnosticsd, …). Two are not that:

1. **`driver-uart` → `init`, after `init`'s window has closed.** Same endpoint, a
   different cause: not a receiver that fails to wake the sender but no receiver at
   all. **Measured** (below, R6): the driver takes one byte, blocks in its send, reads no
   more. Not a second endpoint, not a second kernel defect, and not fixed here.
2. **`service-manager` → `init`'s relay endpoint** (`INIT_RELAY_SEND_SLOT`, four
   relays, one per M6/M8 service). `init` consumes exactly those; the send would block
   only if a service re-sent `SERVICE_READY` after `init` stopped reading. **Latent, not
   live**; left alone.

Neither is a second live instance of this defect, so nothing was escalated; both are
named so they are not rediscovered.

## R3 — the two-byte tier, failing first, and what each stage showed

`tests/qemu/profiles/uart-rx.toml`: `inject_bytes` `"Q"` → `"QW"`, expected marker for
`W` added. Neither byte is a trigger byte. Four runs, each kept in the scratch
directory:

| Stage | Result |
|---|---|
| Unfixed kernel, unfixed `init` (`r3-prefix-1`) | **exit 1.** `driver-uart: byte received` once; `uart byte='Q' delivered`; **`W` missing** |
| Kernel fixed, `init` unfixed (`r3-kernelfix-only`) | **exit 1.** `byte received` **twice**; `Q` delivered; **`W` still missing** — the second cause above |
| Kernel and `init` fixed, markers carry ordinals (`r3-postfix`) | **exit 0**, 5 markers. `byte received` ×2, `uart byte='Q' delivered over IPC (#1)`, `uart byte='W' delivered over IPC (#2)` |

**Why this does not pass for the wrong reason.** The second byte can reach `init` only
if the driver's first `sys_ipc_send` returned, which only the `try_recv` wake does (there
is no other receive on that endpoint) — and it can be read by the driver only after
`sys_irq_ack`, which follows that send. Stage two shows the kernel half on its own
(driver took the second byte); stage three shows the delivery. The markers assert the
bytes by value and, through the `(#n)` ordinal `init` now prints, **in order** — a
count or a "console ready" line would not have. The old marker text is a prefix of the
new line, so every other profile that matches `…delivered over IPC` still does.

`init` change, exactly: a trigger byte (`F R P L D`) ends the window as before; any other
byte leaves it open. `Q` and `W` are deliberately not triggers. Existing single-trigger
profiles are unchanged in behaviour (the full `test-all` below is the evidence).

## R4 — the reply edge, constructed

**Form: a QEMU tier, not a host test.** The reply edge and the wake live in the kernel's
syscall handlers; the kernel is a binary crate with no `[lib]` target, so nothing in it
runs under `cargo test`, and `fjell-ipc` models the endpoint queue but not the handlers
that matter here. A host test would have asserted a model.

The exchange (`ipc` profile, marker `NEG:IPC:TRY_RECV_CALL_REPLIED:PASS`): `neg-test`
calls `sample-service` with `TRY_RECV_CALL_TEST` (0x064); the service replies, then
**calls its own dedicated endpoint** (object 6, `TRY_RECV_CALL_MSG` 0x065) and blocks;
`neg-test` drains that endpoint with `sys_ipc_try_recv` **only** and replies with label
`TRY_RECV_CALL_REPLY` (0x066). The **caller** prints the marker, and only on that label
(`sys_ipc_call_words` returns the reply label; `sys_ipc_call` returns just the status, so
my first draft could never have printed — found by the first post-fix run, which woke the
caller and printed `sample: try_recv call: unexpected reply label`; the failing-first
evidence below was re-taken with the corrected test, kernel fix reverted for the run).

| Stage | Result |
|---|---|
| Unfixed kernel (`r4-prefix2`) | **exit 1.** `NEG:HARNESS:TRY_RECV_REPLY_REFUSED` from `neg-test`; the marker absent; no `sample:` line at all — the caller was never woken |
| Fixed kernel (`r4-postfix2`) | **exit 0**, 4 markers, `NEG:IPC:TRY_RECV_CALL_REPLIED:PASS` |

## R2 — the kernel change

`complete_receive(tf, tidx, ct, tasks, sched, &msg)` in `cap/syscall.rs`: reply edge for a
call (with its lease binding) and no wake; wake for a one-way send; `deliver`; the
`IpcRecv` audit record. **`sys_ipc_recv`'s `Delivered` arm now calls it, and so does
`sys_ipc_try_recv` — the path is shared, not duplicated**, and it uses the existing
`wake` (the RFC 034 / RFC-v0.7.4-003 helper), not a third way. `sys_ipc_try_recv` takes
`&mut CapTable`, `&mut TaskTable` and the scheduler; `dispatch_ipc_try_recv` passes the
`(table, sched, ct, et)` it already destructures from its **one** `get_kernel_state()`.
Inside the handler the shared borrow of `ct` that resolves the endpoint id is confined to
a block that ends before `complete_receive` takes it mutably. No second
`get_kernel_state()`, no raw pointer, no `unsafe`, no escalation.

One behaviour change worth stating: a delivered `try_recv` now appends an `IpcRecv` audit
record, as `recv` always did; it appended none before. Only deliveries log, not
`WouldBlock` polls.

The driver is **unchanged**. I did not reorder its send and `irq_ack`; the defect is in
`try_recv`.

## R6 — "interactive", stated as what works

ROADMAP 0.25 said *"A byte typed at the console now reaches a userspace service … Interactive."*
**Measured:** bytes typed while `init` polls are delivered **in order, every one**
(`uart-rx` types two). `init`'s window is bounded (20 000 poll iterations) and ends early
at a trigger byte, after which `init` exits. A throwaway profile (not committed; removed)
typed `AB` after the window closed, injecting after `init`'s
`v0.25: no uart byte received`: the driver printed `byte received` **once**, never
delivered it, and never read `B` — it is blocked in its send with no receiver. So:
**delivery is correct while someone listens; nothing listens after `init` exits; there is
no long-lived console consumer, no echo, no line editing, no shell.** "Interactive" is
not the right word. The ROADMAP carries a dated correction; the original sentence is left.
A long-lived consumer is a design decision and not this line's.

## R7 — E-089 closed

`rfcs/ERRATA.md`: `CLOSED`, Summary row updated; the closure records the second cause. 
`docs/src/releasing/v1-limitations.md` needed nothing (E-089 was never ACCEPTED).

## R8 — gates

See the review request. The three binaries that moved are `fjell-init`, `fjell-neg-test`
and `fjell-sample-service`, exactly the three whose sources changed; the baseline was
re-recorded after a rebuild, with the binaries, in the same commit. ABI snapshot 472 → 476:
three `tags` constants (`TRY_RECV_CALL_TEST/MSG/REPLY`) and one marker — protocol tag
numbers and a marker string, no syscall or `BootInfo` change.

## Left alone, and noticed

- `STIE` stays off; no preemption change.
- Redesigning the console, `sys_ipc_try_recv_msg`, and whether polling suits `init`: out of scope.
- `sys_ipc_try_recv`'s wrapper returns only `a1` (the packed tag); a polling receiver of a
  call cannot read the payload words through it. Harmless to this line (the tier uses
  labels); a receiver that wanted the words would need `sys_ipc_recv_msg`'s shape.
- A reply edge is a single slot per task: a server that `try_recv`s a second call before
  replying to the first silently overwrites the first edge (as `recv` does; this comes from the onboarding kernel survey and I did not re-check it in this line). Not changed.
