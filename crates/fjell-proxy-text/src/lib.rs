//! Text rendering for semantic nodes — reusable across crates.
#![no_std]

pub use fjell_semantic_format::*;

fn w(s: &str) { fjell_syscall::sys_debug_write(s); }
fn wln(s: &str) { fjell_syscall::sys_debug_writeln(s); }

fn sev(s: Severity) -> &'static str {
    match s { Severity::Low=>"Low", Severity::Normal=>"Normal",
              Severity::Important=>"Important", Severity::Critical=>"Critical" }
}
fn stat(s: Status) -> &'static str {
    match s { Status::Unknown=>"Unknown", Status::Ok=>"Ok",
              Status::Degraded=>"Degraded", Status::Warning=>"Warning",
              Status::Failed=>"Failed" }
}
fn res(r: EventResult) -> &'static str {
    match r { EventResult::Ok=>"Ok", EventResult::Denied=>"Denied",
              EventResult::Failed=>"Failed", EventResult::TimedOut=>"TimedOut",
              EventResult::NotApplicable=>"N/A" }
}
fn u64_str(mut n: u64, buf: &mut [u8; 20]) -> &str {
    if n == 0 { buf[0]=b'0'; return core::str::from_utf8(&buf[..1]).unwrap(); }
    let mut i = 20;
    while n > 0 { i -= 1; buf[i] = b'0' + (n%10) as u8; n /= 10; }
    core::str::from_utf8(&buf[i..]).unwrap_or("?")
}

pub fn render_state(n: &StateNode) {
    w("[STATE]["); w(stat(n.status)); w("] "); wln(n.title.as_str());
    if !n.summary.fallback.is_empty() { wln("Summary:"); w("  "); wln(n.summary.as_str()); }
    if !n.facts.is_empty() {
        wln("Facts:");
        let mut buf = [0u8; 20];
        for f in n.facts.iter() {
            w("  "); w(f.key.as_str()); w(" = ");
            match f.value {
                FactValue::U64(v) => wln(u64_str(v, &mut buf)),
                FactValue::Bool(v) => wln(if v {"true"} else {"false"}),
                FactValue::Text(t) => wln(t.as_str()),
                _ => wln("?"),
            }
        }
    }
    wln("");
}

pub fn render_event(n: &EventNode) {
    w("[EVENT]["); w(sev(n.severity)); w("]["); w(res(n.result)); w("] "); wln(n.title.as_str());
    if !n.description.fallback.is_empty() {
        wln("Description:"); w("  "); wln(n.description.as_str());
    }
    if let Some(seq) = n.related_audit_seq {
        let mut buf = [0u8; 20];
        wln("Audit:"); w("  seq = "); wln(u64_str(seq, &mut buf));
    }
    wln("");
}

/// Render an IntentNode and return the selected ActionId (SmokeScenario: always [1]).
pub fn render_intent(n: &IntentNode) -> Option<ActionId> {
    w("[INTENT]["); w(sev(n.severity)); w("] "); wln(n.title.as_str());
    if !n.description.fallback.is_empty() {
        wln("Description:"); w("  "); wln(n.description.as_str());
    }
    if !n.consequences.is_empty() {
        wln("Consequences:");
        for c in n.consequences.iter() { w("  - "); wln(c.text.as_str()); }
    }
    if !n.actions.is_empty() {
        wln("Actions:");
        let mut buf = [0u8; 20];
        let mut idx = 1u64;
        for a in n.actions.iter() {
            w("  ["); w(u64_str(idx, &mut buf)); w("] "); wln(a.label.as_str());
            idx += 1;
        }
        wln("");
        return n.actions.get(0).map(|a| a.action_id);
    }
    wln("");
    None
}


// ── M8 semantic node constructors ─────────────────────────────────────────────

pub fn render_measurement_status(seq: u64, dropped: u64) {
    let mut n = StateNode {
        kind:    StateKind::MeasurementStatus,
        title:   TextToken::new("Measurement status"),
        summary: TextToken::new("measurement chain active"),
        status:  Status::Ok,
        facts:   FixedVec::new(),
    };
    let _ = n.facts.push(StateFact {
        key: TextToken::new("measurement.seq"), value: FactValue::U64(seq), importance: Importance::Normal,
    });
    let _ = n.facts.push(StateFact {
        key: TextToken::new("measurement.dropped"), value: FactValue::U64(dropped), importance: Importance::Normal,
    });
    render_state(&n);
}

pub fn render_attestation_status() {
    let mut n = StateNode {
        kind:    StateKind::AttestationStatus,
        title:   TextToken::new("Attestation status"),
        summary: TextToken::new("local attestation record generated"),
        status:  Status::Ok,
        facts:   FixedVec::new(),
    };
    let _ = n.facts.push(StateFact {
        key: TextToken::new("profile"), value: FactValue::Text(TextToken::new("fjell.local.v1")), importance: Importance::Normal,
    });
    let _ = n.facts.push(StateFact {
        key: TextToken::new("signature"), value: FactValue::Text(TextToken::new("development-ed25519")), importance: Importance::Normal,
    });
    render_state(&n);
}

pub fn render_freshness_status(status_ok: bool, generation: u64, key_epoch: u64) {
    let mut n = StateNode {
        kind:    StateKind::BundleFreshnessStatus,
        title:   TextToken::new("Bundle freshness"),
        summary: if status_ok { TextToken::new("bundle metadata valid") } else { TextToken::new("bundle rejected") },
        status:  if status_ok { Status::Ok } else { Status::Failed },
        facts:   FixedVec::new(),
    };
    let _ = n.facts.push(StateFact { key: TextToken::new("generation"), value: FactValue::U64(generation), importance: Importance::Normal });
    let _ = n.facts.push(StateFact { key: TextToken::new("key_epoch"),  value: FactValue::U64(key_epoch),  importance: Importance::Normal });
    let _ = n.facts.push(StateFact {
        key: TextToken::new("status"),
        value: FactValue::Text(if status_ok { TextToken::new("valid") } else { TextToken::new("rejected") }),
        importance: Importance::Normal,
    });
    render_state(&n);
}

pub fn render_recovery_status(snapshots: u64) {
    let mut n = StateNode {
        kind:    StateKind::RecoveryStatus,
        title:   TextToken::new("Recovery status"),
        summary: TextToken::new("recovery target available"),
        status:  Status::Ok,
        facts:   FixedVec::new(),
    };
    let _ = n.facts.push(StateFact {
        key: TextToken::new("recovery_target"), value: FactValue::Bool(true), importance: Importance::Normal,
    });
    let _ = n.facts.push(StateFact {
        key: TextToken::new("snapshots_available"), value: FactValue::U64(snapshots), importance: Importance::Normal,
    });
    render_state(&n);
}

pub fn render_recovery_intent() {
    let mut n = IntentNode {
        kind:         IntentKind::ActionRequest,
        severity:     Severity::Important,
        title:        TextToken::new("Recovery action available"),
        description:  TextToken::new("Manual rollback or diagnostics available"),
        consequences: FixedVec::new(),
        actions:      FixedVec::new(),
        expires_at_tick: None,
    };
    let _ = n.consequences.push(Consequence {
        level: Severity::Important,
        text:  TextToken::new("Rollback boots the last confirmed slot"),
    });
    let _ = n.actions.push(ActionSpec {
        action_id:           ActionId(1),
        label:               TextToken::new("Inspect snapshots"),
        kind:                ActionKind::InspectSnapshots,
        required_capability: None,
        reversibility:       Reversibility::Reversible,
        confirmation:        ConfirmationPolicy::None,
    });
    let _ = n.actions.push(ActionSpec {
        action_id:           ActionId(2),
        label:               TextToken::new("Select rollback"),
        kind:                ActionKind::SelectRollback,
        required_capability: None,
        reversibility:       Reversibility::Irreversible,
        confirmation:        ConfirmationPolicy::Required,
    });
    let _ = render_intent(&n);
}

pub fn render_freshness_rejected_event() {
    let n = EventNode {
        kind:              EventKind::BundleFreshnessRejected,
        severity:          Severity::Important,
        result:            EventResult::Failed,
        title:             TextToken::new("Bundle freshness rejected"),
        description:       TextToken::new("candidate bundle rejected as stale"),
        subject:           None,
        related_audit_seq: None,
    };
    render_event(&n);
}

pub fn render_rollback_selected_event() {
    let n = EventNode {
        kind:              EventKind::RollbackSelected,
        severity:          Severity::Important,
        result:            EventResult::Ok,
        title:             TextToken::new("Rollback preserved last confirmed slot"),
        description:       TextToken::new("rollback selected to last confirmed slot"),
        subject:           None,
        related_audit_seq: None,
    };
    render_event(&n);
}
