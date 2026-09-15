#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Ok(m) = fjell_cap_manifest::parse_manifest(text) {
            let _ = fjell_cap_manifest::lint_manifest(&m, 1);
            let _ = fjell_cap_manifest::manifest_digest(&m);
        }
    }
});
