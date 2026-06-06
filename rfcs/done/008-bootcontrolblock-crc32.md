# RFC 008: Implement CRC32 for BootControlBlock and StoreSuperblock

**RFC ID:** 008  
**Status:** Implemented  
**Affects:** `crates/fjell-upgrade-format/src/lib.rs`,
             `crates/fjell-store-format/src/lib.rs`

## Problem (RB-07)

`BootControlBlock::is_valid()` checks only the magic field.
`StoreSuperblock::is_valid()` also checks only magic.
The `crc32: u32` field in both structs is always 0.

A corrupt BCB mirror cannot be detected; the wrong mirror could be selected.
A corrupt superblock cannot be detected during recovery scan.

## Proposed fix

Implement a simple CRC32 (polynomial 0xEDB88320, Castagnoli variant) in `no_std`:

```rust
/// Compute CRC32 over a byte slice.  This is the ISO 3309 / Castagnoli CRC
/// used by Ethernet, ZIP, and PNG.  No lookup table is needed at this scale.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
```

For `BootControlBlock::seal(&mut self)`:
```rust
self.crc32 = 0;
let bytes = unsafe { core::slice::from_raw_parts(
    self as *const _ as *const u8, core::mem::size_of::<Self>()) };
self.crc32 = crc32(bytes);
```

For `BootControlBlock::is_valid()`:
```rust
pub fn is_valid(&self) -> bool {
    if self.magic != BOOT_CTL_MAGIC { return false; }
    let mut copy = *self;
    copy.crc32 = 0;
    let bytes = unsafe { core::slice::from_raw_parts(
        &copy as *const _ as *const u8, core::mem::size_of::<Self>()) };
    crc32(bytes) == self.crc32
}
```

Apply the same pattern to `StoreSuperblock`.

## Impact

| Crate | Change |
|---|---|
| `fjell-upgrade-format/src/lib.rs` | `seal()` + updated `is_valid()` + `crc32()` fn |
| `fjell-store-format/src/lib.rs` | Same pattern |
| `fjell-init/src/main.rs` | Call `.seal()` before writing BCB/superblock to disk |

## Test plan

1. Unit test: `bcb.seal(); assert!(bcb.is_valid())`.
2. Unit test: corrupt one byte after seal; assert `!bcb.is_valid()`.
3. `cargo xtask qemu-test m7` passes.
