# RFC-0.39-003 R1–R7: find out why a live page reads as zeros

**Governing RFC:** [../done/RFC-0.39-003-find-out-why-a-live-page-reads-as-zeros.md](../done/RFC-0.39-003-find-out-why-a-live-page-reads-as-zeros.md)
**Handoff:** [../handoffs/RFC-0.39-003-find-out-why-a-live-page-reads-as-zeros/implementation-handoff.md](../handoffs/RFC-0.39-003-find-out-why-a-live-page-reads-as-zeros/implementation-handoff.md)

Written against base `f2e9f06`. The line **found the cause** (R5 ending, not R6): **E-097 is
closed**; **E-098 is narrowed and stays OPEN**.

---

## R1 — a measured reproducer

Procedure: `cargo xtask qemu-negative semantic`, one run per sample, fresh zero-filled
disk each run (the harness deletes and recreates it), the exit status and the count and
offset of NUL bytes in the serial log recorded per run. The signature is always the same:
**46 NULs at byte 7210**.

| Configuration | Runs | Failed |
|---|---|---|
| Main tree, tip, nominal (alone, 18:36–19:15) | 40 | **23** |
| Main tree, interleaved with the worktree (19:17–20:37) | 40 | **18** |
| Scratch worktree, interleaved with the above | 40 | **0** |
| Main tree, default disk, 20:39–21:58 | 40 | 0 |
| Main tree, disk on tmpfs (same window) | 40 | 0 |

**The worktree anomaly survived and is the most informative fact in the file.** It is not a
time effect: the two trees were run alternately, seconds apart (pairs: both fail 0, main
only 18, worktree only 0, neither 22). It is not the disk's content (recreated per run), the
kernel ELF (equal hash), the prebuilts (0 differ), the QEMU command line or the host tool.
**It is the filesystem the disk file lives on**: the main tree is on btrfs over LUKS, the
worktree on tmpfs.

The nominal rate is **not a usable baseline** — it fell to 0 of 40 in the last hour with
nothing changed, because host storage latency varies. The usable one is an intervention: a
low-bandwidth fsync storm (4 KiB write + fsync in a loop, ~1.5 ms quiet, 3.5–10 ms loaded) on
the btrfs filesystem, alternating two variants in the same tree:

| Same tree, storm running, 20 pairs | Runs | Failed |
|---|---|---|
| disk file on btrfs (default) | 20 | **19** (the one pass was run 1, before the storm took hold) |
| disk file on tmpfs (`disk =` in the profile) | 20 | **0** |

## R2 — the allocator can say who owns a frame (shipped)

The allocator was constructed with `owner = None` (`kmain`), so `owner_of` always answered
`None`. It is now built with a static `[FrameOwner; 32768]` (one per 4 KiB frame of 128 MiB,
zero-initialised BSS, ~190 KiB). **Shipped, not scratch**, for three reasons: the previous
investigation stalled on exactly this; the DMA release check (R3) needs it to verify
ownership before it zeroes anything; and a memory defect of this class is undiagnosable
without it. The cost is that BSS grows and every allocation records a tag. The kernel
crate's allocator unit tests are `#[cfg(test)]` in a `no_std` binary that cannot be built for
the host, so **no gate has ever run them** — a finding; I ran them, and two new ones, in a
scratch harness crate (5 pass). I did not wire them into a gate: that is a new host crate.

## R3 — E-098's verified defects

1. **The free's result is checked.** Four sites released a DMA frame — `revoke_by_id`,
   `revoke_by_pa`, `release_task` and `sys_dma_alloc`'s table-full rollback (the last named
   nowhere in E-098) — each zeroing first and discarding `free_frame`'s result. They now
   share `free_dma_frame`: the frame must be a real frame, and the allocator must record it
   as `UserStack { task: owner }`; **only then** is it zeroed and freed, and a refused free is
   reported. **What the kernel does when a free fails:** it leaves the frame allocated — neither
   zeroed nor freed — and prints `kernel: free of DMA frame … was refused (…); the frame
   stays allocated`. One 4 KiB frame leaks, loudly. It does not panic, because the failure is
   recoverable and the alternative (zeroing or double-freeing memory someone else holds) is
   the thing being prevented. No such failure occurs in the full `test-all` run.
2. **The backwards justification is gone.** The comments now say the stale `R|W|U` mapping of
   a freed frame is not made safe by zeroing, that nothing makes it safe, and that it is
   E-098's open half. They also correct a claim the old comment made as fact: that the
   function "unmaps user PTE" and "flushes the TLB" as steps 1–2 (it does neither; it only
   issues a global `sfence`), and an account of a "page-table teardown" in `release_task`
   that does not exist (nothing removes a task's address space).
3. **The deferral is withdrawn**, not re-dated: "to v0.3" is replaced with a statement that it
   lapsed at 0.38.0 and the open question is E-098's.

## R4 — nothing forbidden

No timeout raised, no retry, no `clean-console` relaxation, no `continue-on-error`, and **no
padding to move placement**. (One experiment padded `measuredd` by 88 bytes in a scratch
worktree to *test* whether size alone triggered the failure, which it did not; that was
diagnosis, discarded, and is not in any commit.)

## R5 — the cause, and the rate against R1

**The cause.** `link.ld` places a 4 MiB stack in BSS (`__stack_bottom` `0x80085000` ..
`__stack_top` `0x80485000` in the build where it was found). Trap entry resets `sp` to
`__stack_top` every trap, so kernel frames always sit just under it. `kmain` reserved
`[RAM_BASE, __bss_end)` and then a 2 MiB `BootAllocator` window starting at `__bss_end` —
where the stack begins. That window (which allocates nothing) covered the stack's **lower**
half by coincidence, and **nothing covered the upper 2 MiB**. Those frames were free; the
allocator gave them to tasks; the kernel then wrote its stack through whichever task's text,
stack or page-table frame sat under `__stack_top`. Which task depends on allocation order,
which depends on timing — hence "placement", the bimodal `0x80451000`/`0x80481000`, and the
filesystem dependence.

**How it was established.**

* `upgraded`'s physical text frames compared with the embedded image at its exit: **0 bytes
  differ in passing runs, ~1 KB in failing runs** (page 0 from offset ~3600; page 1's whole
  image region zero, and foreign data at its tail). The foreign data is pointers into kernel
  statics (`0x80051e98`, `0x80052697`) and into the same two frames (`0x80483d1c`,
  `0x80482eb8`) — a stack.
* `upgraded`'s frames (`0x80481000`/`0x80482000`) are under `__stack_top` and above the end of
  the boot window (`0x80285000`).
* The new boot invariant, on the old layout: `mm: frame 0x802b5000 is free but lies inside the
  kernel image or stack [0x80000000, 0x804b5000)` — boot stops.
* The device is not involved: `storaged`'s DMA page was `0x8042d000`, nowhere near `upgraded`.

**The fix.** Reserve the whole stack as `KernelStack`; start the boot window after it;
`kmain` panics, naming the frame, if `first_free_in(RAM_BASE, __stack_top)` finds one.

**The rate.** Same storm, same alternating design as the baseline above, with the fixed
kernel:

| Storm running, fixed kernel | Runs | Failed |
|---|---|---|
| main tree, disk on btrfs | **40** | **0** (baseline: 19 of 20) |
| main tree, disk on tmpfs | 40 | 0 |
| scratch worktree (byte-identical kernel), disk on btrfs | **40** | **0** |

The storm was heavier than in the baseline (mean fsync latency 7.4 ms and 9.7 ms against
3.5 ms), so this is not a quiet-host result. Sample size equals R1's baseline in both
locations.

**What R5 does not show.** The second failure shape seen once (`TEST:M7:FAIL`, `M6: block I/O
error`) was not reproduced after the first; that it shares this cause is an inference. The
invariant checks the kernel image and stack only, not that no other frame is wrongly free.

## R6

Not applicable — the cause was found.

## R7 — gates

See the review request. Gate 7 is red with **one** OPEN erratum (E-098), as designed.
