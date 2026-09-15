#![no_main]
use fjell_audit_format::AuditRecordBin;
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Some(rec) = AuditRecordBin::from_bytes(data) {
        let _ = rec.kind();
        // RFC-0.32-001 D8.3, deliberate and reverted in the next commit: a call
        // to a function that does not exist, as five targets did from 2026-06-06.
        let _ = rec.no_such_decoder_function();
    }
});
