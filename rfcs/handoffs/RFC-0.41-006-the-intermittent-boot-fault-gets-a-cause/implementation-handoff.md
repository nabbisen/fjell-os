# Developer Handoff — RFC-0.41-006

**Governing RFC:** [RFC-0.41-006](../../accepted/RFC-0.41-006-the-intermittent-boot-fault-gets-a-cause.md)
**Milestone:** 0.41 · **Blocks the cut** — E-104 is OPEN and Gate 7 refuses the rehearsal
**Status:** inherited from the governing RFC (Accepted, 0.41)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design conflict,
**stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Why does a task fault at `0x400aa` about one boot in 270, and what stops it?** Not "does the
suite pass now" — at that rate the suite passes most of the time regardless. The measure is a
**named cause** and a **measured rate against a baseline**.

**You found this and reported it instead of re-running until green.** That was right, and nothing
below is a criticism of the work that surfaced it.

## 0.1 What is already established — do not re-derive it

* **Rate: 2 failures in 19 full suites**, ~28 boots each → **~1 in 270 boots**. (Your 1-in-11, my
  1-in-8 which stopped on its reproduction.)
* **Not tier-specific:** `semantic` (pre-fix), `mmio`, `svc`. In the `svc` failure **`mmio` passed
  in the same suite**.
* **`M6: block I/O error` is `init`'s own**, three sites in `fjell-init/src/main.rs` (`:724`,
  `:742`, `:753`), each guarding a `storaged_write` and each followed by `sys_exit(1)` — so
  `TEST:M7:FAIL (init did not exit cleanly)` is the consequence. **The failing write is not always
  the same one.**
* **`arg0` is `tf.sepc`, `arg1` is `tf.stval`** (`trap/fault.rs:36`). So the fault is at PC
  `0x400aa`, `stval` 0. **All 31 services link at `BASE_ADDRESS = 0x00040000`** — I checked all of
  them — so that is **170 bytes into a service's text**, in or near entry.
* **Controlled:** a freshly produced passing `svc` run has **0 faults and 0 block I/O errors in
  1747 lines**; the failing one has 1 and 1.
* **The bytes at offset `0xaa` differ between service images**, so the PC does **not** identify the
  service.

## 0.2 Two traps I fell into. Read these before you run anything

1. **`tests/qemu/artifacts/<profile>/serial.log` is UNTRACKED.** Every run overwrites it and
   `git checkout` does **not** restore it. I "ruled out" the fault lead by comparing the failing log
   **against itself**, having not noticed the file had been overwritten — and I said so out loud
   before catching it. **Copy a log you care about out of the tree before the next run.**
2. **The per-tier logs (`NN-qemu-neg-*.log`) carry serial context only ON FAILURE** — 8 lines for a
   passing `svc`, 73 for a failing one. They **cannot** be passing controls, and a zero count in one
   means nothing. The only valid control is a passing `serial.log` produced on purpose.
3. And the one from last round, which applies doubly here: **`qemu-run` uses the committed
   prebuilt, and the service images are linked into the kernel.** Any source change needs
   `cargo xtask build-services` **and** `cargo xtask build` before it reaches a run.

## 1. Order of work

**Step 1 — R1 first, because nothing else is diagnosable without it.** `auditd` prints `task` and
`tick`. `AuditRecordBin` already carries both; the renderer drops them. `0xFFFF` must render as
"not task-scoped", not as a slot number. Do not disturb what the existing profile judges match —
check `expect_echoed` / `expect_accounted` / `expect_observed` still pass.

This is the same defect RFC-0.41-001 fixed for the drop count, in the same renderer. It is worth
landing on its own merits even if the rest of this line takes longer.

**Step 2 — R5's harness, and prove it before trusting it.** Build a boot-level loop: boot, stop at
the point `init`'s M6 section has passed or failed, count. **Then reproduce the known rate with it
on the unfixed tree.** A harness that cannot reproduce the failure is measuring something else —
report that rather than reading it as good news.

Expect this to be where the time goes, and it is the right place for it.

**Step 3 — R2: name the task, then look at its memory.** With R1 and R2's harness, catch a failure
and identify the task. Then repeat E-097's measurement rather than inventing one: compare the
task's text frames against the embedded image at the fault. E-097's version got *"0 bytes differ in
passing runs and ~1 KB differ in failing runs, the debris being pointers into kernel statics"* —
that is what turned correlation into cause.

**Step 4 — R3: why did the boot invariant not fire?** Answer it explicitly: a different overlap, an
overlap that happens after boot, or an invariant narrower than its name. That answer decides whether
widening it is part of this line.

**Step 5 — R4: the cause, then the fix.** No timeout, no retry, no relaxed marker, no tolerant
`init`.

## 2. What must be shown, not asserted

* **The harness reproduces the baseline rate** before any fix.
* **The faulting task named**, from the trail rather than from inference.
* **A physical comparison** of its text against the image, in a failing and a passing run.
* **A rate after the fix**, from the same harness, large enough to mean something against 1-in-270 —
  and if it is not large enough, **say so**.

## 3. Gates

`consistency-check --all` (21 subchecks) · `test-all` · `cargo run -p fjell-repro-check` (the real
two-build mode) · `abi-snapshot --verify` — regenerate **before** the rehearsal if the surface moves.
`auditd.bin` and the kernel will move; re-record the baseline in the same commit.

**The rehearsal will stay red at Gate 7 until E-104 closes.** That is expected and is not something
to work around. **Closing E-104 is the architect's call**, on a cause — and `v1-limitations.md` has
no E-104 row to rewrite because OPEN errata carry no row obligation; confirm that at closure rather
than assuming it.

## 4. Escalate rather than decide

* If R2 implicates **E-098**'s `revoke_by_pa` path. That is a larger line and an explicit non-goal.
* If the cause appears to be in QEMU or the host rather than the kernel.
* If the cause is in the harness — a legitimate finding, but the fix then belongs to the harness and
  the assertion stays.
* If instrumentation changes the rate. E-097 was placement-sensitive and its rate varied with the
  working directory for reasons nobody explained; if your instrument moves the rate, that is
  information, not an obstacle — report it.
* If you cannot distinguish a fix from luck. Say that plainly; it is the honest failure mode of this
  line and E-104 says what closes it is a cause.

## 5. What to report

One review request: the record line's new shape; the harness and **its reproduction of the baseline
rate**; the faulting task, named; the physical comparison in both directions; the answer to R3; the
cause; the fix; and the post-fix rate with an honest statement of what it does and does not
establish.
