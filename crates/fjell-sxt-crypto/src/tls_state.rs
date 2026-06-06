//! TLS 1.3 handshake state machine (RFC v0.4-003 §6.2 / §7.1).
//!
//! Models the client-side handshake:
//!   Closed → ClientHelloSent → ServerHelloReceived →
//!   HandshakeComplete → AppData → CloseNotifySent
//!
//! This is a pure state-machine model; the wire-format parsing and crypto
//! key-schedule live above this layer.

/// TLS 1.3 client-side handshake state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TlsState {
    /// No connection established.
    Closed,
    /// ClientHello has been sent; awaiting ServerHello.
    ClientHelloSent,
    /// ServerHello received; processing EncryptedExtensions/Certificate/Finished.
    ServerHelloReceived,
    /// Handshake complete; application data may flow.
    HandshakeComplete,
    /// Application data phase.
    AppData,
    /// CloseNotify has been sent; waiting for close.
    CloseNotifySent,
    /// An error occurred; error code is embedded.
    Faulted(u16),
}

/// Errors from the TLS state machine or typed RPC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SxtError {
    /// Unknown or disallowed ChannelKind.
    UnknownKind         = 0x01,
    /// Server name not in pinned-anchor table.
    ServerNameNotPinned = 0x02,
    /// TLS handshake failed (alert from server or MAC error).
    HandshakeFailed     = 0x03,
    /// Certificate verification against pinned anchor failed.
    CertVerifyFailed    = 0x04,
    /// HTTP response failed strict-parse rules.
    HttpStrictReject    = 0x05,
    /// Channel has been closed normally.
    ChannelClosed       = 0x06,
    /// Channel is in Faulted state.
    ChannelFaulted      = 0x07,
    /// No netd Session capability available.
    NoSessionCap        = 0x08,
    /// Session capability has been revoked.
    SessionRevoked      = 0x09,
    /// HTTP response body exceeded `MAX_RESPONSE_BYTES`.
    BodyTooLarge        = 0x0A,
    /// Internal implementation error.
    Internal            = 0xFFFF,
}

/// TLS handshake sub-state tracker used during the `ClientHelloSent` phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TlsHandshakeState {
    pub tls_state:           TlsState,
    pub server_hello_done:   bool,
    pub certificate_seen:    bool,
    pub cert_verify_passed:  bool,
    pub finished_verified:   bool,
    pub anchor_epoch:        u32,
}

impl TlsHandshakeState {
    pub const fn new() -> Self {
        Self {
            tls_state:          TlsState::Closed,
            server_hello_done:  false,
            certificate_seen:   false,
            cert_verify_passed: false,
            finished_verified:  false,
            anchor_epoch:       0,
        }
    }

    /// Start the handshake: transition to ClientHelloSent.
    pub fn start(&mut self) -> Result<(), SxtError> {
        if self.tls_state != TlsState::Closed {
            return Err(SxtError::Internal);
        }
        self.tls_state = TlsState::ClientHelloSent;
        Ok(())
    }

    /// Record reception of ServerHello.
    pub fn on_server_hello(&mut self) -> Result<(), SxtError> {
        if self.tls_state != TlsState::ClientHelloSent {
            return Err(SxtError::HandshakeFailed);
        }
        self.tls_state = TlsState::ServerHelloReceived;
        self.server_hello_done = true;
        Ok(())
    }

    /// Record a validated Certificate message.
    pub fn on_certificate(&mut self) -> Result<(), SxtError> {
        if self.tls_state != TlsState::ServerHelloReceived {
            return Err(SxtError::HandshakeFailed);
        }
        self.certificate_seen = true;
        Ok(())
    }

    /// Record a passed CertVerify check.
    pub fn on_cert_verify_pass(&mut self, anchor_epoch: u32) -> Result<(), SxtError> {
        if !self.certificate_seen { return Err(SxtError::CertVerifyFailed); }
        self.cert_verify_passed = true;
        self.anchor_epoch = anchor_epoch;
        Ok(())
    }

    /// Record verification of the Finished MAC.
    pub fn on_finished(&mut self) -> Result<(), SxtError> {
        if !self.cert_verify_passed { return Err(SxtError::HandshakeFailed); }
        self.finished_verified = true;
        self.tls_state = TlsState::HandshakeComplete;
        Ok(())
    }

    /// Transition to AppData; returns Err if handshake not complete.
    pub fn enter_app_data(&mut self) -> Result<(), SxtError> {
        if self.tls_state != TlsState::HandshakeComplete {
            return Err(SxtError::HandshakeFailed);
        }
        self.tls_state = TlsState::AppData;
        Ok(())
    }

    /// Initiate close: transition to CloseNotifySent.
    pub fn close(&mut self) -> Result<(), SxtError> {
        if matches!(self.tls_state, TlsState::Faulted(_)) {
            return Err(SxtError::ChannelFaulted);
        }
        self.tls_state = TlsState::CloseNotifySent;
        Ok(())
    }

    /// Mark the channel as faulted.
    pub fn fault(&mut self, code: u16) {
        self.tls_state = TlsState::Faulted(code);
    }

    /// Whether the channel is in the AppData phase (ready for typed RPC).
    pub fn is_established(&self) -> bool {
        self.tls_state == TlsState::AppData
    }
}

// Suppress the unused import if net-format feature not enabled.
#[allow(dead_code)]
mod net {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum ChannelKind {
        UpdateMetadata = 0x01,
        Diagnostics    = 0x02,
        Attestation    = 0x03,
        FleetEnroll    = 0x04,
    }
}
