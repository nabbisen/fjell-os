# Developer Handoff — RFC-0.30-002

**Governing RFC:** [RFC-0.30-002](../../proposed/RFC-0.30-002-checks-that-name-themselves.md)
**Milestone:** 0.30
**Status:** inherited from the governing RFC (Proposed — awaiting owner acceptance)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. These are small, and that is exactly why they slipped

Both errata were dated `0.29`. Nothing forced them, larger work displaced them,
and `errata-tracking` refused the 0.29.0 cut on both. They are now dated `0.30`.

**A second slip would be a pattern rather than an accident.** They go first in
the milestone, not last.

## 0.1 Reproduce E-038 before you fix it — it is not what the erratum says

```
$ mv rfcs/accepted /tmp/holdout
$ cargo run -p fjell-consistency-check -- --all
```

You will see **four** affected subchecks, not three, and three of them do print
something:

```
--- consistency-check: rfc-status-folder ---
consistency-check: cannot read rfcs/accepted
--- consistency-check: handoff-status ---
--- consistency-check: errata-tracking ---
consistency-check: cannot read rfcs/accepted
--- consistency-check: doc-counts ---
consistency-check: cannot read one or more rfcs/ lifecycle folders
```

`handoff-status` prints its header and **nothing else** — it is the only truly
silent one, and the erratum does not name it.

**Move the folder back afterwards** and confirm `git status` is clean. Do not
delete it: it holds a tracked keeper and a tracked RFC.

## 0.2 Design decisions settled — do not re-open

1. **Every subcheck ends with a result line naming itself** (D1). The point is
   not the message; it is that a reader can tell which of ten checks failed.
2. **Fix the class** (D2) — make it structurally hard for an eleventh subcheck
   to be added without one.
3. **Diagnose `handoff-status` before matching it to the others** (D3). It fails
   differently and the difference may matter.
4. **E-035 must not fail a legitimately mid-milestone tree** unless §5 decides
   it should (D4).

---

## 1. Order

**Reproduce E-038 → R1/R2 → §5 answered → R3 (E-035) → R4 demonstrations →
close.**

## 2. §5 — and my lean has a real cost I want argued

**When is the ABI enumeration owed?** I lean to shape 1 — fail Gate 4 on any
`Added != 0`, so the enumerate-and-regenerate happens in the line that adds the
item, where the knowledge is. That is RFC-0.24-003's whole argument.

**But shape 1 makes `release-rehearsal` red during ordinary work**, between the
addition landing and the regeneration. A gate that is routinely red is a gate
people learn to ignore, and that may be exactly why it was never done this way.

If you choose shape 1, say what stops it becoming background noise. If you
reject it, say what enforces the step instead — *"the release cycle documents
it"* is what we have now, and it is what E-035 is.

## 3. E-038's erratum text is wrong and gets corrected, not just closed

It says *"three subchecks emit no output at all."* It is **four**, and three of
them emit a message — what none emits is a **result line naming itself**.
Correct the entry as part of closing it; a closed erratum that misdescribes what
it closed is worse than an open one.

## 4. Prohibited shortcuts

- Do not fix the four by hand and leave the eleventh subcheck to remember.
- Do not make `handoff-status` match the others before understanding why it
  differs.
- Do not close E-038 without correcting its text.
- Do not take on **E-037**; it is the next line.
- Do not touch the kernel, the ABI surface, or any service.
- Do not run `cargo fmt --all --check` in your head, and put it in the evidence
  list.

## 5. Required evidence

1. The E-038 reproduction, before and after, showing **four** named result lines
   where there were none.
2. **`handoff-status`'s diagnosis** — why it printed nothing.
3. **§5 answered in writing**, with the cost of shape 1 addressed directly.
4. E-035's enforcement, **demonstrated failing** on a deliberately
   un-regenerated baseline.
5. Whatever makes an eleventh subcheck structurally unable to omit its result
   line, demonstrated.
6. **E-035 and E-038 `CLOSED`, with E-038's text corrected** — register and
   `v1-limitations.md` in the same commit.
7. `release-rehearsal` green; `test-all` all tiers; `syscall-surface` 35/29/6;
   `callsite-audit` 5 checks.
8. `cargo fmt --all --check`.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Your §5 answer**, and specifically whether a routinely-red Gate 4 is
  acceptable. That is the judgement I most want argued rather than assumed.
- What `handoff-status` was actually doing.
- Anything that turned out **not** to have the defect.
- Any count of mine you re-derived and found different. Nine consecutive lines
  have corrected one, and the last was an assumption I had built a risk section
  on.
