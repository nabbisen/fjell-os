// Fuzz target: release metadata v1 (RFC v0.6-003)
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let _ = fjell_upgrade_format::release_metadata::parse(data);
});
