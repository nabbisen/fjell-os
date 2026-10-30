# Developer Handoff — RFC-0.41-002

**Governing RFC:** [RFC-0.41-002](../../accepted/RFC-0.41-002-no-service-receives-on-a-shared-endpoint-object.md)
**Milestone:** 0.41 · **Lands after RFC-0.41-001** — hard ordering, see §0.2
**Status:** inherited from the governing RFC (Accepted, 0.41)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design conflict,
**stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can two tasks receive on one endpoint object?** Today two do, on object 0, and only the absence
of any sender keeps it from firing. When no task receives on object 0, when no default arm hands
out a capability to a shared object, and when a **check rejects** the two-receiver shape, this line
is done.

**Not the measure:** that `auditd` and `netd` no longer collide. That is the instance. The default
grant is the defect.

## 0.2 Why the ordering is hard

RFC-0.41-001 removes `auditd`'s receive on object 0 **as a consequence of its redesign** — it will
receive on no endpoint at all. If you start here first you will build a separation for a receiver
that is about to vanish, and the two lines will conflict in `spawn.rs`. Wait for 0.41-001 to land.

## 0.1 Traps

1. **The grant is not the send.** The architect's enumeration established that nothing *sends* to
   object 0. It did **not** establish that no image *reads* slot 0 — a presence check, an
   unconditional install, a test fixture. R4 exists for that, and it is the risky part of this
   line.
2. **A text search is a hypothesis.** For "does this image use slot 0?", `grep` finds candidate
   sites; whether a service depends on the capability is behavioural. Resolve it by running the
   profile that exercises the service, and say which images you checked by which means.
3. **A service can fail later than boot.** A green boot does not show that an image which lost the
   grant still works. Run the profiles per service, not in aggregate.
4. **`netd`'s own comment is wrong.** `src/main.rs:78` calls slot 0 *"the netdev endpoint"*. It is
   the shared default. Do not take it as evidence that `netd` owns an object.
5. **`grep` here skips NUL-containing files** — use `/usr/bin/grep -a`. A blind "0 sites" has
   closed an erratum wrongly in this project before.

## 1. Order of work

**Step 1 — confirm 0.41-001 has landed** and that `auditd` receives on nothing. If it still has a
receive loop, stop.

**Step 2 — the survey, before any removal (R4).** For each of the 20 images that fall to `_ => 0`,
record whether slot 0 is read at all, and by what evidence. Produce a table: image, verdict, how
established. **This table is the deliverable that makes step 4 safe** — do not compress it to a
sentence.

**Step 3 — `netd` gets its own object (R2).** A named constant, a dedicated arm in the `ep_obj`
match (`crates/fjell-kernel/src/task/spawn.rs:272`), the service receiving on it. Follow
RFC-0.28-001's shape for `service-manager`. Replace the false comment with a true one.

At the end of this step, **no task receives on object 0.** Verify it directly and say how.

**Step 4 — `_ => 0` stops granting an endpoint (R3).** The default arm hands out **no** endpoint
capability. Expect this to be the work. If step 2 found an image that genuinely needs an endpoint,
**it earns a dedicated arm** — it does not get the shared one back. Escalate before adding an arm
for an image whose need you are unsure of.

**Step 5 — the check (R5).** Add the invariant where topology is already checked —
`capability-topology` is the natural home. **Then break it on purpose:** construct the
two-receiver shape, watch the check name it, and remove the construction. Record both halves.

**Step 6 — the stale comment in `fjell-sample-service` (R6).** It names *"auditd/bootctl's
shared-object-0 receive loops"*. `bootctl` has had its own object since RFC-0.33-001 D8 and after
0.41-001 `auditd` has none. Say what is true: the send goes to the service's own dedicated
object 6, which is why it is safe.

## 2. What must be shown, not asserted

* **No receiver on object 0**, established directly rather than by reading the diff.
* **The check fails on the bad shape.** Absence of a complaint on a fixed tree is not evidence that
  a check works. This is the requirement most likely to be skipped and the one the architect will
  look for first.
* **Each affected service exercised**, with the profile named. Say which services were only built
  and not run — that is an honest gap, and hiding it is not.

## 3. Gates, and the ones that will move

* `cargo xtask consistency-check --all` — `capability-topology` reads **6 installed / 1 waived /
  7 checked** today and will move. State the new figures.
* `cargo xtask test-all` — 33 tiers. Read the raw logs of the profiles exercising the affected
  services; a green aggregate is not enough here.
* `cargo run -p fjell-repro-check` — real two-build mode, not the xtask's `--skip-build` default.

**Do not widen `capability-topology`'s existing waiver.** It waives one declared-but-not-installed
slot (DIAGNOSTICSD slot 2) to a named line. A waiver that absorbs new absences stops being a record
of one accounted-for exception.

**Expected to move:** prebuilts for `netd`, `sample-service`, the kernel, and any image whose
slot-0 handling changes; the repro baseline, re-recorded in the same commit as the rebuild.

## 4. Escalate rather than decide

* If an image needs an endpoint and you cannot tell which object it should own.
* If removing the default grant makes a service fail in a way you cannot attribute.
* If the check cannot express "no two tasks receive on the same object" without reading state it
  has no access to.
* If `netd`'s dedicated object appears to need a protocol change. It should not; this is a topology
  change.

## 5. What to report

One review request with: the step-2 survey table in full; how you established that no task receives
on object 0; the check, **and the failing case you ran through it**; the new
`capability-topology` figures; each service exercised and by which profile, with the
build-only ones named; the prebuilt digests that moved; and the two-build repro result.
