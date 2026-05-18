# RFC 023: BCB mirror selection test + store corruption detection test

**RFC ID:** 023  
**Status:** Accepted (implementation deferred to M7.1)  
**Affects:** `crates/fjell-upgrade-format/src/lib.rs`, `crates/fjell-store-format/src/lib.rs`

## Problem (RB-08, H-04 partial)

`BootControlBlock` mirror selection (higher valid generation) is defined but untested.
`StoreSuperblock` / `RecordHeader` CRC recovery scan is defined but untested.

## Proposed fix

### BCB mirror selection test

```rust
#[test]
fn bcb_mirror_selection_chooses_higher_valid_generation() {
    let mut a = BootControlBlock::new(1); a.seal();
    let mut b = BootControlBlock::new(2); b.seal();
    assert_eq!(select_bcb_mirror(&a, &b).generation, 2);

    // Corrupt mirror B — should fall back to A
    b.magic = [0u8; 8];
    assert_eq!(select_bcb_mirror(&a, &b).generation, 1);
}
```

### StoreSuperblock recovery test

```rust
#[test]
fn superblock_corrupt_selects_valid_mirror() { ... }
```

Requires `select_bcb_mirror(a: &BootControlBlock, b: &BootControlBlock)` to be a
public function (currently inline in fjell-init).
