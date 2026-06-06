# RFC 005: sys_mmio_map — exclude kernel RAM from mappable range

**RFC ID:** 005  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/src/trap/syscall.rs`

---

## 1. Problem (RB-02)

`sys_mmio_map(phys_addr, size)` accepts any aligned physical address and maps it
with `R | W | U` into the caller's page table via `remap_page`.

Since the kernel uses an identity map (VA = PA for `0x8000_0000`+), a malicious
user task can call `sys_mmio_map(0x8000_0000, 4096)` to obtain a user-accessible
mapping of kernel text.  This completely breaks memory isolation.

```
RAM_BASE = 0x8000_0000
RAM_END  = 0x8800_0000  (128 MiB)

ATTACK: sys_mmio_map(0x8000_0000, 4096)
RESULT: user task can read/write kernel code page
```

---

## 2. Proposed fix

Add a range exclusion check at the top of `sys_mmio_map`:

```rust
pub fn sys_mmio_map(tf: &mut TrapFrame) {
    use crate::platform::qemu_virt::{RAM_BASE, RAM_END};
    let phys_addr  = tf.gpr[REG_A0] & !0xFFF;
    let size_bytes = (tf.gpr[REG_A1] + 0xFFF) & !0xFFF;

    // Reject any request that overlaps kernel RAM.
    // MMIO ranges are all below 0x8000_0000 on QEMU virt.
    let end_addr = phys_addr.saturating_add(size_bytes);
    if phys_addr < RAM_END && end_addr > RAM_BASE {
        // Overlap with RAM — reject unconditionally.
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }
    // ... rest of mapping logic unchanged
}
```

Additionally: add explicit MMIO allowlist check (permit only known QEMU virt MMIO
regions defined in `platform::qemu_virt::MMIO_REGIONS`).

---

## 3. Rationale

- Defense in depth: even after RB-01/RB-04 capability gating is added, a
  misconfigured driver should not be able to map kernel RAM.
- The kernel RAM is always above `RAM_BASE = 0x8000_0000`; all virtio-mmio,
  UART, CLINT, PLIC are below that address on QEMU virt.
- An MMIO allowlist is more conservative than a blocklist and better matches
  the principle of least privilege.

---

## 4. Impact

| Crate | Change |
|---|---|
| `fjell-kernel/src/trap/syscall.rs` | 5-line range check added at top of sys_mmio_map |
| No other crate changes | |

---

## 5. Test plan

1. Attempt `sys_mmio_map(0x8000_0000, 4096)` from user space → must return `InvalidCap`.
2. Attempt `sys_mmio_map(0x1000_8000, 0x1000)` (valid virtio MMIO) → must succeed.
3. `cargo xtask qemu-test m7` must still pass.

---

## 6. Implementation notes

- `saturating_add` prevents overflow on 64-bit addresses.
- The check `phys_addr < RAM_END && end_addr > RAM_BASE` is an overlap test
  equivalent to `!(end_addr <= RAM_BASE || phys_addr >= RAM_END)`.
- Future work (M8): replace with MMIO region capability lookup.
