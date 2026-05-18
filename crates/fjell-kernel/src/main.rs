//! Fjell OS Kernel — v0.0.2 / M2: Memory and Task Isolation
//!
//! Boot flow:
//!   _start  (boot.rs, M-mode assembly)
//!   └─ m_mode_setup  (M-mode Rust)
//!      └─ mret → s_mode_entry  (S-mode Rust)
//!         └─ kmain  → scheduler bootstrap → first_entry (sret, noreturn)
//!                                        → trap_dispatch (all future scheduling)

#![no_std]
#![no_main]

mod arch;
mod audit;
mod boot;
mod cap;
mod console;
mod lease;
mod mm;
mod platform;
mod task;
mod trap;
mod uart;

use core::{cell::UnsafeCell, mem::MaybeUninit, panic::PanicInfo};

#[cfg(target_arch = "riscv64")]
use arch::riscv64::{csr, satp};
use audit::ring::{AuditKindInternal, AUDIT};
use mm::{
    address::{PhysFrame, VirtAddr},
    boot_alloc::BootAllocator,
    frame_alloc::{FrameAllocator, FrameOwner, FRAME_SIZE},
    region::VmRegionKind,
    vspace::{AddressSpace, AddressSpaceId, VmPerms},
};
use platform::qemu_virt::{MMIO_REGIONS, RAM_BASE, RAM_END, mmio_region_table, MMIO_REGION_COUNT};
use task::{
    scheduler::{Scheduler, PRIORITY_IDLE, PRIORITY_USER},
    tcb::{Task, TaskState, TaskTable},
    // user_image constants are still defined for reference; not used in M4 main.
    TaskId,
};
use trap::entry::init_trap;

// ── Linker symbols ────────────────────────────────────────────────────────────

unsafe extern "C" {
    static __bss_end:    u8;
    static __stack_top:  u8;
    /// RFC 009 (W^X): end of .text section.  Pages below this address get R|X.
    static __text_end:   u8;
    /// RFC 009 (W^X): end of .rodata section.  Pages in [text_end, rodata_end) get R only.
    static __rodata_end: u8;
}

fn kernel_end_pa() -> usize {
    let bss_end = unsafe { &__bss_end as *const u8 as usize };
    (bss_end + 0xFFF) & !0xFFF
}

// ── Static kernel state ───────────────────────────────────────────────────────

pub(crate) struct KS<T>(UnsafeCell<T>);
// SAFETY: single-hart M2; no concurrent access.
unsafe impl<T> Sync for KS<T> {}

static FRAME_BITMAP: KS<[u64; 512]>            = KS(UnsafeCell::new([0u64; 512]));
static TASK_TABLE:   KS<MaybeUninit<TaskTable>> = KS(UnsafeCell::new(MaybeUninit::uninit()));
/// Static frame allocator storage — must NOT be a kmain local.
///
/// If `FrameAllocator` were on the kmain stack, the trap handler (which resets
/// sp to `__stack_top` on every entry) would overwrite it after `first_entry`.
/// Keeping it in BSS means it lives at a fixed address for the kernel lifetime.
static FRAME_ALLOC: KS<MaybeUninit<mm::frame_alloc::FrameAllocator<'static>>>
    = KS(UnsafeCell::new(MaybeUninit::uninit()));

/// Raw pointer to the kernel frame allocator stored after kmain initialises it.
/// Accessed by `sys_task_spawn` during trap handling.
static FA_RAW_PTR: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Return a `'static`-lifetime pointer to the frame allocator.
///
/// # Safety
/// Must be called after `FA_RAW_PTR` is stored in kmain.  Single-hart;
/// caller is responsible for exclusive access (no concurrent spawn calls).
pub unsafe fn fa_static_ptr() -> *mut mm::frame_alloc::FrameAllocator<'static> {
    FA_RAW_PTR.load(core::sync::atomic::Ordering::Relaxed) as *mut _
}
static SCHEDULER:    KS<MaybeUninit<Scheduler>> = KS(UnsafeCell::new(MaybeUninit::uninit()));
static CAP_TABLE:    KS<MaybeUninit<cap::table::CapTable>>      = KS(UnsafeCell::new(MaybeUninit::uninit()));
static EP_TABLE:     KS<MaybeUninit<cap::table::EndpointTable>> = KS(UnsafeCell::new(MaybeUninit::uninit()));
static LEASE_TABLE:  KS<MaybeUninit<lease::LeaseTable>>         = KS(UnsafeCell::new(MaybeUninit::uninit()));
/// Kernel root page table frame — needed by sys_task_spawn to clone kernel half.
static KERNEL_ROOT_PFN: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);
/// Per-hart trap scratch record. Layout: [0] = kernel sp, [1] = TrapFrame ptr.
/// Must be static — sscratch holds a pointer to it across sret/trap boundaries.
/// RFC 007: DMA virtual address base for per-task allocations.
///
/// DMA frames are mapped at user VA 0x6000_0000+ (VPN[2]=1), well below the
/// kernel half (VPN[2]=2, 0x8000_0000+) and above user code (VPN[2]=0,
/// 0x0000_0000+).  This avoids the AlreadyMapped conflict that arose when
/// DMA frames were placed at their kernel identity-map VA.
pub(crate) const DMA_USER_VA_BASE: usize = 0x6000_0000;


// ── RFC 036 (v0.2.0): DmaRegion state machine ────────────────────────────────

/// Maximum concurrent DMA allocations across all tasks.
const MAX_DMA_REGIONS: usize = 16;

/// RFC 036 §2 — DmaRegion cleanup state machine.
///
/// Transitions:
/// ```text
/// Active → Revoked → Zeroized → Freed
///        ↘ Quarantined → Zeroized → Freed   (uncertain device quiesce)
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DmaRegionState {
    /// Region is in use by the owning task.
    Active,
    /// Revoked: device access prevented; zeroize pending.
    Revoked,
    /// Quarantined: device quiesce uncertain; await timeout.
    ///
    /// RFC 036: in v0.2.0 the quarantine timeout is a DEFERRED stub —
    /// the timer-callback path needed for real quarantine is planned for
    /// the first device driver that requires it.  At v0.2 scale (virtio-blk
    /// cooperative model), synchronous zeroize on revoke is sufficient.
    #[allow(dead_code)]  // RFC 036 deferred path — constructed when timer-callback lands
    Quarantined,
    /// Physical page zeroed; safe to return to allocator.
    Zeroized,
    /// Frame returned to allocator; slot is free.
    Freed,
}

/// One entry in the per-kernel DMA region tracking table (RFC 036 §2).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DmaRegionEntry {
    /// Task that owns this DMA region (`index == 0xFFFF` = Freed/empty).
    pub owner:    crate::task::TaskId,
    /// User VA where the frame is mapped in `owner`'s page table.
    /// Read when unmapping the page during explicit revoke (v0.3 unmap path).
    #[allow(dead_code)]  // stored for future unmap; read path lands in v0.3
    pub user_va:  usize,
    /// Physical frame address.
    pub frame_pa: usize,
    /// Cleanup state.
    pub state:    DmaRegionState,
}

impl DmaRegionEntry {
    const fn free() -> Self {
        DmaRegionEntry {
            owner:    crate::task::TaskId::new(0xFFFF, 0),
            user_va:  0,
            frame_pa: 0,
            state:    DmaRegionState::Freed,
        }
    }
    pub fn is_free(self) -> bool { self.owner.index == 0xFFFF }
}

pub(crate) struct DmaRegionTable {
    entries: [DmaRegionEntry; MAX_DMA_REGIONS],
}

impl DmaRegionTable {
    const fn new() -> Self {
        DmaRegionTable { entries: [const { DmaRegionEntry::free() }; MAX_DMA_REGIONS] }
    }

    /// Allocate a new Active region.  Returns false if the table is full.
    pub fn alloc(
        &mut self,
        owner: crate::task::TaskId,
        user_va: usize,
        frame_pa: usize,
    ) -> bool {
        for e in self.entries.iter_mut() {
            if e.is_free() {
                *e = DmaRegionEntry {
                    owner, user_va, frame_pa,
                    state: DmaRegionState::Active,
                };
                return true;
            }
        }
        false
    }

    /// Explicitly revoke a DMA region by physical address (RFC 036 §2).
    ///
    /// Transitions: Active → Revoked → Zeroized → Freed (synchronous in v0.2).
    ///
    /// The quarantine path (`Active → Quarantined`) is a DEFERRED stub.
    pub fn revoke_by_pa(&mut self, owner: crate::task::TaskId, frame_pa: usize) -> bool {
        let fa = unsafe { crate::fa_static_ptr() };
        for e in self.entries.iter_mut() {
            if e.owner == owner && e.frame_pa == frame_pa && e.state == DmaRegionState::Active {
                e.state = DmaRegionState::Revoked;
                // Synchronous zeroize + free (v0.2; quarantine deferred).
                if e.frame_pa != 0 {
                    unsafe { core::ptr::write_bytes(e.frame_pa as *mut u8, 0, 4096); }
                    e.state = DmaRegionState::Zeroized;
                    if let Ok(frame) = crate::mm::frame_alloc::PhysFrame::from_pa(e.frame_pa) {
                        unsafe { let _ = (*fa).free_frame(frame); }
                    }
                }
                *e = DmaRegionEntry::free();
                return true;
            }
        }
        false
    }

    /// Lifecycle revoke: zeroize and release all Active regions owned by `task`.
    ///
    /// Called on task exit, fault, or restart.  Invariant DMA-004.
    pub fn release_task(&mut self, owner: crate::task::TaskId) {
        let fa = unsafe { crate::fa_static_ptr() };
        for e in self.entries.iter_mut() {
            if e.owner == owner && e.state == DmaRegionState::Active {
                let pa = e.frame_pa;
                if pa != 0 {
                    // Zeroize before free — invariant DMA-001.
                    unsafe { core::ptr::write_bytes(pa as *mut u8, 0, 4096); }
                    if let Ok(frame) = crate::mm::frame_alloc::PhysFrame::from_pa(pa) {
                        unsafe { let _ = (*fa).free_frame(frame); }
                    }
                }
                *e = DmaRegionEntry::free();
            }
        }
    }
}

struct DmaRegionTableStatic(core::cell::UnsafeCell<DmaRegionTable>);
unsafe impl Sync for DmaRegionTableStatic {}
static DMA_REGION_TABLE: DmaRegionTableStatic =
    DmaRegionTableStatic(core::cell::UnsafeCell::new(DmaRegionTable::new()));
pub(crate) fn dma_table() -> &'static mut DmaRegionTable {
    unsafe { &mut *DMA_REGION_TABLE.0.get() }
}

/// Per-request DMA VA bump allocator (RFC 007).
/// Monotonically increases; no free list needed for v0.2.
pub(crate) static DMA_VA_NEXT: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(DMA_USER_VA_BASE);


/// Kernel trap scratch: [0]=kernel_sp, [1]=TrapFrame_ptr, [2]=temp_user_sp_save,
/// [3]=temp_user_t6_save  (RFC 001: slot added to fix t6 register save correctness)
pub(crate) static TRAP_SCRATCH: KS<[usize; 4]> = KS(UnsafeCell::new([0usize; 4]));

macro_rules! ks_init {
    ($ks:expr, $val:expr) => { unsafe { (*$ks.0.get()).write($val) } };
}
macro_rules! ks_get {
    ($ks:expr) => { unsafe { (*$ks.0.get()).assume_init_mut() } };
}

/// Called by `trap/dispatch.rs` to access all mutable kernel state.
///
/// # Safety
/// All tables must have been initialised before any trap fires.
/// Single-hart M3/M4; no concurrent access.
pub unsafe fn get_kernel_state() -> (
    &'static mut task::tcb::TaskTable,
    &'static mut task::scheduler::Scheduler,
    &'static mut cap::table::CapTable,
    &'static mut cap::table::EndpointTable,
) {
    (
        ks_get!(TASK_TABLE),
        ks_get!(SCHEDULER),
        ks_get!(CAP_TABLE),
        ks_get!(EP_TABLE),
    )
}

/// Return the lease table reference (used by M4 syscall handlers).
///
/// # Safety
/// Must be called after `LEASE_TABLE` is initialised.
pub unsafe fn get_lease_table() -> &'static mut lease::LeaseTable {
    ks_get!(LEASE_TABLE)
}

// ── kprintln! — usable from trap/dispatch.rs ─────────────────────────────────

#[macro_export]
macro_rules! kprintln {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::println!($($arg)*));
}

// ── M-mode shim ───────────────────────────────────────────────────────────────

#[cfg(target_arch = "riscv64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn m_mode_setup(hart_id: usize, dtb_pa: usize) -> ! {
    // SAFETY: M-mode CSR writes; called exactly once from boot assembly.
    unsafe {
        csr::write_medeleg(0xFFFF);
        csr::write_mideleg(0x0222);
        csr::write_mstatus(1usize << 11); // MPP = S

        // Configure PMP entry 0: grant S-mode RWX access to all physical memory.
        //
        // RISC-V PMP is deny-by-default for S/U-mode: if no PMP entry matches,
        // the access is denied.  Without at least one permissive entry the CPU
        // will fault on the very first S-mode instruction fetch after mret.
        //
        // pmpaddr0 = all-ones: in NAPOT mode this encodes the entire 2^64-byte
        //   address space starting at PA 0.
        // pmpcfg0  = 0x1F: A=NAPOT(11), X=1, W=1, R=1, L=0 (unlocked).
        csr::write_pmpaddr0(usize::MAX);
        csr::write_pmpcfg0(0x1F);

        csr::write_mepc(s_mode_entry as *const () as usize);
        core::arch::asm!(
            "mv a0, {hart}", "mv a1, {dtb}", "mret",
            hart = in(reg) hart_id, dtb = in(reg) dtb_pa,
            options(noreturn),
        );
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[unsafe(no_mangle)]
pub extern "C" fn m_mode_setup(_: usize, _: usize) -> ! { loop {} }

// ── S-mode entry ──────────────────────────────────────────────────────────────

#[cfg(target_arch = "riscv64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn s_mode_entry(hart_id: usize, dtb_pa: usize) -> ! {
    // SAFETY: called once via mret; first use of console.
    unsafe { console::init() };
    println!("Fjell OS kernel started.");
    println!("mode: S");
    kmain(hart_id, dtb_pa)
}

#[cfg(not(target_arch = "riscv64"))]
#[unsafe(no_mangle)]
pub extern "C" fn s_mode_entry(_: usize, _: usize) -> ! { loop {} }

/// Host-build stub: binary needs a no-arg kmain for `#[no_main]`.
#[cfg(not(target_arch = "riscv64"))]
#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! { loop {} }

// ── Kernel main ───────────────────────────────────────────────────────────────

#[cfg(target_arch = "riscv64")]
#[allow(unused_unsafe)] // fa!() macro contains unsafe that may nest under caller's unsafe blocks
fn kmain(_hart_id: usize, dtb_pa: usize) -> ! {
    println!("platform: qemu-virt");

    let platform = platform::detect(dtb_pa);
    println!("memory: detected ({} MiB)", platform.ram_size / (1024 * 1024));

    // BootAllocator (watermark used to calculate kernel_end_pa only).
    let boot_start = kernel_end_pa();
    let boot_end   = boot_start + 2 * 1024 * 1024;
    let _boot = BootAllocator::new(boot_start, boot_end);
    println!("mm: boot allocator ready");

    // FrameAllocator — stored in a STATIC so the trap handler (which resets
    // sp to __stack_top on every entry) cannot overwrite it.
    let bitmap = unsafe { &mut *FRAME_BITMAP.0.get() };
    unsafe {
        FRAME_ALLOC.0.get().write(MaybeUninit::new(FrameAllocator::new(
            (RAM_BASE >> 12) as u64,
            ((RAM_END - RAM_BASE) / FRAME_SIZE) as u64,
            bitmap, None,
        )));
    }
    macro_rules! fa { () => {
        // SAFETY: single-hart; FRAME_ALLOC initialised above; no aliasing.
        unsafe { (*FRAME_ALLOC.0.get()).assume_init_mut() }
    } }
    // Expose raw pointer to trap-time task-spawn handler.
    FA_RAW_PTR.store(
        unsafe { (*FRAME_ALLOC.0.get()).as_mut_ptr() } as usize,
        core::sync::atomic::Ordering::Relaxed,
    );

    fa!().reserve_range(RAM_BASE, kernel_end_pa(), FrameOwner::KernelText)
         .expect("rsv kern");
    fa!().reserve_range(boot_start, boot_end, FrameOwner::ReservedBoot)
         .expect("rsv boot");
    if dtb_pa != 0 {
        // DTB may be placed anywhere in RAM by firmware/QEMU.  If the region
        // overlaps an already-reserved range (e.g. it sits inside the kernel
        // image), treat it as already accounted for rather than panicking.
        let _ = fa!().reserve_range(dtb_pa, dtb_pa + 4096, FrameOwner::Dtb);
    }
    for &(start, end, _) in MMIO_REGIONS {
        if start < RAM_BASE { let _ = fa!().reserve_range(start, end, FrameOwner::Mmio); }
    }
    println!("mm: frame allocator ready  ({} free frames)", fa!().free_count());

    // Kernel page table.
    let kernel_root = fa!().alloc_frame(FrameOwner::KernelPageTable)
                           .expect("kernel root PT");
    // SAFETY: freshly allocated 4-KiB frame.
    unsafe { core::ptr::write_bytes(kernel_root.pa() as *mut u8, 0, 4096) };

    // Identity-map kernel + boot scratch + kernel stack.
    //
    // The identity map must reach __stack_top (linker symbol) because:
    //  - the kernel stack lives between __stack_bottom and __stack_top,
    //    which is 4 MiB above BSS end — well above boot_end (BSS+2MiB).
    //  - Sv39 is enabled before kmain returns, so any stack access after
    //    enable_sv39 must be covered by the page table.
    //
    // boot_end (BSS + 2 MiB) is kept as the bump-allocator ceiling; the
    // stack pages are additionally mapped here.
    let stack_top  = unsafe { &__stack_top   as *const u8 as usize };
    let text_end   = unsafe { &__text_end    as *const u8 as usize };
    let rodata_end = unsafe { &__rodata_end  as *const u8 as usize };
    let map_end    = (stack_top + 0xFFF) & !0xFFF;

    // RFC 009/018: W^X — three-region split (sections are 4 KiB page-aligned).
    //   .text   [RAM_BASE  .. text_end)   → R | X   (execute, not writable)
    //   .rodata [text_end  .. rodata_end) → R        (read-only, not executable)
    //   rest    [rodata_end.. map_end)    → R | W   (read-write, not executable)
    // link.ld ALIGN(4096) ensures no page straddles two regions (RFC 018).
    let mut va = RAM_BASE;
    while va < map_end {
        let f = PhysFrame::from_pa(va).unwrap();
        let perms = if va < text_end {
            VmPerms::R | VmPerms::X    // .text: execute only, not writable
        } else if va < rodata_end {
            VmPerms::R                  // .rodata: read-only, not executable
        } else {
            VmPerms::R | VmPerms::W    // .data / .bss / stack: read-write, not executable
        };
        // SAFETY: kernel_root valid; sfence inside enable_sv39.
        unsafe {
            mm::page_table::map_page(kernel_root.pa(), VirtAddr(va), f, perms, fa!())
                .expect("kernel map");
        }
        va += 4096;
    }
    // UART identity-map.
    let uart_f = PhysFrame::from_pa(0x1000_0000).unwrap();
    // SAFETY: same.
    unsafe {
        mm::page_table::map_page(kernel_root.pa(), VirtAddr(0x1000_0000), uart_f,
            VmPerms::R | VmPerms::W, fa!())
            .expect("uart map");
    }

    // Map all 8 virtio-mmio slots (0x10001000..0x10009000) in the kernel
    // page table so sys_platform_info_get can scan them from kernel mode.
    for i in 0..8usize {
        let va_pa = 0x1000_1000 + i * 0x1000;
        if let Ok(f) = PhysFrame::from_pa(va_pa) {
            unsafe {
                let _ = mm::page_table::map_page(kernel_root.pa(), VirtAddr(va_pa),
                    f, VmPerms::R | VmPerms::W, fa!());
            }
        }
    }

    // Enable Sv39.
    // SAFETY: all required kernel mappings present; sfence inside.
    unsafe { satp::enable_sv39(kernel_root.pfn as usize) };
    // Store kernel root PFN for use by sys_task_spawn.
    KERNEL_ROOT_PFN.store(kernel_root.pfn as usize, core::sync::atomic::Ordering::Relaxed);
    println!("vm: sv39 enabled");
    AUDIT.lock_free_append(AuditKindInternal::Boot, 0, 0, 0);

    // Install trap vector.
    // SAFETY: called once before any user-mode entry or interrupt enable.
    unsafe { init_trap() };
    println!("trap: stvec installed");

    // Initialise task table and scheduler.
    ks_init!(TASK_TABLE, TaskTable::new());
    ks_init!(SCHEDULER,  Scheduler::new());
    ks_init!(CAP_TABLE,  cap::table::CapTable::new());
    ks_init!(EP_TABLE,   cap::table::EndpointTable::new());
    ks_init!(LEASE_TABLE, lease::LeaseTable::new());
    println!("M3: capability table initialized");
    println!("M3: endpoint table initialized");
    let table     = ks_get!(TASK_TABLE);
    let sched     = ks_get!(SCHEDULER);
    let cap_table = ks_get!(CAP_TABLE);   // RFC 004: used for bootstrap cap install
    let et        = ks_get!(EP_TABLE);

    // Allocate the shared IPC endpoint for the M3 smoke test.
    let ep_obj_id = et.alloc().expect("alloc endpoint");
    println!("M3: endpoint created (id={})", ep_obj_id);
    // Endpoint 1: storaged private endpoint (storaged listens; init calls).
    let storaged_ep_id = et.alloc().expect("alloc storaged endpoint");
    let _ = storaged_ep_id;  // id=1
    // Endpoints 2-4: M8 service private endpoints.
    let measuredd_ep_id = et.alloc().expect("alloc measuredd endpoint"); let _ = measuredd_ep_id; // id=2
    let attestd_ep_id   = et.alloc().expect("alloc attestd endpoint");   let _ = attestd_ep_id;   // id=3
    let recoveryd_ep_id = et.alloc().expect("alloc recoveryd endpoint"); let _ = recoveryd_ep_id; // id=4

    // Idle task — no capabilities needed.
    let idle_ksp = unsafe { &__stack_top as *const u8 as usize };
    let mut idle = Task::new(TaskId::new(0, 0), PRIORITY_IDLE,
                             AddressSpaceId(0), idle_ksp, 0);
    idle.state = TaskState::Runnable;
    let idle_id = table.insert(idle).expect("idle insert");
    sched.set_idle(idle_id);

    // ── M4: spawn fjell-init as the first user task ───────────────────────────
    //
    // fjell-init orchestrates the entire user-space service plane.
    // All other services (configd, cap-broker, auditd, service-manager,
    // sample-service) are spawned by init via sys_task_spawn / sys_task_start.
    {
        use fjell_abi::service::ImageId;
        use task::image::{SERVICE_BASE_VA, SERVICE_STACK_TOP};
        use task::image::image_bytes;

        let init_bytes = image_bytes(ImageId::INIT).expect("init image missing");
        let tid    = TaskId::new(1, 0);
        let asp_id = AddressSpaceId(1);

        let root_f = fa!().alloc_frame(FrameOwner::KernelPageTable).expect("init root");
        let mut aspace = AddressSpace::new(asp_id, root_f);
        aspace.clone_kernel_half(kernel_root);

        let uart_f = PhysFrame::from_pa(0x1000_0000).unwrap();
        aspace.map_page(VirtAddr(0x1000_0000), uart_f,
            VmPerms::R | VmPerms::W, VmRegionKind::Mmio, fa!())
            .expect("init uart map");

        // Map all 8 virtio-mmio slots (R|W, no U) for kernel-mode scanning.
        for i in 0..8usize {
            let mmio_pa = 0x1000_1000 + i * 0x1000;
            if let Ok(f) = PhysFrame::from_pa(mmio_pa) {
                let _ = aspace.map_page(VirtAddr(mmio_pa), f,
                    VmPerms::R | VmPerms::W, VmRegionKind::Mmio, fa!());
            }
        }

        // Map text pages (flat binary may span multiple pages)
        let pages = (init_bytes.len() + 4095) / 4096;
        for pg in 0..pages {
            let f = fa!().alloc_frame(FrameOwner::UserText { task: tid }).expect("init text");
            let start = pg * 4096;
            let end   = (start + 4096).min(init_bytes.len());
            unsafe {
                let dst = core::slice::from_raw_parts_mut(f.pa() as *mut u8, 4096);
                dst.fill(0);
                dst[..(end - start)].copy_from_slice(&init_bytes[start..end]);
            }
            aspace.map_page(VirtAddr(SERVICE_BASE_VA + pg * 4096), f,
                VmPerms::R | VmPerms::X | VmPerms::U, VmRegionKind::UserText, fa!())
                .expect("init text map");
        }

        // RFC 007: DMA is now allocated per-task via sys_dma_alloc at VA 0x6000_0000+.
        // No pre-mapping needed here.

        // Map all 16 stack pages (64 KiB, from 0x80000 to 0x90000).
        const INIT_STACK_PAGES: usize = 16;
        let stack_base = SERVICE_STACK_TOP - INIT_STACK_PAGES * 4096;
        for pg in 0..INIT_STACK_PAGES {
            let sf = fa!().alloc_frame(FrameOwner::UserStack { task: tid }).expect("init stack");
            aspace.map_page(VirtAddr(stack_base + pg * 4096), sf,
                VmPerms::R | VmPerms::W | VmPerms::U, VmRegionKind::UserStack, fa!())
                .expect("init stack map");
        }

        let kstack_f = fa!().alloc_frame(FrameOwner::KernelStack).expect("init kstack");

        let mut t = Task::new(tid, PRIORITY_USER, asp_id,
                              kstack_f.pa() + 4096, SERVICE_STACK_TOP);
        t.satp_root_pfn     = root_f.pfn as usize;
        t.trap_frame.sepc   = SERVICE_BASE_VA;
        t.trap_frame.gpr[2] = SERVICE_STACK_TOP;   // sp
        t.trap_frame.gpr[11] = 0;                   // a1 = BootInfo ptr (0 = use defaults)
        t.trap_frame.sstatus = 1 << 5;              // SPIE, SPP=0
        t.state = TaskState::Runnable;

        let ins_id = table.insert(t).expect("init insert");
        sched.enqueue_runnable(ins_id, PRIORITY_USER);
        AUDIT.lock_free_append(AuditKindInternal::TaskCreate, 1, 0, 0);

        // RFC 004: Grant init task bootstrap capabilities so it can spawn
        // services and manage leases.  Service-manager receives TaskCreate
        // and TaskControl via cap_derive after init spawns it.
        {
            use fjell_cap::{CapKind, CapRights, CapState, ObjectScope};
            use fjell_cap::slot::Capability;
            let cs = cap_table.cspace_mut(1 /* init task index */)
                .expect("init CSpace");
            // Slot 28: TaskCreate — init can spawn service tasks.
            let _ = cs.install_raw(28, Capability {
                kind: CapKind::TaskCreate, object_id: 0,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            // Slot 29: TaskControl — init can start spawned tasks.
            let _ = cs.install_raw(29, Capability {
                kind: CapKind::TaskControl, object_id: 0,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            // Slot 30: LeaseAdmin — init can create/revoke leases.
            let _ = cs.install_raw(30, Capability {
                kind: CapKind::LeaseAdmin, object_id: 0,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });

            // Slot 0: shared IPC endpoint (for service broadcasts).
            let _ = cs.install_raw(0, Capability {
                kind: CapKind::Endpoint, object_id: 0,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            // Slot 1: cap-broker private endpoint (endpoint id=5, RFC 040).
            // Init uses this to send BOOTSTRAP_COMPLETE.
            let _ = cs.install_raw(1, Capability {
                kind: CapKind::Endpoint, object_id: 5,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            // Slot 2: storaged private endpoint (endpoint id=1).
            let _ = cs.install_raw(2, Capability {
                kind: CapKind::Endpoint, object_id: 1,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            // Slots 3-5: M8 service private endpoints.
            let _ = cs.install_raw(3, Capability {  // measuredd (ep id=2)
                kind: CapKind::Endpoint, object_id: 2,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            let _ = cs.install_raw(4, Capability {  // attestd (ep id=3)
                kind: CapKind::Endpoint, object_id: 3,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            let _ = cs.install_raw(5, Capability {  // recoveryd (ep id=4)
                kind: CapKind::Endpoint, object_id: 4,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            // Slots 31-34: MmioRegion — one per QEMU virt MMIO region (RFC 016).
            let mmio_table = mmio_region_table();
            for (i, _region) in mmio_table.iter().enumerate().take(MMIO_REGION_COUNT) {
                let slot = 31 + i;
                let _ = cs.install_raw(slot, Capability {
                    kind: CapKind::MmioRegion,
                    object_id: i as u32,
                    rights: CapRights::ALL,
                    badge: 0, scope: ObjectScope::Any, state: CapState::Active,
                    parent: None, lease: None,
                });
            }
        }
        println!("M4: init task ready");
    }

    println!("sched: started");

    // Choose the first task and enter user mode.
    // From this point on, all scheduling is handled in trap_dispatch.
    let first_id = sched.choose_next();
    sched.set_current(first_id);
    let first_satp = if let Some(t) = table.get_mut(first_id) {
        t.state = TaskState::Running;
        t.accounting.run_count += 1;
        t.satp_root_pfn
    } else { 0 };

    let first_tf = &table.get(first_id).unwrap().trap_frame;

    // Switch to the first task's address space.
    // SAFETY: first_satp comes from the task's root PhysFrame.pfn.
    if first_satp != 0 {
        unsafe { satp::enable_sv39(first_satp) };
    }

    // Set up sscratch pointing to the STATIC trap scratch record.
    // scratch[0] = boot stack top (kernel sp restored on trap entry).
    // scratch[1] = &TrapFrame of the first task.
    // Must be static — sscratch is read on every future trap, long after
    // this stack frame is gone.
    // SAFETY: TRAP_SCRATCH is static; valid for the entire kernel lifetime.
    unsafe {
        let s = &mut *TRAP_SCRATCH.0.get();
        s[0] = idle_ksp;
        s[1] = first_tf as *const _ as usize;
        csr::write_sscratch(s.as_ptr() as usize);
    }

    // SAFETY: first_tf is valid; sepc in user VA; sstatus.SPP=0.
    unsafe { trap::dispatch::first_entry(first_tf) }
}

// ── Panic handler ─────────────────────────────────────────────────────────────

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n[KERNEL PANIC] {}", info);
    loop {}
}
