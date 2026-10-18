# Developer Handoff — RFC-0.37-001

**Governing RFC:** [RFC-0.37-001](../../done/RFC-0.37-001-a-bundle-gets-asked-for-and-produced.md)
**Milestone:** 0.37
**Status:** inherited from the governing RFC (Implemented, 0.37.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can something ask for a diagnostic bundle and get one that says what it
contains?** Today nothing can ask, the asker has exited, and the one field with a
real source behind it is addressed with a tag its server does not answer. When a
console byte produces a signed bundle whose availability flags are `true` for what
was obtained and `false` for what has no source, this line is done.

**Not the measure:** that a bundle was built. A bundle of honest zeros would pass
that. **The flags are the assertion.**

## 0.1 Order is load-bearing here, more than usual

**E-083 closes before any capability is installed.** If you install slot 4 first,
`query_measurement_head` sends `0x320` to a service whose arm is `0x303`, gets
nothing, and the flag stays `false` — and the install will look like it did
something. Worse, a careless line would set the flag `true` on a digest it never
received. **Fix the tags, demonstrate the mismatch first, then install.**

## 1. Order

1. **R1** — re-derive all three findings with controls, **and sweep for a fourth**
   locally-declared tag that duplicates or collides with a shared one. E-083 came
   out of exactly this sweep. If you find one, **stop and escalate**.
2. **R2** — E-083: the shared `measuredd::{GET_HEAD, HEAD_REPLY}`, the local
   `0x320`/`0x321` gone, the five `SXT_*` constants from one declaration. Show the
   mismatch failing first.
3. **R3** — `fleetd` becomes a service, with something asserting it is alive after
   `ready`.
4. **R4** — `diagnosticsd`'s receive slot installed; the install list justified.
5. **R5 (replaced 2026-10-18)** — slot 4 is **not** installed; the flag reads
   `false` and R6 asserts that it does. Your escalation withdrew the original R5:
   `measuredd`'s reply carries eight of thirty-two digest bytes, so there is no
   live digest to obtain (**E-085**). **Proceed with R2, R3, R4, R6 onward.**
6. **R6** — the console-triggered tier, asserting the flags at the deciding
   service's own output.
7. **R7** — `capability-topology` counts and waivers.
8. **R8** — E-078, E-081, E-083 closed; E-082 and E-084 untouched.
9. **R9** — gates, rebuild, baseline, CI.

## 2. Traps specific to this line

0. **R5 is withdrawn — do not install slot 4.** See the governing RFC's amended
   R5 and E-085. The flag reading `false` is the correct outcome and the tier must
   assert it.
1. **Installing a slot is not evidence the conversation works.** The tag mismatch
   is invisible from the install side: both sides compile, the capability exists,
   and nothing is received. Only a live exchange proves it.
2. **`fleetd` is the first program this project gives an event loop to.** Previous
   lines fixed loops that misbehaved. Expect `MAX_TASKS`, stack and endpoint limits
   — the symptom when one is hit is **`init: spawn error`** in `health-fail`, not a
   failure in your own tier.
3. **A flag set by anything other than the code that got the value is worse than
   the zero it replaces.** RFC-0.36-001 wrote a test for this
   (`unavailable_fields_are_zeroed_not_the_callers_raw_value`); make sure it still
   means something when a value is genuinely obtained.
4. **`push_bundle` must keep failing closed.** Slot 2 stays out. If you find
   yourself installing it to make something pass, that is the export boundary and
   it is not in this line.
5. **The dead section header in `fleetd` must not survive.** `// ── IPC event loop
   (v0.8.1 …) ───` above a real loop is fine; above anything else it is the defect
   E-081 named.
6. **Guest services change, so prebuilts move.** Rebuild **then** re-record the
   baseline, same commit (trap 2). `fjell-repro-check` exits **2** for a usage
   error, **1** for a real failure; pass only the flags `--help` names, never
   beside a live `test-all` (trap 7).

## 3. What to put in the review request

- R1's probes, controls, and the result of the fourth-tag sweep — including
  "nothing else" with the control that makes it credible.
- The tag mismatch demonstrated failing, then fixed.
- `fleetd`'s loop, its installed slots with the necessity argument for each, and
  the evidence it is alive after `ready`.
- The tier's output: the bundle, and **each flag's value with the reason** — every
  flag `false`, including `measurement_head_available` (corrected 2026-10-18: this
  line said `true` for it before R5's withdrawal; slot 4 is not installed in this
  line, so there is no live digest for the flag to report true of — see the
  governing RFC's amended R5 and its own R6 correction).
- `capability-topology`'s new counts and any re-pointed waiver.
- E-078, E-081, E-083 closed; E-082 and E-084 visibly untouched.
- Each gate by its own exit status, as the required set, failures included; the
  rebuild and baseline; a CI run id.

Hand back one path: your review request. Do not tag anything.
