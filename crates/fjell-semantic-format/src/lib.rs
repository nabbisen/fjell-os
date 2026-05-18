//! Semantic data model for Fjell OS M5.
//!
//! IntentNode / StateNode / EventNode and supporting types implementing the
//! ABDD principle: services emit structured *meaning*, not pixels.
//! All types are `no_std`, heap-free, and fixed-capacity.

#![no_std]

// ── Capacity constants ────────────────────────────────────────────────────────
pub const MAX_TEXT_BYTES:         usize = 128;
pub const MAX_ACTIONS:            usize = 8;
pub const MAX_CONTEXT_KEYS:       usize = 4;
pub const MAX_CONSEQUENCES:       usize = 4;
pub const MAX_FACTS:              usize = 16;
pub const MAX_RELATED_NODES:      usize = 4;
pub const MAX_AFFECTED_RESOURCES: usize = 4;
pub const MAX_ACTION_INPUTS:      usize = 4;
pub const MAX_PULL_ITEMS:         usize = 8;
pub const SEMANTIC_RING_SIZE:     usize = 128;
pub const EXPORT_CHUNK_SIZE:      usize = 256;
pub const SCHEMA_VERSION:         u16   = 1;

// ── FixedVec ──────────────────────────────────────────────────────────────────

/// A heap-free, fixed-capacity vector.
#[derive(Clone, Copy, Debug)]
pub struct FixedVec<T: Copy, const N: usize> {
    items: [Option<T>; N],
    len:   usize,
}

impl<T: Copy, const N: usize> FixedVec<T, N> {
    pub const fn new() -> Self {
        Self { items: [None; N], len: 0 }
    }
    pub fn push(&mut self, v: T) -> bool {
        if self.len >= N { return false; }
        self.items[self.len] = Some(v);
        self.len += 1;
        true
    }
    pub fn get(&self, i: usize) -> Option<&T> {
        if i >= self.len { return None; }
        self.items[i].as_ref()
    }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items[..self.len].iter().filter_map(|x| x.as_ref())
    }
}

// ── TextToken ─────────────────────────────────────────────────────────────────

/// Future-proof text token: carries a stable ID for i18n/TTS alongside a
/// UTF-8 fallback that is used in M5.
#[derive(Clone, Copy, Debug)]
pub struct TextToken {
    pub id:       TextId,
    pub fallback: BoundedText,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextId(pub u32);

#[derive(Clone, Copy, Debug)]
pub struct BoundedText {
    pub len:   u16,
    pub bytes: [u8; MAX_TEXT_BYTES],
}

impl BoundedText {
    pub fn from_str(s: &str) -> Self {
        let mut bytes = [0u8; MAX_TEXT_BYTES];
        let len = s.len().min(MAX_TEXT_BYTES);
        bytes[..len].copy_from_slice(&s.as_bytes()[..len]);
        BoundedText { len: len as u16, bytes }
    }
    pub fn as_str(&self) -> &str {
        let n = self.len.min(MAX_TEXT_BYTES as u16) as usize;
        core::str::from_utf8(&self.bytes[..n]).unwrap_or("?")
    }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}

impl TextToken {
    pub fn new(s: &str) -> Self {
        TextToken { id: TextId(0), fallback: BoundedText::from_str(s) }
    }
    pub fn as_str(&self) -> &str { self.fallback.as_str() }
}

// ── Shared enums ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity { Low, Normal, Important, Critical }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status { Unknown, Ok, Degraded, Warning, Failed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult { Ok, Denied, Failed, TimedOut, NotApplicable }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Importance { Low, Normal, High, Critical }

// ── NodeId / CorrelationId ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId { pub producer_index: u16, pub local_sequence: u32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CorrelationId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionId(pub u16);

#[derive(Clone, Copy, Debug)]
pub struct ResourceName(pub BoundedText);

impl ResourceName {
    pub fn new(s: &str) -> Self { ResourceName(BoundedText::from_str(s)) }
    pub fn as_str(&self) -> &str { self.0.as_str() }
}

// ── IntentNode ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct IntentNode {
    pub kind:             IntentKind,
    pub title:            TextToken,
    pub description:      TextToken,
    pub severity:         Severity,
    pub actions:          FixedVec<ActionSpec, MAX_ACTIONS>,
    pub consequences:     FixedVec<Consequence, MAX_CONSEQUENCES>,
    pub expires_at_tick:  Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentKind {
    Information, Confirmation, Warning, ErrorRecovery,
    ActionRequest, InspectionRequest, ExportRequest,
}

#[derive(Clone, Copy, Debug)]
pub struct ActionSpec {
    pub action_id:            ActionId,
    pub label:                TextToken,
    pub kind:                 ActionKind,
    pub required_capability:  Option<CapabilityRequirement>,
    pub reversibility:        Reversibility,
    pub confirmation:         ConfirmationPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Confirm, Cancel, Retry, Inspect, Export,
    StartService, StopService, RestartService,
    ApplyConfig, RollbackConfig, RevokeLease,
}

#[derive(Clone, Copy, Debug)]
pub struct CapabilityRequirement {
    pub resource_class: BoundedText,
    pub resource_name:  ResourceName,
    pub rights:         u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reversibility { Reversible, PartiallyReversible, Irreversible, Unknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationPolicy { None, Required, RequiredForCritical }

#[derive(Clone, Copy, Debug)]
pub struct Consequence {
    pub level:   Severity,
    pub text:    TextToken,
}

// ── StateNode ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct StateNode {
    pub kind:    StateKind,
    pub title:   TextToken,
    pub summary: TextToken,
    pub status:  Status,
    pub facts:   FixedVec<StateFact, MAX_FACTS>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateKind {
    SystemOverview, ServiceGraph, ServiceStatus, ConfigStatus,
    AuditSummary, CapabilitySummary, LeaseSummary, PowerSummary,
}

#[derive(Clone, Copy, Debug)]
pub struct StateFact {
    pub key:        TextToken,
    pub value:      FactValue,
    pub importance: Importance,
}

#[derive(Clone, Copy, Debug)]
pub enum FactValue {
    Bool(bool),
    U64(u64),
    I64(i64),
    Text(TextToken),
    Ratio { numerator: u64, denominator: u64 },
}

// ── EventNode ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct EventNode {
    pub kind:              EventKind,
    pub title:             TextToken,
    pub description:       TextToken,
    pub severity:          Severity,
    pub result:            EventResult,
    pub subject:           Option<ResourceName>,
    pub related_audit_seq: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    ServiceStarted, ServiceReady, ServiceFailed,
    ConfigValidated, ConfigRejected,
    CapabilityGranted, CapabilityDenied, LeaseRevoked,
    AuditExported, ActionAccepted, ActionDenied,
    ActionCompleted, ActionFailed,
}

// ── SemanticEnvelope ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct SemanticEnvelope {
    pub schema_version:  u16,
    pub stream:          StreamKind,
    pub node_id:         NodeId,
    pub sequence:        u64,
    pub correlation_id:  Option<CorrelationId>,
    pub payload:         SemanticPayload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamKind { Intent, State, Event }

#[derive(Clone, Copy, Debug)]
pub enum SemanticPayload {
    Intent(IntentNode),
    State(StateNode),
    Event(EventNode),
}

impl SemanticEnvelope {
    pub fn new_state(node_id: NodeId, seq: u64, n: StateNode) -> Self {
        SemanticEnvelope {
            schema_version: SCHEMA_VERSION,
            stream: StreamKind::State,
            node_id, sequence: seq, correlation_id: None,
            payload: SemanticPayload::State(n),
        }
    }
    pub fn new_event(node_id: NodeId, seq: u64, n: EventNode) -> Self {
        SemanticEnvelope {
            schema_version: SCHEMA_VERSION,
            stream: StreamKind::Event,
            node_id, sequence: seq, correlation_id: None,
            payload: SemanticPayload::Event(n),
        }
    }
    pub fn new_intent(node_id: NodeId, seq: u64, n: IntentNode) -> Self {
        SemanticEnvelope {
            schema_version: SCHEMA_VERSION,
            stream: StreamKind::Intent,
            node_id, sequence: seq, correlation_id: None,
            payload: SemanticPayload::Intent(n),
        }
    }
}

// ── ActionRequest / ActionResult ──────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct ActionRequest {
    pub correlation_id: CorrelationId,
    pub source_node:    NodeId,
    pub action_id:      ActionId,
}

#[derive(Clone, Copy, Debug)]
pub struct ActionResult {
    pub correlation_id: CorrelationId,
    pub result:         EventResult,
    pub message:        TextToken,
}

// ── Schema validation ─────────────────────────────────────────────────────────

/// Validate an IntentNode against the M5 invariants.
pub fn validate_intent(n: &IntentNode) -> Result<(), &'static str> {
    use IntentKind::*;
    match n.kind {
        Confirmation | ActionRequest if n.actions.is_empty() =>
            Err("INTENT-001: Confirmation/ActionRequest must have actions"),
        _ => Ok(()),
    }
}

/// Validate a StateNode.
pub fn validate_state(n: &StateNode) -> Result<(), &'static str> {
    if n.status == Status::Failed && n.summary.fallback.is_empty() {
        return Err("STATE-003: Failed status requires non-empty summary");
    }
    Ok(())
}

// ── StreamFilter ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct StreamFilter {
    pub include_intent: bool,
    pub include_state:  bool,
    pub include_event:  bool,
    pub min_severity:   Severity,
}

impl StreamFilter {
    pub fn all() -> Self {
        StreamFilter {
            include_intent: true, include_state: true, include_event: true,
            min_severity: Severity::Low,
        }
    }
    pub fn matches(&self, env: &SemanticEnvelope) -> bool {
        match env.stream {
            StreamKind::Intent => self.include_intent,
            StreamKind::State  => self.include_state,
            StreamKind::Event  => self.include_event,
        }
    }
}

// ── ExportFormat ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat { PlainText, JsonLines, TomlSummary }

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_intent_node() {
        let mut n = IntentNode {
            kind: IntentKind::ActionRequest,
            title: TextToken::new("Test"),
            description: TextToken::new("desc"),
            severity: Severity::Normal,
            actions: FixedVec::new(),
            consequences: FixedVec::new(),
            expires_at_tick: None,
        };
        n.actions.push(ActionSpec {
            action_id: ActionId(1),
            label: TextToken::new("Confirm"),
            kind: ActionKind::Confirm,
            required_capability: None,
            reversibility: Reversibility::Reversible,
            confirmation: ConfirmationPolicy::None,
        });
        assert!(validate_intent(&n).is_ok());
    }

    #[test]
    fn invalid_empty_action_list() {
        let n = IntentNode {
            kind: IntentKind::ActionRequest,
            title: TextToken::new("Test"),
            description: TextToken::new("desc"),
            severity: Severity::Normal,
            actions: FixedVec::new(),
            consequences: FixedVec::new(),
            expires_at_tick: None,
        };
        assert!(validate_intent(&n).is_err());
    }

    #[test]
    fn state_failed_requires_summary() {
        let n = StateNode {
            kind: StateKind::ServiceStatus,
            title: TextToken::new("T"),
            summary: TextToken::new(""),
            status: Status::Failed,
            facts: FixedVec::new(),
        };
        assert!(validate_state(&n).is_err());
    }

    #[test]
    fn bounded_text_roundtrip() {
        let t = BoundedText::from_str("hello");
        assert_eq!(t.as_str(), "hello");
    }
}
