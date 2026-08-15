# Developer Handoff — RFC-0.24-001

**Governing RFC:** [RFC-0.24-001](../../done/RFC-0.24-001-instrument-audit.md)
**Milestone:** 0.24
**Status:** inherited from the governing RFC (Implemented-with-Errata, 0.24.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This is an audit, not a fixing line

**You are not repairing anything.** You are answering one question, ~55 times:

> *What would make this instrument report success without having checked?*

Findings get **reported and dispositioned**, never fixed in-pass. Every line in
this project that mixed audit with repair nearly went unbounded, twice. When you
find something — and you will — write it down and move to the next instrument.

**The exception is narrow:** you may modify an instrument *only* as far as
writing a demonstration requires, and the RFC's non-goal holds otherwise.

## 0.1 What "done" means for one instrument

Four answers plus a demonstration:

1. **Claim** — what it claims, in one sentence, as a reader would understand it.
2. **Actual** — what it examines, read from the code, not the label.
3. **Modes** — which of the five taxonomy modes it could exhibit.
4. **Demonstration** — it observed **failing** on a deliberately broken input.

**An instrument with no demonstration is recorded `UNAUDITED`, never `sound`.**
That distinction is the whole integrity of this exercise. If something resists
demonstration — Gate 9 is manual, some CI jobs cannot be broken locally — record
`UNAUDITED` with the reason and move on. Do not manufacture a demonstration you
do not believe.

## 0.2 Design decisions settled — do not re-open

1. **One register, appended to as you go:**
   `docs/verification/instrument-audit.md`. One row per instrument, the four
   answers, and a status of `sound` / `finding` / `UNAUDITED`.
2. **Submit per pass, not at the end.** Four passes, four review requests. The
   RFC makes passes independently cuttable; that only works if each is reviewed
   as it lands.
3. **Timebox per instrument.** The question is narrow. If an instrument is
   taking disproportionate effort, record what you have, mark it `UNAUDITED`
   with the reason, and move on. Depth on one instrument is worth less than
   coverage across the pass.

---

## 1. Change scope

**In scope:** `docs/verification/instrument-audit.md` (new); test files needed
for demonstrations. (§6 is withdrawn.)

**Explicitly NOT in scope:**

- **Fixing any finding.** Report it.
- E-013's fix (`fjell-kernel` has no `[lib]`). Its *finding* is pass 2's
  exemplar; its *fix* is architectural and stays a separate item.
- Rewriting the gate or tier harness.
- Adding new instruments. This audits the ones that exist.
- Any kernel, ABI, capability, lease, IPC, or crypto behaviour.
- Gate 12 `syscall-surface` must stay **35/26/9**.

---

## 2. Pass 1 — the twelve release-rehearsal gates

Highest release-criticality, so first. Gates 2, 3, 4, 11 and 12 were touched by
RFC-v0.22-001 and already have demonstrations — **verify those demonstrations
still hold and still fail**, rather than assuming; that is cheap and it is
exactly the assumption under examination.

Gate 9 is manual. It will be `UNAUDITED` with that as the reason, which is a
legitimate outcome, not a gap.

## 3. Pass 2 — the nineteen `test-all` tiers

Expect the richest findings here. Two are already known and serve as worked
examples of what a finding looks like:

- **Tier 1** skips `fjell-kernel` entirely (`--lib` with no `[lib]` target) —
  mode 1, scope blindness. Record it; do not fix.
- **Tier 5** auto-recorded a missing baseline and passed, until v0.21.3 —
  mode 3, fail-open. Verify the fix still fails closed.

For the QEMU tiers, a demonstration means deliberately breaking the thing the
marker names, running the profile, and observing it fail — the pattern used for
`v0.7-sync` in RFC-v0.23-002. Revert afterwards and confirm the tree is clean.

## 4. Pass 3 — the eight committed artifacts

`trust-report.txt`, `v1-readiness.md`, `abi/snapshot.json`,
`repro/baseline-digests.txt`, `syscall/expected.toml`, `ERRATA.md`,
`v1-limitations.md`, `rfcs/README.md`.

These are mode 5 candidates — a committed record asserting state that no longer
holds. The question per artifact: **what makes this go stale, and would anything
notice?** `trust-report.txt` sat at `Version: 1.0.0` for months because every
gate run regenerated it and the convention was to revert that.

A "demonstration" here means: make the artifact stale, and show whether any
instrument catches it.

## 5. Pass 4 — the sixteen CI jobs

Lowest priority, and the most likely to be `UNAUDITED` — many cannot be broken
locally. Record what you can determine by reading, mark the rest honestly.

## 6. ~~One small adjacent fix~~ — WITHDRAWN, the premise was false

**Withdrawn 2026-08-02.** This asked for `publish = false` to be added to ten
manifests said to lack it. **All 89 already have it.** The architect's
measurement used `grep "^publish = false"` against manifests that use aligned
formatting (`publish               = false`), so a whitespace-brittle pattern
reported ten false negatives.

Verified by the implementer during Pass 1, who checked rather than following the
instruction, and confirmed with `git blame` that the lines predate this RFC.

Left in place rather than deleted, because a handoff instruction derived from a
bad measurement is itself an instance of what this RFC audits — and because a
withdrawn item that vanishes teaches nothing. **No action.**

## 7. Prohibited shortcuts

- Do not fix a finding. Report it.
- Do not record an instrument as sound without a demonstration.
- Do not manufacture a demonstration you do not believe — `UNAUDITED` is honest.
- Do not argue a finding away as acceptable. Apply the E-012 test: *would this
  classification stand with no gate watching?*
- Do not let one instrument consume the pass.
- Do not mark unexecuted commands as passed.

## 8. Required evidence, per pass

1. The register rows for that pass, with all four answers each.
2. **Each demonstration, run and shown failing** — the point of the line.
3. A count: instruments audited, findings raised, `UNAUDITED` with reasons.
4. `cargo xtask release-rehearsal` still green, and Gate 12 still 35/26/9.

## 9. Review request

Standard format, in `.git-exclude/review-request/`, **one per pass**.

Flag for focused review: any instrument you were tempted to mark sound without
a demonstration, and any finding you think might be arguable — those are the two
places this line will fail if it fails.
