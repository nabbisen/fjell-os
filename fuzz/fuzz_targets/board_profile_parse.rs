// Fuzz target: BoardProfile binary parse (RFC v0.6-003)
// Uses the canonical digest functions as oracle: if parse produces a
// struct, recompute digest — it must be deterministic.
#![no_main]
use libfuzzer_sys::fuzz_target;
use fjell_platform_format::{board_digest, PlatformProfile, BoardProfile};
fuzz_target!(|data: &[u8]| {
    // No structured parser for BoardProfile yet (loaded from storaged);
    // test the digest functions are panic-free on arbitrary profiles.
    if data.len() < 4 { return; }
    let mut pp = PlatformProfile::qemu_virt_default();
    // Mangle the isa_extensions to exercise digest coverage.
    pp.isa_extensions = fjell_platform_format::IsaExtensions(
        u64::from_le_bytes(data[..8.min(data.len())].try_into().unwrap_or([0u8;8]))
    );
    let pd = fjell_platform_format::platform_digest(&pp);
    // Digest must be deterministic.
    let pd2 = fjell_platform_format::platform_digest(&pp);
    assert_eq!(pd.0, pd2.0);
});
