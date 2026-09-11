# RFC-0.31-002: A CI that builds the product

**Status:** Accepted — by the owner (nabbisen), 2026-09-12; implementation may begin (RFC 000)
**Milestone:** 0.31
**Tracks.** **E-041** — the workflow has had one green run in 152 (2026-05-05,
before any QEMU-building job existed); no CI job has ever built the kernel or
a service; every "runs in CI on every push" sentence since June was written
from `ci.yml`'s text. **Also E-037's first survivor** (consolidation): this
line is E-037's shape 1 by necessity, not by choice — see Finding 2.
**Touches.** `.github/workflows/ci.yml`, `tools/fjell-consistency-check`
(`toolchain-declarations`), `docs/src/release/v0-release-cycle.md`,
`docs/release/release-handoff.md`, the release-record shape, `README.md`.
**Does not touch the kernel, the ABI surface, or any service.** The one
service-adjacent job it may change (`ci-test-v07-formats`) changes only the
feature flag the job passes, per RFC-0.29-001's own recorded fix.
**Relates to:** RFC-0.30-003 (whose §7 argument this corrects); RFC-0.30-001
(whose `ci-repro-check` has never run); RFC-0.29-001 (which named the
`test-v07-formats` guard and the empty proptest tier); RFC-v0.22-001.

## Summary

Everything below is **observed with `gh run view`**, not read from the
workflow file. That distinction is the erratum.

### Finding 1 — 28 failing jobs, three causes, one habit

| Cause | Jobs | First error line |
|---|---|---|
| **(1) apt `rust-src` cannot `build-std`** | `qemu-smoke` ×8 (**`m7`, `m8` included**), `qemu-negative` ×12, `qemu-v07` ×3, `repro-check`, `cross-check`, `test-services` (step 2) — **25** | `"/usr/lib/rust-1.91/lib/rustlib/src/rust/library/Cargo.lock" does not exist, unable to build with the standard library` |
| **(2) feature guard** | `test-v07-formats` — 1 | `fjell-sxt-crypto requires the crypto-profile-development feature` |
| **(3) does not compile** | `proptest` — 1 | `error[E0405]: cannot find trait Strategy in this scope` ×10 |

Cause 2 was **named by RFC-0.29-001** — *"exposed to the identical masking —
named, not fixed there"* — and left. Cause 3 sits beside RFC-0.29-001's other
finding that `test-all`'s local proptest tier passes by running nothing: on CI
the same tier fails by not compiling. Same tier, two ways of not testing.

Passing: `check`, `format`, `docs`, `test-host`, `host-bins`, `unsafe-audit`,
`schema-gate`, `verus`, `negative-matrix` (the listing step only), and the
two `continue-on-error` profiles. **Every job that passes is one that does
not build for `riscv64gc-unknown-none-elf`.**

### Finding 2 — the apt toolchain cannot build this product, so E-037's §7 was arguing about a CI that builds nothing

`crates/fjell-tools/src/qemu.rs:63,107` passes `-Z build-std=core,compiler_builtins`
under `RUSTC_BOOTSTRAP=1`. That needs the standard library's source *and its
`Cargo.lock`*. Ubuntu 24.04's `rust-src` package ships the source without the
lock. There is no apt package that fixes this. **The 17 hand-copied
`apt-get install rustc-1.91` blocks E-037 counted have been installing a
toolchain that structurally cannot compile the kernel** — since the first
QEMU job was added.

RFC-0.30-003 §7 chose a drift gate over rustup-in-CI on the argument that
CI's independence from `rust-toolchain.toml` *"contained the 1.98.1 drift to
local builds"*. That independence is real and it is coincident with CI never
having built anything. The argument compared a working local build against a
CI that has no build. **Shape 1 — rustup, reading `rust-toolchain.toml` — is
the only way CI can ever run `build-std`.** `rustup` is already on the
runner: `ci.yml:442` (`rustup target add`), `:548-549` (`apt-get install
rustup`, nightly), `:583-589` (Verus).

### Finding 3 — my own drift gate fails the moment this is fixed

`toolchain-declarations` (RFC-0.30-003) compares every `rustc-<v>`/`cargo-<v>`
mention in `ci.yml` against `rust-toolchain.toml`'s channel, and **fails if
it finds none** (`toolchain_declarations.rs:92`, *"expected at least one"*).
Replace the apt blocks with rustup and the gate goes red on a correct tree —
the same collision E-037's second survivor already has with an exact pin.
This line has to change the gate it inherits, and the change is the point:
after shape 1, `ci.yml` should carry **zero** versioned toolchain mentions,
because its declaration *is* `rust-toolchain.toml`.

### Finding 4 — nothing in the cycle reads CI, by design, and says so

`v0-release-cycle.md:345`: *"It adds no CI enforcement. This is a documented,
followed-by-hand cycle."* That sentence is honest and it is why nothing
noticed. Every release gate runs locally; every release record's evidence is
local; **nothing shipped on a claim CI made, because CI never made one.** The
badge at the top of `README.md` has been red for four months, and it was
right.

### Finding 5 — the sentence that has been false in eleven places

*"Runs in CI on every push."* Written into RFC-0.30-001's answer, E-036's
closure, T20, CHANGELOG 0.30.0, the 0.30.0 release record, RFC-0.29-001's
answer, E-015's closure, RFC-0.30-003's §7, and E-040's closure — each time
from `ci.yml`, never from a run. All corrected with dated notes at `689a281`.
The defect this RFC closes is not that the sentence was false; it is that
**the process had no step at which it could have been found true or false.**

## The settled part

**D1 — CI installs the toolchain through rustup from `rust-toolchain.toml`.**
One composite action (`.github/actions/toolchain/` or equivalent), used by
every job that builds, replacing the 17 blocks. The file is the declaration;
CI reads it; E-037's "twenty-two places" drops to five.

**D2 — Absence of `rust-toolchain.toml` fails CI closed.** RFC-0.30-003's
objection to shape 1 was that rustup silently falls back to the runner's
ambient default. The answer is not to avoid rustup; it is to check. The
composite action asserts the file exists and that `rustc -vV`'s `release`
matches its `channel` (major.minor prefix, or exact once pinned) **before**
building anything. `toolchain-declarations` already fails on the missing
file locally; CI must too.

**D3 — `toolchain-declarations` inverts for CI** (Finding 3): after this line
it asserts `ci.yml` contains **no** versioned `rustc-`/`cargo-` mention, and
that every building job uses the composite action. The apt-era scan becomes a
regression guard against the blocks coming back. The four documentation
sites it checks are unchanged.

**D4 — Green is observed, not declared.** E-041 closes only when every
service-building job has been seen green with `gh run view`, on a run of a
commit in this line, and the run id and conclusions are written into the
review request. A `ci.yml` diff is not evidence of anything.

**D5 — The dead `qemu-smoke` matrix entries go** (`m1`–`m6`), in the same
line as the fix that lets `m7`/`m8` run, so the matrix change is proven by a
green run rather than argued from RFC-0.31-001.

**D6 — Causes 2 and 3 are fixed here, not re-named.** `test-v07-formats`
passes the feature RFC-0.29-001 specified; `proptest`'s CI command compiles
or the job is removed with the reason recorded — a job that has never
compiled is not coverage.

**D7 — The release cycle gains a step that reads CI.** Before the tag: the
release commit's workflow conclusion, per job, recorded in the release record
from `gh run view`, with the run id. A red job blocks the tag or gets an
accepted-risk statement — the existing rule, applied to an instrument the
cycle previously never consulted. `release-handoff.md` gets the step;
`v0-release-cycle.md:345` gets rewritten.

**D8 — Demonstrated failing** (RFC-v0.22-001): D2 on a run with the file
moved aside (CI red, naming the file); D3 on an apt block reintroduced
(subcheck red, naming the line).

## The open question — §6

**What does CI cache, and what does a green run cost?** Rustup installs the
toolchain per job unless cached; `rust-toolchain.toml` names `rust-src`,
`clippy`, `rustfmt`, `rust-analyzer` and the RISC-V target. Uncached, that is
a download per job across ~30 jobs.

1. **No cache.** Simplest; measure the cost first. If a job's install is
   under a minute the question may not be worth machinery — RFC-0.30-001's
   R1 found "doubles CI time" was 4–7 seconds once measured.
2. **`actions/cache` on `~/.rustup` and `~/.cargo`**, keyed on the hash of
   `rust-toolchain.toml` and `Cargo.lock`. Standard; adds a cache key that
   must be right.
3. **A minimal profile in CI** — install only `rust-src` + target, not
   `rust-analyzer`/`clippy`/`rustfmt`, via the action rather than the file's
   `components`. Cheaper, but CI then installs a *different* component set
   than the file declares, which is E-037's family in a new coat.

**Measure before choosing** (R1). I lean to 1 until the number says
otherwise, and to 2 over 3 if it does — 3 makes CI's toolchain diverge from
the declaration this whole line exists to make authoritative.

## Requirements

**R1 — Measure first**: one job converted to rustup, uncached, timed against
its apt predecessor; the number in the answer document before §6 is decided.
**R2 — D1/D2 built**; all building jobs on the composite action; the apt
blocks gone.
**R3 — D3**: `toolchain-declarations` inverted for CI, with the four doc sites
unchanged and the apt-block regression guard demonstrated.
**R4 — D5, D6**: matrix trimmed; causes 2 and 3 fixed.
**R5 — D4**: every service-building job observed green; run id and per-job
conclusions in the review request. **This is the closure condition.**
**R6 — D7**: the cycle step and the handoff step, and the record shape.
**R7 — D8** demonstrations.
**R8 — E-041 → `CLOSED`; E-037 → its consolidation survivor closed**, its
pinning survivor re-stated (an exact pin now works against rustup, so the
apt-naming collision RFC-0.30-003's review recorded is gone — say so), and
whether E-037 closes entirely argued rather than assumed. Register and
`v1-limitations.md` in the same commit.

### Non-goals

- Pinning the channel (E-037 survivor 2) — this line removes the obstacle; it
  does not take the decision.
- Adding coverage. Every job green means every *existing* job green; what CI
  should test is a different question.
- Making the release gates depend on CI. They run locally and that stays;
  D7 records CI, it does not defer to it.
- The Verus and `nightly` toolchains.
- E-014's survivors, E-034.

## Risks

**This is the largest CI change the project has made, and CI is currently
proving nothing, so a broken change and a working one look identical until
D4.** Do R1 on one job and watch it go green before converting thirty.

**A green run is not a green run until someone reads it.** D4 and D7 are the
same rule at two scopes. If this line ships with a green badge and no step
that reads it at the cut, the next four months look like the last four.

**Cache poisoning across a toolchain bump** (§6 shape 2): a cache key that
omits `rust-toolchain.toml`'s hash serves the old toolchain to the new
declaration — silently, the E-037 shape. The key includes the file.
