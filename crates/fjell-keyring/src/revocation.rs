//! Key revocation records and lifecycle state machine — RFC-v0.11-004.
//!
//! Every `TrustAnchor` progresses through:
//!
//! ```text
//! Active(N) ──advance──► Retired(N)
//!            ──revoke──► Revoked(N, reason)
//! Retired(N) ──revoke──► Revoked(N, reason)
//! ```
//!
//! Revoked is terminal. A verifier receiving a bundle signed under a
//! revoked key rejects it with `SigError::SignatureVerifyFailed`
//! after checking the revocation table.

use crate::anchor::TrustAnchor;
use crate::epoch::KeyEpoch;

/// Lifecycle state of a trust anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AnchorState {
    /// Key is current. Grants may be issued. Wire value 0x01.
    Active  = 0x01,
    /// Key has been superseded by a newer epoch. Bundles signed with it
    /// are accepted within the `grace_window_secs` period. Wire 0x02.
    Retired = 0x02,
    /// Key is permanently revoked. Wire 0x03.
    Revoked = 0x03,
}

impl AnchorState {
    /// Parse from wire byte. Returns `None` on unknown tag.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Active),
            0x02 => Some(Self::Retired),
            0x03 => Some(Self::Revoked),
            _    => None,
        }
    }
}

/// Reason code for a revocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum RevocationReason {
    Compromised = 1,
    Rotated     = 2,
    Lost        = 3,
    Ceremony    = 4,
}

impl RevocationReason {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            1 => Some(Self::Compromised),
            2 => Some(Self::Rotated),
            3 => Some(Self::Lost),
            4 => Some(Self::Ceremony),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Compromised => "compromised",
            Self::Rotated     => "rotated",
            Self::Lost        => "lost",
            Self::Ceremony    => "ceremony",
        }
    }
}

/// A signed revocation record (RFC-v0.11-004 §3).
///
/// Wire layout (42 bytes without signature; 106 with):
/// ```text
/// magic:        [u8; 4]   "FREV"
/// schema:       u16       = 1
/// key_id:       [u8; 16]
/// epoch:        u32
/// reason_code:  u16
/// revoked_at:   u64       (wall-clock ns, advisory)
/// signer_key:   [u8; 16]  (key-id of the revoking authority)
/// signature:    [u8; 64]  (Ed25519 over the preceding 42 bytes)
/// ```
#[derive(Clone)]
pub struct RevocationRecord {
    pub key_id:      [u8; 16],
    pub epoch:       KeyEpoch,
    pub reason:      RevocationReason,
    /// Wall-clock nanoseconds at revocation time (advisory; not
    /// relied upon for security decisions).
    pub revoked_at_ns: u64,
    /// Key-id of the authority that signed this record.
    pub signer_key:  [u8; 16],
    /// Ed25519 signature over the canonical header fields.
    pub signature:   [u8; 64],
}

impl RevocationRecord {
    /// Canonical magic.
    pub const MAGIC: &'static [u8; 4] = b"FREV";
    /// Wire schema version.
    pub const SCHEMA: u16 = 1;
    /// Total wire length: 4+2+16+4+2+8+16+64 = 116 bytes.
    pub const WIRE_LEN: usize = 116;

    /// Serialize to a 106-byte buffer.
    pub fn to_bytes(&self) -> [u8; Self::WIRE_LEN] {
        let mut out = [0u8; Self::WIRE_LEN];
        out[0..4].copy_from_slice(Self::MAGIC);
        out[4..6].copy_from_slice(&Self::SCHEMA.to_le_bytes());
        out[6..22].copy_from_slice(&self.key_id);
        out[22..26].copy_from_slice(&self.epoch.0.to_le_bytes());
        out[26..28].copy_from_slice(&(self.reason as u16).to_le_bytes());
        out[28..36].copy_from_slice(&self.revoked_at_ns.to_le_bytes());
        out[36..52].copy_from_slice(&self.signer_key);
        out[52..].copy_from_slice(&self.signature);
        out
    }

    /// Parse from bytes. Returns `None` on any structural error.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::WIRE_LEN { return None; }
        if &bytes[0..4] != Self::MAGIC  { return None; }
        let schema = u16::from_le_bytes(bytes[4..6].try_into().ok()?);
        if schema != Self::SCHEMA { return None; }
        let key_id:     [u8; 16] = bytes[6..22].try_into().ok()?;
        let epoch_raw    = u32::from_le_bytes(bytes[22..26].try_into().ok()?);
        let reason_raw   = u16::from_le_bytes(bytes[26..28].try_into().ok()?);
        let revoked_at   = u64::from_le_bytes(bytes[28..36].try_into().ok()?);
        let signer_key: [u8; 16] = bytes[36..52].try_into().ok()?;
        let signature:  [u8; 64] = bytes[52..116].try_into().ok()?;
        Some(Self {
            key_id,
            epoch: KeyEpoch(epoch_raw),
            reason: RevocationReason::from_u16(reason_raw)?,
            revoked_at_ns: revoked_at,
            signer_key,
            signature,
        })
    }

    /// Build the canonical byte prefix that is signed (header only,
    /// 52 bytes: magic through signer_key, excluding the signature).
    pub fn signed_bytes(&self) -> [u8; 52] {
        let full = self.to_bytes();
        full[..52].try_into().unwrap()
    }
}

/// Maximum number of tracked anchors in the revocation table.
pub const MAX_ANCHORS: usize = 64;

/// In-memory table of anchor lifecycle states.
///
/// Maintained by the keyring; verifiers consult this before accepting
/// a bundle signature (RFC-v0.11-004 §4).
pub struct RevocationTable {
    entries: [Option<RevocationEntry>; MAX_ANCHORS],
    len: usize,
}

impl Default for RevocationTable {
    fn default() -> Self { Self::new() }
}

#[derive(Clone)]
struct RevocationEntry {
    key_id: [u8; 16],
    epoch:  KeyEpoch,
    state:  AnchorState,
    /// Most recent revocation record, if state == Revoked.
    record: Option<RevocationRecord>,
}

impl RevocationTable {
    pub const fn new() -> Self {
        Self { entries: [const { None }; MAX_ANCHORS], len: 0 }
    }

    /// Register an anchor in Active state when it is first installed.
    pub fn activate(&mut self, key_id: [u8; 16], epoch: KeyEpoch) {
        if self.find(key_id, epoch).is_none() && self.len < MAX_ANCHORS {
            self.entries[self.len] = Some(RevocationEntry {
                key_id, epoch,
                state: AnchorState::Active,
                record: None,
            });
            self.len += 1;
        }
    }

    /// Advance an Active anchor to Retired (superseded by a newer epoch).
    /// Returns `false` if the anchor is not found or is already Revoked.
    pub fn retire(&mut self, key_id: [u8; 16], epoch: KeyEpoch) -> bool {
        if let Some(e) = self.find_mut(key_id, epoch) {
            if e.state == AnchorState::Active {
                e.state = AnchorState::Retired;
                return true;
            }
        }
        false
    }

    /// Revoke an anchor (Active or Retired → Revoked, terminal).
    /// Returns `false` if the anchor is already Revoked or not found.
    pub fn revoke(&mut self, record: RevocationRecord) -> bool {
        if let Some(e) = self.find_mut(record.key_id, record.epoch) {
            if e.state != AnchorState::Revoked {
                e.state = AnchorState::Revoked;
                e.record = Some(record);
                return true;
            }
        }
        false
    }

    /// Query the current state of an anchor.
    pub fn state_of(&self, key_id: [u8; 16], epoch: KeyEpoch) -> Option<AnchorState> {
        self.find(key_id, epoch).map(|e| e.state)
    }

    /// Return the revocation record for a revoked anchor, if available.
    pub fn revocation_record(
        &self,
        key_id: [u8; 16],
        epoch: KeyEpoch,
    ) -> Option<&RevocationRecord> {
        self.find(key_id, epoch).and_then(|e| e.record.as_ref())
    }

    fn find(&self, key_id: [u8; 16], epoch: KeyEpoch) -> Option<&RevocationEntry> {
        self.entries[..self.len].iter()
            .filter_map(|e| e.as_ref())
            .find(|e| e.key_id == key_id && e.epoch == epoch)
    }

    fn find_mut(&mut self, key_id: [u8; 16], epoch: KeyEpoch) -> Option<&mut RevocationEntry> {
        let len = self.len;
        self.entries[..len].iter_mut()
            .filter_map(|e| e.as_mut())
            .find(|e| e.key_id == key_id && e.epoch == epoch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: [u8; 16] = [0xAA; 16];
    const EP1: KeyEpoch = KeyEpoch(1);
    const EP2: KeyEpoch = KeyEpoch(2);

    fn dummy_record(key_id: [u8; 16], epoch: KeyEpoch) -> RevocationRecord {
        RevocationRecord {
            key_id, epoch,
            reason: RevocationReason::Rotated,
            revoked_at_ns: 0,
            signer_key: [0xBB; 16],
            signature: [0u8; 64],
        }
    }

    #[test]
    fn activate_and_query() {
        let mut t = RevocationTable::new();
        t.activate(KEY_A, EP1);
        assert_eq!(t.state_of(KEY_A, EP1), Some(AnchorState::Active));
    }

    #[test]
    fn retire_transitions() {
        let mut t = RevocationTable::new();
        t.activate(KEY_A, EP1);
        assert!(t.retire(KEY_A, EP1));
        assert_eq!(t.state_of(KEY_A, EP1), Some(AnchorState::Retired));
    }

    #[test]
    fn revoke_from_active() {
        let mut t = RevocationTable::new();
        t.activate(KEY_A, EP1);
        assert!(t.revoke(dummy_record(KEY_A, EP1)));
        assert_eq!(t.state_of(KEY_A, EP1), Some(AnchorState::Revoked));
    }

    #[test]
    fn revoke_idempotent() {
        let mut t = RevocationTable::new();
        t.activate(KEY_A, EP1);
        t.revoke(dummy_record(KEY_A, EP1));
        // Second revoke returns false (already revoked)
        assert!(!t.revoke(dummy_record(KEY_A, EP1)));
    }

    #[test]
    fn retire_after_revoke_fails() {
        let mut t = RevocationTable::new();
        t.activate(KEY_A, EP1);
        t.revoke(dummy_record(KEY_A, EP1));
        assert!(!t.retire(KEY_A, EP1));
    }

    #[test]
    fn multiple_epochs_independent() {
        let mut t = RevocationTable::new();
        t.activate(KEY_A, EP1);
        t.activate(KEY_A, EP2);
        t.revoke(dummy_record(KEY_A, EP1));
        assert_eq!(t.state_of(KEY_A, EP1), Some(AnchorState::Revoked));
        assert_eq!(t.state_of(KEY_A, EP2), Some(AnchorState::Active));
    }

    #[test]
    fn revocation_record_round_trip() {
        let r = dummy_record(KEY_A, EP1);
        let bytes = r.to_bytes();
        let back = RevocationRecord::from_bytes(&bytes).expect("parse");
        assert_eq!(back.key_id, r.key_id);
        assert_eq!(back.epoch, r.epoch);
        assert!(matches!(back.reason, RevocationReason::Rotated));
    }

    #[test]
    fn revocation_record_bad_magic_rejected() {
        let r = dummy_record(KEY_A, EP1);
        let mut bytes = r.to_bytes();
        bytes[0] = 0xFF;
        assert!(RevocationRecord::from_bytes(&bytes).is_none());
    }

    #[test]
    fn anchor_state_wire_round_trip() {
        for s in [AnchorState::Active, AnchorState::Retired, AnchorState::Revoked] {
            assert_eq!(AnchorState::from_u8(s as u8), Some(s));
        }
        assert!(AnchorState::from_u8(0x00).is_none());
    }
}
