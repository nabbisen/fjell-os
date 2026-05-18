//! Task spawn primitive — M4.
//!
//! Loads a flat service binary from the embedded image table into a fresh
//! address space and returns the new `TaskId`.

use fjell_abi::service::ImageId;
use fjell_abi::error::SysError;
use crate::task::tcb::{Task, TaskState};
use crate::mm::frame_alloc::{FrameAllocator, FrameOwner};
use crate::mm::address::{VirtAddr, PhysFrame};
use crate::mm::vspace::{AddressSpace, AddressSpaceId, VmPerms};
use crate::mm::region::VmRegionKind;
use crate::task::image::{image_bytes, SERVICE_BASE_VA, SERVICE_STACK_TOP};
use fjell_abi::task::TaskId;

const PRIORITY_USER: u8 = 2;

/// Spawn a new task from `image_id`.
///
/// Allocates frames for text + stack, maps them in a new address space,
/// creates a TCB and inserts it into the task table.
///
/// Returns `(TaskId, task_handle_raw)`.  The task is in `Created` state
/// and must be started with `sys_task_start`.
pub fn spawn(
    image_id:    ImageId,
    table:       &mut crate::task::tcb::TaskTable,
    _sched:      &mut crate::task::scheduler::Scheduler,  // reserved for M5 enqueue-on-spawn
    kernel_root: crate::mm::frame_alloc::PhysFrame,
    fa:          &mut FrameAllocator<'_>,
) -> Result<TaskId, SysError>
{
    let bytes = image_bytes(image_id).ok_or(SysError::InvalidCap)?;

    // Find a fresh task slot index.
    let tid_index = table.next_free_index().ok_or(SysError::NoMemory)?;
    let tid       = TaskId::new(tid_index, 0);
    let asp_id    = AddressSpaceId(tid_index);

    // Allocate root page table.
    let root_f = fa.alloc_frame(FrameOwner::KernelPageTable)
                   .map_err(|_| SysError::NoMemory)?;
    let mut aspace = AddressSpace::new(asp_id, root_f);
    aspace.clone_kernel_half(kernel_root);

    // Map UART for kernel debug output from trap handler.
    let uart_f = PhysFrame::from_pa(0x1000_0000).unwrap();
    aspace.map_page(VirtAddr(0x1000_0000), uart_f,
        VmPerms::R | VmPerms::W, VmRegionKind::Mmio, fa)
        .map_err(|_| SysError::NoMemory)?;

    // Map all 8 virtio-mmio slots (0x10001000..0x10008000) with R|W (no U).
    // Supervisor-mode trap handlers (sys_platform_info_get, sys_mmio_map) can
    // then scan/access them.  User-mode drivers call sys_mmio_map to get U+R+W.
    for i in 0..8usize {
        let mmio_pa = 0x1000_1000 + i * 0x1000;
        if let Ok(f) = PhysFrame::from_pa(mmio_pa) {
            let _ = aspace.map_page(VirtAddr(mmio_pa), f,
                VmPerms::R | VmPerms::W, VmRegionKind::Mmio, fa);
        }
    }

    // Allocate text frame, copy flat binary.
    if bytes.len() > 4096 {
        // Allocate additional pages if needed (up to 8 pages for now)
        let pages = (bytes.len() + 4095) / 4096;
        for i in 0..pages {
            let f = fa.alloc_frame(FrameOwner::UserText { task: tid })
                      .map_err(|_| SysError::NoMemory)?;
            let start = i * 4096;
            let end   = (start + 4096).min(bytes.len());
            unsafe {
                let dst = core::slice::from_raw_parts_mut(f.pa() as *mut u8, 4096);
                dst.fill(0);
                dst[..(end - start)].copy_from_slice(&bytes[start..end]);
            }
            aspace.map_page(VirtAddr(SERVICE_BASE_VA + i * 4096), f,
                VmPerms::R | VmPerms::X | VmPerms::U, VmRegionKind::UserText, fa)
                .map_err(|_| SysError::NoMemory)?;
        }
    } else {
        let f = fa.alloc_frame(FrameOwner::UserText { task: tid })
                  .map_err(|_| SysError::NoMemory)?;
        unsafe {
            let dst = core::slice::from_raw_parts_mut(f.pa() as *mut u8, 4096);
            dst.fill(0);
            dst[..bytes.len()].copy_from_slice(bytes);
        }
        aspace.map_page(VirtAddr(SERVICE_BASE_VA), f,
            VmPerms::R | VmPerms::X | VmPerms::U, VmRegionKind::UserText, fa)
            .map_err(|_| SysError::NoMemory)?;
    }

    // Allocate and map all stack pages (64 KiB = 16 pages).
    // The linker script places __stack_bottom = 0x80000, __stack_top = 0x90000.
    // Mapping only the top page caused StorePageFault when stack usage exceeded 4K.
    const STACK_PAGES: usize = 16;
    let stack_base = SERVICE_STACK_TOP - STACK_PAGES * 4096;
    for pg in 0..STACK_PAGES {
        let sf = fa.alloc_frame(FrameOwner::UserStack { task: tid })
                   .map_err(|_| SysError::NoMemory)?;
        aspace.map_page(VirtAddr(stack_base + pg * 4096), sf,
            VmPerms::R | VmPerms::W | VmPerms::U, VmRegionKind::UserStack, fa)
            .map_err(|_| SysError::NoMemory)?;
    }

    // Allocate kernel stack.
    let kstack_f = fa.alloc_frame(FrameOwner::KernelStack)
                     .map_err(|_| SysError::NoMemory)?;

    // Build TCB.
    let mut t = Task::new(tid, PRIORITY_USER, asp_id,
                          kstack_f.pa() + 4096, SERVICE_STACK_TOP);
    t.satp_root_pfn     = root_f.pfn as usize;
    t.trap_frame.sepc   = SERVICE_BASE_VA;
    t.trap_frame.gpr[2] = SERVICE_STACK_TOP;
    t.trap_frame.sstatus = 1 << 5; // SPIE, SPP=0 (user mode)
    t.state = TaskState::Created;

    let ins_id = table.insert(t).map_err(|_| SysError::NoMemory)?;
    Ok(ins_id)
}
