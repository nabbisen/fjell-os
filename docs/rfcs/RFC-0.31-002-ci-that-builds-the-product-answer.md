# RFC-0.31-002 §6 — What does CI cache, and what does a green run cost?

**Governing RFC:** [rfcs/accepted/RFC-0.31-002-ci-that-builds-the-product.md](../../rfcs/accepted/RFC-0.31-002-ci-that-builds-the-product.md)

Answered from R1's measurement, which was taken and written down before the
question was decided — the handoff's required order, and the same order that
turned RFC-0.30-001's "doubles CI time" into 4–7 seconds.

---

## R1 — the number, first

One job converted: `ci-repro-check`. Single, non-matrix, `build-std`, and
its own predecessor had never run.

| | Run | Job | Step | Duration |
|---|---|---|---|---|
| **apt, before** | `34657033682` (`bb4ffbd`) | `103451689454` | `Install Rust 1.91 + LLVM (lld)`<br>23:12:11Z → 23:12:37Z | **26 s** |
| **rustup, uncached** | `34674222129` (`fc5d592`) | `103501122342` | `Install toolchain`<br>04:55:48Z → 04:55:58Z | **10 s** |
| **rustup, uncached, again** | `34674356238` (`ca1dcd5`) | `103501524504` | `Install toolchain`<br>04:59:00Z → 04:59:10Z | **10 s** |

Two independent observations, both 10 s, neither cached.

**Uncached rustup is 16 seconds cheaper per job than the apt path it
replaces.** That is not a close call and it is not the direction the question
was framed in. §6 opens with *"Uncached, that is a download per job across
~30 jobs"* — true, and the download is smaller and faster than the apt
transaction it removes, because `profile = "minimal"` plus four components is
less than `rustc-1.91 cargo-1.91 rust-src` out of the Ubuntu archive, and
because `rustup` is already on the runner and `apt-get update` is not free.

## Answer: shape 1 — no cache

**Chosen because of the number, not in spite of it.** The RFC leans to
shape 1 "until the number says otherwise". The number says the same thing
the RFC leaned: rustup is faster than what it replaces, so across ~30 jobs
this line *reduces* total install time by roughly eight minutes of runner
wall-clock rather than adding any.

Shapes 2 and 3 are both refused, and for different reasons.

**Shape 2 (`actions/cache`) is refused as negative value.** A cache exists to
buy back time. There is no time here to buy back: the ceiling on what a
perfect cache could save is 10 seconds a job, and a cache restore of
`~/.rustup` and `~/.cargo` is not reliably faster than that. Against that, it
costs a key that has to be right forever. The RFC's own Risks section names
what a wrong key does — *"a cache key that omits `rust-toolchain.toml`'s hash
serves the old toolchain to the new declaration, silently, the E-037 shape"*.
Adding a mechanism whose best case is roughly zero and whose failure mode is
a silent toolchain substitution is a bad trade at any price. If a future
component set makes the install minutes rather than seconds, shape 2 with
`hashFiles('rust-toolchain.toml', 'Cargo.lock')` in the key is the answer
then; it is not the answer to 10 seconds.

**Shape 3 (a thinner component set in CI) is refused on the RFC's own
grounds, which the number now makes unanswerable.** Shape 3 asks CI to
install a *different* set than the file declares — it would be the
twenty-third place the toolchain is written down, in the line whose entire
purpose is making `rust-toolchain.toml` the single authority. Its only
argument was cost. Cost is 10 seconds. There is no argument left.

**What the D2 check does to this question.** Shape 1's historical objection
was never cost — it was RFC-0.30-003 §7's: rustup on a missing or unreadable
declaration falls back to the runner's ambient `stable` and goes green. That
objection is answered by the composite action's assert-then-verify pair, not
by the cache decision, and D8's first demonstration is the evidence. Without
that check, shape 1 at any speed would still be wrong.

## What R1 also turned up, which no measurement was looking for

R1's first run (`34674222129`) failed, and failing is how it earned its
keep. It got the toolchain right and died one layer further down:

```
error: linker `ld.lld` not found
```

`.cargo/config.toml:18` names `ld.lld` as the linker for
`riscv64gc-unknown-none-elf`. Ubuntu's `llvm` package does not ship it — the
separate `lld` package does — and **every apt block in this workflow has
asked for `llvm` alone.** The omission has never been observable, because
every job carrying it died at `-Z build-std` several minutes before any
linker was invoked. It became observable in the first run where the
toolchain worked, and it was fixed in the next commit.

This is the shape of the whole erratum in one job: a second, independent,
never-once-executed defect, sitting behind the first one, invisible for
exactly as long as the thing in front of it failed. It is also why R1 is
ordered before the rollout. Had thirty jobs been converted at once, `ld.lld`
would have been one red line among thirty and indistinguishable from a
broken conversion.

**`ci-repro-check` is green on run `34674356238`, job `103501524504`** — the
first job in this workflow's history to build the kernel and the services.
`cargo xtask two-build-check` ran 04:59:10Z → 04:59:37Z: 27 seconds to build
the product twice and compare it. RFC-0.30-001's check has existed since
0.30 and this is the first time it has executed anywhere but a developer's
machine.
