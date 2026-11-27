# RFC-0.50-001: The kernel's decidable logic runs outside a boot — the 27 tests that never compiled, and the one that was never written

**Status:** Proposed — drafted by the implementer for the architect's review; **nothing in this line is implemented**
**Milestone:** 0.50
**Tracks.** Handoff line 1 (`.git-exclude/handoff/handoff-0.50.md`); **E-134 is the subject** (ACCEPTED at 0.49, `v1-limitations.md` row 29); whether it is ever marked resolved is the architect's, at a cut, and this RFC states the condition (R3) rather than claiming it.
**Errata:** E-134 is the subject. This RFC adds **no** erratum; it records two findings the measurements made (§"Where the handoff and the tree differ").
**Touches (proposed):** three new `publish = false` host-testable crates; `crates/fjell-kernel/src/{mm/user_ptr.rs, lease/mod.rs, task/tcb.rs, task/scheduler.rs, trap/dispatch.rs}` (the logic moves out, the hardware half stays and calls in); `crates/fjell-kernel/Cargo.toml`; one new check (the guard, D6).
**Not in this RFC:** making the *kernel binary* host-testable; any change to what the 27 assertions assert; tests for `task/destroy.rs`; **freeing a page-table frame** (the walker is 0.51's, and this line is its prerequisite).

---

## What is true now

Re-measured at `b5d488d` (the commands are under *Reproduce*; none of these figures is the handoff's on trust).

| Fact | Measured |
|---|---|
| `fjell-kernel` declares `[[bin]]` and **no `[lib]`** | `crates/fjell-kernel/Cargo.toml:9`–`:11` |
| `#[test]` functions in `fjell-kernel/src` | **27**: `mm/user_ptr.rs` **9**, `trap/dispatch.rs` **8**, `lease/mod.rs` **7**, `task/scheduler.rs` **3**. Every `#[cfg(test)]` module in the crate is one of these four, and no other file has either |
| `cargo test -p fjell-kernel` | **does not build** (see *Reproduce* for the exact output and exit code) |
| Why the 27 are invisible, not merely unrun | `cargo_metadata::bare_metal_crate_names` *derives* "bare-metal" from where a crate lives (`crates/fjell-kernel`, `crates/services/*`, `crates/drivers/*`) and both host runs `--exclude` every one. **The exclusion is structural and silent**: the host-lib tier counts 996, the host-bins tier 1719, and neither can say that a crate in the exclusion list carries tests it never compiled. A test written there is written to a reader, not to a machine |
| `ReadyQueue::remove` | exists at `task/scheduler.rs:112` (and `TaskQueue::remove` at `:77`), **with no test at either**; it is the function whose bug — a `non_empty_mask` bit left set on an emptied bucket — **hung the machine** at 0.49 and was found only by booting |

The other bare-metal crates are not in the same state: of the 31 services and 3 drivers, only `fjell-proxy-text` and `fjell-driver-virtio-net` carry `#[test]`, and **both have a lib target the host runs** (`--lib`), so their tests compile. `fjell-kernel` is the one crate whose tests are structurally uncompilable. (Measured by `grep -c '#\[test\]'` per crate and the manifest's `[lib]` section; a service with tests and no lib target would be a second instance, and there is none. `fjell-driver-virtio-net`'s lib is `src/core/lib.rs`, not `src/lib.rs`: a probe for `src/lib.rs` alone reports it as having none, and I nearly wrote that.)

---

## Where the handoff and the tree differ

1. **Moving the tests is not enough, and one of the four files cannot be moved by moving it.** `lease/mod.rs`'s `LeaseTable::revoke` ends by calling `wake_or_cancel_blocked_ipc_for_lease`, which reaches `crate::get_kernel_state()` and the audit ring, and `revoke_owned_by` calls `revoke`. **So four of the seven lease tests (`revoke_increments_epoch`, `check_active_rejects_old_epoch`, `check_active_rejects_revoked_state`, `lifecycle_revoke_revokes_owned_leases` — the ones that call `revoke` or `revoke_owned_by`) would, if compiled in place on the host, call into kernel globals** — they were never going to pass as written, and nobody could know because they never built. The seam is not "where the file is" but "where the hook is" (D1, lease).
2. **A static in an extracted library is shared by every test in the process.** `trap/dispatch.rs`'s `exited_ok_by_image` reads a **global latch** (`EXITED_CLEAN`, added by 0.49) **or** scans the table. Moved as it stands, the latch becomes process-global state in a host test binary that runs tests in parallel threads: one test that latched an image would turn a later test's `assert!(!exited_ok_by_image(...))` into a false failure. The 8 tests assert the *scan* (none of them latches), so the scan is the subject that moves and the latch **stays in the kernel** (D1, dispatch).
3. **`TaskTable` and `Task` have no test anywhere, and 0.49 changed them.** `next_free() -> (index, generation)`, `id_at`, `get_by_index(_mut)`, and the generation bump in `remove` are the slot-reuse mechanism RFC-0.49-001 rests on. The eight dispatch tests need `Task::new`, `TaskTable::insert` and `FaultInfo` from `task/tcb.rs`, so **moving that file is not optional** (it is what makes the 8 compile) — and once it is host-visible the reuse mechanism can be tested for free. D5 asks whether to take that.
4. **`user_ptr`'s bound is a replicated constant** (`USER_ADDR_MAX = platform::qemu_virt::RAM_BASE`; the file says it *"is replicated here to keep the pure-logic module self-contained"*). Extracted, the crate owns a copy, and **a replicated constant is the E-125 shape** unless something ties the two. D2 ties them at compile time.
5. **An observation the 9 tests make visible, not a defect I am fixing.** `UserPtr::new` rejects a range whose end is exactly `USER_ADDR_MAX` (`end >= USER_ADDR_MAX`, the v0.2.9 H-01 fix), so `addr = KBASE - 1, len = 1` — one valid byte below the kernel — is `RangeCrossesKernel`. `range_crosses_kernel_rejected` asserts that on purpose (*"End exactly at KBASE is also rejected (strict <)"*). It is conservative by one byte and the tests say so; **I move that assertion unchanged**.

---

## The decisions this RFC asks for

**D1 — Where the seams are, argued against the four files.** *Recommended: three subject crates, one per seam, none of them in the stable-ABI snapshot.*

| File (lines, tests) | What is pure | What touches the machine | Seam |
|---|---|---|---|
| `mm/user_ptr.rs` (225, **9**) | `UserPtr::new`, `UserCopyError`, the arithmetic | one constant (`RAM_BASE`); `From<UserCopyError> for SysError` (`fjell-abi`) | **`fjell-user-ptr`** (new, ~120 lines): owns `UserPtr`, `UserCopyError`, `USER_ADDR_MAX`. The kernel's `mm/user_ptr.rs` becomes `pub use fjell_user_ptr::*` plus the const-assert in D2, so every kernel use (`user_copy.rs`, `trap/syscall.rs`: 11 lines outside the file) is unchanged |
| `lease/mod.rs` (572, **7**) | the whole table: `create`, `allocate`, `check_active`, `revoke`'s epoch/state change, `revoke_owned_by`, `LeaseChecker` | `revoke` calls the wake hook and a `kprintln!`; one `const _` assert against `MAX_TASKS` | **`fjell-lease`** (new): `LeaseTable<H: OnRevoke>`, where `OnRevoke` has the two hooks (`revoked(id, new_epoch)`, `retired_before_wrap(id)`). The kernel keeps `wake_or_cancel_blocked_ipc_for_lease` and the `kprintln`, implements `OnRevoke` for a zero-sized `KernelRevoke`, and exposes `pub type LeaseTable = fjell_lease::LeaseTable<KernelRevoke>` — **the name every call site already uses** |
| `task/tcb.rs` (329, 0) + `trap/dispatch.rs` (762, **8**) | `Task`, `TaskTable` (slots, generations), `TaskState`, `FaultInfo`; and the two table queries `exited_ok_by_image`'s scan half and `done_by_image` | `Task::new` reads `DEVICE_VMA_BASE`; `AddressSpaceId` is a `mm::vspace` newtype; the latch static and `check_smoke_pass` (kprintln) | **`fjell-task`** (new): the TCB data, `TaskTable`, `ReadyQueue`/`TaskQueue` and priority constants, and the two queries. `AddressSpaceId(pub u16)` moves with it (re-exported from `mm::vspace`); `DEVICE_VMA_BASE` is replicated with the D2 assert. The kernel keeps the latch, `check_smoke_pass`, `Scheduler` |
| `task/scheduler.rs` (301, **3**) | `TaskQueue`, `ReadyQueue` — depend on `TaskId` and `MAX_TASKS` only | `Scheduler::set_current` calls `AUDIT.note_running` | `ReadyQueue` moves to `fjell-task`; **`Scheduler` stays** (thin, one kernel hook), and its un-tested methods (`forget`, `choose_next`) are listed under *What this does not do* rather than moved behind a second hook trait for its own sake |

*Why three crates and not one `fjell-kernel-core`:* the project's own pattern is **subject** crates (`fjell-frame-alloc`, `fjell-cap`, `fjell-ipc`, `fjell_driver_virtio_net::scan`), and a single grab-bag crate is the shape a reader would call a seam chosen for convenience. *Why not `fjell-cap` for the lease table:* **`fjell-cap` is one of the eight `STABLE_CRATES` in `fjell-abi-snapshot`**, so every public item added there joins the stable ABI surface and a kernel-internal table would be frozen by `abi-snapshot` (0.49 already added `task_kill` there, and the snapshot rose 565 → 600; one more precedent is the wrong direction). The three new crates are `publish = false` and outside `STABLE_CRATES`, so the snapshot does not move.

**D2 — What ties a replicated constant to its original.** *Recommended: a compile-time assertion in the kernel, so the cross build fails if they disagree.* `const _: () = assert!(fjell_user_ptr::USER_ADDR_MAX == crate::platform::qemu_virt::RAM_BASE);` and the same for `DEVICE_VMA_BASE`. It runs on every kernel build (which is the riscv build), costs nothing at run time, and **keeps the 9 tests byte-for-byte**: they call `UserPtr::new(addr, len)` and compare against `USER_ADDR_MAX`, the crate's own. The alternative — `UserPtr::new(addr, len, limit)` — changes every call and every test and leaves the real bound untested on the host.

**D3 — What the kernel keeps, per file** (so a reader can see the line was drawn, not blurred):

* `mm/user_ptr.rs`: the re-export and the assert. **Nothing else.**
* `lease/mod.rs`: `KernelRevoke`, `wake_or_cancel_blocked_ipc_for_lease` (≈ 120 lines of kernel state), the `MAX_LEASES >= MAX_TASKS + MAX_LEASES_PER_OWNER` assert, and the alias.
* `task/tcb.rs`: re-exports. `AddressSpaceId` stays *defined* in `fjell-task` and is re-exported by `mm::vspace`.
* `task/scheduler.rs`: `Scheduler` and its audit hook.
* `trap/dispatch.rs`: the `EXITED_CLEAN` latch, `latch_clean_exit`, `exit_latched`, `check_smoke_pass`, and a **wrapper** `exited_ok_by_image(table, image) = exit_latched(image) || fjell_task::…::exited_ok_by_image(table, image)` — same name at the 6 kernel call sites (5 in `check_smoke_pass`, 1 in `destroy.rs`); the crate's function keeps the old name and signature, so the 8 tests call exactly what they called.

**D4 — Whether any of the 27 assertions change.** *Recommended answer: no, and the RFC lists every place the test text changes at all.* Three mechanical differences, **each of them a construction detail, none an assertion**:

* `lease`: the 7 tests build a table with `LeaseTable::new()`; in the crate the type has a hook parameter, so the test module defines `type T = LeaseTable<NoHook>;` and writes `T::new()`. **Seven one-token substitutions; the 7 assertion bodies are untouched.** (`NoHook` records the calls, so a *new* assertion can show the hook ran; that is an addition, not a change to one of the seven.)
* `dispatch`: `task_with(...)` builds a `Task` through `Task::new(TaskId::new(0,0), 0, AddressSpaceId(0), 0, 0)`; the signature is unchanged and `AddressSpaceId` comes from the new crate. **Nothing changes in the test body.** The one test whose subject is a *table index* (`exited_ok_ignores_a_different_task_at_the_old_hardcoded_index`, which asserts `imposter_id.index == 19`) is the reason `TaskTable` moves whole rather than the queries being re-typed over an iterator: re-typing would remove the index and make that a rewrite.
* `user_ptr`, `scheduler`: **none.**

If any assertion has to be weakened to fit a signature during implementation, it is reported in the stage-2 review request as *a rewrite*, by name, and not absorbed.

**D5 — Scope of what moves with `tcb.rs`.** *Asks the architect:* once `TaskTable` is host-visible, the slot-reuse mechanism 0.49 built (`next_free`'s `(index, generation)`, `id_at`, `remove`'s generation bump, a stale id naming nothing) has **no test and now could**. *Recommended: take a small set* (≤ 6 tests: a freed slot is reused with generation + 1; the old id no longer resolves; `get_by_index` names the current occupant; `next_free` on a full table is `None`; `id_at` of a free slot is `None`) *because the move is what exposes them and 0.51's walker and restart policy are built on exactly this mechanism.* **Not recommended:** anything wider (`Scheduler`, `destroy`) — the handoff's own out-of-scope list.

**D6 — The guard that stops the class re-forming.** E-134 is a **class**, and the instance it was filed for is the only one today. `bare_metal_crate_names` already knows which crates cannot compile their tests; *recommended:* a check that **a crate in that list with no `[lib]` target contains no `#[test]` and no `#[cfg(test)]`** — it would have failed the day `user_ptr.rs`'s first test was written. Where it lives is a choice (a new subcheck `dead-tests`, or the `repo-scope`-style classified list); *recommended: a subcheck, because it is a repository property and not a tool-internal one.* It fails closed on a crate it cannot classify (no `Cargo.toml`, an unreadable `[lib]`), and it **prints what it saw** (crates examined, `[lib]` crates skipped and why), because a green with no numbers is the E-014 shape.

**D7 — The `remove` test.** *The second deliverable, as the handoff says.* In `fjell-task`, tests that **construct the 0.49 defect**: enqueue into a bucket, remove it through `ReadyQueue::remove`, assert `dequeue_next()` is `None` **and** `non_empty_mask` is zero (the field is private; the test sits in the module, as the existing three do); a second that removes the *only* task from the *higher* of two buckets and asserts the lower bucket's task still dequeues (the exact hang: "hands the CPU to idle while a lower bucket still has runnable tasks"); and `Scheduler`-free coverage of `remove` of an absent id returning 0. **Shown red** against a reconstruction of the pre-fix code (the `if q.len == 0 { self.non_empty_mask &= !(1 << bucket); }` block deleted) **in a scratch worktree** (`pwd` check in the same command), because an assertion that has never failed has no demonstrated discriminating power.

**D8 — `cargo test -p fjell-kernel`.** After the move the crate has no tests, and the command **still does not build** (`E0152`: the bin carries a `panic_handler`, and the test harness links `std`, which already defines one). *Recommended:* leave it so and **say so** — adding `test = false` to the `[[bin]]` would make the command exit 0 with "0 tests", a green that says nothing about a crate whose own limitation is that it cannot be tested on the host; the honest state is *"the kernel has no tests of its own; its decidable logic is tested in three crates and its boot behaviour by the QEMU tiers"*, written in `v1-limitations.md` row 29's successor (the architect's) and in the crate's `Cargo.toml` comment (mine). *Alternative if the architect prefers a runnable command:* `test = false` **plus** the D6 guard, so the command is green because there is provably nothing to run.

---

## Requirements

**R1** the three crates, each `#![no_std]`-compatible and **compiling for `riscv64gc-unknown-none-elf` as well as the host** (the kernel links them) — shown by building both. **R2** the 27 moved and passing in the host-lib tier, so that tier's count rises by **≥ 27 plus the new tests**, the difference stated. **R3** the kernel keeps no `#[test]` and no `#[cfg(test)]` (count reported; **if any remains, E-134 is not closed**). **R4** D2's asserts. **R5** D7's tests, red by named failure against the reconstruction. **R6** D6's guard, red on a synthetic crate and on the pre-change kernel. **R7** `abi-snapshot` unchanged (the crates are outside `STABLE_CRATES`); `unsafe-audit` unchanged or its delta explained (`ReadyQueue` and the table have no `unsafe`; the kernel's statics keep theirs). **R8** `test-all`, `repro-check` (a kernel change **does** change the kernel ELF — its digest is not in the baseline, which holds prebuilts only — and the services are untouched unless a moved type is linked into one; measured, and the baseline re-recorded in the same commit if it moves).

## Negative cases — what I will show

* **N1.** *A test that passes because its subject no longer does the work:* for each of the four moved subjects, a mutation **in the crate** (not in the kernel) that the moved test must catch — `user_ptr`: `>=`→`>` on the range end (RED by `range_crosses_kernel_rejected`); `lease`: `revoke` forgets to advance the epoch (RED by `check_active_rejects_old_epoch`); `dispatch`: the scan matches on index not image (RED by the index-19 test); scheduler: `remove` forgets the mask (RED by D7). Each with a checked backup, a sha-exact restore, and a named failure; a compile error is INVALID.
* **N2.** *Logic left behind in the kernel while the test moved:* a grep of the kernel for `fn exited_ok_by_image`, `fn done_by_image`, `LeaseObject`, `fn validate`-shaped arithmetic returns only the wrappers D3 lists; reported with its output.
* **N3.** *A crate that builds for the host and not for riscv* (R1), *and the reverse*.
* **N4.** *The latch leaking into the host tests* (finding 2): a test in the crate that would fail if a process-global latch existed — it does not exist in the crate, and the kernel's wrapper is shown to still say yes after the corpse is gone (the 0.49 latch note, `latch_note`, in the `task-lifecycle` profile, which must stay green).
* **N5.** *The host-lib count moving by less than 27 without explanation* — the figure before and after, and the new-test count, in the review request.

## What this RFC does not do, and why

**Does not** make the kernel binary host-testable (a `[lib]` over the whole kernel needs `cfg` gating around `asm!`, linker symbols and lang items in every module and buys a host build of code that cannot run on the host); **does not** test `Scheduler`'s `forget`/`choose_next`, `task/destroy.rs`, or the latch (all boot-covered by `task-lifecycle`; the latch is a 6-line static); **does not** change `USER_ADDR_MAX`'s off-by-one conservatism (finding 5); **does not** free a page-table frame.

## Reproduce

```
sed -n '1,12p' crates/fjell-kernel/Cargo.toml
/usr/bin/grep -c '#\[test\]' crates/fjell-kernel/src/mm/user_ptr.rs crates/fjell-kernel/src/trap/dispatch.rs \
  crates/fjell-kernel/src/lease/mod.rs crates/fjell-kernel/src/task/scheduler.rs
/usr/bin/grep -rc '#\[test\]' crates/fjell-kernel/src | /usr/bin/grep -v ':0'          # nothing outside the four
/usr/bin/grep -n 'fn remove' crates/fjell-kernel/src/task/scheduler.rs               # :77 and :112, no test names either
/usr/bin/grep -n 'STABLE_CRATES' -A10 tools/fjell-abi-snapshot/src/main.rs            # fjell-cap is stable; the new crates would not be
cargo test -p fjell-kernel 2>&1 | tail -4 ; echo "exit=$?"
```
