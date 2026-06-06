//! Allow-listed audit event kind tags (RFC v0.4-005 §6.1).

/// Kernel boot banner emitted by the kernel on first tick.
pub const AUDIT_KERNEL_BOOT_BANNER:             u16 = 0x0010;
/// Service manager transitioned to ready.
pub const AUDIT_SERVICE_MANAGER_READY:          u16 = 0x0020;
/// A `TrustProvider` was successfully registered.
pub const AUDIT_TRUST_PROVIDER_REGISTERED:      u16 = 0x0040;
/// A `TrustProvider` encountered a fault.
pub const AUDIT_TRUST_PROVIDER_FAULTED:         u16 = 0x0041;
/// The keyring advanced to a new active epoch.
pub const AUDIT_KEYRING_ACTIVE_EPOCH_ADVANCED:  u16 = 0x0050;
/// Upgrade state machine transitioned.
pub const AUDIT_UPGRADE_STATE_TRANSITION:       u16 = 0x0060;
/// Anti-rollback rejected a downgrade attempt.
pub const AUDIT_UPGRADE_ROLLBACK_REJECTED:      u16 = 0x0070;
/// Boot-time rollback blocked a slot.
pub const AUDIT_BOOT_ROLLBACK_BLOCKED_SLOT:     u16 = 0x0080;
/// An `AttestationRecordV2` was signed.
pub const AUDIT_ATTESTATION_RECORD_SIGNED:      u16 = 0x0090;
/// Attestation verification failed.
pub const AUDIT_ATTESTATION_VERIFY_FAILED:      u16 = 0x0091;
/// virtio-net driver encountered a fault.
pub const AUDIT_NET_DRIVER_FAULTED:             u16 = 0x00A0;
/// `secure-transportd` certificate verification failed.
pub const AUDIT_SXT_CERT_VERIFY_FAILED:         u16 = 0x00A1;
/// `secure-transportd` TLS handshake failed.
pub const AUDIT_SXT_HANDSHAKE_FAILED:           u16 = 0x00A2;
/// Device entered the recovery path.
pub const AUDIT_RECOVERY_ENTERED:               u16 = 0x00B0;

/// The complete allow-list in priority order (used by the builder).
pub const ALLOWED_AUDIT_KINDS: &[u16] = &[
    AUDIT_KERNEL_BOOT_BANNER,
    AUDIT_SERVICE_MANAGER_READY,
    AUDIT_TRUST_PROVIDER_REGISTERED,
    AUDIT_TRUST_PROVIDER_FAULTED,
    AUDIT_KEYRING_ACTIVE_EPOCH_ADVANCED,
    AUDIT_UPGRADE_STATE_TRANSITION,
    AUDIT_UPGRADE_ROLLBACK_REJECTED,
    AUDIT_BOOT_ROLLBACK_BLOCKED_SLOT,
    AUDIT_ATTESTATION_RECORD_SIGNED,
    AUDIT_ATTESTATION_VERIFY_FAILED,
    AUDIT_NET_DRIVER_FAULTED,
    AUDIT_SXT_CERT_VERIFY_FAILED,
    AUDIT_SXT_HANDSHAKE_FAILED,
    AUDIT_RECOVERY_ENTERED,
];

/// Returns `true` if the given `kind_tag` appears on the audit allow-list.
///
/// Any tag NOT on the allow-list is silently dropped by the bundle builder
/// (§6.3 redaction rule 1).
pub fn is_audit_event_allowed(kind_tag: u16) -> bool {
    ALLOWED_AUDIT_KINDS.contains(&kind_tag)
}
