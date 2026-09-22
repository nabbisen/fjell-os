//! Boot-control service — RFC 057, RFC-0.33-001.
//!
//! `bootctl` owns the [`BootControlBlock`] (D1): the state ADR-0009 describes —
//! which slot runs, which was last confirmed, which is staged — and the
//! transitions between them, which live in `fjell-upgrade-format` where the
//! model checks them (`fjell-bootctl-model`'s `tests/refines.rs`). This file is
//! the IPC around that.
//!
//! What it does not do, stated so a reader of the markers is not misled:
//!
//! * **The block is in memory only.** Nothing reads it from, or writes it to,
//!   disk; a reset starts a fresh one. Persistence is a store client, which is a
//!   line of its own (RFC-0.33-001 §A).
//! * **It selects state, not an image.** There is one kernel image, so "roll
//!   back to slot A" changes what this block says and nothing else (D7).
//! * **It does not confirm or roll back on command.** `service-manager` *reports*
//!   health (`BOOT_HEALTH_REPORT`); the block decides what that means (§E).
//!
//! Slot layout:
//!   0 = Endpoint — `BOOTCTL_EP_OBJECT`, its own dedicated object (RFC-0.33-001
//!       D8; previously the shared object 0, raced by other receivers)
//!   1 = Reboot cap (CapKind::Reboot, REBOOT right) — installed by the kernel
//!       (RFC-0.33-001 D8; `spawn.rs`)
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_service_api::tags;
use fjell_syscall::{
    ipc_sender_image_id, sys_debug_writeln, sys_exit, sys_ipc_recv_msg, sys_ipc_reply, sys_reboot,
};
use fjell_upgrade_format::{BootControlBlock, HealthVerdict, SlotState};

const SLOT_OWN_EP: u32 = 0;
const SLOT_REBOOT: u32 = 1;

/// Image generation of the block's initial slot. Nothing stages another image
/// yet, so there is no other generation to name.
const INITIAL_GENERATION: u64 = 1;

/// `BOOT_STATE_REPLY`'s state word: what the running boot's slot is.
const STATE_UNCONFIRMED: usize = 0;
const STATE_CONFIRMED: usize = 1;
const STATE_FAILED: usize = 2;

const HEALTHY: usize = 0;

fn state_word(bcb: &BootControlBlock) -> usize {
    let Ok(active) = bcb.active() else {
        // A block whose active slot is not a slot is not a state to report as
        // healthy; say "failed".
        return STATE_FAILED;
    };
    let info = bcb.slot(active);
    if info.state == SlotState::Failed {
        STATE_FAILED
    } else if info.confirmed == 1 {
        STATE_CONFIRMED
    } else {
        STATE_UNCONFIRMED
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("bootctl: started (RFC 057)");

    // Every boot is a trial of the active slot: it consumes a try until the
    // health report confirms it.
    let mut bcb = BootControlBlock::new(INITIAL_GENERATION);
    if bcb.begin_boot().is_err() {
        sys_debug_writeln("bootctl: boot-control block refused the boot; not serving");
        sys_exit(1);
    }

    loop {
        let (label, w0, _, _, _, sender) = match sys_ipc_recv_msg(SLOT_OWN_EP) {
            Ok(m) => m,
            Err(_) => {
                let _ = sys_ipc_reply(usize::MAX);
                continue;
            }
        };
        let label = label & 0xFFFF;
        match label {
            l if l == (tags::BOOT_PENDING_QUERY & 0xFFFF) => {
                let _ = sys_ipc_reply(tags::BOOT_STATE_REPLY | (state_word(&bcb) << 16));
            }
            l if l == (tags::BOOT_HEALTH_REPORT & 0xFFFF) => {
                // Only the service that observes health may report it. The
                // sender is the kernel's word, not the message's.
                if ipc_sender_image_id(sender) != fjell_abi::service::ImageId::SERVICE_MANAGER.0 {
                    sys_debug_writeln(
                        "bootctl: health report from a sender other than service-manager: refused",
                    );
                    let _ = sys_ipc_reply(usize::MAX);
                    continue;
                }
                match bcb.apply_health(w0 == HEALTHY) {
                    Ok(HealthVerdict::Confirmed) => {
                        sys_debug_writeln("bootctl: health passed; active slot confirmed");
                        let _ = sys_ipc_reply(0);
                    }
                    Ok(HealthVerdict::NoFallback) => {
                        // The failing slot is the last confirmed one. A reset
                        // would change nothing and, with no persisted try
                        // counter, would repeat: report it, do not reset.
                        sys_debug_writeln(
                            "bootctl: health FAILED on the last confirmed slot; no fallback, not resetting",
                        );
                        let _ = sys_ipc_reply(usize::MAX);
                    }
                    Ok(HealthVerdict::MustRollBack) => {
                        sys_debug_writeln("bootctl: health FAILED; rolling back");
                        let _ = sys_ipc_reply(0);
                        let _ = bcb.reboot();
                        // RFC-0.33-001 D8: the dispatch arm and the Reboot
                        // capability both now exist, so this resets the
                        // machine — it does not return on success. The `loop`
                        // below is reached only if the device did not do what
                        // it is documented to do (`sys_platform_reboot`'s own
                        // doc comment), which the caller cannot repair by
                        // retrying.
                        let _ = sys_reboot(CapHandle(SLOT_REBOOT), 0);
                        loop {}
                    }
                    Err(_) => {
                        sys_debug_writeln("bootctl: health report refused by the block");
                        let _ = sys_ipc_reply(usize::MAX);
                    }
                }
            }
            l if l == (tags::BOOT_SHUTDOWN & 0xFFFF) => {
                let _ = sys_ipc_reply(0);
                sys_exit(0);
            }
            _ => {
                let _ = sys_ipc_reply(usize::MAX);
            }
        }
    }
}
