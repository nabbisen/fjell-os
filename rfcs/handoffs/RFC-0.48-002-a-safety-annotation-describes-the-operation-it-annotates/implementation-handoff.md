# Developer Handoff — RFC-0.48-002

**Governing RFC:** [RFC-0.48-002](../../done/RFC-0.48-002-a-safety-annotation-describes-the-operation-it-annotates.md)
**Milestone:** 0.48 · second line
**Status:** inherited from the governing RFC (Implemented, 0.48.0)
**Audience:** implementation model

---

## 1. The headline figure was a count of what the scanner could see

`unsafe-audit` reported **291 sites, 291 commented, 0 missing**, Gate 2 passed on it, and *"full unsafe
audit with SAFETY comments"* is a v0.1.0 deliverable cited on the crates.io page as how this project
verifies itself. **It could not see 49 declarations** (E-131): **41 `#[unsafe(no_mangle)]`**,
**1 `#[unsafe(naked)]`**, **6 `unsafe extern "C" fn`** where `extern "C"` sits between `unsafe` and
`fn`, and **1 `unsafe impl<T>`**.

**The one `unsafe impl` is the one that matters, and the cause is one character.** The scanner matched
`"unsafe impl "` *with a trailing space*; nine declarations exist, eight match, and the one that does
not is `unsafe impl<T> Sync for KS<T>` — the kernel-state wrapper every `get_kernel_state()` call goes
through. **A `Sync` soundness claim about the most widely shared static in the system was invisible.**
`#[unsafe(naked)]` sits one line above the trap entry stub, so two invisible declarations share the
function every trap enters through.

**The architect's count of 48 was wrong** and the implementer restored 49 — a narrow search inside an
entry about narrow matching.

## 2. What the line does

The scanner finds every `unsafe` declaration, and the audit **refuses a `category=` its operation does
not support**: **344 sites, 344 commented, 0 missing, 4 permanent exemptions, 0 seeded.** 292 are
judged against an operation shape; **52 have no body to judge** and are judged on a comment and a tag.
New categories `linker-symbol` and `symbol-export` close a taxonomy gap where six sites claimed
`phys-id-map-assumption` for the address of a linker symbol.

**The headline rises 291 → 344 and the reason must travel with it.** The kernel wrote no new `unsafe`;
**the scanner's eyes opened.**

## 3. What a green means, pinned by a test in the PASS line

*A tag is not contradicted by the operation beneath it.* **It is not a claim that any comment's prose
is true.** A purely lexical rule was wrong 41% of the time, mostly on correct tags behind a call, so
the rule admits call-through to an `unsafe fn` of the same category — and **161 distinct comment texts
of 311** remain the measure of how much prose review is still owed.

## 4. The sequencing inversion, which was better than the architect's constraint

The handoff said the sweep must follow line 1. The implementer proposed landing the **gate** first with
a seeded exemption list whose entries the sweep shrinks to zero, **so the list's length is the sweep's
progress**. Adopted. In the event the sweep completed in the same stage: **0 seeded, 4 permanent**,
each with its reason in `exempt.rs`.
