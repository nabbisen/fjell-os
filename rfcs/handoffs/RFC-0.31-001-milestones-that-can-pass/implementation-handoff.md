# Developer Handoff — RFC-0.31-001

**Governing RFC:** [RFC-0.31-001](../../accepted/RFC-0.31-001-milestones-that-can-pass.md)
**Milestone:** 0.31
**Status:** inherited from the governing RFC (Accepted, 2026-09-12)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This is the line RFC-0.29-002 stopped one arm short of

R5 of that RFC deleted `v0.6-verification` from `smoke.rs` because nothing
emitted its marker. It searched for *that marker* and found nothing; it did not
ask which of the milestones the file offers can pass at all. Six more were in
the same `match`, and E-015 was closed over them. **You are finishing that
sweep, and the point of D2 is that nobody has to finish it a third time.**

## 0.1 Re-derive the history before touching anything (R1)

The RFC's table was built from `git log -S`. Rebuild it:

```
for n in 1 2 3 4 5 6; do
  echo "M$n kernel:"; git log --oneline -S"TEST:M$n:PASS" -- crates/fjell-kernel
  echo "M$n init  :"; git log --oneline -S"TEST:M$n:PASS" -- crates/fjell-init
done
git log --oneline -S'Emit milestone markers atomically' -- crates/fjell-kernel
grep -n 'kprintln!("TEST:' crates/fjell-kernel/src/trap/dispatch.rs
```

Expect: **kernel empty for every n in 1…6**; init carrying M4→M5→M6 in
succession, each replacing the last; the kernel emitting exactly `M7` (PASS
and FAIL), `M8`, `V0.4-NET`, `V0.5-PLATFORM`, `V0.7-SYNC`. Report any row that
differs, in either direction. Ten consecutive lines have corrected a figure of
mine; the history table is nine `git log` invocations and I would not be
surprised.

## 0.2 Settled — do not re-open

1. **The six route to `unknown milestone`** (D1). No QEMU boot for a name
   nothing can pass.
2. **A bidirectional check reads `dispatch.rs`** (D2): every accepted marker
   is emitted; every emitted `…:PASS` is accepted.
3. **One list** feeds the `match`, the usage string, and the check (D3).
4. **Nothing is reconnected** (D4). M7 is the cumulative pass; the kernel says
   so at `dispatch.rs:465`.
5. **`SMOKE_PROFILES` stays four.** What `test-all` gates is a different set
   and not this line's question.

---

## 1. Order

**R1 (re-derive) → §5 answered in writing → R2 (delete the six, D3 the usage
string) → D2 check built → R4 (both directions demonstrated failing) → R6
(the module comment) → R5 (E-040 closed) → evidence.**

§5 before code: the shape you choose decides whether there is a list to
delete from at all.

## 2. §5 — and I have named my own counter-argument

**Where does the accepted-milestone list live?** I lean to shape 1 — a
`const` of `(name, marker)` pairs, checked against kernel source by the D2
test. The change is small and the test makes a hand list safe.

**The argument against me is RFC-0.29-001.** It derived the negative-test
categories from the profiles on disk instead of keeping a checked list, and
that was right: the list had already gone stale three separate ways. Say
whether the smoke set is different in kind — five names, changing perhaps
once a milestone, versus fourteen profiles that change when anyone adds one —
or whether I am under-reaching because the derived version is more work.
Either answer is acceptable; **"the architect leaned to 1" is not**.

If you choose 2 or 3, the `m8` default (no argument → `m8`) stays an explicit
rule, never "the last marker in the file".

## 3. D2 — the check must be blind to nothing

The check parses `dispatch.rs` for `kprintln!("TEST:…:PASS")` literals. A
marker built by `concat!` or a `const` would be invisible to it, and
**invisible reads as absent, which reads as "nothing to check"** — mode 3 of
the defect class, fail-open on absence. So the check also asserts that every
occurrence of `TEST:` in the file is inside a literal it recognised. If
someone later changes how a marker is constructed, the check fails on the
construction, not silently on the missing marker.

`callsite_audit` reads kernel source from a test already; follow its shape.

## 4. R4 — both directions, on real files

Demonstrate each, then revert, with `git status` empty after:

1. **An accepted marker nothing emits** — add a bogus arm (or leave one of the
   six in place while building the check). The check names it.
2. **An emitted marker nothing accepts** — comment out one real arm (`m8` is
   the clearest). The check names `TEST:M8:PASS` as emitted-but-unaccepted.
3. **A marker the parser cannot see** — temporarily turn one `kprintln!`
   literal into a `concat!`. The check fails on the construction (§3), not by
   passing with one fewer marker.

Three demonstrations, not two. The third is the one that keeps the check
honest.

## 5. Prohibited shortcuts

- Do not wire up M1–M6, and do not leave the arms in "for history" — the
  history is in the RFC and the erratum.
- Do not build the D2 check to read a list of expected markers that is itself
  a hand copy. It reads `dispatch.rs`.
- Do not skip demonstration 3.
- Do not touch `SMOKE_PROFILES`, `test_all.rs`, the kernel, or any service.
- Do not close E-040 without R6 — the module comment *"preserved verbatim
  from the v0.1.0 runner"* is the sentence that explains the erratum and must
  say so, not be quietly deleted.
- Do not run `cargo fmt --all --check` in your head.

## 6. Required evidence

1. **R1 re-derivation**, with every disagreement against the RFC reported.
2. **§5 answered in writing**, engaging the RFC-0.29-001 counter-argument.
3. `cargo xtask qemu-test m5` → `unknown milestone`, in under a second, no
   QEMU boot, and the `known:` line listing exactly the five.
4. **Three failing demonstrations** (§4), transcripts, reverted.
5. **E-040 `CLOSED`** with `v1-limitations.md` in the same commit; R6 done.
6. `test-all` all tiers (`SMOKE_PROFILES` unchanged, four smokes still run);
   `release-rehearsal` green; `consistency-check --all` 11/11.
7. `cargo fmt --all --check`.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Your §5 answer**, and whether you think I am under-reaching.
- **Demonstration 3** — what the check said when the literal became a
  `concat!`.
- Anything in the history table that disagreed with the RFC.
- Anything else in `smoke.rs` or `qemu_run.rs` that turned out to be
  preserved from a runner that no longer exists.
