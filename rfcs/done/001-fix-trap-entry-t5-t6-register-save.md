# RFC 001: Fix t5 (x30) and t6 (x31) register save in trap entry

**RFC ID:** 001  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/src/trap/entry.rs`

---

## 1. Problem

In `supervisor_trap_entry` (trap/entry.rs), the save phase saves **incorrect values** for
`gpr[30]` (t5) and `gpr[31]` (t6):

```asm
# Current (incorrect):
csrr   t5, sscratch       # t5 = scratch_addr  (not user_t5!)
ld     t5, 16(t5)         # t5 = scratch[2] = user_sp
sd     t5, 2*8(t6)        # gpr[2]  = user_sp  ✓
...
sd     x30, 30*8(t6)      # gpr[30] = user_sp  ✗  (still holds user_sp from above)

csrr   t5, sscratch       # t5 = scratch_addr  (not user_t6!)
sd     t5, 31*8(t6)       # gpr[31] = scratch_addr  ✗
```

After an ecall, restored `t5 = user_sp` and `t6 = scratch_addr` rather than the
pre-ecall user register values.

**Observable symptom:** `fjell-init` must re-call `sys_mmio_map` after every ecall
that spans a block-I/O sequence, because the MMIO base pointer held in a register is
corrupted on return (workaround committed in `fjell-init/src/main.rs`, lines 197, 207,
and elsewhere with the comment *"Re-read base after ecalls (t5/t6 reg save issue)"*).

**ABI note:** t5 and t6 are *caller-saved* in the RISC-V ABI.  The Rust compiler does
**not** guarantee their preservation across function-call boundaries.  For the current
cooperative smoke-test workload the corruption happens to be unobservable.  Any change
in compiler output or any future use-case that relies on t5/t6 will expose latent
failures.

---

## 2. Proposed fix

Use `scratch[3]` (add a fourth slot to `TRAP_SCRATCH`) to save the true user-t6 before
`csrrw t6, sscratch, t6` clobbers it, and save the true user-t5 before it is clobbered
by the user-sp retrieval.

### scratch layout after fix

```
TRAP_SCRATCH[0]  kernel sp
TRAP_SCRATCH[1]  &TrapFrame
TRAP_SCRATCH[2]  temp user sp  (unchanged)
TRAP_SCRATCH[3]  temp user t6  (NEW)
```

### Save phase (corrected)

```asm
# Step 1: swap t6 ↔ sscratch; now t6 = scratch_addr, sscratch = user_t6.
csrrw  t6, sscratch, t6

# Step 2: stash user_t6 (currently in sscratch) into scratch[3] via t5.
csrr   t5, sscratch          # t5 = user_t6
sd     t5, 24(t6)            # scratch[3] = user_t6

# Step 3: save user_sp into scratch[2] BEFORE loading kernel_sp.
sd     sp, 16(t6)            # scratch[2] = user_sp

# Step 4: load kernel sp from scratch[0].
ld     sp, 0(t6)

# Step 5: restore sscratch = scratch_addr for next trap entry.
csrw   sscratch, t6

# Step 6: load TrapFrame ptr from scratch[1].
ld     t6, 8(t6)             # t6 = &TrapFrame  (scratch_addr now lost from register)

# Save x1..x29 (as before) ...

# Step 7: save user_sp (gpr[2]) — retrieve from scratch[2].
csrr   t5, sscratch          # t5 = scratch_addr
ld     t5, 16(t5)            # t5 = scratch[2] = user_sp
sd     t5,  2*8(t6)          # gpr[2] = user_sp  ✓

# Step 8: save true user_t5 (x30).
# At this point t5 holds user_sp; real user_t5 is gone — we must save it
# BEFORE step 7 overwrites t5.  Reorder: save x30 *before* loading user_sp.
# See implementation note below.

# Step 9: save user_t6 (gpr[31]) — retrieve from scratch[3].
csrr   t5, sscratch          # t5 = scratch_addr
ld     t5, 24(t5)            # t5 = scratch[3] = user_t6
sd     t5, 31*8(t6)          # gpr[31] = user_t6  ✓
```

**Implementation note — saving true user_t5 (x30):**  
After `csrrw t6, sscratch, t6` the original x30 (t5) is still live in x30.  It must be
saved with `sd x30, 30*8(t6)` **immediately**, before any subsequent instruction
overwrites x30.  Concretely, reorder the save sequence so `sd x30, 30*8(t6)` comes
directly after the TrapFrame pointer is loaded into t6 (step 6), before any `csrr t5`
instruction overwrites x30.

---

## 3. Rationale

- Minimal change: 4 instructions added to save phase, 1 slot added to `TRAP_SCRATCH`.
- No change to restore phase needed (it already reads gpr[30]/gpr[31] from TrapFrame).
- Alternative (use a second sscratch): RISC-V S-mode has only one `sscratch`; not viable.
- Alternative (save into stack): kernel stack is not yet loaded at this point in the entry.

---

## 4. Impact

| Crate | Change |
|---|---|
| `fjell-kernel/src/trap/entry.rs` | Save sequence reorder + scratch[3] use |
| `fjell-kernel/src/main.rs` | `TRAP_SCRATCH: [usize; 3]` → `[usize; 4]` |
| `fjell-init/src/main.rs` | Remove 4 workaround `sys_mmio_map` re-read calls |

No syscall ABI change.  No format crate change.

---

## 5. Test plan

1. `cargo xtask qemu-test m7` must still pass after the fix.
2. Add a unit test (host-side, simulated) that:
   - Sets up a mock TrapFrame with known values in all 32 registers.
   - Calls the save routine (or a Rust equivalent that exercises the same reorder logic).
   - Asserts `tf.gpr[30] == original_x30` and `tf.gpr[31] == original_x31`.
3. Remove the workaround comments in `fjell-init/src/main.rs` as a correctness signal.

---

## 6. Implementation notes

- The reorder means the sequence is no longer strictly x1…x31; document the
  non-sequential ordering with a clear comment.
- `TRAP_SCRATCH` is a `static` in `main.rs`; its array size must be updated from 3 to 4
  before the trap handler is installed (before `init_trap()`).
- The `first_entry` function in `dispatch.rs` also writes to `TRAP_SCRATCH`; verify it
  does not need updating (it only writes slots [0] and [1]).
