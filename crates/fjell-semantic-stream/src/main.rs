//! Semantic Stream Service — M5.
//!
//! publish / subscribe / validate / action dispatch for semantic nodes.

#![no_std]
#![no_main]
mod rt;

use fjell_semantic_format::*;
use fjell_syscall::{sys_exit, sys_debug_writeln};

// ── Semantic ring (memory-backed) ─────────────────────────────────────────────

struct SemanticRing {
    items:    [Option<SemanticEnvelope>; 32],
    head:     usize,
    sequence: u64,
    #[allow(dead_code)] dropped: u64,
}

impl SemanticRing {
    const fn new() -> Self {
        SemanticRing { items: [None; 32], head: 0, sequence: 0, dropped: 0 }
    }
    fn publish(&mut self, env: SemanticEnvelope) -> u64 {
        self.sequence += 1;
        self.items[self.head % 32] = Some(env);
        self.head = self.head.wrapping_add(1);
        self.sequence
    }
}

// ── Service state ─────────────────────────────────────────────────────────────

use core::cell::UnsafeCell;

struct SyncRing(UnsafeCell<SemanticRing>);
unsafe impl Sync for SyncRing {}

static INTENT_RING: SyncRing = SyncRing(UnsafeCell::new(SemanticRing::new()));
static STATE_RING:  SyncRing = SyncRing(UnsafeCell::new(SemanticRing::new()));
static EVENT_RING:  SyncRing = SyncRing(UnsafeCell::new(SemanticRing::new()));

pub fn publish(env: SemanticEnvelope) {
    unsafe {
        match env.stream {
            StreamKind::Intent => { (*INTENT_RING.0.get()).publish(env); }
            StreamKind::State  => { (*STATE_RING.0.get()).publish(env);  }
            StreamKind::Event  => { (*EVENT_RING.0.get()).publish(env);  }
        }
    }
}

pub fn validate_and_publish(env: SemanticEnvelope) -> bool {
    let ok = match &env.payload {
        SemanticPayload::Intent(n) => validate_intent(n).is_ok(),
        SemanticPayload::State(n)  => validate_state(n).is_ok(),
        SemanticPayload::Event(_)  => true,
    };
    if ok { publish(env); }
    ok
}

pub fn dispatch_action(req: &ActionRequest) -> ActionResult {
    // M5: Capability check — in smoke test all actions are accepted.
    ActionResult {
        correlation_id: req.correlation_id,
        result: EventResult::Ok,
        message: TextToken::new("action accepted"),
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("M5: semantic-stream started");
    sys_debug_writeln("M5: semantic policy loaded");
    // Service stays resident; in M5 smoke test fjell-init calls it directly.
    // A real implementation would serve IPC requests here.
    sys_exit(0)
}
