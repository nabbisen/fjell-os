# Developer Handoff — RFC-0.25-001

**Governing RFC:** [RFC-0.25-001](../../done/RFC-0.25-001-external-interrupt-plane.md)
**Milestone:** 0.25
**Status:** inherited from the governing RFC (Implemented, 0.25.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This line is different from the last three

0.24 was instruments: host tools, unit-testable, cheap to demonstrate.

**This is the kernel.** `fjell-kernel` has no `[lib]` target (E-013), so
**none of R1–R5 can be unit-tested on the host.** Every claim rests on a QEMU
run. That is not a gap in your work — it is a recorded, accepted limitation
that this line is the first to actually collide with.

Two consequences:

1. **Budget for QEMU cycles**, not `cargo test` cycles.
2. **Do not imply coverage that cannot exist.** "Demonstrated in QEMU, log
   attached" is the honest evidence here. Writing "tested" without saying where
   is the substitution that cost the 0.24 audit two `sound` verdicts.

If the absence of host testing makes this genuinely unworkable — not merely
slower — **stop and escalate.** E-013 becoming a prerequisite is a legitimate
finding, not a failure to deliver.

## 0.1 The thing most likely to go wrong

**Lost wakeups.** An interrupt that fires between a task deciding to block and
actually blocking must not be lost. This is the classic bug in exactly this
code, it is intermittent, and QEMU timing will not reliably expose it.

Design decision D3 exists because of it: **reuse the IPC blocking path**
(`scheduler.rs` already has `Blocked` and a wake path). Do not write a second
blocking mechanism. If the IPC path cannot be reused as-is, that is a design
conflict — escalate rather than forking it.

## 0.2 Design decisions settled — do not re-open

1. **The kernel routes; it does not read device data.** On an external
   interrupt the kernel claims from the PLIC, finds the bound task, unblocks
   it. The *driver* reads its own device registers afterwards.
2. **UART is split by register.** Kernel keeps `THR` and `LSR` **bit 5 only**.
   The driver owns `RBR`, `IER`, `LSR` bits 0–4. **Reading `LSR` clears error
   bits, so the kernel must never act on them.** Put that invariant in a
   comment at *both* sites — it is the kind of thing a later change silently
   breaks.
3. **Three syscalls only** — `IrqBind`, `IrqWait`, `IrqAck`. The other six
   undispatched syscalls are not yours to decide.
4. **Single-hart, PLIC context 1.** No SMP.

---

## 1. Order

**R3 → R2 → R1 → R4 → R5 → R6 → R7.**

Rights constants first because R4 cannot enforce what does not exist. Trap
decode before the PLIC so you can *see* an external interrupt arriving before
you try to route it — put a temporary log line in the `SupervisorExternal` arm,
confirm it fires, then build the PLIC underneath it.

**R7 last, and deliberately.** `tests/syscall/expected.toml` goes from
**35/26/9** to **35/29/6**, and the three names leave the undispatched set.
Gate 12 checks the explicit set rather than a count *specifically* so this
cannot happen silently. Change it in the same commit as R4's dispatch arms and
say so in the review request.

## 2. Per-requirement notes

**R1 — PLIC.** QEMU `virt`, base `0x0c00_0000`, S-mode context 1 for hart 0.
Init, per-source enable, priority, threshold, claim, complete.

**The case that will bite you: an interrupt with no bound task.** It must
still be claimed and completed, or the PLIC re-raises it forever and the system
livelocks. Demonstrate that case on purpose (R2 in the RFC's risk table) —
do not assume it cannot happen.

**R2 — trap decode.** `TrapKind::SupervisorExternal`, `scause` cause 9 with the
interrupt bit set. Today it reaches `Other(scause)`, which is logged-and-ignored
in user mode and **panics in kernel mode** — so an external interrupt taken
while in the kernel currently kills the system.

**R3 — rights.** `IRQ_BIND`, `IRQ_UNBIND`, `IRQ_ACK` currently exist *only
inside a doc comment* on the `Interrupt` variant (`rights.rs:175`). Define them
as real constants and check them.

**R4 — dispatch arms.** Capability-checked against the `Interrupt` kind and the
rights from R3. `sys_irq_wait` blocks per D3.

**R5 — UART.** Add `RBR` (offset 0, read), `IER` (offset 1), `LSR` (offset 5).
Enable the RX interrupt in `IER` at init, once. Respect D2's split.

**R6 — `crates/drivers/fjell-driver-uart`.** Follow the shape of
`fjell-driver-virtio-net`: bind, wait, read, notify over IPC, ack. It needs an
`MmioRegion` capability for the UART and an `Interrupt` capability for its IRQ
line (UART0 is IRQ 10 on QEMU `virt`).

**Deliver bytes over IPC and stop there.** No shell, no echo policy, no line
editing. Those are the next line's decisions and they are explicitly out of
scope.

## 3. Required demonstrations

All in QEMU. Capture the serial log for each.

| # | Demonstrate | Must show |
|---|---|---|
| 1 | External interrupt decoded | The `SupervisorExternal` arm reached — not `Other(scause)` |
| 2 | PLIC claim/complete | An interrupt claimed, handled, completed, and **not re-raised** |
| 3 | **Unbound interrupt** | An IRQ with no bound task claimed and completed; system continues, no livelock |
| 4 | Rights enforced | A task without `IRQ_BIND` is **refused** — the refusal, not just the success |
| 5 | Blocking works | A task blocks in `sys_irq_wait`, wakes on the interrupt, and does not spin |
| 6 | **A typed character arrives** | Byte typed at the QEMU console → read by `fjell-driver-uart` → delivered over IPC |
| 7 | `driver-virtio-net` | Gets past `sys_irq_bind` for the first time. **Record what happens next; do not fix it** |

Demonstration 6 is the one this line exists for. Demonstration 4 is the one
most often skipped — a boundary is shown to exist by it saying no.

Fold 6 into a **fail-closed QEMU profile** so it cannot rot, following the
pattern of `tests/qemu/profiles/semantic.toml`.

## 4. Prohibited shortcuts

- Do not write a second blocking mechanism. Reuse the IPC path or escalate.
- Do not let the kernel read device data registers.
- Do not touch the other six undispatched syscalls.
- Do not build a shell, an echo policy, or line editing.
- Do not change `expected.toml` before R4 lands, or without saying so.
- Do not claim host test coverage. There is none available here — say where
  each claim was demonstrated.
- Do not chase `driver-virtio-net` beyond the bind. Record and move on.

## 5. Required evidence

1. **Seven demonstrations**, each with its QEMU serial log.
2. The fail-closed QEMU profile for demonstration 6.
3. Gate 12 `syscall-surface` reporting **35/29/6** with the corrected name set.
4. `cargo xtask release-rehearsal` green.
5. `cargo xtask test-all` — all tiers, plus the new profile.
6. `cargo fmt --all --check` — **run it, do not predict it.**
7. A note on what `driver-virtio-net` did once it got past bind.

## 6. Review request

Standard format, in `.git-exclude/review-request/`. One request.

Flag for focused review:

- **Anything about the wake path you were unsure of.** That is where a real
  bug would hide, and it is the one I will read hardest.
- Whether the `LSR` split held in practice, or whether kernel TX and driver RX
  interfered in a way D2 did not anticipate.
- What `driver-virtio-net` did.
- Anything you found while reading the kernel that is not in this RFC. Three of
  the four defects in the last line were found that way.
