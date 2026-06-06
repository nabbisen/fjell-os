# RFC-v0.7.3-002: Crypto Profile Documentation and Production-Mode Gate

**Status.** Implemented (v0.7.1)

## Status

Draft (closes review finding **C-H-01**)

## Target Version

`v0.7.3`

## Summary

Reclassify `fjell-sxt-crypto` as a development/reference profile by
default; require an explicit `crypto-profile-development` feature flag
to enable it; document the constant-time and HKDF limitations
accurately; fix the silent HKDF info truncation; and add a CI gate
that prevents release builds from enabling the development crypto
profile.

## Motivation

The crates review §6 H-01 documented specific concerns about
`fjell-sxt-crypto`:

```text
- AES implementation claims "table-free, constant-time reference
  implementation" but uses a 256-byte S-box table and data-dependent
  indexing → cache-timing leakage on cache-bearing targets.

- HKDF expand uses assert!(n <= 255) → panic instead of Result on
  long output requests.

- HKDF expand's fixed round buffer silently truncates info if it
  exceeds ROUND_BUF_MAX:

      let end = (pos + part.len()).min(ROUND_BUF_MAX);
      buf[pos..end].copy_from_slice(&part[..end - pos]);
      pos = (pos + part.len()).min(ROUND_BUF_MAX);

  This is a context-collision risk.
```

The v0.7.0 handoff acknowledged hand-rolled crypto as a risk and
flagged replacement for v0.9+.  This RFC takes the intermediate step:
mark the current crate as development-profile, fix the silent
truncation immediately, and gate release builds.

## Goals

```text
- fjell-sxt-crypto's public API requires a development feature flag.
- Constant-time claims are removed unless they are actually true on
  the target platform.
- HKDF expand returns Result on excessive output or info length.
- HKDF expand never silently truncates info.
- Release-build CI rejects any binary that links fjell-sxt-crypto in
  default-feature mode.
- A clear migration plan to a vetted no_std crypto crate is filed
  (target: v0.9.0).
```

## Non-Goals

```text
- Not replacing the crypto implementation in this RFC. Replacement
  is a v0.9 deliverable (RFC to follow).
- Not adding new cipher suites.
- Not formally verifying constant-time behaviour.
```

## External Design

### Feature-gated public API

```toml
# crates/fjell-sxt-crypto/Cargo.toml
[features]
default = []
# WARNING: Enables the development crypto profile.
# Release builds MUST NOT enable this feature.
# Migration path: v0.9 replaces this crate with a vetted no_std lib.
crypto-profile-development = []
```

The crate's `lib.rs` becomes:

```rust
#![no_std]
#![cfg_attr(not(feature = "crypto-profile-development"),
            doc = "## NOT ENABLED")]

#[cfg(not(feature = "crypto-profile-development"))]
compile_error!(
    "fjell-sxt-crypto requires the crypto-profile-development feature.  \
     This crate is a development/reference profile and is NOT \
     suitable for production cryptographic use.  See \
     docs/src/security/crypto-profile.md."
);

#[cfg(feature = "crypto-profile-development")]
pub mod aes128;
#[cfg(feature = "crypto-profile-development")]
pub mod x25519;
// etc.
```

Consumers (`secure-transportd`, etc.) must add the feature explicitly
in their `Cargo.toml`.  This forces every consumer to acknowledge the
development-profile status.

### Documentation strings

In `aes128.rs`:

```rust
//! AES-128 reference implementation.
//!
//! Status: DEVELOPMENT PROFILE.  This implementation uses a 256-byte
//! S-box with data-dependent indexing; on cache-bearing targets, this
//! is potentially vulnerable to cache-timing side-channel attacks.
//!
//! Do not use in production.  See RFC-v0.7.3-002 and
//! docs/src/security/crypto-profile.md.
```

In `hkdf.rs`:

```rust
//! HKDF-SHA256 (RFC 5869).
//!
//! Status: DEVELOPMENT PROFILE.  See aes128.rs disclaimer.
//!
//! Output length is limited to 255 * HASH_LEN bytes per RFC 5869 §2.3.
//! This implementation now RETURNS an error instead of panicking on
//! oversized output (RFC-v0.7.3-002).  Info length is unbounded; the
//! round buffer grows as needed.
```

### HKDF correctness fixes

```rust
pub fn hkdf_expand(
    prk:  &[u8; 32],
    info: &[u8],
    out:  &mut [u8],
) -> Result<(), HkdfError> {
    let n_blocks = (out.len() + 31) / 32;
    if n_blocks > 255 {
        return Err(HkdfError::OutputTooLong);
    }

    let mut t_prev: [u8; 32] = [0; 32];
    let mut t_len = 0usize;

    for i in 1..=n_blocks {
        let mut mac = HmacSha256::new(prk);
        if t_len > 0 {
            mac.update(&t_prev[..t_len]);
        }
        mac.update(info);          // info written in full — no truncation
        mac.update(&[i as u8]);
        let t = mac.finalize();

        let take = core::cmp::min(32, out.len() - (i - 1) * 32);
        out[(i - 1) * 32..(i - 1) * 32 + take].copy_from_slice(&t[..take]);

        t_prev = t;
        t_len  = 32;
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HkdfError {
    OutputTooLong = 0x01,
}
```

The fixed `ROUND_BUF_MAX` is removed.  Info is streamed into HMAC
directly, eliminating both the panic and the silent truncation.

### Release-build gate

```bash
# ci-no-crypto-development-in-release
cargo build --release --no-default-features
ldd target/release/* | grep -v "linux-vdso\|libc\|ld-linux" \
  | xargs -I{} sh -c "nm {} 2>/dev/null | grep -i sxt_crypto" \
  && exit 1 || exit 0

# For workspace members:
cargo metadata --format-version=1 \
  | jq -r '.packages[] | select(.name == "fjell-sxt-crypto") | .features | keys[]' \
  | grep crypto-profile-development \
  && {
    cargo build --release --message-format=json \
      | jq -r 'select(.features?) | .features[]' \
      | grep crypto-profile-development \
      && exit 1
  }
exit 0
```

The exact form depends on the build system; the principle is: detect
feature activation in release builds and fail.

## Data Model

### `HkdfError`

```rust
#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HkdfError {
    OutputTooLong = 0x01,
}
```

This replaces the existing `assert!` panic site.

## Internal Design

### Migration path (v0.9 target)

`docs/src/security/crypto-roadmap.md`:

```text
v0.7.3:  development-profile gate added; HKDF fixed
v0.7.x:  no further changes to fjell-sxt-crypto
v0.8.0:  evaluate vetted no_std crypto candidates:
         - RustCrypto's aes, x25519-dalek-ng, hkdf
         - dalek-cryptography ed25519-dalek
         - Mundane (Google)
v0.9.0:  RFC-v0.9-XXX replaces fjell-sxt-crypto with the chosen lib.
         fjell-sxt-crypto becomes a thin re-export for backward
         compatibility, then deprecated.
v1.0.0:  fjell-sxt-crypto removed.
```

### Audit-time tests

```text
HKDF:LONG_INFO_REJECTED_NOT_TRUNCATED    (host unit test)
HKDF:TOO_LONG_OUTPUT_RETURNS_ERROR       (host unit test)
AES:CONSTANT_TIME_CLAIM_AUDIT_GATE       (doc-link test; verifies
                                          the crate's doc references
                                          the crypto-roadmap.md)
SXT:DISABLED_IN_PRODUCTION_WITH_DEV_CRYPTO  (CI gate)
```

## Security Design

The threat being addressed here is not active exploitation — it is
**false confidence**.  Marketing hand-rolled crypto as
"constant-time reference implementation" can mislead reviewers,
downstream consumers, and the v0.8 fleet-operations decision-makers.

Making the development status structurally visible (feature flag,
compile_error, documentation, CI gate) ensures the next reviewer
cannot accidentally treat this as production-grade.

## Memory / Resource Design

Removing the fixed `ROUND_BUF_MAX` removes a stack buffer.  HMAC's
streaming `update()` does not require buffered info.

## Compatibility and Migration

- Every downstream crate of `fjell-sxt-crypto` must add
  `features = ["crypto-profile-development"]` to its dependency.
  Internal callers (`secure-transportd`, `attestd`) are updated as
  part of this RFC.
- `hkdf_expand` signature changes from `pub fn hkdf_expand(prk, info, out)`
  (returning `()`) to returning `Result<(), HkdfError>`.  Callers must
  handle the error.

## Test Strategy

```text
- hkdf_expand(out.len() > 255 * 32) returns OutputTooLong
- hkdf_expand with info.len() = 65535 produces full info into HMAC
  (verified against an RFC 5869 test vector with long info)
- compile_error fires if a downstream crate does not enable the
  feature
- ci-no-crypto-development-in-release gate fires on a release build
  with the feature accidentally enabled
```

## Acceptance Criteria

```text
- fjell-sxt-crypto compile-errors without the development feature.
- HKDF:LONG_INFO_REJECTED_NOT_TRUNCATED test passes.
- HKDF:TOO_LONG_OUTPUT_RETURNS_ERROR test passes.
- SXT:DISABLED_IN_PRODUCTION_WITH_DEV_CRYPTO CI gate is green.
- docs/src/security/crypto-roadmap.md exists.
- ADR-v0.7.3-002 filed.
```

## Documentation Requirements

```text
- docs/src/security/crypto-profile.md — current state, risks, threat
  model.
- docs/src/security/crypto-roadmap.md — migration plan to v0.9.
- README.md gains a "Security Profile" section linking to the above.
- Every Cargo.toml that depends on fjell-sxt-crypto carries a
  comment near the dependency: "DEV PROFILE — see crypto-roadmap.md".
```

## Open Questions

```text
1. Should we ship the v0.9 RFC alongside this one? Proposal: yes, as
   a draft, so the migration target is visible. Leave it as Draft
   status until the candidate library is chosen.

2. Should the dev-profile compile_error also fire on cargo test?
   Proposal: yes — running tests with the development crypto is a
   conscious choice that should require the same feature flag.

3. What about the AES-NI assumption on hosts? Proposal: not relevant
   here; the development profile is the table-based reference. The
   v0.9 replacement chooses its own ISA strategy.
```

## Release Gate

```text
- compile_error fires on a default-feature build
- HKDF tests pass
- CI gate confirms no release artifact links the development feature
- ADR-v0.7.3-002 accepted
```
