//! `compatible` property matching helpers.

/// A single `compatible` string (NUL-free bytes).
pub type CompatString = &'static [u8];

/// Default allow-list for the QEMU `virt` machine.
pub const ALLOWED_COMPAT_QEMU_VIRT: &[CompatString] = &[
    b"riscv-virtio",
    b"ns16550a",
    b"virtio,mmio",
    b"riscv,plic0",
    b"riscv,clint0",
    b"google,goldfish-rtc",
];

/// Check if a DTB `compatible` value (potentially multi-string, NUL-separated)
/// contains any of the strings in `allowed`.
pub fn matches_compat(value: &[u8], allowed: &[CompatString]) -> bool {
    for part in value.split(|&b| b == 0) {
        if part.is_empty() { continue; }
        for &a in allowed {
            if part == a { return true; }
        }
    }
    false
}
