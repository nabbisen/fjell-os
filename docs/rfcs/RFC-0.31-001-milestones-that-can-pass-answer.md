# RFC-0.31-001 §5 — Where does the accepted-milestone list live?

**Governing RFC:** [rfcs/done/RFC-0.31-001-milestones-that-can-pass.md](../../rfcs/done/RFC-0.31-001-milestones-that-can-pass.md)

Answered before R2, per the handoff's required order — the shape decides
whether there is a list to delete from at all.

---

## Answer: shape 1 — a `const` in `smoke.rs`, checked against kernel source by a test

`MILESTONES: &[(&str, &str)]` in `crates/fjell-tools/src/smoke.rs`. The
`match`, the `known:` usage line and the D2 check all read it; nothing else
carries a copy.

**The reason is not that the change is small.** The handoff rules that out
explicitly, and it should — "smaller" is how every un-derived list in this
project got justified once.

## The RFC-0.29-001 counter-argument, taken seriously

0.29-001 derived the negative-test categories from `tests/qemu/profiles/
*.toml` instead of keeping a checked list, and that was right: the hand list
had gone stale three separate ways. If that reasoning transfers, shape 2 or 3
wins here and shape 1 is under-reaching.

**It does not transfer, and the difference is what each derives *from*.**

| | 0.29-001 (derive was right) | Here (derive is worse) |
|---|---|---|
| Authority | `tests/qemu/profiles/*.toml` — a directory whose **file existence is the fact** | `crates/fjell-kernel/src/trap/dispatch.rs` — **Rust source in another crate** |
| Derivation | a directory listing | **string-scanning for `kprintln!("TEST:…")`** |
| Residual convention | none — filename *is* the category name | a marker→CLI-name rule (`TEST:V0.4-NET:PASS` → `v0.4-net`) **and** the `m8` default, both still hand-stated |
| If the derivation goes blind | impossible — a file either exists or does not | **entirely possible** — `concat!`, a `const`, a renamed macro |

0.29-001's derivation *removed* a literal-matching predicate and replaced it
with a structural fact. Shapes 2 and 3 would *install* one, in the execution
path.

## The decisive argument: it is the same fragile parser either way — the question is what it is load-bearing *for*

The RFC's own Risks section concedes the parser is fragile: a marker built by
`concat!` or a `const` is invisible to it. That fragility exists under all
three shapes, because all three need to read `dispatch.rs` at some point.
What differs is **what breaks when the parser goes blind**:

- **Shape 1** — the parser is load-bearing for a **test**. A construction it
  cannot see turns the test red, in CI, on the commit that changed the
  construction, in front of the person who changed it. The runner keeps
  working off its explicit list throughout.
- **Shapes 2 and 3** — the parser is load-bearing for **what the tool
  accepts**. A construction it cannot see silently *shrinks the accepted
  set*: `cargo xtask qemu-test m8` starts answering `unknown milestone` for a
  milestone that works perfectly, and the tool is confidently wrong about the
  kernel with nothing red anywhere.

**A fragile predicate belongs behind a gate, not in the execution path.**
That is the same principle RFC-0.30-001 applied to `--skip-build` (leave the
weak check weak and name it, build the real one beside it) and RFC-0.30-003
applied to CI's toolchain install (do not put the single point of failure
where a silent wrong answer looks like a right one).

## The weak argument I am deliberately not leaning on

The handoff offers "five names changing once a milestone versus fourteen
profiles changing whenever anyone adds one." That is true and it is the
**worst** available reason: a list that changes rarely is precisely a list
that goes stale unwatched, which is what happened here across six
milestones. Rarity is the argument *against* hand lists, not for them. What
makes this one safe is the D2 check, not its update rate.

## So: under-reaching, or not?

**Not under-reaching** — but the architect's stated reason ("the change is
small, the test makes a hand list safe") is only half of it, and the weaker
half. Shape 1 is correct because the derived shapes move a known-fragile
string parser out of a test and into the runner's behaviour, trading a loud
failure for a quiet one. The D2 check is what makes the hand list safe; the
execution-path argument is what makes the hand list *better than derivation*
rather than merely tolerable.

## What the check asserts, and the one thing the RFC's Risks section asks for that needed adjusting

Three assertions, all reading the real `crates/fjell-kernel/src/trap/
dispatch.rs`:

1. Every marker in `MILESTONES` is emitted by a `kprintln!` literal.
2. Every emitted `…:PASS` literal is in `MILESTONES`. (`TEST:M7:FAIL (init
   did not exit cleanly)` is emitted and is not a `:PASS`; it is excluded by
   this rule as written, and is still covered by assertion 3.)
3. **After comments are stripped**, every remaining `TEST:` in the file lies
   inside a literal assertion 1 recognised — so a marker built by `concat!`
   or via a `const` fails on the construction rather than reading as absence
   (the RFC's Risks section, mode 3 of the defect class).

The Risks section asks that `TEST:` appear "*only* inside those literals it
recognised." Taken literally that fails on the shipped tree today:
`dispatch.rs:566` is a **doc comment** that names `TEST:V0.7-SYNC:PASS` while
explaining an index. Rather than carve out an exception, the check strips
comments first — a comment cannot emit anything — using
`callsite_audit::strip_comments_only`, the string-aware stripper that module
already uses for the same reason. No second copy of a comment parser, and
`concat!` still fails because code is not a comment.
