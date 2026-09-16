// Fuzz target: the SemanticEnvelope wire decoder (RFC-0.32-002 D1/R7).
//
// This is the decoder on the live cross-service path — the bytes
// fjell-sample-service sends to fjell-semantic-stream, and that
// semantic-stream re-encodes to fjell-proxy-text. Before RFC-0.32-002 those
// bytes were reinterpreted as a struct (`chunked::reassemble`), which
// RFC-0.32-001 D7 deliberately left unfuzzed because fuzzing undefined
// behaviour only rediscovers it. It is now safe code that returns Result,
// so it can be fuzzed for what it does rather than what it is.
#![no_main]
use libfuzzer_sys::fuzz_target;

use fjell_semantic_format::wire;

fuzz_target!(|data: &[u8]| {
    // 1. Whatever the bytes, decoding is an error value or a envelope — never
    //    a panic, and never undefined behaviour.
    let Ok((envelope, used)) = wire::decode(data) else {
        // Anything decode rejects, decode_exact must also reject.
        assert!(wire::decode_exact(data).is_err());
        return;
    };
    assert!(used <= data.len());

    // 2. The encoding is canonical: anything that decodes re-encodes to
    //    exactly the bytes it was decoded from. If two byte strings could
    //    decode to the same value, one of them is a second encoding of a
    //    message, which is the ambiguity a framing check cannot see.
    let mut out = [0u8; wire::MAX_WIRE_BYTES];
    let n = wire::encode(&envelope, &mut out).expect("a decoded envelope must re-encode");
    assert_eq!(n, used, "re-encoding changed the length");
    assert_eq!(&out[..n], &data[..used], "re-encoding changed the bytes");

    // 3. decode_exact agrees with decode about trailing bytes.
    if used == data.len() {
        assert!(wire::decode_exact(data).is_ok());
    } else {
        assert!(wire::decode_exact(data).is_err());
    }
});
