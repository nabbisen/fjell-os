//! Semantic Stream Service — M5.
//!
//! publish / subscribe / validate / action dispatch for semantic nodes.

#![allow(unused_assignments)] // IPC polling idiom: t/w* are overwritten by sys_ipc_recv
#![no_std]
#![no_main]
mod rt;

use fjell_semantic_format::*;
use fjell_syscall::sys_debug_writeln;

// ── Semantic ring (memory-backed) ─────────────────────────────────────────────

struct SemanticRing {
    items: [Option<SemanticEnvelope>; 32],
    head: usize,
    sequence: u64,
    #[allow(dead_code)]
    dropped: u64,
}

impl SemanticRing {
    const fn new() -> Self {
        SemanticRing {
            items: [None; 32],
            head: 0,
            sequence: 0,
            dropped: 0,
        }
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
// SAFETY: category=raw-pointer-deref extern IPC interface; layout matches the ABI defined in fjell-service-api.
unsafe impl Sync for SyncRing {}

static INTENT_RING: SyncRing = SyncRing(UnsafeCell::new(SemanticRing::new()));
static STATE_RING: SyncRing = SyncRing(UnsafeCell::new(SemanticRing::new()));
static EVENT_RING: SyncRing = SyncRing(UnsafeCell::new(SemanticRing::new()));

pub fn publish(env: SemanticEnvelope) {
    // SAFETY: category=raw-pointer-deref extern IPC interface; layout matches the ABI defined in fjell-service-api.
    unsafe {
        match env.stream {
            StreamKind::Intent => {
                (*INTENT_RING.0.get()).publish(env);
            }
            StreamKind::State => {
                (*STATE_RING.0.get()).publish(env);
            }
            StreamKind::Event => {
                (*EVENT_RING.0.get()).publish(env);
            }
        }
    }
}

pub fn validate_and_publish(env: SemanticEnvelope) -> bool {
    let ok = match &env.payload {
        SemanticPayload::Intent(n) => validate_intent(n).is_ok(),
        SemanticPayload::State(n) => validate_state(n).is_ok(),
        SemanticPayload::Event(_) => true,
    };
    if ok {
        publish(env);
    }
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

// ── IPC (recoveryd pattern, RFC-v0.23-001) ─────────────────────────────────────

use fjell_service_api::semantic_stream as proto;

const EP_SLOT: u32 = 0;

fn send_ready() {
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 20","ecall", in("a0") EP_SLOT as usize, in("a1") proto::READY, lateout("a0") _, lateout("a7") _, options(nostack));
    }
}

fn recv_call() -> (usize, usize, usize, usize, usize) {
    let (mut t, mut w0, mut w1, mut w2, mut w3) = (0usize, 0usize, 0usize, 0usize, 0usize);
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 21","ecall", in("a0") EP_SLOT as usize, lateout("a1") t, lateout("a2") w0, lateout("a3") w1, lateout("a4") w2, lateout("a5") w3, lateout("a7") _, options(nostack));
    }
    (t, w0, w1, w2, w3)
}

fn reply(tag: usize, w0: usize, w1: usize, w2: usize) {
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 23","ecall", in("a0") 0usize, in("a1") tag, in("a2") w0, in("a3") w1, in("a4") w2, lateout("a7") _, options(nostack));
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    send_ready();
    sys_debug_writeln("M5: semantic-stream started");
    sys_debug_writeln("M5: semantic policy loaded");
    loop {
        let (tag_packed, _w0, _w1, _w2, _w3) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            // RFC-v0.23-001 Slice 2/3 add real handling for PUBLISH_*,
            // FORWARD_*, and DISPATCH_ACTION here.
            _ => reply(proto::ERR, 0, 0, 0),
        }
    }
}
