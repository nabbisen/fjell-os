# RFC-0.26-001 R2 — The M6 hang, explained

**Governing RFC:** [rfcs/accepted/RFC-0.26-001-scheduler-priority-unification.md](../../rfcs/accepted/RFC-0.26-001-scheduler-priority-unification.md)
**Deliverable:** D1 — this document is the explanation the RFC requires before
any unification lands. Written from direct observation (QEMU serial logs),
not inference; every claim below names the log line or source line it comes
from.

## 1. Reproduction (R1)

`crates/fjell-kernel/src/task/spawn.rs`'s local `PRIORITY_USER` was changed
from `2` to `32` (matching `task::scheduler::PRIORITY_USER`) with nothing
else touched. Rebuilt, ran `cargo xtask qemu-test m8` (default 60s timeout),
then re-ran the identical build against a 300-second timeout to rule out
"merely slow" before calling it a hang.

Both runs stop at the same line and never progress further:

```
M5: semantic operations ready
M6: virtio-mmio blk discovered
qemu-system-riscv64: terminating on signal 15 from pid <pid> (timeout)
```

`ps` during the 300s run showed `qemu-system-riscv64` pinned at ~100% CPU
for the full duration — this is a live busy loop, not a blocked wait with
nothing to do.

## 2. Which task, which wait (R2)

**Neither `devmgr` nor `storaged` ever executes a single instruction.**
Grepping the 300-second log for either service's own debug output (`devmgr:
starting`, or the raw diagnostic bytes `storaged`'s virtio-mmio bus scan
writes) finds nothing — not even their first line. Both were successfully
`sys_task_spawn`ed and `sys_task_start`ed by `init` (`crates/services/
fjell-init/src/main.rs`'s M6 section: `spawn(ImageId::DEVMGR, "")`, then
after the platform-info print, `spawn(ImageId::DRIVER_VIRTIO_BLK, "")`,
`spawn(ImageId::STORAGED, "")`, `wait_storaged_ready(2)`), so both are
sitting `Runnable` in the ready queue. They simply never get a turn.

`init` itself is not the blocker: `wait_storaged_ready` is a genuine
blocking `sys_ipc_recv` (raw `a7=21`, `IpcRecv` — not `IpcTryRecv`), which
removes `init` from the ready queue entirely the moment it's called. Once
blocked, `init`'s own priority bucket is irrelevant — it isn't competing for
anything.

**The actual occupant of the highest bucket is `svc-timeout`
(`crates/services/fjell-svc-timeout/src/main.rs`), and it never leaves:**

```rust
pub extern "C" fn service_main() -> ! {
    // Spin forever — intentionally never sends READY.
    loop {
        sys_yield();
    }
}
```

This is by design — RFC 042's start-timeout negative test
(`fjell-neg-test`'s `test_svc_start_timeout`, called at line 864 of
`crates/services/fjell-neg-test/src/main.rs`, well before `init` ever
reaches M6) needs a service that provably never becomes ready, and detects
that by observing it still alive after a wait window. It is spawned once,
early (M4-era, concurrently with `init`'s own M5/M6 progress), and then
loops calling `sys_yield()` for the rest of the run — nothing ever kills it.

**The mechanism, traced through the scheduler:**

- `svc-timeout` is spawned via `sys_task_spawn`/`sys_task_start`, so its
  *first* enqueue uses whatever `sys_task_start` hardcodes (`2`, before this
  RFC's fix) — bucket 0.
- After its first `sys_yield()`, every subsequent re-enqueue goes through
  `schedule_next`'s yield path, which uses `task.priority` — the value
  `spawn.rs` set at spawn time. With `spawn.rs`'s constant changed to `32`,
  this is now bucket **1**.
- `svc-timeout` never blocks and never exits. Every single scheduling turn
  it receives, it immediately re-enqueues itself into bucket 1 via the same
  yield path. Bucket 1 is therefore **permanently non-empty** for the rest
  of the run.
- `Scheduler::dequeue_next` always drains the highest non-empty bucket
  first (`7 - non_empty_mask.leading_zeros()`). With bucket 1 permanently
  occupied by `svc-timeout`, bucket 1 is drained on *every* scheduling
  decision, and bucket 0 — where `devmgr`, `driver-virtio-blk`, and
  `storaged` sit from their initial `sys_task_start` enqueue — is never
  reached again, for the rest of the run.

**Why this was invisible before RFC-0.25-001, and still invisible with only
`spawn.rs` changed:** before this RFC, `spawn.rs`'s constant was `2`, so
`svc-timeout`'s post-first-yield re-enqueues landed in bucket 0 — the same
bucket as `devmgr`/`driver-virtio-blk`/`storaged`, sharing it via ordinary
FIFO round-robin (delayed, not starved). Changing only `spawn.rs`'s constant
(RFC-0.25-001's abandoned attempt, and this RFC's R1 reproduction) moves
`svc-timeout`'s *re*-enqueue to bucket 1 while leaving `sys_task_start`'s
*initial* enqueue for newly spawned M6 services at the old hardcoded bucket
0 — creating a **new** mismatch between the two enqueue paths that did not
exist before. The M6 hang is a product of exactly that mismatch, not of the
priority *value* in isolation.

## 3. Classification

This is neither of the two shapes the RFC anticipated cleanly:

- It is not a **missing synchronisation** — no service is waiting on a
  signal nobody sends.
- It is not **accidental ordering that nothing depends on** — something
  very concrete depends on it: bucket 0 ever being reached at all.

It is a **scheduler fairness gap**: a bucket-based ready queue with no
liveness guarantee once a single never-blocking task occupies the highest
bucket alone. `svc-timeout` existing and behaving exactly as documented
(an intentional, permanent, by-design infinite `sys_yield()` loop) is not
itself a bug — negative-test harnesses need a task that never completes.
The bug is that the *two independent copies* of "what bucket does a
spawned task start/continue in" could disagree, and when they did, one of
them (bucket 0) became permanently unreachable rather than merely
under-served.

## 4. The fix (R3/R4/R5)

Both enqueue paths now read the **same** value:

- `crates/fjell-kernel/src/task/spawn.rs`: the local shadowing `const
  PRIORITY_USER: u8 = 2` is removed; the file now imports
  `task::scheduler::PRIORITY_USER` directly. Every spawned task's `Task.priority`
  is the real constant.
- `crates/fjell-kernel/src/trap/syscall.rs::sys_task_start`: the initial
  enqueue now reads `task.priority` (the value `spawn.rs` just set) instead
  of a disconnected hardcoded `2`. This is the same source `schedule_next`'s
  yield path already used for every *subsequent* enqueue — the two paths
  can no longer drift apart, because there is only one value left to read.
- RFC-0.25-001's `image_id`-keyed stopgap for `driver-uart` in
  `sys_task_start` is removed entirely — with both enqueue paths unified,
  every task (including `driver-uart` and `init`) shares one priority, so
  the special case has nothing left to compensate for.

No distinct `PRIORITY_INIT` was introduced: nothing in this investigation
found a reason `init` needs to be genuinely privileged over other tasks —
its former advantage was the bug, not a policy anyone had chosen.

**Result:** `cargo xtask qemu-test m8` passes cleanly, `devmgr` and
`storaged` both run and reach `M6: storaged ready`, and both
`NEG:SVC:START_TIMEOUT_DETECTED:PASS` and `NEG:SVC:FAULT_DETECTED:PASS` are
emitted (the latter never printed in the hung runs either — `neg-test`
itself was stalled waiting for its own next scheduling turn after spawning
`svc-timeout`, for the identical reason). `uart-rx` and `uart-rx-unbound`
both pass with the stopgap removed.

## 5. Collateral noted, not chased

**`cargo xtask test-all` is 19/21 with the full fix applied — two profiles
regress, filed as Errata E-019 (`docs/rfcs/ERRATA.md`).** Both were
reproduced twice (`cargo xtask qemu-run --profile ipc` /
`--profile semantic`), deterministically failing both times — QEMU TCG is
fully deterministic given the same binary and inputs, so this is not
flakiness.

- **`ipc`:** `fjell-neg-test` reaches `NEG:SVC:FAULT_DETECTED:PASS` and then
  never reaches any of the three `NEG:IPC:*` markers.
  `test_ipc_blocked_recv` (`crates/services/fjell-neg-test/src/main.rs:435-439`)
  documents the assumption directly: *"By the cooperative-scheduling
  contract, sample-service immediately calls `sys_ipc_recv(SLOT_LEASED_EP)`
  and blocks before the scheduler returns to neg-test."* That contract
  depended on `init`-era preemption dynamics this RFC deliberately removed.
- **`semantic`:** `fjell-sample-service`'s `emit_sample_intent()`
  (`crates/services/fjell-sample-service/src/main.rs:141-144`) fires once at
  startup on the documented assumption that *"semantic-stream and proxy-text
  are already spawned and ready by this point"* — asserted, not
  synchronised on. `sample-service demo intent` and
  `proxy-text: action DENIED (capability not held)` never appear once that
  assumption breaks.

Both are the **same root cause as the M6 hang** (§2/§3): code assuming a
specific relative scheduling order between concurrently-started tasks
instead of synchronising on it explicitly. They surface as silently-skipped
assertions rather than a hang because the assumption here is about
*relative arrival order between two already-running peers*, not about one
priority bucket permanently starving another that never gets its first
turn. Per the governing RFC's explicit instruction (§2, "Expect collateral,
and do not absorb it... do not chase it"): reproduced and characterised
here, not fixed. Fixing either is real design work (an explicit
`READY`/rendezvous exchange in the affected service) belonging to its own
line, not folded into this one to keep the tier count green.

`storaged`'s virtio-mmio bus-scan diagnostic writes eight raw bytes
(`sys_debug_write_byte(0x90 + devid)`, non-printable) with no trailing
newline before its own `sys_debug_writeln("M6: storaged ready")` — so that
exact byte sequence appears prepended to `storaged`'s own copy of that line
in the serial log (`init` also prints an identical, clean "M6: storaged
ready" of its own, immediately after `wait_storaged_ready` returns, so the
text appears twice). This is `storaged`'s own pre-existing diagnostic
format (present in the source before this RFC), not a new corruption
introduced by unifying priorities — it was simply never visible before,
because boot never reached this point in the priority configurations this
RFC changed. Not a scheduler defect; not investigated further here, per the
RFC's own instruction not to absorb or chase newly-visible collateral into
this line's scope.
