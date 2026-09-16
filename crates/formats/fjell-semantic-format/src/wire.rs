//! The wire form of a [`SemanticEnvelope`] (RFC-0.32-002 D1).
//!
//! Services exchange envelopes as **bytes with a defined layout**, not as a
//! view of one service's memory. Everything here is safe code: there is no
//! `unsafe` block in this module, and there is nothing for one to do.
//!
//! What that buys, concretely:
//!
//! - **An unknown tag is an error value, not a value.** Every enum is encoded
//!   as an explicit `u8` with an exhaustive mapping in both directions, so a
//!   byte that names no variant returns [`WireError::BadTag`] instead of
//!   producing a `SemanticEnvelope` whose discriminant is invalid — which is
//!   undefined behaviour before any code inspects it.
//! - **No padding crosses the boundary.** Nothing is copied out of struct
//!   memory, so no uninitialised byte of a sender's stack can be transferred
//!   (RFC-0.32-002 Finding 3).
//! - **Lengths are explicit and checked.** A `BoundedText` writes its `len`
//!   and exactly that many bytes; a `FixedVec` writes its count. A decoder
//!   refuses a length its type cannot hold, and refuses a buffer that ends
//!   early, rather than reading past it.
//! - **Truncation is detectable.** The encoding is self-delimiting: `decode`
//!   reports how many bytes it consumed, and [`decode_exact`] refuses trailing
//!   bytes, so a short or over-long transfer cannot pass as a whole message.
//!
//! The format is versioned ([`WIRE_VERSION`]) and a decoder refuses a version
//! it does not know, so a future change is a rejection rather than a
//! misreading.

use crate::*;

/// Magic prefix: `FJSE` — Fjell semantic envelope.
pub const WIRE_MAGIC: [u8; 4] = *b"FJSE";

/// Version of *this encoding*, distinct from the model's [`SCHEMA_VERSION`].
/// A decoder refuses any other value.
pub const WIRE_VERSION: u16 = 1;

/// Largest envelope this encoding can produce, for sizing a transfer buffer.
///
/// Composed from the model's own capacity constants, one term per nested
/// shape, so it cannot drift from them: raising `MAX_FACTS` or
/// `MAX_TEXT_BYTES` moves this with it. `wire::tests` encodes the widest
/// envelope the model can hold and asserts it fits.
pub const MAX_WIRE_BYTES: usize =
    HEADER_WIRE + max3(MAX_INTENT_WIRE, MAX_STATE_WIRE, MAX_EVENT_WIRE);

/// magic, wire version, schema version, stream tag, node id, sequence,
/// correlation (present), payload tag.
const HEADER_WIRE: usize = 4 + 2 + 2 + 1 + 2 + 4 + 8 + 9 + 1;
/// `len` and its bytes.
const MAX_BTEXT_WIRE: usize = 2 + MAX_TEXT_BYTES;
/// A `TextToken`: its id and its fallback text.
const MAX_TEXT_WIRE: usize = 4 + MAX_BTEXT_WIRE;
/// `resource_class`, `resource_name`, `rights`.
const MAX_CAP_WIRE: usize = MAX_BTEXT_WIRE * 2 + 4;
/// id, label, kind, capability presence and body, reversibility, confirmation.
const MAX_ACTION_WIRE: usize = 2 + MAX_TEXT_WIRE + 1 + 1 + MAX_CAP_WIRE + 1 + 1;
/// level and text.
const MAX_CONSEQUENCE_WIRE: usize = 1 + MAX_TEXT_WIRE;
/// key, value tag, the widest value (`Text`, a whole token), importance.
const MAX_FACT_WIRE: usize = MAX_TEXT_WIRE + 1 + MAX_TEXT_WIRE + 1;
const MAX_INTENT_WIRE: usize = 1
    + MAX_TEXT_WIRE * 2
    + 1
    + 1
    + MAX_ACTIONS * MAX_ACTION_WIRE
    + 1
    + MAX_CONSEQUENCES * MAX_CONSEQUENCE_WIRE
    + 9;
const MAX_STATE_WIRE: usize = 1 + MAX_TEXT_WIRE * 2 + 1 + 1 + MAX_FACTS * MAX_FACT_WIRE;
const MAX_EVENT_WIRE: usize = 1 + MAX_TEXT_WIRE * 2 + 1 + 1 + 1 + MAX_BTEXT_WIRE + 9;

const fn max3(a: usize, b: usize, c: usize) -> usize {
    let ab = if a > b { a } else { b };
    if ab > c { ab } else { c }
}

/// Why a byte string is not an envelope.
///
/// Every variant is a refusal: the decoder returns one of these instead of
/// constructing a value it cannot justify.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    /// The buffer ended before the field being read.
    UnexpectedEnd,
    /// The first four bytes are not [`WIRE_MAGIC`].
    BadMagic,
    /// The encoding version is not [`WIRE_VERSION`].
    UnsupportedVersion(u16),
    /// A tag byte names no variant of its enum.
    BadTag,
    /// A length or count exceeds what the model's fixed capacity can hold.
    TooLong,
    /// The envelope's `stream` and its payload name different variants.
    StreamPayloadMismatch,
    /// Bytes remain after a complete envelope (`decode_exact` only).
    TrailingBytes,
    /// The output buffer is too small for the encoded envelope.
    BufferTooSmall,
}

// ── writer / reader ───────────────────────────────────────────────────────────

struct Writer<'a> {
    out: &'a mut [u8],
    pos: usize,
}

impl<'a> Writer<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Writer { out, pos: 0 }
    }
    fn bytes(&mut self, b: &[u8]) -> Result<(), WireError> {
        let end = self.pos.checked_add(b.len()).ok_or(WireError::TooLong)?;
        if end > self.out.len() {
            return Err(WireError::BufferTooSmall);
        }
        self.out[self.pos..end].copy_from_slice(b);
        self.pos = end;
        Ok(())
    }
    fn u8(&mut self, v: u8) -> Result<(), WireError> {
        self.bytes(&[v])
    }
    fn u16(&mut self, v: u16) -> Result<(), WireError> {
        self.bytes(&v.to_le_bytes())
    }
    fn u32(&mut self, v: u32) -> Result<(), WireError> {
        self.bytes(&v.to_le_bytes())
    }
    fn u64(&mut self, v: u64) -> Result<(), WireError> {
        self.bytes(&v.to_le_bytes())
    }
    fn i64(&mut self, v: i64) -> Result<(), WireError> {
        self.bytes(&v.to_le_bytes())
    }
}

struct Reader<'a> {
    inp: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(inp: &'a [u8]) -> Self {
        Reader { inp, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.pos.checked_add(n).ok_or(WireError::UnexpectedEnd)?;
        if end > self.inp.len() {
            return Err(WireError::UnexpectedEnd);
        }
        let s = &self.inp[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, WireError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn bool(&mut self) -> Result<bool, WireError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(WireError::BadTag),
        }
    }
}

// ── enum tags: exhaustive, both directions ────────────────────────────────────
//
// Written as matches rather than `as u8` casts so that adding a variant to the
// model is a compile error here, not a silently unencodable value.

macro_rules! tagged {
    ($ty:ty, $to:ident, $from:ident, $( $variant:path => $tag:expr ),+ $(,)?) => {
        fn $to(v: $ty) -> u8 { match v { $( $variant => $tag, )+ } }
        fn $from(b: u8) -> Result<$ty, WireError> {
            match b { $( $tag => Ok($variant), )+ _ => Err(WireError::BadTag) }
        }
    };
}

tagged!(StreamKind, stream_to, stream_from,
    StreamKind::Intent => 1, StreamKind::State => 2, StreamKind::Event => 3);
tagged!(Severity, sev_to, sev_from,
    Severity::Low => 1, Severity::Normal => 2, Severity::Important => 3, Severity::Critical => 4);
tagged!(Status, status_to, status_from,
    Status::Unknown => 1, Status::Ok => 2, Status::Degraded => 3, Status::Warning => 4,
    Status::Failed => 5);
tagged!(EventResult, res_to, res_from,
    EventResult::Ok => 1, EventResult::Denied => 2, EventResult::Failed => 3,
    EventResult::TimedOut => 4, EventResult::NotApplicable => 5);
tagged!(Importance, imp_to, imp_from,
    Importance::Low => 1, Importance::Normal => 2, Importance::High => 3,
    Importance::Critical => 4);
tagged!(IntentKind, ik_to, ik_from,
    IntentKind::Information => 1, IntentKind::Confirmation => 2, IntentKind::Warning => 3,
    IntentKind::ErrorRecovery => 4, IntentKind::ActionRequest => 5,
    IntentKind::InspectionRequest => 6, IntentKind::ExportRequest => 7);
tagged!(ActionKind, ak_to, ak_from,
    ActionKind::Confirm => 1, ActionKind::Cancel => 2, ActionKind::Retry => 3,
    ActionKind::Inspect => 4, ActionKind::Export => 5, ActionKind::StartService => 6,
    ActionKind::StopService => 7, ActionKind::RestartService => 8, ActionKind::ApplyConfig => 9,
    ActionKind::RollbackConfig => 10, ActionKind::RevokeLease => 11,
    ActionKind::SelectRollback => 12, ActionKind::InspectSnapshots => 13,
    ActionKind::ExportDiagnostics => 14, ActionKind::GenerateAttestation => 15);
tagged!(Reversibility, rev_to, rev_from,
    Reversibility::Reversible => 1, Reversibility::PartiallyReversible => 2,
    Reversibility::Irreversible => 3, Reversibility::Unknown => 4);
tagged!(ConfirmationPolicy, conf_to, conf_from,
    ConfirmationPolicy::None => 1, ConfirmationPolicy::Required => 2,
    ConfirmationPolicy::RequiredForCritical => 3);
tagged!(StateKind, sk_to, sk_from,
    StateKind::SystemOverview => 1, StateKind::ServiceGraph => 2, StateKind::ServiceStatus => 3,
    StateKind::ConfigStatus => 4, StateKind::AuditSummary => 5, StateKind::CapabilitySummary => 6,
    StateKind::LeaseSummary => 7, StateKind::PowerSummary => 8, StateKind::EvidenceMatrix => 9,
    StateKind::SecurityAuditState => 10, StateKind::MeasurementStatus => 11,
    StateKind::AttestationStatus => 12, StateKind::BundleFreshnessStatus => 13,
    StateKind::RecoveryStatus => 14);
tagged!(EventKind, ek_to, ek_from,
    EventKind::ServiceStarted => 1, EventKind::ServiceReady => 2, EventKind::ServiceFailed => 3,
    EventKind::ConfigValidated => 4, EventKind::ConfigRejected => 5,
    EventKind::CapabilityGranted => 6, EventKind::CapabilityDenied => 7,
    EventKind::LeaseRevoked => 8, EventKind::AuditExported => 9, EventKind::ActionAccepted => 10,
    EventKind::ActionDenied => 11, EventKind::ActionCompleted => 12, EventKind::ActionFailed => 13,
    EventKind::DmaQuarantineTimeout => 14, EventKind::RollbackInitiated => 15,
    EventKind::SecurityBoundaryViolation => 16, EventKind::MeasurementAppended => 17,
    EventKind::AttestationGenerated => 18, EventKind::BundleFreshnessValid => 19,
    EventKind::BundleFreshnessRejected => 20, EventKind::RecoveryTargetEntered => 21,
    EventKind::RollbackSelected => 22);

// ── leaf types ────────────────────────────────────────────────────────────────

fn put_text(w: &mut Writer, t: &BoundedText) -> Result<(), WireError> {
    let n = t.len as usize;
    if n > MAX_TEXT_BYTES {
        return Err(WireError::TooLong);
    }
    w.u16(t.len)?;
    w.bytes(&t.bytes[..n])
}

fn get_text(r: &mut Reader) -> Result<BoundedText, WireError> {
    let len = r.u16()?;
    let n = len as usize;
    if n > MAX_TEXT_BYTES {
        return Err(WireError::TooLong);
    }
    let src = r.take(n)?;
    let mut bytes = [0u8; MAX_TEXT_BYTES];
    bytes[..n].copy_from_slice(src);
    Ok(BoundedText { len, bytes })
}

fn put_token(w: &mut Writer, t: &TextToken) -> Result<(), WireError> {
    w.u32(t.id.0)?;
    put_text(w, &t.fallback)
}

fn get_token(r: &mut Reader) -> Result<TextToken, WireError> {
    let id = TextId(r.u32()?);
    Ok(TextToken {
        id,
        fallback: get_text(r)?,
    })
}

fn put_opt_u64(w: &mut Writer, v: Option<u64>) -> Result<(), WireError> {
    match v {
        None => w.u8(0),
        Some(x) => {
            w.u8(1)?;
            w.u64(x)
        }
    }
}

fn get_opt_u64(r: &mut Reader) -> Result<Option<u64>, WireError> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some(r.u64()?)),
        _ => Err(WireError::BadTag),
    }
}

// ── payload bodies ────────────────────────────────────────────────────────────

fn put_cap(w: &mut Writer, c: &CapabilityRequirement) -> Result<(), WireError> {
    put_text(w, &c.resource_class)?;
    put_text(w, &c.resource_name.0)?;
    w.u32(c.rights)
}

fn get_cap(r: &mut Reader) -> Result<CapabilityRequirement, WireError> {
    Ok(CapabilityRequirement {
        resource_class: get_text(r)?,
        resource_name: ResourceName(get_text(r)?),
        rights: r.u32()?,
    })
}

fn put_action(w: &mut Writer, a: &ActionSpec) -> Result<(), WireError> {
    w.u16(a.action_id.0)?;
    put_token(w, &a.label)?;
    w.u8(ak_to(a.kind))?;
    match &a.required_capability {
        None => w.u8(0)?,
        Some(c) => {
            w.u8(1)?;
            put_cap(w, c)?;
        }
    }
    w.u8(rev_to(a.reversibility))?;
    w.u8(conf_to(a.confirmation))
}

fn get_action(r: &mut Reader) -> Result<ActionSpec, WireError> {
    let action_id = ActionId(r.u16()?);
    let label = get_token(r)?;
    let kind = ak_from(r.u8()?)?;
    let required_capability = match r.u8()? {
        0 => None,
        1 => Some(get_cap(r)?),
        _ => return Err(WireError::BadTag),
    };
    Ok(ActionSpec {
        action_id,
        label,
        kind,
        required_capability,
        reversibility: rev_from(r.u8()?)?,
        confirmation: conf_from(r.u8()?)?,
    })
}

fn put_intent(w: &mut Writer, n: &IntentNode) -> Result<(), WireError> {
    w.u8(ik_to(n.kind))?;
    put_token(w, &n.title)?;
    put_token(w, &n.description)?;
    w.u8(sev_to(n.severity))?;
    w.u8(n.actions.len() as u8)?;
    for a in n.actions.iter() {
        put_action(w, a)?;
    }
    w.u8(n.consequences.len() as u8)?;
    for c in n.consequences.iter() {
        w.u8(sev_to(c.level))?;
        put_token(w, &c.text)?;
    }
    put_opt_u64(w, n.expires_at_tick)
}

fn get_intent(r: &mut Reader) -> Result<IntentNode, WireError> {
    let kind = ik_from(r.u8()?)?;
    let title = get_token(r)?;
    let description = get_token(r)?;
    let severity = sev_from(r.u8()?)?;
    let n_actions = r.u8()? as usize;
    if n_actions > MAX_ACTIONS {
        return Err(WireError::TooLong);
    }
    let mut actions = FixedVec::new();
    for _ in 0..n_actions {
        actions.push(get_action(r)?);
    }
    let n_cons = r.u8()? as usize;
    if n_cons > MAX_CONSEQUENCES {
        return Err(WireError::TooLong);
    }
    let mut consequences = FixedVec::new();
    for _ in 0..n_cons {
        let level = sev_from(r.u8()?)?;
        consequences.push(Consequence {
            level,
            text: get_token(r)?,
        });
    }
    Ok(IntentNode {
        kind,
        title,
        description,
        severity,
        actions,
        consequences,
        expires_at_tick: get_opt_u64(r)?,
    })
}

fn put_fact_value(w: &mut Writer, v: &FactValue) -> Result<(), WireError> {
    match v {
        FactValue::Bool(b) => {
            w.u8(1)?;
            w.u8(u8::from(*b))
        }
        FactValue::U64(x) => {
            w.u8(2)?;
            w.u64(*x)
        }
        FactValue::I64(x) => {
            w.u8(3)?;
            w.i64(*x)
        }
        FactValue::Text(t) => {
            w.u8(4)?;
            put_token(w, t)
        }
        FactValue::Ratio {
            numerator,
            denominator,
        } => {
            w.u8(5)?;
            w.u64(*numerator)?;
            w.u64(*denominator)
        }
    }
}

fn get_fact_value(r: &mut Reader) -> Result<FactValue, WireError> {
    match r.u8()? {
        1 => Ok(FactValue::Bool(r.bool()?)),
        2 => Ok(FactValue::U64(r.u64()?)),
        3 => Ok(FactValue::I64(r.i64()?)),
        4 => Ok(FactValue::Text(get_token(r)?)),
        5 => Ok(FactValue::Ratio {
            numerator: r.u64()?,
            denominator: r.u64()?,
        }),
        _ => Err(WireError::BadTag),
    }
}

fn put_state(w: &mut Writer, n: &StateNode) -> Result<(), WireError> {
    w.u8(sk_to(n.kind))?;
    put_token(w, &n.title)?;
    put_token(w, &n.summary)?;
    w.u8(status_to(n.status))?;
    w.u8(n.facts.len() as u8)?;
    for f in n.facts.iter() {
        put_token(w, &f.key)?;
        put_fact_value(w, &f.value)?;
        w.u8(imp_to(f.importance))?;
    }
    Ok(())
}

fn get_state(r: &mut Reader) -> Result<StateNode, WireError> {
    let kind = sk_from(r.u8()?)?;
    let title = get_token(r)?;
    let summary = get_token(r)?;
    let status = status_from(r.u8()?)?;
    let n_facts = r.u8()? as usize;
    if n_facts > MAX_FACTS {
        return Err(WireError::TooLong);
    }
    let mut facts = FixedVec::new();
    for _ in 0..n_facts {
        let key = get_token(r)?;
        let value = get_fact_value(r)?;
        facts.push(StateFact {
            key,
            value,
            importance: imp_from(r.u8()?)?,
        });
    }
    Ok(StateNode {
        kind,
        title,
        summary,
        status,
        facts,
    })
}

fn put_event(w: &mut Writer, n: &EventNode) -> Result<(), WireError> {
    w.u8(ek_to(n.kind))?;
    put_token(w, &n.title)?;
    put_token(w, &n.description)?;
    w.u8(sev_to(n.severity))?;
    w.u8(res_to(n.result))?;
    match &n.subject {
        None => w.u8(0)?,
        Some(s) => {
            w.u8(1)?;
            put_text(w, &s.0)?;
        }
    }
    put_opt_u64(w, n.related_audit_seq)
}

fn get_event(r: &mut Reader) -> Result<EventNode, WireError> {
    let kind = ek_from(r.u8()?)?;
    let title = get_token(r)?;
    let description = get_token(r)?;
    let severity = sev_from(r.u8()?)?;
    let result = res_from(r.u8()?)?;
    let subject = match r.u8()? {
        0 => None,
        1 => Some(ResourceName(get_text(r)?)),
        _ => return Err(WireError::BadTag),
    };
    Ok(EventNode {
        kind,
        title,
        description,
        severity,
        result,
        subject,
        related_audit_seq: get_opt_u64(r)?,
    })
}

// ── envelope ──────────────────────────────────────────────────────────────────

/// Encode `env` into `out`, returning the number of bytes written.
///
/// Fails with [`WireError::BufferTooSmall`] rather than truncating.
pub fn encode(env: &SemanticEnvelope, out: &mut [u8]) -> Result<usize, WireError> {
    let mut w = Writer::new(out);
    w.bytes(&WIRE_MAGIC)?;
    w.u16(WIRE_VERSION)?;
    w.u16(env.schema_version)?;
    w.u8(stream_to(env.stream))?;
    w.u16(env.node_id.producer_index)?;
    w.u32(env.node_id.local_sequence)?;
    w.u64(env.sequence)?;
    put_opt_u64(&mut w, env.correlation_id.map(|c| c.0))?;
    match &env.payload {
        SemanticPayload::Intent(n) => {
            w.u8(1)?;
            put_intent(&mut w, n)?;
        }
        SemanticPayload::State(n) => {
            w.u8(2)?;
            put_state(&mut w, n)?;
        }
        SemanticPayload::Event(n) => {
            w.u8(3)?;
            put_event(&mut w, n)?;
        }
    }
    Ok(w.pos)
}

/// Decode an envelope from the front of `inp`, returning it and the number of
/// bytes consumed. Trailing bytes are the caller's to judge; see
/// [`decode_exact`].
pub fn decode(inp: &[u8]) -> Result<(SemanticEnvelope, usize), WireError> {
    let mut r = Reader::new(inp);
    if r.take(4)? != WIRE_MAGIC {
        return Err(WireError::BadMagic);
    }
    let wire_version = r.u16()?;
    if wire_version != WIRE_VERSION {
        return Err(WireError::UnsupportedVersion(wire_version));
    }
    let schema_version = r.u16()?;
    let stream = stream_from(r.u8()?)?;
    let node_id = NodeId {
        producer_index: r.u16()?,
        local_sequence: r.u32()?,
    };
    let sequence = r.u64()?;
    let correlation_id = get_opt_u64(&mut r)?.map(CorrelationId);
    let payload = match r.u8()? {
        1 => SemanticPayload::Intent(get_intent(&mut r)?),
        2 => SemanticPayload::State(get_state(&mut r)?),
        3 => SemanticPayload::Event(get_event(&mut r)?),
        _ => return Err(WireError::BadTag),
    };
    // The envelope names its stream twice; a message that disagrees with
    // itself is refused rather than silently resolved in favour of one.
    let agrees = matches!(
        (stream, &payload),
        (StreamKind::Intent, SemanticPayload::Intent(_))
            | (StreamKind::State, SemanticPayload::State(_))
            | (StreamKind::Event, SemanticPayload::Event(_))
    );
    if !agrees {
        return Err(WireError::StreamPayloadMismatch);
    }
    Ok((
        SemanticEnvelope {
            schema_version,
            stream,
            node_id,
            sequence,
            correlation_id,
            payload,
        },
        r.pos,
    ))
}

/// Decode an envelope that must be the whole of `inp`.
///
/// This is what a receiver wants: a transfer carrying more bytes than the
/// envelope needs is a framing error, not a message with something appended.
pub fn decode_exact(inp: &[u8]) -> Result<SemanticEnvelope, WireError> {
    let (env, used) = decode(inp)?;
    if used != inp.len() {
        return Err(WireError::TrailingBytes);
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_intent() -> SemanticEnvelope {
        let mut actions = FixedVec::new();
        actions.push(ActionSpec {
            action_id: ActionId(1),
            label: TextToken::new("confirm"),
            kind: ActionKind::Confirm,
            required_capability: Some(CapabilityRequirement {
                resource_class: BoundedText::from_str("service"),
                resource_name: ResourceName::new("svc.sample"),
                rights: 0b11,
            }),
            reversibility: Reversibility::Reversible,
            confirmation: ConfirmationPolicy::Required,
        });
        let mut consequences = FixedVec::new();
        consequences.push(Consequence {
            level: Severity::Normal,
            text: TextToken::new("nothing irreversible happens"),
        });
        SemanticEnvelope::new_intent(
            NodeId {
                producer_index: 6,
                local_sequence: 1,
            },
            1,
            IntentNode {
                kind: IntentKind::ActionRequest,
                title: TextToken::new("sample-service demo intent"),
                description: TextToken::new("demonstrates the ABDD live path"),
                severity: Severity::Normal,
                actions,
                consequences,
                expires_at_tick: Some(9_000),
            },
        )
    }

    fn sample_state() -> SemanticEnvelope {
        let mut facts = FixedVec::new();
        facts.push(StateFact {
            key: TextToken::new("uptime"),
            value: FactValue::U64(42),
            importance: Importance::Normal,
        });
        facts.push(StateFact {
            key: TextToken::new("healthy"),
            value: FactValue::Bool(true),
            importance: Importance::High,
        });
        facts.push(StateFact {
            key: TextToken::new("drift"),
            value: FactValue::I64(-7),
            importance: Importance::Low,
        });
        facts.push(StateFact {
            key: TextToken::new("name"),
            value: FactValue::Text(TextToken::new("store")),
            importance: Importance::Low,
        });
        facts.push(StateFact {
            key: TextToken::new("used"),
            value: FactValue::Ratio {
                numerator: 3,
                denominator: 4,
            },
            importance: Importance::Critical,
        });
        SemanticEnvelope::new_state(
            NodeId {
                producer_index: 2,
                local_sequence: 9,
            },
            7,
            StateNode {
                kind: StateKind::ServiceStatus,
                title: TextToken::new("services"),
                summary: TextToken::new("all running"),
                status: Status::Ok,
                facts,
            },
        )
    }

    fn sample_event() -> SemanticEnvelope {
        SemanticEnvelope::new_event(
            NodeId {
                producer_index: 3,
                local_sequence: 4,
            },
            11,
            EventNode {
                kind: EventKind::ActionAccepted,
                title: TextToken::new("action accepted"),
                description: TextToken::new(""),
                severity: Severity::Important,
                result: EventResult::Ok,
                subject: Some(ResourceName::new("svc.sample")),
                related_audit_seq: Some(1234),
            },
        )
    }

    fn roundtrip(env: &SemanticEnvelope) -> (SemanticEnvelope, usize) {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(env, &mut buf).expect("encode");
        (decode_exact(&buf[..n]).expect("decode"), n)
    }

    #[test]
    fn intent_round_trips() {
        let env = sample_intent();
        let (back, n) = roundtrip(&env);
        assert_eq!(back.schema_version, env.schema_version);
        assert_eq!(back.stream, StreamKind::Intent);
        assert_eq!(back.node_id, env.node_id);
        assert_eq!(back.sequence, env.sequence);
        let SemanticPayload::Intent(a) = &env.payload else {
            panic!()
        };
        let SemanticPayload::Intent(b) = &back.payload else {
            panic!("variant changed")
        };
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.title.as_str(), b.title.as_str());
        assert_eq!(a.description.as_str(), b.description.as_str());
        assert_eq!(a.actions.len(), b.actions.len());
        let (x, y) = (a.actions.get(0).unwrap(), b.actions.get(0).unwrap());
        assert_eq!(x.action_id, y.action_id);
        assert_eq!(x.label.as_str(), y.label.as_str());
        assert_eq!(x.kind, y.kind);
        assert_eq!(
            x.required_capability.unwrap().rights,
            y.required_capability.unwrap().rights
        );
        assert_eq!(
            x.required_capability.unwrap().resource_name.as_str(),
            y.required_capability.unwrap().resource_name.as_str()
        );
        assert_eq!(a.consequences.len(), b.consequences.len());
        assert_eq!(a.expires_at_tick, b.expires_at_tick);
        // The size this costs on the wire, against 4936 bytes of struct memory.
        assert!(n < 400, "encoded intent is {n} bytes");
    }

    #[test]
    fn state_round_trips_including_every_fact_value() {
        let env = sample_state();
        let (back, _) = roundtrip(&env);
        let SemanticPayload::State(a) = &env.payload else {
            panic!()
        };
        let SemanticPayload::State(b) = &back.payload else {
            panic!("variant changed")
        };
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.status, b.status);
        assert_eq!(a.facts.len(), b.facts.len());
        for (x, y) in a.facts.iter().zip(b.facts.iter()) {
            assert_eq!(x.key.as_str(), y.key.as_str());
            assert_eq!(x.importance, y.importance);
            match (&x.value, &y.value) {
                (FactValue::Bool(p), FactValue::Bool(q)) => assert_eq!(p, q),
                (FactValue::U64(p), FactValue::U64(q)) => assert_eq!(p, q),
                (FactValue::I64(p), FactValue::I64(q)) => assert_eq!(p, q),
                (FactValue::Text(p), FactValue::Text(q)) => assert_eq!(p.as_str(), q.as_str()),
                (
                    FactValue::Ratio {
                        numerator: p,
                        denominator: pd,
                    },
                    FactValue::Ratio {
                        numerator: q,
                        denominator: qd,
                    },
                ) => {
                    assert_eq!(p, q);
                    assert_eq!(pd, qd)
                }
                _ => panic!("fact value variant changed"),
            }
        }
    }

    #[test]
    fn event_round_trips() {
        let env = sample_event();
        let (back, _) = roundtrip(&env);
        let SemanticPayload::Event(a) = &env.payload else {
            panic!()
        };
        let SemanticPayload::Event(b) = &back.payload else {
            panic!("variant changed")
        };
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.result, b.result);
        assert_eq!(a.subject.unwrap().as_str(), b.subject.unwrap().as_str());
        assert_eq!(a.related_audit_seq, b.related_audit_seq);
    }

    #[test]
    fn empty_input_is_refused() {
        assert_eq!(decode_exact(&[]).unwrap_err(), WireError::UnexpectedEnd);
    }

    #[test]
    fn bad_magic_is_refused() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_intent(), &mut buf).unwrap();
        buf[0] = b'X';
        assert_eq!(decode_exact(&buf[..n]).unwrap_err(), WireError::BadMagic);
    }

    #[test]
    fn unknown_wire_version_is_refused_by_value() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_intent(), &mut buf).unwrap();
        buf[4..6].copy_from_slice(&99u16.to_le_bytes());
        assert_eq!(
            decode_exact(&buf[..n]).unwrap_err(),
            WireError::UnsupportedVersion(99)
        );
    }

    #[test]
    fn unknown_stream_tag_is_an_error_not_a_discriminant() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_intent(), &mut buf).unwrap();
        buf[8] = 0xFF; // stream tag
        assert_eq!(decode_exact(&buf[..n]).unwrap_err(), WireError::BadTag);
    }

    #[test]
    fn unknown_payload_tag_is_refused() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_intent(), &mut buf).unwrap();
        // payload tag sits after magic(4) version(2) schema(2) stream(1)
        // node_id(6) sequence(8) correlation(1 for None, 9 for Some)
        let payload_tag_at = 4 + 2 + 2 + 1 + 6 + 8 + 1;
        assert_eq!(
            buf[payload_tag_at], 1,
            "expected the Intent payload tag here"
        );
        buf[payload_tag_at] = 7;
        assert_eq!(decode_exact(&buf[..n]).unwrap_err(), WireError::BadTag);
    }

    #[test]
    fn stream_and_payload_must_agree() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_intent(), &mut buf).unwrap();
        buf[8] = 2; // claim State while carrying an Intent
        assert_eq!(
            decode_exact(&buf[..n]).unwrap_err(),
            WireError::StreamPayloadMismatch
        );
    }

    #[test]
    fn truncation_at_every_length_is_refused_never_accepted() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_intent(), &mut buf).unwrap();
        for cut in 0..n {
            match decode_exact(&buf[..cut]) {
                Err(_) => {}
                Ok(_) => panic!("a {cut}-byte prefix of a {n}-byte envelope decoded"),
            }
        }
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_intent(), &mut buf).unwrap();
        assert_eq!(
            decode_exact(&buf[..n + 1]).unwrap_err(),
            WireError::TrailingBytes
        );
        // ...and `decode` reports the length so a caller can tell.
        let (_, used) = decode(&buf[..n + 1]).unwrap();
        assert_eq!(used, n);
    }

    #[test]
    fn a_text_longer_than_capacity_is_refused() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_event(), &mut buf).unwrap();
        // title's length field: after magic..correlation(1) + payload tag(1) + kind(1) + id(4)
        let at = 4 + 2 + 2 + 1 + 6 + 8 + 1 + 1 + 1 + 4;
        buf[at..at + 2].copy_from_slice(&((MAX_TEXT_BYTES + 1) as u16).to_le_bytes());
        assert_eq!(decode_exact(&buf[..n]).unwrap_err(), WireError::TooLong);
    }

    #[test]
    fn more_facts_than_capacity_is_refused() {
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&sample_state(), &mut buf).unwrap();
        let mut i = 4 + 2 + 2 + 1 + 6 + 8 + 1 + 1 + 1; // .. through payload tag and kind
        i += 4 + 2 + "services".len(); // title
        i += 4 + 2 + "all running".len(); // summary
        i += 1; // status
        assert_eq!(buf[i], 5, "expected the fact count here");
        buf[i] = (MAX_FACTS + 1) as u8;
        assert_eq!(decode_exact(&buf[..n]).unwrap_err(), WireError::TooLong);
    }

    #[test]
    fn a_buffer_too_small_is_refused_not_truncated() {
        let mut small = [0u8; 16];
        assert_eq!(
            encode(&sample_intent(), &mut small).unwrap_err(),
            WireError::BufferTooSmall
        );
    }

    #[test]
    fn a_zero_filled_buffer_is_refused() {
        // The stale-buffer case (RFC-0.32-002 Finding 2): today this decodes
        // into a well-formed, fabricated envelope. Here it is an error.
        let zero = [0u8; 4960];
        assert_eq!(decode_exact(&zero).unwrap_err(), WireError::BadMagic);
    }

    #[test]
    fn an_arbitrary_byte_pattern_is_refused() {
        let ff = [0xFFu8; 4960];
        assert!(decode_exact(&ff).is_err());
    }

    #[test]
    fn max_wire_bytes_is_large_enough_for_the_widest_envelope() {
        let mut facts = FixedVec::new();
        for _ in 0..MAX_FACTS {
            facts.push(StateFact {
                key: TextToken::new(core::str::from_utf8(&[b'k'; MAX_TEXT_BYTES]).unwrap()),
                value: FactValue::Text(TextToken::new(
                    core::str::from_utf8(&[b'v'; MAX_TEXT_BYTES]).unwrap(),
                )),
                importance: Importance::Critical,
            });
        }
        let env = SemanticEnvelope::new_state(
            NodeId {
                producer_index: u16::MAX,
                local_sequence: u32::MAX,
            },
            u64::MAX,
            StateNode {
                kind: StateKind::RecoveryStatus,
                title: TextToken::new(core::str::from_utf8(&[b't'; MAX_TEXT_BYTES]).unwrap()),
                summary: TextToken::new(core::str::from_utf8(&[b's'; MAX_TEXT_BYTES]).unwrap()),
                status: Status::Failed,
                facts,
            },
        );
        let mut buf = [0u8; MAX_WIRE_BYTES];
        let n = encode(&env, &mut buf).expect("the widest envelope must fit MAX_WIRE_BYTES");
        assert!(n <= MAX_WIRE_BYTES);
        assert_eq!(decode_exact(&buf[..n]).unwrap().sequence, u64::MAX);
    }
}
