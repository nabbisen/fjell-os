# RFC 002: Fix BootControlBlock initial slot B state

**RFC ID:** 002  
**Status:** Implemented  
**Affects:** `crates/fjell-upgrade-format/src/lib.rs`

---

## 1. Problem

`BootControlBlock::new()` initialises `slot_b` with `SlotInfo::bootable(generation)`:

```rust
// fjell-upgrade-format/src/lib.rs — current (incorrect)
pub fn new(generation: u64) -> Self {
    BootControlBlock {
        ...
        slot_a: SlotInfo::bootable(generation),   // ✓ slot A is the active boot slot
        slot_b: SlotInfo::bootable(generation),   // ✗ slot B should be Empty
        ...
    }
}
```

`SlotInfo::bootable()` produces:

```rust
state:           SlotState::Bootable
image_generation: generation
confirmed:        1
tries_allowed:    3
remaining_tries:  3
```

This is wrong for an unprovisioned slot.  On a freshly formatted disk, slot B has never
been staged with an image.  Marking it `Bootable` with `confirmed=1` and
`remaining_tries=3` means:

- Any future `bootctl` implementation that selects the slot with highest `tries_allowed`
  or any slot in `Bootable` state could choose slot B even though it contains no valid image.
- The `bootctl` invariant `BOOTCTL-003: last_confirmed_slot is always Bootable or Confirmed`
  is trivially satisfied by the wrong slot, masking the invariant violation.

---

## 2. Proposed fix

```rust
pub fn new(generation: u64) -> Self {
    BootControlBlock {
        magic:                BOOT_CTL_MAGIC,
        version:              1,
        generation,
        active_slot:          SlotId::A as u8,
        last_confirmed_slot:  SlotId::A as u8,
        candidate_slot:       NO_CANDIDATE,
        slot_a: SlotInfo::bootable(generation),  // slot A: active confirmed boot slot
        slot_b: SlotInfo::empty(),               // slot B: unprovisioned
        crc32:                0,
    }
}
```

`SlotInfo::empty()` already exists and produces the correct state:

```rust
state:           SlotState::Empty
image_generation: 0
confirmed:        0
tries_allowed:    3
remaining_tries:  3
```

---

## 3. Rationale

The only reason slot B was set to `bootable` in the initial implementation was a
copy-paste error when constructing the struct literal.  `SlotInfo::empty()` is the
semantically correct choice: the BCB is written at store-format time when no upgrade has
ever been staged.

Alternative considered: leaving `slot_b: SlotInfo::bootable(0)` and adding a validity
check that ignores slots with `image_generation == 0`.  Rejected because it adds
complexity to every reader and conceals the intent of the `Empty` state.

---

## 4. Impact

| Crate | Change |
|---|---|
| `fjell-upgrade-format/src/lib.rs` | One-line change in `BootControlBlock::new()` |
| `fjell-init/src/main.rs` | No change required (smoke test logic reads BCB but does not branch on slot state) |

No syscall ABI change.  No kernel change.

---

## 5. Test plan

1. Add a unit test in `fjell-upgrade-format/src/lib.rs`:

```rust
#[test]
fn boot_control_block_initial_slot_b_is_empty() {
    let bcb = BootControlBlock::new(1);
    assert_eq!(bcb.slot_b.state, SlotState::Empty);
    assert_eq!(bcb.slot_b.image_generation, 0);
    assert_eq!(bcb.slot_b.confirmed, 0);
}
```

2. `cargo test --package fjell-upgrade-format` must pass.
3. `cargo xtask qemu-test m7` must still pass.

---

## 6. Implementation notes

- The smoke test (`fjell-init`) currently writes BCB to disk and does not read it back;
  the test does not exercise the slot-selection logic.  The fix is therefore not
  observable in the smoke test output, but is required for correctness when `bootctl`
  is implemented as a real service.
- `SlotInfo::empty()` sets `tries_allowed = 3` and `remaining_tries = 3`; these are
  logically irrelevant for an `Empty` slot but are consistent with the struct defaults.
  If preferred, they can be set to 0 in `empty()` without affecting any current code.
