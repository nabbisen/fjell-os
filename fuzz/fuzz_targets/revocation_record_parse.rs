#![no_main]
use fjell_keyring::revocation::RevocationRecord;
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Some(rec) = RevocationRecord::from_bytes(data) {
        let wire = rec.to_bytes();
        let again = RevocationRecord::from_bytes(&wire).expect("a re-encoded record must decode");
        assert_eq!(again.to_bytes(), wire, "decode(encode(x)) must be stable");
    }
});
