# Developer Handoff — RFC-0.28-005

**Governing RFC:** [RFC-0.28-005](../../done/RFC-0.28-005-scan-scope-and-sweep.md)
**Milestone:** 0.28 — **this line is what stands between 0.28 and its cut**
**Status:** inherited from the governing RFC (Implemented, 0.28.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This is a sweep, and sweeps are where scope quietly grows

Three errata, none large. The failure mode for a line like this is not a bug —
it is finishing with eleven other things fixed and none of the three closed.

**E-015 is the obvious temptation.** E-025 is one instance of it, the family is
sitting right there, and closing it would feel like the real work. It is a
non-goal. If the work makes the family look cheap, **escalate** — that is a
finding worth having, and a different line.

## 0.1 §6 first, and the obvious answer is wrong for one tool

**Should the tools scan what `git` tracks, or the filesystem minus exclusions?**

"Bounded by `git ls-files`" is clean, principled, and immune to scratch
directories. It is also wrong for `unsafe-audit`: that tool would stop seeing a
new `.rs` file until it was committed, and **uncommitted work is exactly when an
unsafe-site audit should speak up.**

My lean is shape 3 — different tools, different jobs, each answer argued. It is
a lean. Argue it; shape 1's simplicity is a real advantage and I may be
over-fitting to one tool's needs.

## 0.2 Design decisions settled — do not re-open

1. **Scope derived, not enumerated** (D1). Three hand-written lists that
   pairwise disagree is E-015 caught in the act.
2. **One mechanism, not three** (D2) — whatever §6 chooses.
3. **E-029's annotation is superseded, not deleted** (D3). The archived
   `RFC-0.26-002` citation is left alone entirely.
4. **Everything here is an instrument change** (D4) — demonstrated failing
   first, per RFC-v0.22-001.

---

## 1. Order

**§6 answered → R1 (E-025) → R2 (E-030) → R3 (E-029) → all three closed.**

R1 first because §6's answer is only real once it survives contact with both
walkers. R3 last: it is a re-run and a promotion, and it wants a tree that is
otherwise settled.

## 2. R1 — the demonstration is the symptom, not a unit test

D5 is specific: generate the trust report **with a checkout under
`.git-exclude/tmp/`**, before and after, and show the unsafe inventory going
**622/622 → 311/311** and the cap-manifest count **2 → 1**.

A unit test proving the walker skips a directory is necessary and not
sufficient. The thing that went wrong was a whole-report number that looked
plausible; the demonstration has to be that number.

**Then check the other direction:** on a tree with *no* scratch directories, the
count must be **311 before and 311 after**. If it moves, the new bound excluded
something real, and that is a finding to report rather than a number to accept.

## 3. R2 — the pair, not the file

`version-currency` currently checks `README.md`. It must also check that these
two agree:

```
Cargo.toml                     [workspace.package] version
crates/fjell-os/Cargo.toml     fjell-abi = { path = "...", version = "<same>" }
```

Demonstrate it on a deliberately mismatched pair. Note the failure is
**fail-closed** — a mismatch stops the workspace resolving — so this check buys
time at a cut, not correctness. Say that in the check's own message, so whoever
hits it knows what it did and did not save them.

## 4. R3 — supersede, do not tidy

Re-run `semantic`, promote with `cargo xtask evidence promote`, real provenance.
Then cite the new log in `RFC-0.26-004-readiness-channel-answer.md`
**alongside** the existing annotation.

The annotation is the honest record of what was true between 0.26 and 0.28.
Deleting it once a fresh log exists would erase the fact that the evidence was
missing — which is the thing E-029 was filed to record.

## 5. Prohibited shortcuts

- Do not take on E-015. Escalate if it looks cheap.
- Do not fix E-025 by adding `.git-exclude` to two literal lists — that is the
  defect, applied twice more.
- Do not delete E-029's annotation.
- Do not accept a moved `unsafe` count on a clean tree.
- Do not touch the kernel, ABI, or any service.
- Do not claim host coverage for R3 — E-013; the promoted log is the evidence.
- Do not run `cargo fmt --all --check` in your head, and put it in the evidence
  list.

## 6. Required evidence

1. **§6 answered in writing**, with the rejected shapes and the `unsafe-audit`
   cost addressed explicitly.
2. R1's before/after with a scratch checkout present (**622 → 311**, **2 → 1**),
   and the clean-tree check that 311 does not move.
3. R2 demonstrated failing on a mismatched version pair.
4. R3's promoted log, with the annotation still present alongside the new
   citation.
5. **E-025, E-029, E-030 all `CLOSED`** — register and `v1-limitations.md` in
   the same commit.
6. `release-rehearsal` green; `test-all` **21/21**; `syscall-surface`
   **35/29/6**; `callsite-audit` 5 checks.
7. `cargo fmt --all --check`.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Your §6 answer**, particularly if you chose one rule for all three tools —
  that is the answer most likely to be right for the wrong reason.
- Any count that moved on a clean tree.
- Anything E-015 made look cheap, so I can decide whether it is a line.
- Any count of mine you re-derived and found different. Four lines running have
  corrected one; I would be more surprised by none than by another.
