// Verus proof: capability rights non-amplification.
// RFC-v0.17-002. Pilot target "capability".
//
// MAPS TO real Fjell code:
//   crates/fjell-cap/src/rights.rs :: CapRights::is_subset_of
//     pub fn is_subset_of(self, parent) -> bool { self.0 & !parent.0 == 0 }
//
// The spec `subset` below is byte-for-byte the same predicate. The Rust
// conformance test (crates/fjell-cap/tests/verus_conformance.rs) drives the
// shipped `CapRights::is_subset_of` over the same cases proved here.
//
// STATUS: written; machine-checks once the Verus toolchain in
// verification/verus/TOOLCHAIN.md is installed. The conformance test is the
// part validated in ordinary CI today.
//
// ASSUMPTIONS (must stay written down — appendix B anti-pattern #7):
//   A1. Rights are modeled as a u64 bitset, matching CapRights(pub u64).
//   A2. "mint" produces a child whose bits are checked by `mint_allowed`
//       before issue; this models the require_cap/cap_mint gate, not CSpace.

use vstd::prelude::*;

verus! {

pub type Rights = u64;

/// child is a subset of parent iff it sets no bit outside parent.
/// Identical to CapRights::is_subset_of.
pub open spec fn subset(child: Rights, parent: Rights) -> bool {
    (child & !parent) == 0
}

/// cap_mint may issue `child` from `parent` only if child ⊆ parent.
/// Models invariant CAP-RIGHTS-003 (no amplification).
pub open spec fn mint_allowed(parent: Rights, child: Rights) -> bool {
    subset(child, parent)
}

/// cap_copy preserves rights exactly (invariant CAP-RIGHTS-002).
pub open spec fn copy_rights(r: Rights) -> Rights { r }

// CAP-RIGHTS-001 / 003: an allowed mint never amplifies.
proof fn mint_never_amplifies(parent: Rights, child: Rights)
    requires mint_allowed(parent, child),
    ensures (child & !parent) == 0,
{
}

// Zero rights are a subset of anything (a fully-attenuated capability).
proof fn zero_is_subset(parent: Rights)
    ensures subset(0, parent),
{
}

// Reflexivity: equal rights are an allowed mint (CAP-RIGHTS-001 boundary).
proof fn equal_rights_allowed(parent: Rights)
    ensures mint_allowed(parent, parent),
{
}

// CAP-RIGHTS-002: copy does not change rights.
proof fn copy_preserves_rights(r: Rights)
    ensures copy_rights(r) == r,
{
}

// Transitivity of subset: a grandchild minted from a child stays within
// the original parent. This is the chain-attenuation property.
proof fn subset_is_transitive(a: Rights, b: Rights, c: Rights)
    requires subset(a, b), subset(b, c),
    ensures subset(a, c),
{
    assert((a & !c) == 0) by(bit_vector)
        requires (a & !b) == 0, (b & !c) == 0;
}

} // verus!
