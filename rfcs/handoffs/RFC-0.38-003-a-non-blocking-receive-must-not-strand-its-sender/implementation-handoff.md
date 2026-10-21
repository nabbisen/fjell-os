# Developer Handoff — RFC-0.38-003

**Governing RFC:** [RFC-0.38-003](../../accepted/RFC-0.38-003-a-non-blocking-receive-must-not-strand-its-sender.md)
**Milestone:** 0.38
**Status:** inherited from the governing RFC (Accepted, 0.38)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does the console deliver a second byte?** Today it does not: the driver blocks inside
its send, never acknowledges the interrupt, and every injecting profile sends exactly
one character so nothing has ever asked. When a tier injects two bytes and asserts both
arrive, and that tier has been seen failing before the fix, this line is done.

**Not the measure:** that `try_recv` now calls a wake function. The measure is two
bytes through the whole path, asserted.

## 0.1 The reason this line exists is the test inputs, not the code

The kernel change is small — `try_recv` should do on delivery what `recv` already does,
and `recv`'s code is right there to follow. If that were all, this would be a one-hour
fix.

What is worth your attention is that **nine QEMU profiles inject input and every single
one injects one character**: `"D"`, `"F"` twice, `"L"`, `"P"`, `"Q"`, `"R"` twice,
`"Z"`. A one-byte console passes every tier this project has. The defect was invisible
not because an assertion was weak but because **no input ever reached the second byte**,
and that is a failure mode no amount of reading assertions would surface.

So treat **R3 as the deliverable** and the kernel change as its prerequisite. If you
finish this line with a correct `try_recv` and a one-byte test suite, E-089 closes and
the thing that hid it for eleven releases is still there.

## 0.2 Do not go hunting for the reply-edge failure

`try_recv` has exactly one call site in the tree: `fjell-init/src/main.rs:1173`, on
`UART_RX_EP`, and the only sender to that endpoint is `driver-uart`'s one-way
`sys_ipc_send`. `cap-broker:546` says in a comment that it uses a blocking `recv`
because "try_recv not available".

So **no live caller hits the missing reply edge.** It is a latent gap in a kernel
primitive, in scope because the next service to drain a request/response endpoint with
`try_recv` would hit it silently. R4 asks you to **construct** the case, and to say
whether you did it as a host test against the IPC path or as a QEMU tier. Either is
fine; closing E-089 with that half asserted by nothing is not.

## 1. Order

1. **R1, completely**, including the profile count done by hand and the sweep for any
   other endpoint served only by a non-blocking receive. If a second live instance
   exists, escalate before widening anything.
2. **R3's tier, demonstrated failing, first.** Before touching the kernel, extend a
   profile to two bytes and watch the second one never arrive. That transcript is the
   proof E-089 was real and it cannot be reconstructed afterwards.
3. **R4's constructed reply case, also failing first.**
4. **R2, the kernel change.** Follow `sys_ipc_recv`'s path and reuse the RFC 034 /
   RFC-v0.7.4-003 wake helpers; do not add a third way to wake a task. The signature
   moves to `&mut TaskTable` plus the scheduler, so the `trap/syscall.rs` dispatch site
   changes with it.
5. **Re-run R3 and R4 green.**
6. **R6**, the ROADMAP interactive claim — checked, not assumed.
7. **R7**, closing E-089, and **R8**, the gates.

## 2. Traps specific to this line

1. **A two-byte tier that passes for the wrong reason.** If the second byte arrives
   through retry, polling luck or timing rather than a wake, the tier is green and the
   primitive is still broken. Where you can reach the mechanism, assert the mechanism.
2. **Asserting a marker instead of the bytes.** A "console ready" line or a count is not
   the property. Assert the bytes, in order.
3. **Fixing the driver instead of the kernel.** It is tempting to reorder
   `driver-uart`'s send and `sys_irq_ack`, or to make the send non-blocking, and the
   console would appear to work. The defect is in `try_recv`; a driver-side workaround
   leaves the primitive wrong for the next caller and would not pass review. If you
   believe the driver *also* needs a change, say so separately and say why.
4. **The borrow change is where this line could grow its own E-088.** The handler has no
   mutable access today and the dispatch site has to change with it. If the borrows do
   not work out cleanly, escalate rather than reaching for `unsafe` or a raw pointer —
   that is exactly how the lease-binding check ended up reverted and absent for three
   releases.
5. **`/usr/bin/grep -a`, not `grep`.** Plain `grep` skips NUL-containing files here.
   Control every absence probe, including "no other blocking recv on `UART_RX_EP`",
   against a file you know contains the string.
6. **One `test-all` at a time.** It shares `fjell-disk.img`. And rebuild **before**
   re-recording the repro baseline, committing the `.bin`s with it — a dispatch-signature
   change moves the prebuilts.
7. **Do not touch preemption.** `STIE` stays off. Both this bug and the one RFC 019 was
   written to fix exist because of that, and changing it is a different line entirely.

## 3. What to put in the review request

Per the roles document §7, and specifically for this line:

- **The profile census**, counted by hand, with the command: how many profiles inject,
  and what each injects. If my nine is wrong, say so.
- **R1's sweep result** as a conclusion plus its control — other endpoints served only
  by a non-blocking receive, other senders that can block with nobody positioned to
  wake them.
- **Four transcripts**: R3 and R4 failing before, both passing after. The failing pair
  matters more than the passing pair.
- **Which form you chose for R4** (host test or QEMU tier) and why.
- **The borrow shape** in R2, in two or three sentences, and whether `recv`'s path was
  shared or duplicated. If duplicated, why it could not be shared.
- **R6's verdict on "interactive"**, stated as what works rather than as a yes.
- **Exact commands and exit statuses** for every gate in R8.
- **Any place my RFC is wrong.** My statement of the chain here was re-derived, but my
  figures have been wrong twice this week and corrected by other people's reading. The
  tree wins.
