//! Verus conformance: boot-control mirror selection.
//! RFC-v0.17-004 §6. Bridges the Verus proof
//! (verification/verus/boot-control/mirror_selection.rs) to the shipped
//! `select_bcb_mirror`. Covers the full selection matrix.
//!
//! Marker on success: CONFORMANCE:BOOT-CONTROL-MIRROR:PASS

use fjell_upgrade_format::{
    BootControlBlock, BcbMirrorSelection, select_bcb_mirror,
};

/// Build a valid, sealed BCB at a given generation.
fn valid_bcb(generation: u64) -> BootControlBlock {
    let mut b = BootControlBlock::new(generation);
    b.generation = generation;
    b.seal(); // recomputes CRC so is_valid() holds
    b
}

/// Build an invalid BCB (corrupt the magic so is_valid() fails).
fn invalid_bcb() -> BootControlBlock {
    let mut b = BootControlBlock::new(1);
    b.seal();
    b.magic = *b"BADMAGIC"; // break validity (magic is [u8; 8])
    b
}

#[test]
fn a_valid_b_invalid_selects_a() {
    // BCB-VERUS-001
    let a = valid_bcb(5);
    let b = invalid_bcb();
    assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::SelectedA(_)));
}

#[test]
fn a_invalid_b_valid_selects_b() {
    let a = invalid_bcb();
    let b = valid_bcb(5);
    assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::SelectedB(_)));
}

#[test]
fn both_valid_higher_generation_a_wins() {
    // BCB-VERUS-002
    let a = valid_bcb(9);
    let b = valid_bcb(4);
    assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::SelectedA(_)));
}

#[test]
fn both_valid_higher_generation_b_wins() {
    let a = valid_bcb(4);
    let b = valid_bcb(9);
    assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::SelectedB(_)));
}

#[test]
fn both_valid_same_generation_deterministic_tiebreak() {
    // BCB-VERUS-003: equal generation → explicit tie-break variant (A).
    let a = valid_bcb(7);
    let b = valid_bcb(7);
    assert!(matches!(select_bcb_mirror(&a, &b),
        BcbMirrorSelection::BothValidSameGeneration(_)));
}

#[test]
fn both_invalid_none_valid() {
    // BCB-VERUS-004
    let a = invalid_bcb();
    let b = invalid_bcb();
    assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::NoneValid));
}

#[test]
fn selection_is_total_over_matrix() {
    // BCB-VERUS-005: every combination yields exactly one defined outcome.
    let valids = [valid_bcb(1), valid_bcb(2)];
    let inval = invalid_bcb();
    for a in [&valids[0], &valids[1], &inval] {
        for b in [&valids[0], &valids[1], &inval] {
            let _ = select_bcb_mirror(a, b); // must not panic; returns a variant
        }
    }
    println!("CONFORMANCE:BOOT-CONTROL-MIRROR:PASS");
}
