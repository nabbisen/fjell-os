//! # `fjell-config-sync`
//!
//! Reference configuration-sync service for the Fjell SDK developer
//! ecosystem trial (RFC-v0.14-002).
//!
//! Written strictly within `fjell-sdk`'s stable surface; reaching past
//! the SDK boundary is a `ci-sdk-purity` gate failure.
//!
//! ## What it does
//!
//! Watches a local configuration store, applies remote signed updates,
//! and emits a `CONFIG.DIGEST_REPORTED` semantic record whenever the
//! active configuration changes.
//!
//! ## SDK surface exercised
//!
//! | Capability | SDK item |
//! |------------|----------|
//! | Capability handles | `fjell_sdk::cap::CapHandle` |
//! | IPC message tag | `fjell_sdk::service_api::v0_7` |
//! | Semantic emit | `fjell_sdk::sdk_emit::is_known_tag` |
//! | ABI error | `fjell_sdk::abi::SysError` |
//! | SDK version | `fjell_sdk::SDK_API_REV` |

// host-testable library; no_std when built as the actual service
use fjell_sdk::{SDK_API_REV, sdk_emit};
use fjell_sdk::abi::SysError;
use fjell_sdk::cap::CapHandle;

// ── Configuration digest ──────────────────────────────────────────────────────

/// A 32-byte content hash of the current configuration blob.
#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(Debug)]
pub struct ConfigDigest(pub [u8; 32]);

impl ConfigDigest {
    /// A zeroed digest signals "no configuration applied yet."
    pub fn zero() -> Self { Self([0u8; 32]) }
    pub fn is_zero(&self) -> bool { self.0 == [0u8; 32] }

    /// Compute a digest from raw bytes using a simple FNV-based hash.
    pub fn of(bytes: &[u8]) -> Self {
        let mut h = [0u64; 4];
        let bases = [0xcbf29ce484222325u64, 0xcbf29ce484222327,
                     0xcbf29ce484222329, 0xcbf29ce484222331];
        for (i, &b) in bases.iter().enumerate() {
            let mut hv: u64 = b;
            for &byte in bytes {
                hv ^= byte as u64;
                hv = hv.wrapping_mul(0x100000001b3);
                hv ^= i as u64;
            }
            h[i] = hv;
        }
        let mut out = [0u8; 32];
        for (i, &hv) in h.iter().enumerate() {
            out[i*8..(i+1)*8].copy_from_slice(&hv.to_le_bytes());
        }
        Self(out)
    }
}

// ── In-memory configuration state ────────────────────────────────────────────

/// In-memory state of the config-sync service.
pub struct ConfigState {
    /// Digest of the most recently applied configuration.
    pub active_digest: ConfigDigest,
    /// Monotonic update counter.
    pub update_count:  u32,
    /// Whether the SDK version at compile time is compatible.
    pub sdk_compat:    bool,
}

impl ConfigState {
    pub fn new() -> Self {
        Self {
            active_digest: ConfigDigest::zero(),
            update_count:  0,
            sdk_compat:    SDK_API_REV >= 1,
        }
    }

    /// Apply a new configuration blob. Returns the new digest.
    pub fn apply_update(&mut self, blob: &[u8]) -> ConfigDigest {
        let digest = ConfigDigest::of(blob);
        self.active_digest = digest;
        self.update_count += 1;
        digest
    }

    /// Whether the service should emit `CONFIG.DIGEST_REPORTED` for this update.
    pub fn should_emit_report(&self) -> bool {
        !self.active_digest.is_zero()
            && sdk_emit::is_known_tag(0x0503)   // CONFIG.DIGEST_REPORTED
    }

    /// Whether the service should emit `CONFIG.UPDATED` after applying a blob.
    pub fn should_emit_updated(&self) -> bool {
        sdk_emit::is_known_tag(0x0501)   // CONFIG.UPDATED
    }
}

impl Default for ConfigState {
    fn default() -> Self { Self::new() }
}

// ── Message handler skeleton ──────────────────────────────────────────────────

/// The IPC message kinds this service handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ConfigIpcTag {
    /// Inbound: apply a new config blob.
    ConfigUpdate = 0xC001,
    /// Inbound: query the current digest.
    ConfigQuery  = 0xC002,
    /// Outbound: report the active digest.
    DigestReport = 0xC003,
}

/// Handle one inbound IPC word from a `CONFIG_UPDATE` or `CONFIG_QUERY`.
///
/// Returns the reply tag (high word) and a result/error code (low word).
/// Real IPC marshalling is handled by the kernel IPC path; this skeleton
/// demonstrates the handler logic using only `fjell_sdk` types.
pub fn handle_ipc(
    tag: u16,
    _cap: CapHandle,
    state: &mut ConfigState,
    blob: &[u8],
) -> Result<(u16, u32), SysError> {
    match tag {
        x if x == ConfigIpcTag::ConfigUpdate as u16 => {
            let digest = state.apply_update(blob);
            Ok((ConfigIpcTag::DigestReport as u16, digest.0[0] as u32))
        }
        x if x == ConfigIpcTag::ConfigQuery as u16 => {
            Ok((ConfigIpcTag::DigestReport as u16, state.update_count))
        }
        _ => Err(SysError::InvalidArg),
    }
}

// ── Lessons learned log (RFC-v0.14-002 §8 obligation) ────────────────────────

/// Captured lessons from authoring against the SDK (RFC-v0.14-002 §8).
/// Each entry is a brief note; see `docs/sdk/lessons-from-v0.14.md`
/// for the full record.
pub const LESSONS: &[&str] = &[
    "L1 [manifest]: `intents` field requires pre-existing catalog tags — \
     we needed to allocate 0x0501–0x0503 as new CONFIG domain tags before \
     writing the manifest.",

    "L2 [sdk_emit]: `is_known_tag` returns false for newly-allocated tags \
     until the catalog v1 snapshot is regenerated. The typed emitter API \
     (RFC-v0.14-003) makes this a compile-time error instead.",

    "L3 [handle_ipc]: The IPC tag space is shared and collisions are \
     not yet tracked; a service registry (post-v1.0) would prevent \
     `ConfigIpcTag::ConfigUpdate` from conflicting with another service.",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_digest_deterministic() {
        let d1 = ConfigDigest::of(b"my config");
        let d2 = ConfigDigest::of(b"my config");
        assert_eq!(d1, d2);
    }

    #[test]
    fn config_digest_different_inputs() {
        let d1 = ConfigDigest::of(b"config_a");
        let d2 = ConfigDigest::of(b"config_b");
        assert_ne!(d1, d2);
    }

    #[test]
    fn zero_digest() {
        assert!(ConfigDigest::zero().is_zero());
        assert!(!ConfigDigest::of(b"x").is_zero());
    }

    #[test]
    fn apply_update_advances_counter() {
        let mut s = ConfigState::new();
        assert_eq!(s.update_count, 0);
        s.apply_update(b"v1");
        assert_eq!(s.update_count, 1);
        s.apply_update(b"v2");
        assert_eq!(s.update_count, 2);
    }

    #[test]
    fn sdk_compat_check() {
        let s = ConfigState::new();
        assert!(s.sdk_compat, "SDK_API_REV must be >= 1");
    }

    #[test]
    fn handle_update_returns_digest() {
        let mut s = ConfigState::new();
        let blob = b"test config blob";
        let result = handle_ipc(ConfigIpcTag::ConfigUpdate as u16,
                                CapHandle(0), &mut s, blob);
        assert!(result.is_ok());
        let (tag, _) = result.unwrap();
        assert_eq!(tag, ConfigIpcTag::DigestReport as u16);
    }

    #[test]
    fn handle_query_returns_count() {
        let mut s = ConfigState::new();
        s.apply_update(b"blob1");
        let result = handle_ipc(ConfigIpcTag::ConfigQuery as u16,
                                CapHandle(0), &mut s, &[]);
        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn handle_unknown_tag_returns_error() {
        let mut s = ConfigState::new();
        let result = handle_ipc(0xFFFF, CapHandle(0), &mut s, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn lessons_non_empty() {
        assert!(!LESSONS.is_empty(), "SDK trial must produce at least one lesson");
    }

    #[test]
    fn sdk_emit_integration() {
        // Verify SDK's is_known_tag works for standard catalog tags
        assert!(sdk_emit::is_known_tag(0x0101), "UPDATE.STAGING_ADVANCED must be known");
        // CONFIG tags 0x0501+ are NOT in v1 catalog (allocated in v0.14)
        // This test captures the lesson L2 above
    }
}
