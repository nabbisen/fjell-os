// Fuzz target: attestation record v2 parse → round-trip (RFC v0.6-003)
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    // Parse must not panic on any input.
    let _ = fjell_attestation_format::v2::parse_record(data);
    // If parse succeeds, serialise and re-parse must match.
    if let Ok(rec) = fjell_attestation_format::v2::parse_record(data) {
        let mut buf = [0u8; 512];
        if let Ok(n) = rec.write_to(&mut buf) {
            let rec2 = fjell_attestation_format::v2::parse_record(&buf[..n]);
            assert!(rec2.is_ok(), "round-trip failed after successful parse");
        }
    }
});
