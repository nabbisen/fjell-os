# Developer Handoff — RFC-0.31-003

**Governing RFC:** [RFC-0.31-003](../../accepted/RFC-0.31-003-a-toolchain-that-stays-current.md)
**Milestone:** 0.31
**Status:** inherited from the governing RFC (Accepted, 2026-09-12)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The one thing that makes this line different from every other

Every other line in this milestone had an instrument that would catch it if it
went wrong. **This one disables the instrument as a side effect of doing the
work.**

`tests/repro/baseline-digests.txt` exists to notice when a committed binary's
digest changes unexpectedly. This commit changes **most of them on purpose**
*(24 of 29, as measured at review; this handoff said "all 29")*.
For exactly one commit, "every digest moved" is both the expected outcome and
what a serious regression would look like, and `repro-check` cannot tell you
which you have.

So: **a green `repro-check` proves nothing here, and neither does a clean
`cargo check`.** The 24-tier QEMU pass is the only thing standing in that gap.
Treat every instruction below about evidence as following from that.

## 0.1 Re-derive the facts first (R1), each with a positive control

```
grep channel rust-toolchain.toml && rustc -vV | head -1     # declared vs resolved
rustup check | head -1                                       # current stable
ls crates/fjell-kernel/prebuilt/*.bin | wc -l                # expect 29
grep -c '^[0-9a-f]' tests/repro/baseline-digests.txt         # expect 29
RUSTC_BOOTSTRAP=1 cargo +<new> check -p fjell-kernel \
  --target riscv64gc-unknown-none-elf -Z build-std=core,compiler_builtins
```

**A positive control is not optional on any of these.** Run the same command
shape against the *current* toolchain and confirm it produces the result you
already know. My own probe of the host crates reported 43 errors on 1.98.1 and
looked like a finding until the control showed 35 on 1.91.1 from the identical
command — it was compiling RISC-V `asm!` crates for the host and said nothing
about either toolchain. Use `ci-check`'s explicit `-p` list, not `--workspace`.

**The prebuilt count is 29 — and 24 of them change.** This paragraph
originally said only the first half, having "corrected" E-037's 24 to 29 that
morning on the grounds that the tree holds 29. **That correction was wrong**:
29 is the total, 24 is the changed set, and the implementer established it by
measuring the transition rather than inheriting either figure. Two quantities
that had been made to replace each other. If any figure in the RFC disagrees
with the tree, **report it** — twelve consecutive lines have corrected one of
mine, and this is the twelfth.

## 0.2 Settled — do not re-open

1. **Pin exactly** (D1), **at current stable** (D2). The old version was never
   argued for; it was defaulted to.
2. **`quick-start.md` moves to rustup** (D3), and the gate follows it: an apt
   `rustc-<version>` line anywhere becomes a failure.
3. **The floor is decided on evidence** (D4), not moved by reflex.
4. **The QEMU pass is the verification** (D5). Compilation is a precondition.
5. **Behaviour is shown separately from the digest diff** (D6). See §0.
6. **Do not touch kernel, ABI or service source.** If the bump needs one to
   compile, stop and escalate — that is a different line.

---

## 1. Order

**R1 (re-derive) → R2 (quick-start + gate, committed on its own) → §7 answered
→ R3 (bump, pin, rebuild, re-record) → R4/R5 (QEMU + CI + Verus) → R6 (floor)
→ R7 (§7 built) → R8 (errata) → evidence.**

**R2 is committed before the bump and is separately valuable.** It fixes a
live defect — the quick start currently tells a new reader to install a
toolchain that cannot build this project — and it is what makes an exact pin
possible at all. If the bump is later abandoned for any reason, R2 still
should have landed.

## 2. R2 — the collision, and the tutorial that has never worked

`docs/src/tutorials/quick-start.md` says:

```bash
sudo apt install rustc-1.91 cargo-1.91 rust-src lld llvm
```

Two separate problems in one line:

- **It cannot build the project.** Ubuntu's apt `rust-src` ships no
  `library/Cargo.lock`, so `-Z build-std` fails — this is E-041's cause
  exactly, still live in the one document written for someone who has never
  built the project. E-041 was closed when CI went green; nobody removed the
  cause from the tutorial.
- **It blocks the pin.** `toolchain-declarations` compares documentation sites
  to the anchor exactly, and apt carries `rustc-<major>.<minor>` only. A pin at
  a patch level would demand a package that cannot exist.

**Fix:** Rust via rustup, as `local-development.md` already does. Keep apt for
`lld`, `llvm` and `qemu-system-misc` — those are not Rust, and CI installs
exactly `llvm lld` for the same reasons (`ld.lld` is in the separate `lld`
package; `llvm` gives `llvm-objcopy`/`llvm-nm`).

**Then extend the gate**: `quick-start.md` becomes a rustup site, extracted
like `local-development.md`'s, and **an apt `rustc-<version>` line anywhere in
the tree is a failure**, not only in `ci.yml`. Demonstrate both directions
(D7).

**One test is now fiction and must go**:
`an_exact_patch_pin_no_longer_collides_with_ci` uses the fixture
`sudo apt install rustc-1.91.1 cargo-1.91.1`. No such package exists. It
proves the gate accepts a pin; it asserts a tree that cannot exist. Replace it
with one whose fixtures are shapes the tree can actually hold.

## 3. §7 — and I have written the argument against my own lean

**Pinning converts silent drift into silent staleness.** That is not rhetoric:
a floating channel moved under us without a decision, and an exact pin means
the next ten-month gap also happens without one. Four shapes are in the RFC.

**I lean to 4 (record it at the cut) with 3 (a scheduled report) beside it.**
And in the same RFC I wrote that a signal nobody is obliged to read is E-041's
entire subject — which is exactly what 3 is. **Say plainly whether that makes
my lean the cheap option chosen twice.**

**Shape 2 — a gate that fails when the pin is N releases behind — is the
strong answer if a network-dependent check is acceptable.** Every other gate
here is offline and deterministic, and one that can fail because
`static.rust-lang.org` is unreachable is a real departure. Argue it on its
merits; do not inherit my discomfort with it.

## 4. R3/R4 — the bump, and what actually proves it

After bumping and pinning:

1. `cargo xtask build`, then re-record `tests/repro/baseline-digests.txt`.
2. **Confirm the diff touches only prebuilts, and account for every one that
   did not move** — not "looks right", the file list. *(24 of 29 moved; the
   five that did not are byte-identical stubs, and telling that apart from
   "the build skipped them" took an mtime check.)*
3. **Confirm the baseline's `# toolchain:` header reads the new compiler.**
   It is written from `rustc -vV` (RFC-0.30-003), so a stale header means the
   rebuild did not happen under the toolchain you think.
4. `cargo xtask test-all` — **all 24 tiers**, against the bumped tree.
5. A green CI run, **observed with `gh run view`**, with the run id, per
   RFC-0.31-002's rule. `ci.yml` is not evidence of anything.

**Gate 10 (Verus) specifically.** `ci-verus` installs the declared toolchain
*and* `VERUS_TOOLCHAIN=1.95.0` through rustup. That those two coexist at
1.91.1 today is not evidence they coexist at the new version. Check it, and
say you checked it.

## 5. R5 — say what shows the binaries still behave

Separately from the digest diff (§0), state which tiers and which markers
constitute the behavioural evidence. "`test-all` was green" is the claim;
what this line needs recorded is *why that is the right evidence* when
`repro-check` is blind — which tiers boot the kernel, which exercise the
`asm!` blocks the callsite gates guard, which exercise `-Z build-std`'s output
in anger.

## 6. R6 — the floor, on evidence

`rust-version = "1.91"` now reaches `fjell-abi` and `fjell-os` (it reached
nothing until 2026-09-12 — a field that existed and did nothing, inside the
erratum that names that defect). It has never been established that 1.91 *is*
the minimum.

`1.91.0` is installed. Build both published crates at the candidate floor and
record the result, or move the floor to something you have actually built at.
**"It probably still works" is not a floor**, and these two crates are
published, so the floor is a promise to a stranger.

## 7. Prohibited shortcuts

- **Do not let a green `repro-check` stand in for the QEMU pass** (§0). This
  is the one commit where it cannot.
- **If a QEMU tier fails, do not fix forward into kernel source.** Step back
  through intermediate versions to localise it and escalate with the finding.
  Seven minors landing at once is the risk the RFC names; a source edit to
  make a bumped kernel pass is a different line and possibly a real bug.
- Do not run any probe without a positive control (§0.1).
- Do not bump Verus's `1.95.0` or the fuzz `nightly`.
- Do not close E-037 if something survives — name it. It has been narrowed
  twice already; rounding up now would waste both narrowings.
- Do not write "runs in CI" without a run id beside it.
- Do not run `cargo fmt --all --check` in your head.

## 8. Required evidence

1. R1's re-derivation, each probe with its control, and every disagreement
   with the RFC reported.
2. R2 committed separately: `quick-start.md` on rustup, the gate extended,
   both demonstrations, the fiction test replaced.
3. §7 answered in writing, engaging the argument against my lean.
4. The pin, the rebuild, the 29-file diff, and the baseline's new `# toolchain:`
   header.
5. **`test-all`, all 24 tiers**, against the bumped tree.
6. **A green CI run with its id**, and Gate 10's result called out.
7. R5's statement of what shows behaviour, distinct from the digest diff.
8. R6's floor decision, with the build that supports it.
9. E-037 CLOSED, or its survivor named; `v1-limitations.md` in the same commit.
10. `release-rehearsal` green; `consistency-check --all`; `cargo fmt --all --check`.

## 9. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The run id**, first line.
- **Your §7 answer**, and whether you think my lean is the cheap option twice.
- **What convinced you the bumped binaries behave** — the part `repro-check`
  cannot tell you.
- Anything that turned out **not** to need changing.
- Any figure of mine you re-derived and found different. The RFC said 24
  prebuilts until this morning; it is 29.
