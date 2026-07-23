//! Driver state machine (RFC v0.4-001 §7.1).

use fjell_net_format::{NetDeviceState, NetMac};

/// Discrete states of the virtio-net driver task.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DriverState {
    /// Initial state: waiting for capability install from devmgr.
    Boot        = 0x00,
    /// Capabilities received; negotiating virtio features.
    Init        = 0x01,
    /// Feature negotiation complete; IRQ bound; ready to exchange packets.
    Ready       = 0x02,
    /// Processing an RX interrupt; will return to Ready after ack.
    HandleRx    = 0x03,
    /// A fault has been detected; DMA cleanup in progress.
    Faulted     = 0x04,
    /// DMA regions have been revoked and zeroized; awaiting restart decision.
    Quiesced    = 0x05,
    /// devmgr has decided not to restart; caps are withdrawn.
    Withdrawn   = 0x06,
}

/// Errors from state-machine transitions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DriverStateError {
    /// Transition not permitted from the current state.
    InvalidTransition = 0x01,
    /// Feature negotiation produced an incompatible subset.
    FeaturesMismatch  = 0x02,
    /// Device reset timed out; device is quarantined.
    ResetTimeout      = 0x03,
}

/// All mutable state held by the driver task.
#[derive(Clone, Copy, Debug)]
pub struct DriverStateBlock {
    pub state:         DriverState,
    pub mac:           NetMac,
    pub mtu:           u16,
    pub link_up:       bool,
    pub restart_count: u8,
}

impl DriverStateBlock {
    pub const MAX_RESTARTS: u8 = 3;

    pub const fn new() -> Self {
        Self {
            state:         DriverState::Boot,
            mac:           NetMac::ZERO,
            mtu:           0,
            link_up:       false,
            restart_count: 0,
        }
    }

    /// Attempt a state transition.  Returns `Err` if the transition is
    /// not permitted from the current state.
    pub fn transition(&mut self, next: DriverState) -> Result<(), DriverStateError> {
        let ok = match (self.state, next) {
            (DriverState::Boot,     DriverState::Init)     => true,
            (DriverState::Init,     DriverState::Ready)    => true,
            (DriverState::Ready,    DriverState::HandleRx) => true,
            (DriverState::HandleRx, DriverState::Ready)    => true,
            (DriverState::Ready,    DriverState::Faulted)  => true,
            (DriverState::HandleRx, DriverState::Faulted)  => true,
            (DriverState::Faulted,  DriverState::Quiesced) => true,
            (DriverState::Quiesced, DriverState::Init)     => true, // restart
            (DriverState::Quiesced, DriverState::Withdrawn)=> true,
            _ => false,
        };
        if ok { self.state = next; Ok(()) }
        else  { Err(DriverStateError::InvalidTransition) }
    }

    /// Transition to `Faulted` unconditionally (from any live state).
    pub fn fault(&mut self) {
        self.state = DriverState::Faulted;
    }

    /// Return the `NetDeviceState` corresponding to the current driver state.
    pub fn device_state(&self) -> NetDeviceState {
        match self.state {
            DriverState::Boot | DriverState::Init => NetDeviceState::Initialising,
            DriverState::Ready | DriverState::HandleRx => NetDeviceState::Ready,
            DriverState::Faulted | DriverState::Quiesced => NetDeviceState::Faulted,
            DriverState::Withdrawn => NetDeviceState::Revoked,
        }
    }

    /// Whether a restart is permitted (not exceeded `MAX_RESTARTS`).
    pub fn is_faulted(&self) -> bool {
        self.state == DriverState::Faulted
            || self.state == DriverState::Quiesced
            || self.state == DriverState::Withdrawn
    }

    pub fn may_restart(&self) -> bool {
        self.restart_count < Self::MAX_RESTARTS
    }

    /// Record a restart attempt; return `true` if restart is allowed.
    pub fn attempt_restart(&mut self) -> bool {
        if self.restart_count < Self::MAX_RESTARTS {
            self.restart_count += 1;
            true
        } else {
            false
        }
    }
}
