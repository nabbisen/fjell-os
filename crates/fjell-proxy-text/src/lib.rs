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
