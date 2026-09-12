# RFC-0.31-003: A toolchain that is current, and stays that way

**Status:** Accepted — by the owner (nabbisen), 2026-09-12; implementation may begin (RFC 000)
**Milestone:** 0.31
**Tracks.** **E-037**'s last survivor — `channel = "1.91"` floats within
`1.91.x`, so the compiler that produced every committed artefact can move
underneath it. Closing it means an exact pin; taking the pin means deciding
what to pin *at*, and the answer is not the ten-month-old version we are on.
**Touches.** `rust-toolchain.toml`, `Cargo.toml` (the `rust-version` floor),
`docs/src/tutorials/quick-start.md`, `docs/src/internals/local-development.md`,
`docs/release/release-checklist.md`, `tools/fjell-consistency-check`
(`toolchain-declarations`), and — as *output*, not as edits —
`crates/fjell-kernel/prebuilt/*.bin`, `tests/repro/baseline-digests.txt`,
`docs/release/trust-report.txt`. **Does not touch kernel, ABI or service
source.**
**Relates to:** RFC-0.31-002 (which made the bump one line by putting CI on
rustup, and whose review recorded the collision this RFC finds has moved
rather than gone); RFC-0.30-003 (the observed-toolchain record); RFC-0.30-001.

## Summary

### The version, measured

| | |
|---|---|
| Declared `channel = "1.91"` resolves to | **1.91.1** (`ed61e7d7e`, 2025-11-07) |
| Current stable | **1.98.1** (`48a229cea`, 2026-09-01) |
| Gap | **ten months, seven minor versions** |

Compilation on 1.98.1 is **already proven**, each probe with a positive
control: the kernel checks clean under `-Z build-std`
(`cargo +1.98.1 check -p fjell-kernel --target riscv64gc-unknown-none-elf`,
0 errors), and all 22 host packages in `ci-check`'s list check clean on
1.98.1 and 1.91.1 alike. The 2026-09-09 incident corroborates: it rebuilt all
**29** prebuilts at 1.98.1 without a compile error.

**What is unproven is behaviour.** No QEMU tier has ever run against a
1.98.1-built kernel. That is what this line exists to establish, and it is
why this is a line and not an edit.

### Finding 1 — the exact-pin collision moved; it did not go away

RFC-0.31-002's review recorded that pinning was now free, because
`an_exact_patch_pin_no_longer_collides_with_ci` passes and `ci.yml` names no
apt package. **True of `ci.yml`, false of the tree.**
`docs/src/tutorials/quick-start.md` still reads:

```bash
sudo apt install rustc-1.91 cargo-1.91 rust-src lld llvm
```

and `toolchain-declarations` compares every documentation site to the anchor
**exactly** (`v != anchor`). Pin to `1.98.1` and the gate demands
`rustc-1.98.1` — **a package that cannot exist**, because Ubuntu carries
`rustc-<major>.<minor>` and never a patch level. The collision relocated from
`ci.yml` to `quick-start.md`, and the test that certified it gone uses the
fixture `sudo apt install rustc-1.91.1 cargo-1.91.1` — an apt invocation that
would fail. The test proves the **gate** accepts an exact pin. It does not
prove the **tree** can have one.

### Finding 2 — and the same line tells a new user to install a toolchain that cannot build this project

That apt path is precisely the one **E-041** proved cannot `-Z build-std`:
Ubuntu's `rust-src` ships no `library/Cargo.lock`. A reader following the
quick start reaches:

```
error: "/usr/lib/rust-1.91/lib/rustlib/src/rust/library/Cargo.lock" does not exist,
unable to build with the standard library
```

— the identical error that kept CI red for four months. **This is live
today, independently of the bump**, in the one document written for someone
who has never built the project. E-041 was closed on CI going green; its
cause was never removed from the tutorial.

The two findings share one fix: **quick-start installs Rust through rustup**,
as `local-development.md` already does, keeping apt only for `lld`, `llvm`
and `qemu-system-misc`, which are not Rust. That removes the last apt
toolchain instruction in the tree, so an exact pin collides with nothing
anywhere — and a new reader gets a toolchain that works.

### Finding 3 — the floor is now real, and still unverified

`rust-version = "1.91"` reached no crate until 2026-09-12; it now reaches
`fjell-abi` and `fjell-os`, the two crates this project publishes. It has
never been established that 1.91 *is* the minimum — E-037 says so in its own
words, *"a verified floor, not a bisected minimum."* `fjell-abi` checks clean
on 1.91.1, which is evidence for `1.91.x`, not for `1.91.0`.

A floor is a promise to a consumer. Building at 1.98.1 does not change what
the promise should be — the two are independent fields — but it does make an
unverified promise more conspicuous.

## The settled part

**D1 — Pin exactly.** `channel = "<major>.<minor>.<patch>"`. This is E-037's
surviving requirement and the whole point: every committed prebuilt, the
repro baseline and every future evidence log name a compiler that cannot move
underneath them.

**D2 — Pin at current stable, not at what we have.** The conservative choice
was never argued, only defaulted to. This project is a kernel: no consumer
constrains its build toolchain, and the MSRV floor is a separate field that
can stay low (D4). Ten months behind is a posture, not a decision.

**D3 — `quick-start.md` moves to rustup** (Findings 1 and 2), and
`toolchain-declarations` follows it: the quick-start site is extracted as a
rustup site, not an apt one, and **an apt `rustc-<version>` line anywhere in
the tree becomes a gate failure**, not just in `ci.yml`. After this line the
project has no apt toolchain instruction at all.

**D4 — The floor is decided on evidence, not moved by reflex.** Whatever
`rust-version` ends up saying must be a version the published crates have
actually been built at, in this line, and recorded. "It probably still works
on 1.91" is not a floor.

**D5 — The QEMU pass is the verification; compilation is not.** All 24 tiers,
green, against a 1.98.1-built tree, plus a green CI run observed with `gh`
(RFC-0.31-002's rule). A clean `cargo check` is a precondition, not evidence.

**D6 — The prebuilt diff is expected to be total, and that is the one moment
the repro check cannot tell expected from unexpected.** Every one of the 29
binaries will change. The line must therefore show *separately* that each
still behaves: the digest diff is not evidence of correctness here, only of
the bump having happened.

**D7 — Demonstrated failing** (RFC-v0.22-001): the extended gate must be
shown red on an apt `rustc-<v>` line reintroduced into `quick-start.md`, and
on a doc site left behind at the bump.

## The open question — §7

**Pinning is the easy half. What stops the next ten-month drift?**

An exact pin makes every upstream release a deliberate edit — which is
correct, and is also exactly how a project ends up ten months behind without
anyone deciding to be. The pin removes the *silent* drift E-037 is about and
replaces it with *silent staleness*, which is what we have now.

1. **Nothing — bump when someone notices.** The status quo. It produced a
   ten-month gap and a compiler seven minors old in a security-focused
   project. Honest, and demonstrably fails.
2. **A gate that fails when the pin is more than N releases behind current
   stable.** The project's own idiom — make the drift loud. But every other
   gate in this repository is offline and deterministic, and this one needs
   to know what upstream's current stable *is*, which means a network call
   inside a check that today cannot fail for a reason unrelated to the tree.
   That is a real departure and it deserves to be argued, not assumed.
3. **A scheduled CI job** (the `fuzz-nightly` shape — `schedule:` already
   exists in `ci.yml`) that reports staleness without blocking anything.
   Keeps the gates offline and deterministic; costs a signal nobody is
   obliged to read, which is E-041's entire subject.
4. **A release-cycle step**: the cut records how far behind the pin is, the
   same way it now records the CI run (exit criterion 9). Bounded, offline,
   and it fires at the one moment someone is already reading the record — but
   only as often as we cut.

**Answer in writing before implementing.** I lean to **4, with 3 as its
companion** — the cycle already learned this lesson once, and "record it where
someone is already looking" is what closed E-039. But I have just written a
paragraph arguing that a signal nobody must read is E-041's subject, and 3 is
exactly such a signal, so say plainly whether 4 alone is enough or whether
that is me choosing the cheap option twice. **Shape 2 is the strong answer if
a network-dependent gate is acceptable**; argue that on its merits rather than
inheriting my discomfort with it.

## Requirements

**R1 — Re-derive the version facts** before changing anything: the resolved
channel, current stable, and both clean-compile probes, each with a positive
control. Report any disagreement with the table above. Eleven consecutive
lines have corrected a figure of mine.

**R2 — D3 first, before the pin**: `quick-start.md` onto rustup, the gate
extended, both demonstrated (D7). This is separately valuable and fixes a live
user-facing defect (Finding 2) whether or not the bump proceeds.

**R3 — Bump and pin** (D1, D2), then rebuild: `cargo xtask build`, re-record
`tests/repro/baseline-digests.txt`, confirm its `# toolchain:` header reads
the new compiler, and confirm the diff is exactly the 29 prebuilts and
nothing else.

**R4 — D5**: full `test-all`, all 24 tiers, against the bumped tree; a green
CI run, observed, with the run id. **Gate 10 (Verus) specifically** — `ci-verus`
installs the declared toolchain *and* `VERUS_TOOLCHAIN=1.95.0`; the two
coexisting at 1.91.1 today is not evidence that they coexist at 1.98.1.

**R5 — D6**: state, separately from the digest diff, what shows the bumped
binaries still behave — which tiers, which markers.

**R6 — D4**: decide `rust-version` on evidence. Build both published crates
at the candidate floor (1.91.0 is installed) and record the result, or move
the floor to something verified. Say which, and why.

**R7 — §7 answered in writing**, and built.

**R8 — E-037 → `CLOSED`**, or its remaining instance named. Register and
`docs/release/v1-limitations.md` in the same commit. E-037 has been narrowed
twice; if something survives this line, name it rather than rounding up.

### Non-goals

- Changing kernel, ABI or service source. If the bump requires a source
  change to compile, **stop and escalate** — that is a different line.
- Bumping Verus's `1.95.0` or the `nightly` used by `cargo-fuzz`.
- Adding CI coverage. RFC-0.31-002 made every existing job green; what CI
  *should* test is a separate question.
- E-014's two survivors, E-034.

## Risks

**Seven minor versions of codegen land at once.** If a QEMU tier fails, the
bisect space is large and each step costs a full 24-tier run. Take the jump —
compilation is already proven — but if a tier goes red, **do not fix forward
into kernel source**; step back through 1.95 and 1.96 to localise it first,
and escalate with the finding.

**`-Z build-std` is unstable and this project depends on it.** It compiles on
1.98.1. Its *output* — `compiler_builtins` version, panic machinery, codegen
of the `asm!` blocks the callsite gates guard — is what the QEMU pass checks
and nothing else does.

**The repro baseline is blind exactly here** (D6). Its job is to catch
unexpected digest change, and this is the one commit where all 29 change on
purpose. A real regression hides perfectly inside an expected total diff. The
QEMU pass is the only thing standing in that gap, which is why R5 asks for it
explicitly rather than letting a green `repro-check` imply it.

**A pin is a decision to go stale by default** (§7). If this line ships a pin
with no answer to §7, it converts E-037's silent drift into silent staleness
and the next RFC in this seat writes the same paragraph in 2027.
