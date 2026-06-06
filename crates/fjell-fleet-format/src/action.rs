//! Fleet action wire format (RFC v0.8-004).
//!
//! All fleet operations are expressed as typed `FleetAction` messages.
//! No general remote shell. No arbitrary command execution.

use fjell_measure_format::Digest32;

/// The kind of fleet action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum FleetActionKind {
    /// Request a diagnostic snapshot from a node.
    RequestDiag        = 0x01,
    /// Initiate recovery on a node (capability-controlled).
    InitiateRecovery   = 0x02,
    /// Revoke a fleet member's roster entry.
    RevokeMember       = 0x03,
    /// Advance a rollout stage to the next.
    AdvanceRollout     = 0x04,
    /// Pause/freeze a rollout.
    PauseRollout       = 0x05,
    /// Trigger attestation collection from a node.
    CollectAttestation = 0x06,
    /// Query current node state (non-mutating).
    QueryState         = 0x07,
}

impl FleetActionKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::RequestDiag),
            0x02 => Some(Self::InitiateRecovery),
            0x03 => Some(Self::RevokeMember),
            0x04 => Some(Self::AdvanceRollout),
            0x05 => Some(Self::PauseRollout),
            0x06 => Some(Self::CollectAttestation),
            0x07 => Some(Self::QueryState),
            _    => None,
        }
    }

    /// Whether this action is state-mutating (requires stricter authorization).
    pub fn is_mutating(self) -> bool {
        !matches!(self, Self::QueryState | Self::CollectAttestation)
    }
}

/// A fleet action request.
#[derive(Clone, Copy, Debug)]
pub struct FleetAction {
    pub schema_version:    u16,
    /// The fleet this action applies to.
    pub fleet_id:          [u8; 16],
    /// The target node (all-zero = fleet-wide).
    pub target_node_id:    [u8; 16],
    pub action_kind:       FleetActionKind,
    /// Nonce to prevent replay.
    pub nonce:             [u8; 16],
    /// Ed25519 signature over the canonical encoding by the fleet anchor.
    pub signature:         [u8; 64],
    /// Canonical digest (signed content).
    pub action_digest:     Digest32,
    /// Optional payload (action-specific, up to 128 bytes).
    pub payload_len:       u8,
    pub payload:           [u8; 128],
}

impl FleetAction {
    pub fn new(
        fleet_id:    [u8; 16],
        target_node: [u8; 16],
        kind:        FleetActionKind,
        nonce:       [u8; 16],
    ) -> Self {
        Self {
            schema_version: 1,
            fleet_id,
            target_node_id: target_node,
            action_kind: kind,
            nonce,
            signature: [0u8; 64],
            action_digest: Digest32([0u8; 32]),
            payload_len: 0,
            payload: [0u8; 128],
        }
    }

    pub fn is_fleet_wide(&self) -> bool {
        self.target_node_id == [0u8; 16]
    }
}

/// Outcome of a fleet action execution.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FleetActionResult {
    /// Action completed successfully.
    Success,
    /// Action is queued for asynchronous execution.
    Queued,
    /// Action was rejected (see `FleetActionError`).
    Rejected(FleetActionError),
}

/// Why a fleet action was rejected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum FleetActionError {
    /// The action type is not permitted by the current fleet policy.
    PolicyDenied          = 0x01,
    /// The signature did not verify against the fleet anchor.
    SignatureInvalid       = 0x02,
    /// The nonce has been seen before (replay attempt).
    ReplayDetected         = 0x03,
    /// The target node is not a member of this fleet.
    UnknownTarget          = 0x04,
    /// The target node is revoked.
    TargetRevoked          = 0x05,
    /// The action requires a capability the requestor does not hold.
    InsufficientAuthority  = 0x06,
    /// Internal error in the fleet manager.
    InternalError          = 0x07,
}
