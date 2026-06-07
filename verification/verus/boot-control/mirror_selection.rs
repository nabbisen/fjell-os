// Verus proof: boot-control mirror selection.
// RFC-v0.17-004. Pilot target "boot-control".
//
// MAPS TO real Fjell code:
//   crates/fjell-upgrade-format/src/lib.rs :: select_bcb_mirror
//   (BcbMirrorSelection::{SelectedA, SelectedB, BothValidSameGeneration, NoneValid})
//
// The shipped selector returns FOUR outcomes (it distinguishes the
// equal-generation tie-break as its own variant). This model matches that
// exactly, which is richer than the original skeleton's three-way enum.
//
// STATUS: machine-checked (v0.17.1); see verification/verus/TOOLCHAIN.lock.
// The conformance test drives the shipped select_bcb_mirror over the full
// matrix below.
//
// ASSUMPTIONS:
//   A1. Validity (magic + CRC) is already established; CRC is out of scope
//       (BCB-VERUS first model, RFC-v0.17-004 §5).
//   A2. `generation` is a monotonic counter; higher means newer.

use vstd::prelude::*;

verus! {

pub struct Mirror {
    pub valid:      bool,
    pub generation: nat,
}

pub enum Selected {
    A,
    B,
    BothValidSameGeneration, // tie-break: caller uses A
    NoneValid,
}

/// Mirrors select_bcb_mirror exactly.
pub open spec fn select(a: Mirror, b: Mirror) -> Selected {
    if a.valid && b.valid {
        if a.generation > b.generation {
            Selected::A
        } else if b.generation > a.generation {
            Selected::B
        } else {
            Selected::BothValidSameGeneration
        }
    } else if a.valid {
        Selected::A
    } else if b.valid {
        Selected::B
    } else {
        Selected::NoneValid
    }
}

// BCB-VERUS-004: NoneValid only when both invalid.
proof fn none_only_when_both_invalid(a: Mirror, b: Mirror)
    requires select(a, b) == Selected::NoneValid,
    ensures !a.valid && !b.valid,
{
}

// BCB-VERUS-001: a valid mirror beats an invalid one.
proof fn valid_beats_invalid_a(a: Mirror, b: Mirror)
    requires a.valid, !b.valid,
    ensures select(a, b) == Selected::A,
{
}

proof fn valid_beats_invalid_b(a: Mirror, b: Mirror)
    requires !a.valid, b.valid,
    ensures select(a, b) == Selected::B,
{
}

// BCB-VERUS-002: when both valid, higher generation wins.
proof fn higher_generation_b_wins(a: Mirror, b: Mirror)
    requires a.valid, b.valid, b.generation > a.generation,
    ensures select(a, b) == Selected::B,
{
}

proof fn higher_generation_a_wins(a: Mirror, b: Mirror)
    requires a.valid, b.valid, a.generation > b.generation,
    ensures select(a, b) == Selected::A,
{
}

// BCB-VERUS-003: equal generation gives the deterministic tie-break variant.
proof fn equal_generation_is_tiebreak(a: Mirror, b: Mirror)
    requires a.valid, b.valid, a.generation == b.generation,
    ensures select(a, b) == Selected::BothValidSameGeneration,
{
}

// BCB-VERUS-005: selection is total — every input yields exactly one outcome.
// (Verus exhaustiveness over the match arms discharges this implicitly; the
// explicit lemma documents the totality claim.)
proof fn selection_is_total(a: Mirror, b: Mirror)
    ensures
        select(a, b) == Selected::A
        || select(a, b) == Selected::B
        || select(a, b) == Selected::BothValidSameGeneration
        || select(a, b) == Selected::NoneValid,
{
}

} // verus!
