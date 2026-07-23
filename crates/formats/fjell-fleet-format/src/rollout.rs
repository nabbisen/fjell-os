//! Fleet rollout plan wire format (RFC v0.8-003).
//!
//! A `FleetRolloutPlan` describes how a new release is staged across the fleet:
//! canary → regional → full. No node in a later stage receives the update
//! before earlier stages have confirmed.

use fjell_measure_format::Digest32;

pub const ROLLOUT_SCHEMA_VERSION: u16 = 1;
pub const MAX_ROLLOUT_STAGES: usize = 8;

/// Rollout strategy controlling how a stage advances.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RolloutStrategy {
    /// Advance immediately after all nodes in the stage confirm.
    AllConfirmed    = 0x01,
    /// Advance when at least `quorum_pct` percent confirm.
    Quorum          = 0x02,
    /// Advance only on explicit operator approval via fleet intent.
    ManualApproval  = 0x03,
    /// Never advance automatically (requires explicit override).
    Frozen          = 0x04,
}

impl RolloutStrategy {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::AllConfirmed),
            0x02 => Some(Self::Quorum),
            0x03 => Some(Self::ManualApproval),
            0x04 => Some(Self::Frozen),
            _    => None,
        }
    }
}

/// State of one rollout stage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum StageState {
    /// Waiting to begin.
    Pending     = 0x00,
    /// Nodes in this stage are receiving the update.
    Active      = 0x01,
    /// All nodes in this stage have confirmed.
    Confirmed   = 0x02,
    /// One or more nodes in this stage have failed; rollout paused.
    Failed      = 0x03,
    /// Stage was skipped (e.g., no nodes match the filter).
    Skipped     = 0x04,
}

/// One stage in a rollout plan.
#[derive(Clone, Copy, Debug)]
pub struct RolloutStage {
    /// Human-readable stage name (e.g. "canary", "regional-eu", "full").
    pub name:           [u8; 16],
    /// Filter: node profile tag bitmask (0 = all profiles).
    pub profile_filter: u8,
    /// How many nodes must be targeted (0 = all matching).
    pub target_count:   u16,
    /// Strategy for advancing past this stage.
    pub strategy:       RolloutStrategy,
    /// Minimum soak time in seconds before advancing.
    pub soak_seconds:   u32,
    /// Current stage state.
    pub state:          StageState,
    /// Number of nodes that have confirmed in this stage.
    pub confirmed:      u16,
    /// Number of nodes that have failed in this stage.
    pub failed:         u16,
}

impl RolloutStage {
    pub fn new(name: &[u8], strategy: RolloutStrategy) -> Self {
        let mut n = [0u8; 16];
        for (i, &b) in name.iter().enumerate().take(15) { n[i] = b; }
        Self {
            name: n,
            profile_filter: 0,
            target_count:   0,
            strategy,
            soak_seconds:   0,
            state:          StageState::Pending,
            confirmed:      0,
            failed:         0,
        }
    }

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(16);
        core::str::from_utf8(&self.name[..end]).unwrap_or("<invalid>")
    }

    /// Whether this stage should advance to the next.
    pub fn should_advance(&self) -> bool {
        match self.state {
            StageState::Confirmed | StageState::Skipped => true,
            _ => false,
        }
    }
}

/// Fleet-wide staged rollout plan.
#[derive(Clone, Debug)]
pub struct FleetRolloutPlan {
    pub schema_version:     u16,
    pub fleet_id:           [u8; 16],
    /// Identifier of the candidate release being rolled out.
    pub candidate_id:       u32,
    /// Digest of the release metadata being deployed.
    pub release_digest:     Digest32,
    /// Canonical digest (computed by `rollout_digest`).
    pub plan_digest:        Digest32,
    pub stage_count:        u8,
    pub stages:             [Option<RolloutStage>; MAX_ROLLOUT_STAGES],
    /// Index of the currently active stage (0-based).
    pub active_stage:       u8,
}

impl FleetRolloutPlan {
    pub fn new(fleet_id: [u8; 16], candidate_id: u32, release_digest: Digest32) -> Self {
        Self {
            schema_version: ROLLOUT_SCHEMA_VERSION,
            fleet_id,
            candidate_id,
            release_digest,
            plan_digest: Digest32([0u8; 32]),
            stage_count: 0,
            stages: [const { None }; MAX_ROLLOUT_STAGES],
            active_stage: 0,
        }
    }

    /// Append a rollout stage.
    pub fn add_stage(&mut self, stage: RolloutStage) -> Result<(), RolloutError> {
        if self.stage_count as usize >= MAX_ROLLOUT_STAGES {
            return Err(RolloutError::CapacityExhausted);
        }
        self.stages[self.stage_count as usize] = Some(stage);
        self.stage_count += 1;
        Ok(())
    }

    /// The currently active stage (if any).
    pub fn active(&self) -> Option<&RolloutStage> {
        if self.active_stage as usize >= self.stage_count as usize {
            return None;
        }
        self.stages[self.active_stage as usize].as_ref()
    }

    /// Advance to the next stage if the current one should advance.
    pub fn try_advance(&mut self) -> bool {
        let cur = self.active_stage as usize;
        if cur >= self.stage_count as usize { return false; }
        if let Some(stage) = &self.stages[cur] {
            if stage.should_advance() && cur + 1 < self.stage_count as usize {
                self.active_stage += 1;
                return true;
            }
        }
        false
    }

    /// Mark the active stage as confirmed.
    pub fn confirm_active_stage(&mut self) -> Result<(), RolloutError> {
        let cur = self.active_stage as usize;
        if let Some(stage) = self.stages.get_mut(cur).and_then(|s| s.as_mut()) {
            stage.state = StageState::Confirmed;
            Ok(())
        } else {
            Err(RolloutError::StageNotFound)
        }
    }
}

/// Typed error for rollout operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RolloutError {
    CapacityExhausted = 0x01,
    StageNotFound     = 0x02,
    InvalidStrategy   = 0x03,
    AlreadyActive     = 0x04,
}
