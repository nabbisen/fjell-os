# RFC-0.41-006: The intermittent boot fault gets a cause

**Status:** Accepted — accepted 2026-10-31
**Milestone:** 0.41
**Tracks.** **E-104** — about **1 boot in 270**, a task faults at `sepc 0x400aa`, `init`'s M6
block write then fails, `init` calls `sys_exit(1)`, and whichever QEMU profile is running fails on
the forbidden `TEST:<milestone>:FAIL` marker. **This blocks the 0.41.0 cut** (Gate 7).
**Touches** *(indicative)*: `crates/services/fjell-auditd/src/main.rs`,
`crates/fjell-kernel/src/trap/fault.rs`, `crates/fjell-kernel/src/main.rs` (the boot invariant),
`crates/fjell-kernel/src/mm/`, `crates/fjell-frame-alloc/`, `tests/qemu/`, `rfcs/ERRATA.md`.
**Relates to:** **E-097**, whose symptom class this shares and whose *cause* is fixed — its boot
invariant did not fire here, so this reaches the same class by a path that invariant does not
cover; **E-098**, E-097's strongest-lead-and-not-a-cause (`revoke_by_pa` *"clobbering the task's
text/rodata PTEs"*), which is the same symptom class again; **RFC-0.39-003**, which found E-097's
cause and added the invariant; **RFC-0.41-001**, which fixed the drop count `auditd` discarded —
R1 below is the same defect in the same renderer.

## What is wrong

**Measured, not inferred.** 2 failures in 19 full suites across the 0.41 line (the implementer's
1 in 11, the architect's 1 in 8 which stopped on its reproduction). At ~28 QEMU boots per suite
that is roughly **1 boot in 270**.

The signature, identical in every sighting:

```
{"seq":265,"kind":"task.fault","arg0":262314,"arg1":0,"result":0}
M6: block I/O error
TEST:V0.5-PLATFORM:PASS
TEST:M7:FAIL (init did not exit cleanly)
```

Four facts establish where this is and is not:

1. **It is not tier-specific.** Seen in `semantic` (pre-E-097-fix), `mmio`, and `svc` — and in the
   `svc` run, `mmio` **passed in the same suite**. The tier named in a summary is the victim, not
   the cause.
2. **The printed error is `init`'s own.** `M6: block I/O error` appears at three sites in
   `crates/services/fjell-init/src/main.rs` (`:724`, `:742`, `:753`), each guarding a
   `storaged_write` and each **followed by `sys_exit(1)`** — so the unclean exit is the
   consequence, not a second defect. That section runs in **every** boot, which is why no profile
   is safe. **The failing write is not always the same one**: in `svc` the error was the only `M6:`
   line; in `mmio` it came after `block write ok`, `store formatted or recovered` and
   `store append ok`.
3. **A task faults first, at the same PC, and passing runs have no such fault.** `arg0` is
   `tf.sepc` and `arg1` is `tf.stval` (`trap/fault.rs:36`), so this is a fault at **`0x400aa`**
   with `stval` 0. All 31 services link at `BASE_ADDRESS = 0x00040000`, so that PC is **170 bytes
   into a service's text** — in or near entry. Controlled against a freshly produced passing run of
   the same profile: **0 faults and 0 block I/O errors in 1747 lines**, against 1 and 1 in the
   failing run.
4. **The trail cannot say which task it was.** `AuditRecordBin` carries `task: u16` — *"Task slot
   index, or `0xFFFF` if not task-scoped"* — and `tick: u64`. **`auditd` prints neither.** The bytes
   at offset `0xaa` differ between service images, so the PC alone does not identify the service.
   **The one field that would answer the question is recorded by the kernel and dropped by the
   renderer.**

A task whose memory is wrong at entry is E-097's symptom class. E-097's cause — the kernel stack
overlapping frames the allocator handed out — is fixed, and its boot invariant stops the boot on
that condition. **The boot did not stop here.** So this is either a second path to the same class
or an incomplete fix, and which of those it is matters more than the symptom.

## Requirements

**R1 — the trail says which task, and when.** `auditd` prints `task` and `tick` with every record.
This is the diagnostic prerequisite for everything below, and it is **independently right**: it is
the same defect as the per-drain drop count RFC-0.41-001 fixed — a field the kernel records and the
only renderer discards. A trail that says "a task faulted" without saying which task is not a trail
an operator can act on.

Keep the shape machine-readable and do not disturb what the existing profile judges match; `0xFFFF`
must render as "not task-scoped" rather than as a slot number.

**R2 — identify the faulting task and whether its memory is intact at entry.** With R1 in place,
reproduce and name the task. Then use E-097's own technique rather than inventing one: a physical
check comparing the task's text frames against the embedded image at the point of the fault.
E-097's version found *"0 bytes differ in passing runs and ~1 KB differ in failing runs, the debris
being pointers into kernel statics"* — that is the measurement that turned a correlation into a
cause, and it is the measurement to repeat.

**R3 — explain why the boot invariant does not see it.** RFC-0.39-003's invariant stops the boot
when a free frame lies inside the kernel image or stack. It did not fire. Either the overlap is
with something else, or it happens after boot, or the invariant's extent is narrower than its name
suggests. **Say which**, because that answer decides whether the invariant needs widening or
whether this is a different mechanism entirely.

**R4 — a cause, then a fix. Not a quieter tier.** Raising a timeout, retrying the tier, relaxing
the forbidden marker, or making `init` continue past a failed write are all **refused here**, as
E-097 refused them: each converts a visible intermittent failure into an invisible one. If the
cause turns out to be in the harness rather than the kernel, that is a legitimate finding — but it
must be shown, not assumed, and the fix must then be to the harness and not to the assertion.

**R5 — a rate, before and after, and a harness that can measure one.** The baseline is ~1 in 270
boots. A fix cannot be shown against that by running the suite a few times: at 25 minutes a suite,
demonstrating absence is impractical.

**Build a boot-level repro harness** — boot the image repeatedly, stop as soon as `init`'s M6
section has passed or failed, and report a count. A profile boot costs ~60 s because the profile
runs for a fixed duration; a harness that exits at the decision point should cost a fraction of
that and make hundreds of boots per hour possible.

**State the risk with it:** a harness that changes timing may not sample the same condition. So the
harness must first **reproduce the known rate** on the unfixed tree before it is trusted to show
absence after the fix. A harness that cannot reproduce the failure is measuring something else, and
that result must be reported rather than read as a fix.

**R6 — scope.** This line finds and fixes one cause. It does not refactor the allocator, revisit
E-098's `revoke_by_pa` workaround, or widen the boot invariant beyond what R3's answer requires.
If R3 shows the invariant should be wider, that is a requirement of this line; anything further is
not.

### Non-goals

* **E-098.** Named as a related symptom class, not scheduled here. If R2 implicates it, stop and
  escalate — that is a larger line.
* **The `store` tier's standing NOT-GATED failure.** Unrelated and already disclosed.
* **Making `init` tolerant of a failed block write.** That would hide exactly the signal this entry
  exists to follow.
* **Any change to what the profiles assert.** The assertions found this. They stay.

## Risks

**The rate makes every negative result weak.** At 1 in 270, a hundred clean boots prove little. R5
exists for that reason, and the honest failure mode of this line is a fix that cannot be
distinguished from luck. **If that happens, say so** rather than closing the entry on a quiet run —
E-104 is explicit that what closes it is a cause.

**The fault may be a symptom two steps from the cause.** A task faulting at entry means its text or
its page table was already wrong; the write that corrupted it may be long past and in another
subsystem. E-097 took a physical-memory comparison to localise, not reasoning from the fault.

**R1 changes operator-visible output** in the same release that already changed it twice (the gap
notice, and `TaskSwitch` no longer recorded). The 0.41.0 notes must describe the record line's final
shape once, not incrementally.

**The investigation itself perturbs the system.** Instrumentation that changes allocation order can
move or mask a placement-sensitive failure — E-097 was bimodal in a service's physical address, and
its rate varied with the working directory for reasons nobody explained. Prefer observation that
does not allocate; when instrumentation must allocate, say so and re-measure the rate with it in
place.

## Amendments — 2026-11-01, architect, after R1's review

**Two of this RFC's premises were wrong, and both errors are the architect's.**

**(a) "What is wrong" item 3 is withdrawn.** The fault at `sepc 0x400aa` is **not** an anomaly and
**not** evidence of corrupted memory: it is the deliberate null read in `fjell-svc-fault`
(`src/main.rs:35`), which is also why `stval` is 0. The console line is present in **27 of 27**
current passing serial logs. The "control" that appeared to exclude it counted `task.fault` **audit
records**, and that record is often dropped by the ring — the same passing log carries three
`{"gap"}` notices. RFC-0.41-001 added those notices so a reader would know records were lost; this
RFC's author then read a trail declaring itself incomplete as though it were complete. **A record
count is not an event count in a lossy trail.** Consequently **R2's physical text-vs-image
comparison is moot** (there is no wrong memory to find) and **R3's question rested on the same wrong
premise** — its answer is that nothing in the evidence points at the boot invariant, and widening it
is not part of this line.

**(b) "What is wrong" item 4 is withdrawn.** The kernel did **not** record `task` or `tick`:
`AuditRecord` was built with `tick: 0` (`audit/ring.rs:167`) and the drain hard-coded
`task: 0xFFFF` (`trap/syscall.rs:772`). The field existed and was never populated; R1 as written
would have printed `null` and `0` for every record. R1 is satisfied by the kernel change the
implementer added, and `tick` is **a trap count, not wall time** — the timer interrupt is never
enabled, so a timer tick reads 0 forever. **Ruled: keep the trap count**; an `unsafe` MMIO `mtime`
read on the append path would put a fallible read inside the one place 0.41-001 established must not
fail, for a wall clock nothing yet needs.

**What this line established instead, and it is the real result.** `init`'s `M6: block I/O error` is
a `storaged` write whose completion never arrived — `do_io` polls the used ring for 500,000
`sys_yield` rounds, then replies `WRITE_ERR`, and each M6 write site then `sys_exit(1)`s. Throttling
the disk so one request outlives the bound **reproduces the whole signature on demand**. What remains
unknown is what makes one request take seconds; host write latency measured ≤18 ms under load.

**R5's harness exists and honestly cannot measure the rate** — ~25,000 boots, zero natural failures
against 18–90 expected, across eight conditions. That negative, reported rather than banked, is what
R5 was for. **The remaining work is one natural failure captured with `storaged`'s new
`io timeout …` line**, which decides between "the stall is below the guest" (`avail=1 used=0
req_status=0xff`, the throttled control's shape) and "the device or the queue memory is wrong"
(`status & 0x40|0x80`, or `isr=1`) — the latter being the E-097 class, which would redirect this line.

**A separate defect found in passing is filed as E-105** (`storaged` writes every sector to LBA 0,
shifted and truncated, while `init` reports success) and is **not** in this line's scope.
