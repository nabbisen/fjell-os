// Fuzz target: update index parse (RFC v0.6-003)
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    // Stub: fjell-upgrade-format index parser (full parser in v0.7).
    let _ = data.len();
});
