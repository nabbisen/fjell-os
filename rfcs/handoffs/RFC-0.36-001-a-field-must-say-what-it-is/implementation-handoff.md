# Developer Handoff — RFC-0.36-001

**Governing RFC:** [RFC-0.36-001](../../accepted/RFC-0.36-001-a-field-must-say-what-it-is.md)
**Milestone:** 0.36
**Status:** inherited from the governing RFC (Accepted, 0.36)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can a reader of a diagnostic bundle tell what each field means?** Today: a
32-byte zero measurement head reads as a real digest, a 32-byte zero attestation
reads as a real one and was never even asked for, and a fabricated intent reads as
a real record inside something the builder signs. When every field says what it is —
obtained, unavailable, or absent — and the two readiness sends are shown arriving,
this line is done.

**Not the measure:** that `diagnosticsd` works. It still cannot be asked for a
bundle (**E-078**), and R8 exists so that three closures cannot read as though it
can.

## 0.1 Two things to do before anything else

1. **Finish the sweep** (R1). Four findings are named; the RFC asks you to account
   for **every** field of `DiagnosticBundle` and say where its value comes from.
   E-079 exists because E-076's entry asked for this sweep and nobody had done it.
   **If you find a fifth, stop and escalate** — do not widen the line quietly.
2. **Take the goldens** (R2) from the unmodified encoder. `golden.rs` will refuse a
   regenerated digest without a version bump and a note, which is a safety net and
   not a substitute: pin the bytes you are about to change *before* changing them.

## 1. Order

1. **R1** — re-derive, complete the sweep, report.
2. **R2** — goldens, unmodified encoder.
3. **R3/R5** — the two availability flags, adjacent to their fields;
   `DIAG_BUNDLE_VERSION` 2 → 3; frozen description regenerated; `golden.rs`
   updated with a note; the eighteen others shown unchanged.
4. **R4** — delete the fabricated intent.
5. **R6** — both readiness sends, plus the observable and the tier assertion.
6. **R7** — `capability-topology`'s waivers.
7. **R8** — the register and limitations, one commit, with E-078 visibly open.
8. **R9** — the gates, the rebuild, the baseline, CI.

## 2. Traps specific to this line

1. **Do not give the intent a flag.** A flag says *"I could not get this"*; the
   intent was **invented**. Deleting it is the fix. The register says so, so a later
   reader does not "restore" a record that asserted something untrue.
2. **Flags go beside their fields, not at the end of the struct.** The end is where
   the diff is smallest and the description is worst. `bundle-v1.frozen` is the only
   description a second implementation has.
3. **Say what the absent case writes.** RFC-0.35-004's review had to add exactly
   this note to the chain digest: *fixed width, zeroed when absent, not omitted and
   not arbitrary.* Do it here as you go rather than after review.
4. **Whatever sets a flag `true` must be the code that obtained the value.** A
   default of `true`, or a flag set beside a call whose result was discarded, is
   worse than the zero digest it replaces.
5. **Slot 4's waiver must name E-078 after this line, not E-076.** E-076 will be
   closed, and a waiver citing a closed entry is a defect review found last
   milestone.
6. **Per-service readiness output changes the console.** Check `semantic.toml`'s
   `clean_console` and any marker-count assertion before assuming a new line is
   free.
7. **Guest services change, so prebuilts will move.** Rebuild **then** re-record
   the baseline, in the same commit (trap 2), and `fjell-repro-check` exits **2**
   for a usage error and **1** for a real failure — pass only the flags its
   `--help` names, never beside a live `test-all`.

## 3. What to put in the review request

- R1's sweep: **every** `DiagnosticBundle` field, where its value comes from,
  whether that path can fail, and what you did about it.
- The goldens, and the moved encoder reproducing them for the unchanged cases.
- Both flags, their position in the struct, and what the description says about the
  absent case.
- The deleted intent, and the register's wording about why it is not a regression.
- R6: the two sends, the observable, and the tier output showing
  `service-manager` actually sees both services ready.
- `capability-topology`'s new counts and slot 4's re-pointed waiver.
- E-075, E-076, E-079 closed; **E-078 untouched**, and the wording you used so
  three closures do not read as a working diagnostics path.
- Each gate by its own exit status, as the required set, failures included; the
  rebuild and baseline; a CI run id.

Hand back one path: your review request. Do not tag anything.
