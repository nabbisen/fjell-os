//! Remote diagnostics request and response wire formats (RFC v0.8-005).
//!
//! Remote diagnostics is strictly one-directional: the fleet manager
//! sends a `RemoteDiagRequest`; the node responds with a
//! `RemoteDiagResponse` containing a signed `DiagBundle` snapshot.
//!
//! No general remote execution. The node always controls what it exports.
#![no_std]

use fjell_measure_format::Digest32;

/// What class of diagnostic data the requester wants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DiagRequestKind {
    /// Current measurement chain head and kind tallies.
    MeasurementSummary = 0x01,
    /// Current upgrade channel counters.
    ReleaseSummary     = 0x02,
    /// Recent audit bundle (bounded by max_events).
    AuditBundle        = 0x03,
    /// Boot evidence record.
    BootEvidence       = 0x04,
    /// All of the above in one request.
    FullSnapshot       = 0xFF,
}

impl DiagRequestKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::MeasurementSummary),
            0x02 => Some(Self::ReleaseSummary),
            0x03 => Some(Self::AuditBundle),
            0x04 => Some(Self::BootEvidence),
            0xFF => Some(Self::FullSnapshot),
            _    => None,
        }
    }
}

/// Remote diagnostics request.
#[derive(Clone, Copy, Debug)]
pub struct RemoteDiagRequest {
    pub schema_version: u16,
    pub fleet_id:       [u8; 16],
    pub target_node_id: [u8; 16],
    pub request_kind:   DiagRequestKind,
    /// Maximum audit events to include (0 = use default 64).
    pub max_events:     u16,
    /// Nonce to prevent replay.
    pub nonce:          [u8; 16],
    /// Ed25519 signature by the fleet anchor.
    pub signature:      [u8; 64],
    /// Canonical digest of this request.
    pub request_digest: Digest32,
}

impl RemoteDiagRequest {
    pub fn new(
        fleet_id:    [u8; 16],
        target_node: [u8; 16],
        kind:        DiagRequestKind,
        nonce:       [u8; 16],
    ) -> Self {
        Self {
            schema_version: 1,
            fleet_id,
            target_node_id: target_node,
            request_kind: kind,
            max_events: 64,
            nonce,
            signature: [0u8; 64],
            request_digest: Digest32([0u8; 32]),
        }
    }
}

/// Status of a remote diagnostics response.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DiagResponseStatus {
    Ok               = 0x00,
    SignatureInvalid  = 0x01,
    NotAuthorized    = 0x02,
    DataUnavailable  = 0x03,
    InternalError    = 0x04,
    Replay           = 0x05,
}

/// Remote diagnostics response.
#[derive(Clone, Copy, Debug)]
pub struct RemoteDiagResponse {
    pub schema_version:   u16,
    /// Echo of the request nonce.
    pub request_nonce:    [u8; 16],
    pub status:           DiagResponseStatus,
    /// Canonical digest of the response payload.
    pub response_digest:  Digest32,
    /// Ed25519 signature by the responding node's attestation key.
    pub node_signature:   [u8; 64],
    /// Inline payload (up to 512 bytes; larger payloads use chunked IPC).
    pub payload_len:      u16,
    pub payload:          [u8; 512],
}

impl RemoteDiagResponse {
    pub fn ok(nonce: [u8; 16]) -> Self {
        Self {
            schema_version:  1,
            request_nonce:   nonce,
            status:          DiagResponseStatus::Ok,
            response_digest: Digest32([0u8; 32]),
            node_signature:  [0u8; 64],
            payload_len:     0,
            payload:         [0u8; 512],
        }
    }

    pub fn error(nonce: [u8; 16], status: DiagResponseStatus) -> Self {
        let mut r = Self::ok(nonce);
        r.status = status;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diag_request_kind_roundtrip() {
        for (b, expected) in [
            (0x01u8, DiagRequestKind::MeasurementSummary),
            (0x02,   DiagRequestKind::ReleaseSummary),
            (0x03,   DiagRequestKind::AuditBundle),
            (0x04,   DiagRequestKind::BootEvidence),
            (0xFF,   DiagRequestKind::FullSnapshot),
        ] {
            assert_eq!(DiagRequestKind::from_u8(b).unwrap(), expected);
        }
        assert_eq!(DiagRequestKind::from_u8(0x05), None);
    }

    #[test]
    fn remote_diag_request_has_nonce() {
        let r = RemoteDiagRequest::new(
            [0xF1u8; 16], [0x01u8; 16],
            DiagRequestKind::FullSnapshot, [0xAAu8; 16],
        );
        assert_eq!(r.nonce, [0xAAu8; 16]);
        assert_eq!(r.max_events, 64);
    }

    #[test]
    fn remote_diag_response_ok_status() {
        let r = RemoteDiagResponse::ok([0x01u8; 16]);
        assert_eq!(r.status, DiagResponseStatus::Ok);
        assert_eq!(r.request_nonce, [0x01u8; 16]);
    }

    #[test]
    fn remote_diag_response_error_status() {
        let r = RemoteDiagResponse::error([0u8; 16], DiagResponseStatus::NotAuthorized);
        assert_eq!(r.status, DiagResponseStatus::NotAuthorized);
    }
}
