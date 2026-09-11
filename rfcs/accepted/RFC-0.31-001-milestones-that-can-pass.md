# RFC-0.31-001: Six milestones the smoke runner accepts and nothing can pass

**Status:** Accepted — by the owner (nabbisen), 2026-09-12; implementation may begin (RFC 000)
**Milestone:** 0.31
**Tracks.** **E-040** — six of the eleven milestones `cargo xtask qemu-test`
accepts expect a marker no code emits; they boot QEMU, wait out the full
timeout, and report `FAIL` indistinguishably from a regression. E-015 was
closed by RFC-0.29-002 with these six surviving in the same `match` statement
as the one instance it deleted.
**Touches.** `crates/fjell-tools/src/smoke.rs`, and a test in `crates/fjell-tools`
that reads `crates/fjell-kernel/src/trap/dispatch.rs`. **Does not touch the
kernel, the ABI surface, or any service.**
**Relates to:** RFC-0.29-002 (R5 deleted `v0.6-verification` on the same
reasoning and stopped one line early); RFC-0.24-002 Slice 2 (the fail-closed
`unknown milestone` path this RFC routes the six into); RFC-v0.22-001.

## Summary

### What the six are — six former names for one test, not six tests

Traced through history rather than assumed. Each `TEST:Mn:PASS` for `n` in
2…6 was printed **by `fjell-init` from user space** at the end of its run,
only while `Mn` was the *current* milestone, and was **replaced** — not
joined — by `Mn+1`'s marker when the next milestone landed:

| Commit | Date | `fjell-init` printed |
|---|---|---|
| `c587bdb` | 2026-05-18 | `TEST:M4:PASS` (first user-space marker) |
| `ac5ca6d` | 2026-05-18 | `TEST:M5:PASS` |
| `8aa272f` | 2026-05-18 | `TEST:M6:PASS` — and `M5`'s line is gone |
| `9363b91` | 2026-06-06 | kernel takes over: `TEST:M7:PASS` / `TEST:M8:PASS` emitted atomically from `trap/dispatch.rs` |
| `a5b5167` | 2026-07-23 | last commit carrying any `sys_debug_writeln("TEST:M…")` in init |

`git log -S'TEST:Mn:PASS' -- crates/fjell-kernel` is **empty for every n in
1…6**: no kernel commit has ever emitted one. `smoke.rs`'s comment says the
mapping was *"preserved verbatim from the v0.1.0 runner"* — it was, and it
accumulated an arm per milestone while the emitter kept only the current one.

> **Correction, architect, 2026-09-12, at the implementation review.** The
> sentence above — *"empty for every n in 1…6: no kernel commit has ever
> emitted one"* — is wrong, and the implementer's R1 re-derivation found it:
> `TEST:M2:PASS` and `TEST:M3:PASS` were emitted **from the kernel** (`0c0b61a`,
> `7d10af8`), moved to user space at `c587bdb`, and marker emission moved
> back to the kernel at `9363b91`. The replace-don't-join pattern held in both
> planes; this table told only the user-space half. **Why my command was
> empty:** it was run in a `for m in M1 …` loop under zsh as
> `-S"TEST:$m:PASS"`, and zsh reads `$m:P` as a parameter modifier, so the
> search string was never the marker. The implementer's own first pass hit
> the same trap and was caught by a positive control (M7/M8 in the same
> command shape) — the discipline this RFC's D2 exists to enforce, and one I
> had not applied to my own evidence. The conclusion (nothing emits any of
> the six today) is unaffected and is what D1 turns on.

The kernel's own comment settles what they test today
(`dispatch.rs:465`): *"init orchestrates M1–M7 and exits after those
complete."* **`TEST:M7:PASS` is the cumulative pass for everything M1–M6 ever
checked.** A user who runs `qemu-test m5` and gets `FAIL` has not found a
regression in the semantic-operations plane; they have run a name that stopped
meaning anything at `8aa272f`.

### The cost, concretely

`cargo xtask qemu-test m5` builds, boots, waits **60 seconds**, and prints
`FAIL` — the same output a real regression produces. The three untracked
artefact directories that led to E-040 were exactly this: someone ran `m5`,
got `FAIL`, and had no way to tell it apart from a broken kernel. The
fail-closed `unknown milestone` path RFC-0.24-002 built is one `match` arm
away and is never reached.

### Why RFC-0.29-002 missed them

R5 asked *"what emits `TEST:V0.6-VERIFY:PASS`?"* and correctly found nothing.
It did not ask *"which of the milestones this file offers can any of them
pass?"* — one `grep 'kprintln!("TEST:'` against one `match`, six more answers.
The list of accepted names was hand-maintained and checked against nothing;
E-014's family, inside the sweep that closed a scope-blindness erratum.

### Three copies of the list

The accepted names appear in `smoke.rs`'s `match`, in its `known:` usage
string (`smoke.rs:49`), and — for the gated subset — in
`test_all.rs::SMOKE_PROFILES`. The first two are the shape RFC-0.30-002's
review removed from `consistency-check`'s dispatcher; the third is a
legitimately different set (what `test-all` gates) and stays.

## The settled part

**D1 — Every milestone `qemu-test` accepts can pass.** Any name whose marker
no kernel code emits is not a milestone; it is routed to the existing
`unknown milestone` failure, which costs no QEMU boot and says what it is.

**D2 — The general question is asked once, by a check, not per instance.** A
test in `crates/fjell-tools` reads `crates/fjell-kernel/src/trap/dispatch.rs`
and asserts **bidirectionally**: every marker the dispatcher accepts is
emitted by a `kprintln!("TEST:…:PASS")` literal, and every emitted `…:PASS`
literal is accepted by some arm. `callsite_audit` already reads kernel source
from a test; this is the same pattern. A hand-list that a test checks against
its source is the weakest acceptable shape; a list derived from the source is
better and §5 asks which.

**D3 — The usage string derives from the same list the `match` uses.** Two
hand copies of one list is the defect RFC-0.30-002's review removed one crate
over; do not leave it here.

**D4 — Do not reconnect the six.** M2/M3 scaffolding (`task/user_image.rs`)
and the M6 driver comment are real, but they are exercised on the way to M7
today; adding six user-space markers back would recreate the concurrent-UART
garbling `9363b91` moved emission into the kernel to fix, and would test
nothing M7 does not. Deleting is what RFC-0.29-002 did for `v0.6-verification`
on identical grounds.

**D5 — Demonstrated failing** (RFC-v0.22-001): the D2 check must be shown red
on a dispatcher arm whose marker is not emitted, and on an emitted marker no
arm accepts — both directions, on real files, reverted after.

## The open question — §5

**Where does the accepted-milestone list live?**

1. **A `const` in `smoke.rs`** — `(name, marker)` pairs — that the `match`,
   the usage string, and the D2 test all read. One hand list, checked against
   kernel source by a test. Smallest change; the list still exists.
2. **Derived from `dispatch.rs` at build time** (`build.rs` or `include_str!`
   + parse): the accepted set *is* the emitted set, and no list exists to go
   stale. Stronger, but `fjell-tools` then parses kernel source at build time,
   and the `m8` default and the `v0.4-net`-style names need a naming rule from
   marker to CLI name (`TEST:V0.4-NET:PASS` → `v0.4-net` is mechanical;
   `TEST:M8:PASS` → `m8` is too).
3. **Derived at run time**: `qemu-test <name>` scans `dispatch.rs` when
   invoked. Same derivation as 2 without the build-time coupling; costs a file
   read per run and makes the tool's accepted set depend on the checkout's
   kernel source — which is arguably exactly right for a smoke runner.

**Answer in writing before implementing.** I lean to **1** — the D2 test makes
a hand list safe, the change is small, and every prior line in this family
(RFC-0.29-001's category discovery aside) has found the derived version more
machinery than the problem. But RFC-0.29-001 *did* derive the negative
categories from the profiles on disk and it was right to; argue whether the
smoke set is different in kind or whether I am under-reaching.

## Requirements

**R1 — Re-derive the history table above** from `git log -S` before touching
anything, and report any row that disagrees.
**R2 — The six arms deleted**, `m1`–`m6` reaching `unknown milestone` (D1,
D4), with the `known:` line updated per D3.
**R3 — §5 answered and built**, with the D2 check in place either way.
**R4 — Both directions demonstrated failing** (D5).
**R5 — E-040 → `CLOSED`**, register and `docs/release/v1-limitations.md` in
the same commit. E-015's correction note already points here; leave it.
**R6 — `smoke.rs`'s module comment corrected**: *"preserved verbatim from the
v0.1.0 runner"* is the sentence that explains this erratum and should say so.

### Non-goals

- **Wiring up M1–M6** (D4).
- Changing what `test-all` gates — `SMOKE_PROFILES` stays four.
- Changing which markers the kernel emits, or how.
- E-014's two survivors, E-034, E-037.

## Risks

**The D2 test parses kernel source by string.** A `kprintln!` whose literal is
built by `concat!` or a `const` would be invisible to it — the literal-predicate
family again. Today every emitted marker is a plain literal (`dispatch.rs:470–
486`, all six); the test should also assert that `TEST:` appears in the file
*only* inside those literals it recognised, so a new construction cannot slip
past as absence.

**The `m8` default.** `qemu-test` with no argument runs `m8`; §5 shape 2 or 3
must keep that default explicit rather than "the last marker in the file".
