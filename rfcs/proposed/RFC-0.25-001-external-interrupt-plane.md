# RFC-0.25-001: The External Interrupt Plane — and the first console input path

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.25
**Tracks.** Kernel external-interrupt handling; the first user-facing input path.
**Touches.** `crates/fjell-kernel` (trap decode, PLIC, three syscall dispatch
arms, UART registers), `crates/fjell-cap` (rights constants),
`crates/drivers/fjell-driver-uart` (new), `tests/syscall/expected.toml`.
**Relates to:** RFC-v0.4-001 (declared this ABI), ERRATA E-011 (the same
shape: doc-comments describing authority that does not execute), RFC-0.24-001
(Gate 12's `syscall-surface` expected set changes here).

## Summary

**The kernel decodes no external interrupt.** `scause` cause 9,
`SupervisorExternal`, is not a `TrapKind` — it falls to `Other(scause)`, which
the decoder's own doc comment says is *"logged and ignored in user mode; panic
in kernel mode."*

Above that absence sits a complete, documented design: three syscalls with
doc-comments describing PLIC semantics, a capability kind, and a driver whose
entire receive loop is written against them.

**That driver exits at line one of its interrupt path.**

This RFC builds the floor. Console input is its first consumer, and the reason
it is worth doing now: no human can currently interact with this system at all.

## Motivation

### The inventory, measured

| Layer | State |
|---|---|
| `sys_irq_bind` / `sys_irq_wait` / `sys_irq_ack` | **Declared** in `fjell-syscall`, with doc-comments describing PLIC claim/complete semantics |
| `Interrupt` capability kind | **Exists** — `rights.rs:176`, code `0x18` |
| `IRQ_BIND` / `IRQ_UNBIND` / `IRQ_ACK` rights | **Do not exist.** They appear only inside a doc comment on the `Interrupt` variant. No constants are defined |
| `scause` 9 `SupervisorExternal` | **Not decoded.** `decode_trap` handles cause 5 (timer) and nothing else on the interrupt side |
| PLIC driver | **Does not exist** anywhere in the kernel |
| Dispatch arms for the three syscalls | **None** — all three are in `expected.toml`'s undispatched set |
| UART driver | **TX only.** `THR`, `LCR`, `FCR` are defined; no `RBR`, no `IER`, no `LSR` |
| `console.rs` | **No read path of any kind** |

### The row that makes this not a feature request

`crates/drivers/fjell-driver-virtio-net/src/main.rs:135`:

```rust
match fjell_syscall::sys_irq_bind(CAP_IRQ) {
    Ok(())  => sys_debug_writeln("driver-virtio-net: IRQ bound"),
    Err(_)  => {
        sys_debug_writeln("driver-virtio-net: IRQ bind failed");
        sys_exit(1);
    }
}
```

`IrqBind` has no dispatch arm, so the kernel returns `UnknownSyscall`. **The
network driver has never once got past its own initialisation.** Everything
below that line — the RX queue drain, the `PacketRx` notifications to `netd`,
the used-ring handling written under RFC-v0.7.3-001 — has never executed.

This is consistent with a fact the 0.24 audit surfaced from the other
direction: `netd` has no IPC receive loop. Nothing was ever going to call it.

**This RFC does not add a subsystem. It supplies the missing floor under one
that already exists and currently gives up on its first syscall.**

### Why this is E-011's family

E-011 records `sys_cap_install`'s doc-comment claiming a rights check that
never executes, because the syscall has no dispatch arm. These three are the
same shape, one layer wider: the doc-comments describe PLIC interaction, the
capability kind is real, the rights it claims to grant are fiction, and the
consumer is written and shipped.

The project has been carrying a **designed and documented interrupt
architecture with nothing underneath it.**

## Design decisions

Settled here; not for the implementer to re-open.

### D1 — The kernel routes interrupts. It does not read device data.

On `SupervisorExternal` the kernel claims the interrupt from the PLIC,
identifies the bound task, and **unblocks it**. It does not touch the device's
data registers. The driver task, holding an `MmioRegion` capability, reads its
own device after `sys_irq_wait` returns.

This is what the declared ABI already describes and what a microkernel
requires. It is also what makes the console driver a *service*, not a kernel
feature.

### D2 — UART ownership is split by register, and the split is stated

The kernel must keep UART **TX**: a kernel that cannot print cannot be
debugged, and panic output is not negotiable. The new driver owns UART **RX**.

- Kernel: `THR` (write), and `LSR` **bit 5 only** (transmitter-empty polling).
- Driver: `RBR` (read), `IER`, and `LSR` bits 0–4 (data-ready and line errors).

**The hazard, stated because it is real:** reading `LSR` clears its error and
break bits. Two readers race. The invariant is therefore that **the kernel's
TX path must consult bit 5 and must never act on the error bits**, which
belong to the driver. Any future kernel-side UART error handling breaks this
and must come back to design.

The kernel enables the RX interrupt in `IER` once during init and does not
touch `IER` again.

### D3 — Blocking reuses the existing machinery

`sys_irq_wait` blocks exactly as blocked IPC does. The scheduler already has a
`Blocked` state and a wake path (`scheduler.rs:180`). **Reuse it. Do not
invent a second blocking mechanism** — a parallel one would be a new source of
lost-wakeup bugs in the one part of the kernel that must not have them.

### D4 — Three of the nine, and the gate moves deliberately

This implements `IrqBind`, `IrqWait`, `IrqAck` only. `CapInstall`,
`PlatformReboot`, `TaskKill`, `MmioUnmap`, `DmaShare`, and `Reboot` stay
undispatched and stay an open disposition.

`tests/syscall/expected.toml` therefore changes from **35/26/9** to
**35/29/6**, and the undispatched *name set* loses exactly those three.
Gate 12 checks the explicit set, not a count, precisely so this cannot happen
by accident — so it is changed in the same commit as the dispatch arms, with
the change stated in the review request.

## Scope

| # | Requirement |
|---|---|
| **R1** | A PLIC driver in the kernel: init, enable, priority, threshold, claim, complete. QEMU `virt` context 1 (S-mode, hart 0) only |
| **R2** | `TrapKind::SupervisorExternal` decoded (cause 9) and dispatched |
| **R3** | `IRQ_BIND`, `IRQ_UNBIND`, `IRQ_ACK` rights constants — they currently exist only as prose |
| **R4** | Dispatch arms for `IrqBind`, `IrqWait`, `IrqAck`, each capability-checked against the `Interrupt` kind and its rights |
| **R5** | UART `RBR`, `IER`, `LSR` register support, and RX interrupt enable at init |
| **R6** | `crates/drivers/fjell-driver-uart` — binds the UART IRQ, reads bytes, delivers them over IPC |
| **R7** | `expected.toml` 35/29/6 with the corrected name set |

### Non-goals

- **The other six undispatched syscalls.** Their disposition is a separate,
  still-open decision.
- **A shell or command set.** This delivers *bytes to a service*. What reads
  them is the next line's question, not this one's.
- **Multi-hart PLIC contexts.** Single-hart, context 1. SMP is its own
  milestone.
- **Fixing `netd`.** R1–R7 will let `driver-virtio-net` get past bind for the
  first time; what it then does is out of scope — see R4 in Risks.
- Any capability, lease, IPC, or crypto semantic change.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Lost wakeups between interrupt and scheduler | Medium | **Critical** | D3 — reuse the IPC blocking path rather than a parallel one. A kernel that occasionally fails to wake a driver is worse than one with no interrupts at all |
| R2 | An unacked or spurious interrupt livelocks the system | **High** | High | The PLIC requires claim/complete pairing; an interrupt with no bound task must be claimed and completed, not ignored. Demonstrate this case deliberately |
| R3 | Kernel TX and driver RX race on `LSR` | Medium | Medium | D2's invariant, stated in code as a `SAFETY:`/ownership comment at both sites |
| R4 | **`driver-virtio-net` starts running and fails further along** | **High** | Low | *This is progress, not regression.* It has never executed past bind; the code beyond is unproven by construction. Record what happens, do not chase it into scope |
| R5 | **None of this is host-testable** — `fjell-kernel` has no `[lib]` target (E-013) | **Certain** | High | Every claim here rests on QEMU demonstration. Budget for that, and say so in evidence rather than implying unit coverage that cannot exist |
| R6 | Scope creep into a shell | Medium | High | Explicit non-goal. Bytes to a service is the deliverable |

**R5 deserves emphasis.** E-013 is a recorded, accepted limitation, and this is
the first line where it directly constrains the work rather than merely being
disclosed. The kernel changes here cannot be unit-tested at all on the host.
If that proves intolerable, **stop and escalate** — E-013's fix becoming a
prerequisite is a legitimate finding, not a failure.

## Acceptance criteria

- [ ] `SupervisorExternal` decoded; an unhandled external interrupt no longer
      reaches `Other(scause)`.
- [ ] PLIC claim/complete demonstrated, including the **no-bound-task** case.
- [ ] `IRQ_BIND` / `IRQ_UNBIND` / `IRQ_ACK` exist as constants and are enforced;
      a task without the right is refused, and **the refusal is demonstrated**.
- [ ] `sys_irq_wait` blocks and wakes; a task blocked on an IRQ does not spin
      and does not miss an interrupt that arrives before it blocks.
- [ ] **A character typed into the QEMU console reaches
      `fjell-driver-uart` and is delivered over IPC** — captured in a QEMU
      profile, fail-closed, so it cannot rot.
- [ ] `driver-virtio-net` gets past `sys_irq_bind` for the first time.
      Whatever it does next is recorded, not fixed.
- [ ] Gate 12 `syscall-surface` reports **35/29/6** with the three names
      removed from the undispatched set.
- [ ] `cargo xtask release-rehearsal` green; `cargo xtask test-all` all tiers.
- [ ] `cargo fmt --all --check` clean.

## What this makes possible, and what it does not

It makes the project **interactive for the first time** — a person can put a
byte into it and a service can receive that byte. That is a claim this project
cannot currently make in any form.

It does not make the project *usable*. There is no shell, no command set, no
line editing, no echo policy. Those are the next line's decisions, and keeping
them out of this one is what keeps this one finishable.
