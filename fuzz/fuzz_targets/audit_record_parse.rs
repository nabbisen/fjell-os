#![no_main]
use fjell_audit_format::AuditRecordBin;
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Some(rec) = AuditRecordBin::from_bytes(data) {
        let _ = rec.kind();
    }
});
