# RFC-v0.5-003: Architecture Boundary Cleanup and Second-Platform Preparation

## Status

Draft (revised, supersedes pack v0.5-003 draft)

## Target Version

`v0.5.0`.

## Phase

Platform Surface and Semantic Stabilization — Epic C (Arch Cleanup).

## Related Work

- v0.2 `fjell-arch` crate — existing arch abstraction.
- v0.5 RFCs 001/002 — PlatformProfile/BoardProfile and DTB derivation.
- v0.4 RFC 001 — `sys_irq_wait` / `sys_irq_ack` (introduces PLIC-coupled
  state; this RFC widens it to "interrupt controller" abstractly).

---

## 1. Summary

Audit and tighten every place in Fjell where RISC-V-specific assumptions
leak past the `fjell-arch` boundary. Introduce a minimal `ArchOps` trait
that abstracts trap entry/return, satp manipulation, mtimer/clint access,
PLIC operations, fence semantics, and cache maintenance. Land a stub ARM64
crate (`fjell-arch-arm64`) that compiles (no runtime) so the compile-time
boundary is enforced.

This RFC delivers **no new runtime functionality** — it is a refactor
plus a CI gate.

---

## 2. Motivation

A second platform (ARM64) is on the v1.0 horizon. Today, code in
`fjell-kernel`, `fjell-syscall`, `fjell-cap`, `fjell-ipc`, and a handful of
device drivers has direct uses of:

- `riscv` crate intrinsics (`riscv::register::*`);
- inline `asm!` with RISC-V mnemonics outside `fjell-arch`;
- hard-coded PLIC register offsets;
- assumptions about `XLEN = 64`.

If we don't lift these into a trait now, adding ARM64 will require touching
dozens of files. Locking the boundary while we have only one platform is
cheap; doing it under pressure is not.

---

## 3. Goals

```text
- Single trait ArchOps owned by fjell-arch; one implementation per platform.
- No `riscv::*` use outside fjell-arch-riscv64.
- No inline asm with arch mnemonics outside fjell-arch-*.
- CI gate: `cargo deny` configuration rejects riscv crate use anywhere else.
- Stub fjell-arch-arm64 compiles (no run); enforces the trait shape.
- Arch-neutral type names: usize-sized addresses, no `riscv::pa`.
```

## 4. Non-Goals

```text
- No ARM64 runtime in v0.5.
- No SMP changes.
- No re-architecting of the syscall ABI; the ABI types stay arch-neutral
  (already true since v0.2).
```

---

## 5. External Design

### 5.1 The `ArchOps` trait

```rust
pub trait ArchOps: 'static + Sized {
    // identification
    fn family() -> PlatformFamily;
    fn family_version() -> u16;

    // trap path
    /// Called from the trap vector with the saved state.
    fn handle_trap(state: &mut TrapState);
    /// Inverse of handle_trap.
    fn return_to_user(state: &TrapState) -> !;

    // memory translation
    fn satp_install(asid: u16, root_pa: usize);
    fn flush_tlb_all();
    fn flush_tlb_addr(va: usize);

    // timer / clint
    fn mtime() -> u64;
    fn set_timer(mtimecmp: u64);

    // plic / interrupt controller
    fn plic_claim() -> Option<u16>;
    fn plic_complete(irq: u16);
    fn plic_mask(irq: u16);
    fn plic_unmask(irq: u16);
    fn plic_set_priority(irq: u16, prio: u8);

    // cache / fence
    fn fence_io();
    fn fence_full();
    fn fence_release();
    fn fence_acquire();
    fn cache_invalidate_range(va: usize, len: usize);
    fn cache_clean_range(va: usize, len: usize);

    // power
    fn wait_for_interrupt();
    fn cpu_halt() -> !;
}
```

`TrapState` is arch-neutral *outside* the registers array (which is
arch-specific and only inspected within `fjell-arch-*`).

### 5.2 Compile-time selection

```text
fjell-arch                — defines the ArchOps trait + arch-neutral types
fjell-arch-riscv64        — impl for RISC-V (default in v0.5)
fjell-arch-arm64          — stub impl that compiles but panics at runtime
fjell-kernel              — uses the trait via a type alias
```

A workspace feature `arch-riscv64` (default) selects the impl. ARM64 stub
is `arch-arm64-stub`; chosen impl is a top-level `pub type ArchImpl = ...;`
in `fjell-arch`.

### 5.3 Lint enforcement

A `clippy.toml` rule and a `deny.toml`:

```text
[graph]
exclude = ["riscv:*", "riscv-pac:*"]   # allowed only via fjell-arch-riscv64
```

CI step `cargo deny check` runs on the workspace minus `fjell-arch-*`.

---

## 6. Data Model

### 6.1 Arch-neutral types

```rust
pub type Va = usize;
pub type Pa = usize;
pub type Asid = u16;

pub struct TrapState {
    pub epc:        usize,        // exception PC
    pub status:     usize,        // arch status register snapshot
    pub cause:      usize,        // arch cause register snapshot
    pub tval:       usize,        // arch trap value register
    pub regs:       ArchRegs,     // arch-specific bag, accessed only in fjell-arch-*
}
```

`ArchRegs` is a per-arch struct. Outside `fjell-arch-*` no code dereferences
its fields.

### 6.2 No `XLEN`

`fjell-arch` mandates 64-bit pointers; 32-bit platforms are out of scope.
Code uses `usize` and `u64` only; uses of `riscv64imac::*` or similar are
forbidden outside the riscv64 crate.

---

## 7. Internal Design

### 7.1 Migration steps

```text
1. Add fjell-arch::ArchOps trait + ArchImpl type alias.
2. Add fjell-arch-riscv64 crate that moves all existing arch code over.
3. Replace direct riscv crate uses in fjell-kernel with ArchImpl::* calls.
4. Same in fjell-syscall, fjell-cap, fjell-ipc, drivers.
5. Add fjell-arch-arm64 stub crate.
6. Add deny.toml gate and CI step.
7. Verify kernel cross-builds against the riscv64 impl; cargo check the arm64
   stub.
```

Each step lands in its own PR with green tests; the v0.2 negative tests
continue to pass throughout.

### 7.2 Hot-path performance

The trait is fully monomorphised at compile time (no `dyn ArchOps`); zero
runtime cost. CI runs an optimisation-level sanity check (assembly diff
versus a known baseline for the syscall entry).

---

## 8. Security Design

### 8.1 Threat model deltas

This RFC is a refactor; the *boundary* between user and kernel is
unchanged. The only security-relevant aspect is:

```text
Threat T-130: A driver imports the riscv crate directly and reads CSRs.
Mitigation:  cargo deny + CI step rejects the dependency.
```

### 8.2 Audit emission

No new audit kinds.

---

## 9. Memory / Resource Design

No change.

---

## 10. Compatibility and Migration

- All existing host and QEMU tests must continue to pass.
- Cross-build target unchanged (`riscv64gc-unknown-none-elf`).
- A new `cargo check --target aarch64-unknown-none --features arch-arm64-stub`
  is added to CI.

---

## 11. Test Strategy

### 11.1 Static gate

```text
- cargo deny check (workspace minus fjell-arch-*) → no riscv crate use.
- grep -R 'asm!' workspace minus fjell-arch-* → zero matches.
- grep -R 'riscv::' workspace minus fjell-arch-* → zero matches.
```

### 11.2 Smoke

```text
- SMOKE:ARCH:RISCV_BOOTS              — existing boot path still works.
- SMOKE:ARCH:STUB_COMPILES            — cargo check arm64 stub returns 0.
```

### 11.3 Negative

| Marker                                                  | Profile |
|---------------------------------------------------------|---------|
| `NEG:ARCH:RISCV_CRATE_USE_OUTSIDE_ARCH_REJECTED`        | arch    |
| `NEG:ARCH:INLINE_ASM_OUTSIDE_ARCH_REJECTED`             | arch    |

(Both are CI-only negative tests via cargo-deny / grep; no QEMU.)

---

## 12. Acceptance Criteria

```text
- fjell-arch trait defined.
- fjell-arch-riscv64 contains all RISC-V code.
- fjell-arch-arm64 stub compiles.
- Workspace builds without riscv crate use outside fjell-arch-*.
- All v0.2 + v0.3 + v0.4 negative tests still pass.
- ADR-v0.5-003 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.5-003-arch-boundary.md
docs/src/development/v0.5-003-arch-boundary.md
docs/src/adr/v0.5-003-arch-trait-monomorphised.md
docs/src/adr/v0.5-003-riscv-crate-gate.md
```

---

## 14. Open Questions

1. **SMP** — the trait does not encode multi-hart concerns (TLB shootdown,
   IPI). Resolution: add `inter_processor_interrupt()` and per-hart timer
   methods when SMP work begins (post-v1.0).
2. **CapRights widening to u64** — the RFC v0.4-001 OQ flagged this; the
   v0.5-003 cleanup is a natural place to land it. Decision: yes, widen in
   a small companion ADR.
3. **PLIC vs AIA** — AIA (Advanced Interrupt Architecture) is the next-gen
   RISC-V controller. The trait abstracts both; a v0.5.x RFC adds AIA
   support behind a feature flag.

---

## 15. Release Gate (RFC-local)

```text
- Refactor merged.
- All preceding negative tests continue to pass.
- cargo deny + grep gates green.
- ARM64 stub builds.
- ADRs Accepted.
```
