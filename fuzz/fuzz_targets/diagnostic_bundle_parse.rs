// Fuzz target: diagnostic bundle v1 parse (RFC v0.6-003)
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let _ = fjell_diag_format::parse_bundle(data);
});
