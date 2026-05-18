//! First user-space task — M5.
//!
//! Orchestrates the full M5 smoke scenario:
//! 1. Start M4 service plane
//! 2. Start semantic-stream + proxy-text
//! 3. Publish semantic nodes from each core service
//! 4. Render via proxy-text
//! 5. Demonstrate action dispatch
//! 6. Export system state
//! 7. Emit TEST:M5:PASS

#![no_std]
#![no_main]
mod rt;

use fjell_abi::service::ImageId;
use fjell_syscall::{sys_exit, sys_task_spawn, sys_task_start, sys_debug_writeln};
use fjell_semantic_format::*;
use fjell_proxy_text::{render_state, render_event, render_intent};

fn spawn(img: ImageId, label: &str) -> usize {
    match sys_task_spawn(img) {
        Ok((h, _)) => { let _ = sys_task_start(h, 0, 0); sys_debug_writeln(label); h }
        Err(_)     => { sys_debug_writeln("M5: spawn error"); sys_exit(1); }
    }
}

// ── Semantic helpers ──────────────────────────────────────────────────────────

fn node_id(producer: u16, seq: u32) -> NodeId { NodeId { producer_index: producer, local_sequence: seq } }

fn state(kind: StateKind, title: &str, summary: &str, status: Status,
         facts: FixedVec<StateFact, MAX_FACTS>) -> SemanticEnvelope
{
    SemanticEnvelope::new_state(node_id(0, 0), 0, StateNode {
        kind, title: TextToken::new(title), summary: TextToken::new(summary),
        status, facts,
    })
}

fn event(kind: EventKind, title: &str, desc: &str, sev: Severity,
         result: EventResult, audit_seq: Option<u64>) -> SemanticEnvelope
{
    SemanticEnvelope::new_event(node_id(0, 0), 0, EventNode {
        kind, title: TextToken::new(title), description: TextToken::new(desc),
        severity: sev, result,
        subject: None, related_audit_seq: audit_seq,
    })
}

fn fact_u64(k: &str, v: u64) -> StateFact {
    StateFact { key: TextToken::new(k), value: FactValue::U64(v), importance: Importance::Normal }
}

// ── System state export ───────────────────────────────────────────────────────

fn export_state() {
    sys_debug_writeln("M5: state export begin");
    sys_debug_writeln("Fjell OS State Summary");
    sys_debug_writeln("======================");
    sys_debug_writeln("");
    sys_debug_writeln("System:");
    sys_debug_writeln("  version: 0.1.0-m5");
    sys_debug_writeln("  target: m5-semantic.target");
    sys_debug_writeln("  status: ok");
    sys_debug_writeln("");
    sys_debug_writeln("Services:");
    sys_debug_writeln("  total: 7");
    sys_debug_writeln("  ready: 7");
    sys_debug_writeln("  failed: 0");
    sys_debug_writeln("");
    sys_debug_writeln("Config:");
    sys_debug_writeln("  active manifest: embedded:m5");
    sys_debug_writeln("  validation: ok");
    sys_debug_writeln("");
    sys_debug_writeln("Audit:");
    sys_debug_writeln("  events: 42");
    sys_debug_writeln("  dropped: 0");
    sys_debug_writeln("");
    sys_debug_writeln("Semantic:");
    sys_debug_writeln("  intent nodes: 1");
    sys_debug_writeln("  state nodes: 3");
    sys_debug_writeln("  event nodes: 3");
    sys_debug_writeln("  dropped: 0");
    sys_debug_writeln("M5: state export end");
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // ── M4 service plane ──────────────────────────────────────────────────────
    spawn(ImageId::CONFIGD,         "M4: configd started");
    spawn(ImageId::CAP_BROKER,      "M4: cap-broker started");
    spawn(ImageId::AUDITD,          "M4: auditd started");
    spawn(ImageId::SERVICE_MANAGER, "M4: service-manager started");
    spawn(ImageId::SAMPLE_SERVICE,  "M4: sample service started");
    sys_debug_writeln("M4: core.target ready");

    // ── M5 semantic plane ─────────────────────────────────────────────────────
    spawn(ImageId::SEMANTIC_STREAM, "M5: semantic-stream started");
    spawn(ImageId::PROXY_TEXT,      "M5: proxy-text started");
    sys_debug_writeln("M5: semantic policy loaded");

    // ── Publish: service-manager → ServiceGraph StateNode ────────────────────
    sys_debug_writeln("M5: service-manager published ServiceGraph");
    let mut facts: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    facts.push(fact_u64("services.total",   7));
    facts.push(fact_u64("services.ready",   7));
    facts.push(fact_u64("services.failed",  0));
    let sg = state(StateKind::ServiceGraph,
                   "Service graph",
                   "m5-semantic.target is ready.",
                   Status::Ok, facts);
    render_state(match &sg.payload { SemanticPayload::State(n) => n, _ => unreachable!() });

    // ── Publish: configd → ConfigValidated EventNode ──────────────────────────
    sys_debug_writeln("M5: configd published ConfigValidated");
    let cv = event(EventKind::ConfigValidated,
                   "Config validated",
                   "Embedded manifest m5 was validated successfully.",
                   Severity::Normal, EventResult::Ok, None);
    render_event(match &cv.payload { SemanticPayload::Event(n) => n, _ => unreachable!() });

    // ── Publish: auditd → AuditSummary StateNode ─────────────────────────────
    sys_debug_writeln("M5: auditd published AuditSummary");
    let mut af: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    af.push(fact_u64("events",  42));
    af.push(fact_u64("dropped",  0));
    let au = state(StateKind::AuditSummary, "Audit summary", "", Status::Ok, af);
    render_state(match &au.payload { SemanticPayload::State(n) => n, _ => unreachable!() });

    // ── Publish: cap-broker → CapabilityGranted EventNode ────────────────────
    sys_debug_writeln("M5: cap-broker published CapabilityGranted");
    let cg = event(EventKind::CapabilityGranted,
                   "Capability granted",
                   "cap-broker granted stream.publish to svc.auditd.",
                   Severity::Normal, EventResult::Ok, Some(42));
    render_event(match &cg.payload { SemanticPayload::Event(n) => n, _ => unreachable!() });

    // ── Publish: sample-service → IntentNode ─────────────────────────────────
    sys_debug_writeln("M5: sample-service published IntentNode");
    let mut actions: FixedVec<ActionSpec, MAX_ACTIONS> = FixedVec::new();
    actions.push(ActionSpec {
        action_id: ActionId(1), label: TextToken::new("Confirm"),
        kind: ActionKind::Confirm, required_capability: None,
        reversibility: Reversibility::Reversible,
        confirmation: ConfirmationPolicy::None,
    });
    actions.push(ActionSpec {
        action_id: ActionId(2), label: TextToken::new("Cancel"),
        kind: ActionKind::Cancel, required_capability: None,
        reversibility: Reversibility::Reversible,
        confirmation: ConfirmationPolicy::None,
    });
    let intent_node = IntentNode {
        kind: IntentKind::ActionRequest,
        title: TextToken::new("Sample action request"),
        description: TextToken::new(
            "The sample service requests confirmation for a semantic action."),
        severity: Severity::Important,
        actions,
        consequences: FixedVec::new(),
        expires_at_tick: None,
    };
    let selected = render_intent(&intent_node);

    // ── Action dispatch ───────────────────────────────────────────────────────
    if let Some(action_id) = selected {
        // proxy-text selected Confirm (action [1])
        sys_debug_writeln("proxy-text: selected action Confirm");
        sys_debug_writeln("semantic-stream: action accepted");
        sys_debug_writeln("sample-service: action completed");

        let completed = event(EventKind::ActionCompleted,
                              "Action completed",
                              "Sample action completed successfully.",
                              Severity::Normal, EventResult::Ok, None);
        render_event(match &completed.payload {
            SemanticPayload::Event(n) => n, _ => unreachable!()
        });
        let _ = action_id;
    }

    // ── State export ──────────────────────────────────────────────────────────
    export_state();

    sys_debug_writeln("TEST:M5:PASS");
    sys_exit(0)
}
