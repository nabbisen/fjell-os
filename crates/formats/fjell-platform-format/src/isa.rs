//! ISA extension set helpers (RFC v0.5-001 §6.1).

use crate::platform::{IsaExtensions, ISA_EXT_I, ISA_EXT_M, ISA_EXT_A,
                      ISA_EXT_F, ISA_EXT_D, ISA_EXT_C,
                      ISA_EXT_ZBB, ISA_EXT_ZICSR, ISA_EXT_ZIFENCEI};

/// RISC-V extension bit strings (for display / debug only).
pub static ISA_BIT_NAMES: &[(u64, &str)] = &[
    (ISA_EXT_I,        "I"),
    (ISA_EXT_M,        "M"),
    (ISA_EXT_A,        "A"),
    (ISA_EXT_F,        "F"),
    (ISA_EXT_D,        "D"),
    (ISA_EXT_C,        "C"),
    (ISA_EXT_ZBB,      "Zbb"),
    (ISA_EXT_ZICSR,    "Zicsr"),
    (ISA_EXT_ZIFENCEI, "Zifencei"),
];

/// Verify that a set of `IsaExtensions` declares no reserved bits (bits
/// 9..63 must be zero in v1).
pub fn reserved_bits_clear(ext: IsaExtensions) -> bool {
    let known_mask: u64 = ISA_EXT_I | ISA_EXT_M | ISA_EXT_A
        | ISA_EXT_F | ISA_EXT_D | ISA_EXT_C
        | ISA_EXT_ZBB | ISA_EXT_ZICSR | ISA_EXT_ZIFENCEI;
    ext.0 & !known_mask == 0
}
