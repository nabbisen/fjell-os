//! PLIC (Platform-Level Interrupt Controller) driver — RFC-0.25-001 R1.
//!
//! QEMU `virt`: PLIC base `0x0c00_0000`. Single-hart only; hart 0's S-mode
//! context (context 1) is the only context this driver programs. Multi-hart
//! PLIC contexts are out of scope (RFC-0.25-001 non-goals).
//!
//! Register layout (standard SiFive/RISC-V PLIC, matches QEMU's model):
//!   priority[irq]      = base + 4*irq                    (irq = 1..=1023)
//!   enable[context]     = base + 0x2000 + 0x80*context     (bitmap, 1 bit/irq)
//!   threshold[context]  = base + 0x20_0000 + 0x1000*context
//!   claim/complete      = base + 0x20_0004 + 0x1000*context (read=claim, write=complete)
//!
//! # MMIO mapping
//! This driver is called from trap-handling code, which runs under whichever
//! task's page table happens to be active (`satp` is not switched to a
//! kernel-only table during trap handling). `crates/fjell-kernel/src/task/
//! spawn.rs` therefore maps the three PLIC pages this driver touches
//! (priority, enable, threshold/claim) into every task's address space,
//! R|W and not U — the same treatment already given to UART0.

const PLIC_BASE: usize = 0x0c00_0000;
const CONTEXT: usize = 1;

const PRIORITY_BASE: usize = PLIC_BASE;
const ENABLE_BASE: usize = PLIC_BASE + 0x2000 + 0x80 * CONTEXT;
const THRESHOLD: usize = PLIC_BASE + 0x20_0000 + 0x1000 * CONTEXT;
const CLAIM_COMPLETE: usize = PLIC_BASE + 0x20_0004 + 0x1000 * CONTEXT;

/// Physical pages this driver touches — used by `spawn.rs` to map them into
/// every task's address space. Page-aligned; `ENABLE_BASE` (`0x2080`) and
/// `THRESHOLD`/`CLAIM_COMPLETE` (`0x201000`/`0x201004`) each fall inside one
/// of these.
pub const MAPPED_PAGES: [usize; 3] = [
    PLIC_BASE,            // priority[] (irq 0..1023 fits in one 4K page)
    PLIC_BASE + 0x2000,   // enable[context 1]
    PLIC_BASE + 0x201000, // threshold[context 1] / claim-complete[context 1]
];

/// Initialise context 1 (hart 0, S-mode): threshold 0 so any source with
/// priority > 0 can interrupt.
///
/// # Safety
/// Must be called exactly once, before `sie.SEIE` is enabled, and after the
/// three `MAPPED_PAGES` are mapped in the calling task's (or boot) page
/// table.
// SAFETY: category=mmio-access called once during boot, before interrupts are enabled.
pub unsafe fn init() {
    // SAFETY: category=mmio-access PLIC MMIO is mapped by spawn.rs into
    // every task's page table; called once during boot before interrupts
    // are enabled, so no concurrent access.
    // MMIO-ORDER: device_setup
    unsafe { (THRESHOLD as *mut u32).write_volatile(0) };
}

/// Enable interrupt source `irq` for context 1 and give it priority 1 (any
/// nonzero priority is sufficient with threshold 0).
///
/// # Safety
/// `irq` must be a valid PLIC source number (1..=1023). Caller must not
/// invoke this concurrently with itself (single-hart; not an issue today).
// SAFETY: category=mmio-access single hart; irq bounds are the caller's contract.
pub unsafe fn enable(irq: u32) {
    let reg = ENABLE_BASE + 4 * (irq as usize / 32);
    let bit = irq % 32;
    // SAFETY: category=mmio-access PLIC MMIO is mapped by spawn.rs; single hart.
    unsafe {
        // MMIO-ORDER: status_read
        let cur = (reg as *mut u32).read_volatile();
        // MMIO-ORDER: device_setup
        (reg as *mut u32).write_volatile(cur | (1 << bit));
        let prio_reg = (PRIORITY_BASE as *mut u32).add(irq as usize);
        // MMIO-ORDER: device_setup
        prio_reg.write_volatile(1);
    }
}

/// Claim the highest-priority pending interrupt for context 1.
///
/// Returns `0` if nothing is pending (spurious read; `0` is reserved as
/// "no interrupt" by the PLIC spec, never a real source number).
///
/// # Safety
/// A non-zero return value must be paired with a later `complete()` call for
/// the same `irq` — R1's claim/complete pairing — or the PLIC never re-arms
/// that source (Demonstration 3: this holds even when no task is bound).
// SAFETY: category=mmio-access called only from trap-handling context, single hart.
pub unsafe fn claim() -> u32 {
    // SAFETY: category=mmio-access PLIC MMIO is mapped by spawn.rs; single hart.
    // MMIO-ORDER: status_read
    unsafe { (CLAIM_COMPLETE as *mut u32).read_volatile() }
}

/// Complete (acknowledge) a claimed interrupt, re-arming its source.
///
/// # Safety
/// `irq` must be the value most recently returned by `claim()`.
// SAFETY: category=mmio-access irq provenance is the caller's contract; single hart.
pub unsafe fn complete(irq: u32) {
    // SAFETY: category=mmio-access PLIC MMIO is mapped by spawn.rs; single hart.
    // MMIO-ORDER: device_kick
    unsafe { (CLAIM_COMPLETE as *mut u32).write_volatile(irq) };
}
