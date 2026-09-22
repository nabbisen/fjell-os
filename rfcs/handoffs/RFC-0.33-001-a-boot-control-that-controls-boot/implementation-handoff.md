# Developer Handoff — RFC-0.33-001

**Governing RFC:** [RFC-0.33-001](../../accepted/RFC-0.33-001-a-boot-control-that-controls-boot.md)
**Milestone:** 0.33
**Status:** inherited from the governing RFC (Accepted, 2026-09-22)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure is a machine that resets because the system decided to

Not a marker saying it would. **When this line is done, a QEMU run reaches a
health failure, declines to confirm, and the board resets** — and the harness
can tell that from a hang.

This is the first line in two milestones that can break a boot. Everything
below follows from that: the state machine lands before the kernel change, and
the kernel change is one syscall and one MMIO write, audited like every other.

## 0.1 Re-derive first (R1), each absence with a positive control

```
wc -l crates/services/fjell-bootctl/src/main.rs                     # 74
grep -rn 'BootControlBlock' --include='*.rs' crates/services        # expect nothing
grep -rn 'READ_BCB\|WRITE_BCB' --include='*.rs' crates              # constants only, no users
cargo run -q -p fjell-tools -- consistency-check syscall-surface     # 35 declared, 29 dispatched
grep -rn 'sbi\|0x100000\|syscon\|finisher' --include='*.rs' crates/fjell-kernel/src   # expect nothing
```

**The last one is the load-bearing absence** — it says the kernel has no way to
reset the board. Control it: the same search for `0x1000_0000` (the UART) must
find the MMIO the kernel *does* do, or your probe is broken rather than the
kernel empty.

Also re-derive: `fjell-bootctl-model` has no dependents; the `upgrade` profile's
four markers; and that nothing sends `BOOT_*`.

## 0.2 Settled — do not re-open

D1 the block is the service's state; D2 one protocol survives; D3 health is
evaluated in `service-manager` from readiness and faults it already tracks;
D4 one reboot syscall dispatched with a capability check, the duplicate retired
through the ABI snapshot; D5 a real reset, in QEMU; D6 demonstrated on the
health-failure path in a profile of this line's own; D7 **this line does not
boot a different image** and ADR-0009 is corrected to say which half exists.

---

## 1. Order

**R1 → §A–§E answered in writing → D1/D2 (the block, one protocol) → D3 (health)
→ D4/§D (the kernel: syscall dispatch, then the reset device) → D5/D6 (the
profile) → R8 (ADR-0009) → R9 (errata) → evidence.**

**The kernel change comes after the state machine works.** A boot that dies in a
reset path is hard to bisect from a boot that dies in a state machine; land the
part that cannot brick anything first, with the rollback arm still ending in
today's spin, and replace the spin last.

## 2. The kernel part, and how not to lose a day to it

- **Four kernel touches, not one syscall and one write** — corrected at the
  mid-line ruling (D8), where this said "nothing else in the kernel changes":
  the dispatch arm; mapping the reset page kernel-only into task address spaces
  (else the store faults in the kernel — a hang); installing a `Reboot`
  capability in `bootctl`'s CSpace (none is granted anywhere, and `CapInstall` is
  undispatched); and a dedicated `bootctl` endpoint (it shares object 0 today).
  One commit each. **Nothing else in the kernel.**
- **The reset device is the open question (§D).** QEMU `virt` exposes one; the
  `BoardProfile` lists no such device. Propose where the address comes from —
  a profile entry is cleaner than a literal, but **do not add a profile field
  nothing fills**. If a literal with a comment is the honest answer today, say
  so and say what it costs on real hardware.
- **The MMIO audit gate covers every site**, so the write needs its category tag
  and a justification that is true: what ordering it requires, and why a write
  that never returns needs no fence after it.
- **A reset in a test looks like a hang.** Before you rely on the tier, decide
  how the harness distinguishes them: QEMU's own exit versus the profile's
  timeout, and the markers that must already be on the wire *before* the reset.
  **Say which, in the review request.**

## 3. §A — persistence, and the trap in it

The RFC's lean: persist the block, **if the store path can be shown working
first**. Check before you design — and note the correction from the mid-line ruling:
**`init` sends `storaged` six sector writes today**, and writes a
`BootControlBlock` to both mirrors every boot. This handoff said nothing did.
What is missing is the **read** side (`READ_CHUNK` is a placeholder), and
`init`'s write would clobber a persisted block at the next boot.

- If it works, the block survives the reset and the tier can read it back.
- If it does not, **scope down and say so plainly**: the decision and the reset,
  with persistence named as the follow-up. Do not leave a reader thinking the
  state is durable.

**This is the question most likely to swallow the line.** A store client is a
line of its own; if you find yourself writing one, stop and escalate.

## 4. D6 — the demonstration, and what it must not absorb

A profile of this line's own: health fails → the candidate is not confirmed →
the rollback path runs → the machine resets.

**Do not make `upgrade.toml` gated.** Its other three markers
(`UNSIGNED_RELEASE_REJECTED`, `INVALID_SIGNATURE_REJECTED`,
`ACTIVE_SLOT_WRITE_REJECTED`) belong to `verifyd` and `upgraded`. Emitting the
fourth is welcome; flipping `release_gated` is another line's work, and a tier is
gated when *all* of its markers are real.

The confirm path also needs observing — a smoke marker, on the happy path, so
"confirmed" is not only the absence of a rollback.

## 5. Prohibited shortcuts

- **No marker that asserts a reset happened.** The machine's behaviour is the
  evidence (D5).
- Do not skip the capability check "because only bootctl calls it".
- Do not keep both boot-control protocols (D2), and do not invent a third.
- Do not claim slot switching (D7). If ADR-0009's text tempts you, correct the
  text — that is R8.
- Do not touch the kernel beyond the one dispatch arm and the reset site.
- Do not write a store client (§3).
- Do not leave `fjell-bootctl-model` without the dependent this line gives it.
- Do not run `cargo fmt --all --check` in your head.

## 6. Required evidence

1. R1 re-derived, with controls — especially the absent reset path.
2. §A–§E answered in writing.
3. The block as state; one protocol; the model used, with the tests that tie
   them.
4. Health evaluated in `service-manager`, and `HealthTarget` either used or
   explicitly retired.
5. `syscall-surface`'s new numbers, and the ABI snapshot re-recorded for the
   retired syscall.
6. The reset: the dispatch arm, the MMIO site with its audit tag, and where the
   address comes from.
7. **The QEMU tier**: markers before the reset, the reset observed, and how the
   harness told a reset from a hang.
8. The confirm path observed on the happy path.
9. ADR-0009 corrected; E-044 CLOSED or its survivors named (the four remaining
   undispatched syscalls are survivors, not silence), with `v1-limitations.md`.
10. `test-all` all tiers including the new profile; `release-rehearsal`;
    `consistency-check --all` **by exit status**; `cargo fmt --all --check`; a CI
    run id.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **How the harness distinguishes a reset from a hang** — first.
- **Your §A answer**, and whether persistence stayed in scope.
- **Where the reset address came from**, and what that costs on real hardware.
- Anything in `bootctl`, `service-manager` or ADR-0009 that turned out to be
  wrong beyond the RFC's six findings.
- Any figure of mine you re-derived and found different.
