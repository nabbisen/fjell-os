# RFC-0.41-001 R1–R10: the audit trail reaches someone, and says when it did not

**Governing RFC:** [../accepted/RFC-0.41-001-the-audit-trail-reaches-someone-and-says-when-it-did-not.md](../accepted/RFC-0.41-001-the-audit-trail-reaches-someone-and-says-when-it-did-not.md)
**Handoff:** [../handoffs/RFC-0.41-001-the-audit-trail-reaches-someone-and-says-when-it-did-not/implementation-handoff.md](../handoffs/RFC-0.41-001-the-audit-trail-reaches-someone-and-says-when-it-did-not/implementation-handoff.md)

Written against base `3f05c18`. E-102 is **not** closed by this line: it stays ACCEPTED until
RFC-0.41-002 takes the shared-object half (the entry carries a dated progress note).

## Where the RFC cannot be followed as written — R3

**R3 says the wake belongs on the ring's append path.** The append path cannot do it. The two
operations the IRQ path performs — mark the task `Runnable`, enqueue it — need `&mut` to the task
table and the scheduler, and `lock_free_append` is called from code that **already holds both**:
`schedule_next`'s own `TaskExit` and yield records, and the capability, IPC and lease paths (which
take `table`, `sched` and the capability table as parameters). Reaching them again from the append
would alias them. The tree already knows this: `LeaseTable::revoke` says of its own wake that it
"receives the epoch directly so it has no reason to call `get_lease_table()` and alias `&mut self`".

So the split is: **the append records that a parked waiter is owed a wake** (`waiter` → `wake`, ring
state only; nothing that can fail or block), and **the two IRQ-path operations run once at the end of
the same trap**, in `trap::dispatch::deliver_audit_wake`, called from `schedule_next` immediately
before `choose_next` — the one point where both are in hand and nothing else holds them. It runs at the
end of *every* trap, so the delivery is event-driven (the trap that appended), not timer-driven. It
**is** on the scheduler's path, which R3 says it should not be; that is the departure, and it is the
architect's to rule on. If the ruling is "make the append own the wake", the cost is an `unsafe`
re-entry into kernel state from inside code that holds it.

## R1 / R2 — the syscall and the block reason

`AuditWait = 61` (`SyscallNumber`, with a `from_usize` arm and a round-trip test — `IrqWait`'s arm was
once silently missing). `sys_audit_wait` takes the same `AuditDrain` capability and right as
`sys_audit_drain`: a wake on the ring grants authority over nothing. `BlockReason::AuditRing`, no
payload. `Sleep` and `Yield` are untouched (R9). One waiter at a time: a second task gets `BadState`,
unless the registered waiter is stale (no longer `Blocked(AuditRing)`).

## R4 — the lost wakeup: the guard is a level, not a latch

The IRQ path needs a latch because an interrupt is an edge. The condition `sys_audit_wait` waits for is
the ring's own state — non-empty — so a record appended between "a drain returned empty" and the call
needs no remembered event: `AuditRing::park` checks `len > 0` first and returns at once. Check-then-block
is atomic (single hart, inside a trap), and so is the append's claim of the waiter.

**Whether the test enters the window is measured, not assumed.**

* A scratch counter on the guard path found **0 hits** in my first design (24 rounds, one probe each) and
  **0 hits** in my second (40 rounds of two probes, `k = 0..39`): the window was never entered, and either
  test would have passed against a broken guard. The system is deterministic, so a miss is a miss every
  time; the third design below is the one that enters it.
* The window is one scheduling turn wide: `auditd`'s empty drain returns, the other runnable task gets
  one turn, then `auditd` reaches its wait. The test now sweeps: round `k` issues probe `A` (which
  starts `auditd`'s cycle), yields `k` non-appending times, then issues probe `B`, for `k = 0..119`.
* Counted again with the sweep: **1 guard hit in 120 rounds, at `k = 78`** (probes `…756`/`…757`), the
  same on every run.
* **With the guard removed** (`park` ignoring `len > 0`, scratch, reverted and proved byte-identical to
  the saved copy), the tier is **red 2 of 2 runs**, naming exactly that probe: ``probe nr=1096089757 …
  echoed too late: … on line 1264, after the bound `neg-test: audit settled` on line 1263``. Nothing else
  fails. That is the evidence that the sweep can fail and that it fails for this reason.

**What this does not make permanent:** the window position depends on how many traps `auditd` spends per
record. If that changes enough, the window leaves the sweep and the test passes without exercising the
guard. The sweep is wide (120) for that reason, and the review request proposes making the guard path
observable (`sys_audit_wait` reporting whether it slept) so a profile can require a hit.

## R5 — `auditd`

Drains until `sys_audit_drain` returns 0 records, then `sys_audit_wait`. The `sys_ipc_recv(0)` and the
comment describing a sender that does not exist are gone; `auditd` receives on no endpoint. The 64-record
buffer stays. If `sys_audit_wait` itself errors, `auditd` says so and yields rather than spinning.

## R6 — the loss is reported, and where

The line is `{"gap":{"dropped":N,"after_seq":S}}`. **Why it cannot be mistaken for an event:** a record
is `{"seq":…,"kind":…}`; the notice has neither key, only `gap`, so a parser that expects a record fails
on it instead of counting it. **Why `after_seq`:** the ring drops the *newest* record and a dropped record
takes no sequence number, so the gap cannot be read off the records' `seq`. A ring that has dropped
anything is full, so the lost records follow the ring's last pending one, `first_seq + AUDIT_RING_CAPACITY
- 1` where `first_seq` is the first record of the batch that carried the count. `AUDIT_RING_CAPACITY` is
now defined once, in `fjell-audit-format`, and the kernel re-exports it. The notice is held until that
record has been printed and emitted right after it.

**The tier checks the count and the position, not the presence of a notice.** `neg-test` issues a burst of
600 unknown-syscall probes (each yields one audit record whose `arg0` is its number) after a settle; the
profile requires `printed + lost == 600` and `lost > 0`, where `printed` is the distinct probes in the trail
and `lost` the sum of the notices after the burst began; and that the notice sits immediately after the last
probe the ring kept, with the probe numbers jumping across it. Result on every run: **issued 600 = printed
322 + lost 278; the notices claim 278**.

Seen red three ways: the old message-trigger loop (no gap notice at all — the burst never reached the
trail); a notice reporting the records printed instead of the records dropped (`claim 64 lost`, 278
probes missing); a notice one record early. **The last one passed my first version of the check** — it
compared the notice's `after_seq` with the record printed before it, and the mutation changed both
consistently. The check now uses an independent oracle (the numbering of the probes themselves), and the
mutation is refused.

**A tolerance, stated.** A notice counts every record the ring dropped while full, not only the probes, and
the system is still finishing starting: in early runs `init`'s `task.exit` was dropped inside the window, in
later runs nothing was. `noise<=3` allows for that and nothing else — a count too *low* is refused whatever
the allowance, and a wrong quantity is off by hundreds.

## R7 — `TaskSwitch` is not recorded

`audit::ring::RECORD_TASK_SWITCH = false`, **the one place the policy lives**; `lock_free_append` returns
before taking a slot. Nothing in the tree depended on `TaskSwitch` records (searched: the kernel, `neg-test`
and the decoder only). This is a reduction in what the trail contains by default and must be said in the
0.41.0 release notes, with the reason.

## R8 — the ABI surface moves by declaration

498 → **499**: +3 `fjell-audit-format::AUDIT_RING_CAPACITY`, `fjell-syscall::sys_audit_wait`,
`fjell-syscall::sys_probe_unknown`; **−2** `negative_markers::AUDIT_EVIDENCE_GAP` and
`AUDIT_LEASE_REVOCATION_DECODES`; and `SyscallNumber`'s signature changes (the new variant). The gate calls
the removals and the signature change "breaking"; the two removed items are test-marker constants whose only
emitter I deleted (below), and the enum change is the addition itself. Regenerated in this line.
`tests/syscall/expected.toml` 34/30 → 35/31 declared/dispatched.

## R10 — a tier asserts what an operator sees

`neg-test` no longer drains the ring. It **causes**: revokes a lease after boot and prints its id; issues
the probe rounds; issues the burst. The `audit` profile requires `auditd`'s own output to show each — the
revocation as `"kind":"lease.revoked"` with that lease's id, every probe before its round's `settled`
line — through two new profile directives, `expect_echoed` and `expect_accounted`
(`crates/fjell-tools/src/audit_trail.rs`, pure functions of the serial log with their own mutation controls;
197 tests in the tool crate). RFC 041's `EVIDENCE_GAP_DETECTED` and RFC-0.40-002's
`LEASE_REVOCATION_DECODES_AS_REVOKED` were in-service assertions made because `auditd` was blind, and they
compete with a live `auditd` for the same records; both markers and their constants are removed rather than
left as claims nothing makes. The audit block runs late in `neg-test`, after a 30,000-yield settle: while the
system is still starting, a probe is queued behind hundreds of boot records.

## Gates

See the review request.

## R11 — `sys_audit_wait` reports whether it parked (added 2026-10-31, after review)

**The syscall.** `a1` is `1` when the call parked the caller and `0` when it returned at once because the ring
was already non-empty (the guard's path); a plain word, set in both arms of `sys_audit_wait`
(`crates/fjell-kernel/src/trap/syscall.rs`), the parked arm **before** `suspend_current` so it is already in
place when the wake resumes the task past the `ecall`. `fjell-syscall::sys_audit_wait` returns
`Result<bool, SysError>`. `auditd` prints `auditd: wait did not park` for `Ok(false)`.

**The assertion.** A new profile directive, `expect_observed = ["<begin> => <end> => <required>"]`
(`audit_trail::judge_observed`), requires at least one line containing `<required>` between the first `<begin>`
line and the first `<end>` line after it; a missing begin or end marker is an error. `neg-test` now prints
`neg-test: audit sweep begins` / `ends` around the 120 rounds. The `audit` profile comment says in capitals
what it is for: **to detect the sweep ceasing to test, not to assert a kernel property.**

**The red** (the point of R11): with the sweep shortened to 60 rounds (the window is at k = 78), every older
check stays green — 120 probes echoed in time, the burst accounted — and only the new one fails:
`observation check FAILED — no line containing `auditd: wait did not park` between `neg-test: audit sweep
begins` (line 797) and `neg-test: audit sweep ends` (line 1106): the sweep never reached the path it exists to
test`. That is the failure R11 was written to make loud. File restored, `cmp`-identical, rebuild identical to
the green build.

**Drift, measured.** I added N extra `sys_yield`s per record printed in `auditd` (scratch, reverted):
N = 3 moves the window from k = 78 to k = 80 (observed 1); N = 30 still finds it (observed 1), but the burst
accounting fails first (9 unrelated records dropped, allowed 3); N = 45 and 60 lose the window and the echo
check fails as well (auditd too slow for the 400-yield settle). So the window moves about two rounds per three
yields and the sweep's 120 rounds absorb a modest drift; the new check is the one that fails alone only when the
window leaves the sweep while `auditd` is still fast enough to settle, which the shortened sweep shows.

**Where the figures stand.** The window is entered in exactly one place per run (observed 1 in the sweep, 1 in
the whole log, at k = 78) and identically in 5 further runs of the profile on the final tree (the `test-all` run passed the same directive; its own log was overwritten by those repeats, so I quote the repeats).
