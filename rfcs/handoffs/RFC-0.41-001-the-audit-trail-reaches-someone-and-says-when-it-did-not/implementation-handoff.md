# Developer Handoff — RFC-0.41-001

**Governing RFC:** [RFC-0.41-001](../../accepted/RFC-0.41-001-the-audit-trail-reaches-someone-and-says-when-it-did-not.md)
**Milestone:** 0.41 · **Lands before RFC-0.41-002** — that line depends on this one
**Status:** inherited from the governing RFC (Accepted, 0.41)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design conflict,
**stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does a lease revocation raised after boot appear in `auditd`'s own output?** Today it does not,
and cannot: the drain loop waits on a message nothing sends. When an operator reading the console
sees that revocation, and a tier asserts it **there** rather than inside `neg-test`, this line is
done.

**Not the measure:** that `AuditWait` returns. That is the mechanism, not the property.

**Second measure, equal weight:** when the ring drops records, does the trail **say so at the point
it happened**? A trail with silent holes is the defect this line also fixes.

## 0.1 Traps, from the reading that produced the RFC

1. **The comment lies, so do not trust the neighbourhood.** `src/main.rs` says *"Other services (or
   init) send a message to endpoint 0 to trigger a drain."* No such sender exists. Three previous
   lines routed around this gap because the code reads as though the mechanism is present. Treat
   every comment in this path as a hypothesis.
2. **`--skip-build` cannot answer a reproducibility question.** `cargo xtask repro-check` passes
   `--skip-build` by default and only re-hashes what is already on disk. For a real two-build
   comparison run the tool directly: `cargo run -p fjell-repro-check`. This bit the architect in
   this very session.
3. **A comment-only edit can still move a binary.** `panic::Location` embeds line numbers, so
   shifting lines in a service's source changes its image. If you keep an edit line-count-neutral,
   say so; if you do not, expect the prebuilt to move and re-record the baseline in the same commit.
4. **The ring drops the *newest* record, not the oldest** (`audit/ring.rs:116`–`117`). Reason about
   overflow in that direction; the intuition from most ring buffers is backwards here.
5. **A lost wakeup is invisible by construction.** If your guard is wrong, the trail is silently
   incomplete in a *new* way while appearing to work. That is worse than the current defect, which
   at least stops obviously at boot.

## 1. Order of work

**Step 1 — the kernel side, with the IRQ path open beside you.**
`AuditWait = 61` in `crates/fjell-abi/src/syscall.rs` (61 is free; 60 is `AuditDrain`). A new
payload-free `BlockReason` variant in `crates/fjell-kernel/src/task/tcb.rs:112`. The block in
`trap/syscall.rs` modelled on `:1261`; the wake on the ring's append path modelled on
`trap/dispatch.rs:158`–`180`.

**Copy the lost-wakeup guard deliberately, not incidentally.** The IRQ path records the event when
the bound task has not yet waited, so the next wait returns immediately. Write down, in the commit
message, what your equivalent is and why it closes the window.

**Step 2 — `auditd`.** Drain until the kernel reports empty, then `AuditWait`. Delete the
`sys_ipc_recv(0)` **and** the false comment. The 64-record buffer stays; the loop changes.

**Step 3 — report the loss.** `src/main.rs:76` currently reads
`Ok((n_records, _n_dropped)) => {`. Use the value. When it is non-zero, emit a line that an
operator **cannot mistake for an audited event** — the trail is JSON Lines, so a dropped-record
notice must be distinguishable from a record by its shape, not only by its text. Choose, and say
why you chose it.

**Step 4 — `TaskSwitch` filterable at the append site, default off.** At the append site, not the
render site: a record filtered at the console has already consumed a ring slot. The default is the
architect's design position and the owner may reverse it, so **make the default one line to
change**, not a decision spread across three files.

**Step 5 — ABI snapshot, in the same commit as step 1.** Update `tests/abi/snapshot.json` so
`ci-abi-check` reports a declared addition. The surface is 498 today; say the new figure.

**Step 6 — the tier.** See §2. Write the tier before you believe step 1.

## 2. What must be shown, not asserted

**The race, really run (R4).** Produce a record in the window between "a drain returned empty" and
"the task called `AuditWait`", and assert it is still delivered. If you cannot hit that window
deliberately, say so plainly and describe what you did instead — **do not report an absence probe
as a race test.** A test that passes because the window was never entered is the exact failure
this project keeps finding.

**The post-boot revocation, where a person would read it (R10).** Assert on `auditd`'s own output,
not in `neg-test`. RFC-0.40-002 had to use `neg-test` because this consumer was blind; if that is
still necessary, the line is not done.

**The drop report (R6).** Force an overflow — `TaskSwitch` volume during service start-up will do
it if the filter is off, or drive it directly — and assert the notice appears **with the right
count**. A notice that always says "some records were lost" is not a report.

**Seen red.** For each new test, record that you saw it fail before the fix. A test authored
against fixed code and never seen red is not evidence.

## 3. Gates, and the ones that will move

Run, and read your own exit statuses — **check each step's own status, not the status of a pipeline**
(a gate piped into `grep` has let a bad commit through here before):

* `cargo xtask consistency-check --all` — 20 subchecks today.
* `cargo xtask test-all` — 33 tiers. `store` reports `success` while its raw log says
  `profile 'store' FAIL`; that is known and `continue-on-error`. **Do not let it cover a real
  failure** — read the raw log of the tiers this line touches.
* `cargo run -p fjell-repro-check` — the real two-build mode. Not the xtask default.
* `ci-abi-check` — must report the declared addition.

**Expected to move:** the ABI surface (498 → more), the `audit` profile, prebuilts for `auditd` and
the kernel, and the repro baseline — **re-recorded in the same commit as the rebuild**.

## 4. Escalate rather than decide

* If `AuditWait` appears to need a capability `auditd` does not already hold. It should not: the
  wake grants authority over nothing. If it does, the design is wrong, not your code.
* If closing the lost-wakeup window seems to need state in the ring that changes the record format.
  The format is a non-goal; `SCHEMA_VERSION` must not move.
* If the wake on the append path cannot be kept to "mark runnable, enqueue" — if it needs anything
  that can fail or block in that context.
* If any kernel caller turns out to depend on `TaskSwitch` records being present.

## 5. What to report

One review request, with: the new ABI figure; the race test and **how you hit the window**; the
post-boot revocation assertion and where it reads from; the drop-report shape and why it cannot be
misread as an event; the `TaskSwitch` default and the one place that changes it; every tier you saw
red and then green; the two-build repro result (stating it was **not** `--skip-build`); and the
prebuilt digests that moved with the baseline re-recorded alongside.

State any figure you did not measure yourself as not measured. A line count labelled as an
occurrence count has already cost this project one correction.
