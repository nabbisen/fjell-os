# RFC-0.32-001 §7 and §8 — what runs on push, and which toolchain fuzzes

**Governing RFC:** [rfcs/done/RFC-0.32-001-fuzzing-that-fuzzes.md](../../rfcs/done/RFC-0.32-001-fuzzing-that-fuzzes.md)

Answered after R1 and before R4, in the handoff's order. R1 changed both
answers, so its evidence comes first.

---

## What R1 established that these answers rest on

- **No decoder this line keeps has a live caller that receives untrusted
  bytes.** `fjell_semantic_v1::decode`'s non-test callers are the semantic
  toolkit's fixture checks and `fjell-proxy-text`'s `ingest`, which only that
  crate's tests call. The proxy's runtime loop never decodes an encoded
  envelope: `RENDER_COMMIT` goes straight to `chunked::reassemble` over raw
  `SemanticEnvelope` bytes (E-046; D7 keeps it out of fuzzing).
  `AuditRecordBin::from_bytes` is live, in `fjell-auditd`, but its input is
  the kernel's audit ring — the TCB. The two DTB decoders and
  `RevocationRecord::from_bytes` have no caller at all.
- **Thirty seconds of the first local run found a real crash** in a decoder
  nobody had fuzzed: `dtb_derive_board_profile`, *"attempt to add with
  overflow"* at `crates/fjell-dtb-derive/src/parser.rs:108`, where
  `get_string` adds two attacker-supplied `u32` offsets. One `checked_add`
  removes it; 120 seconds after the fix (11.1M executions) found nothing more.
- **Local cost, 32 cores:** a cold sanitizer build of a target in about 4 s;
  replaying a target's committed seeds in 54 ms. CI's cost is measured on CI
  in R4, not extrapolated from this.
- **Toolchain history:** across every scheduled run whose log still exists,
  the nightly floated from `1.98.0-nightly (2026-06-21)` to `1.100.0-nightly`
  while cargo-fuzz stayed `0.13.2`, and **no failure was caused by either** —
  two were a malformed root `Cargo.toml`, the rest defect 1.

---

## §7 — shape 2, plus seed replay, named for what it proves

**On push and pull request: `fuzz-build`.** It builds every target and replays
every committed seed once (`-runs=0`). **On schedule and `workflow_dispatch`:
`fuzz-run`**, each target for 300 seconds.

### Is shape 2's protection real?

Partly, and the part that is not has to be said plainly.

- **It catches a target that stops compiling** — each of Finding 1's three
  defects at the commit that introduces it, instead of on some later Monday.
- **Replay adds a decoder that starts panicking on an input already in the
  corpus.** That includes every crash a run has ever found, once its input is
  committed as a seed. The DTB crash input is the first such seed. The cost
  is milliseconds per target.
- **It does not catch a decoder that starts panicking on an input nobody has
  seen.** Only a run does that, and runs are weekly.

### The hazard, and what stops a build being read as fuzzing

The RFC names it and it is real: a green build job is the easiest thing in
this project to read as "fuzzing works". So:

- The job is **`fuzz-build`**, and its steps are *"Build fuzz targets"* and
  *"Replay committed seeds"*. Nothing on the push path is named `fuzz` alone.
- Exit criterion 9 records `fuzz-build` and the latest scheduled `fuzz-run`
  as **separate rows** (R7).
  *(Superseded by R7 as built, noted at review: criterion 9 reads a
  `workflow_dispatch` run of the release commit as the evidence, with its
  `fuzz-run` jobs' `Done` lines, and the latest scheduled run only as context.
  The same applies to "reads the latest scheduled run" below.)*
- The evidence rule — a run id and libFuzzer's `Done N runs in M second(s)` —
  **applies to `fuzz-run` only**. Replay prints the same line with `N` equal
  to the seed count and `M` zero, and is never cited as fuzzing.

### Is a weekly run enough for the decoders R1 found?

**Yes, for these decoders, and the reason is specific to them:** none has a
live caller receiving untrusted bytes. A weekly cadence is proportionate to
code that is exercised only by its own tests, by the TCB, or by nothing. And
it is not the only cadence: a cut happens about every three days, criterion 9
now reads the latest scheduled run at each one, and R7 makes the cut dispatch
`fuzz-run` against the release commit instead of trusting last Monday's run of
an older tree.

### Shape 3, argued rather than dismissed

The strongest case for shape 3 is R1's own result — thirty seconds found a
crash. I think it argues for something else.

That crash was **first contact**: the first time anything had ever run against
that decoder. First contact is where short runs pay, and this line makes first
contact with every kept decoder regardless of shape, by dispatching `fuzz-run`
before it closes. After first contact, a short push-time run's yield falls
sharply while its cost is paid on every push. What stays valuable is the input
it found, and replay keeps that on every push for milliseconds.

The handoff's condition for shape 3 — a decoder on a real, live trust boundary
— **is met by no decoder this line keeps**. The one live service-to-service
byte path is `reassemble`, which D7 holds out of fuzzing until E-046 makes it
sound. **When E-046's line writes a target against a sound `reassemble`, that
target is the one to argue shape 3 for**: IPC bytes from another service reach
it at runtime. Recorded here as its condition rather than left to be
rediscovered.

### Is my lean the cheap option chosen twice?

Shape 2 alone, named `fuzz`, would be. Shape 2 as built here claims only what
it proves: it compiles, and it replays known inputs. The claim that the
decoders are fuzzed rests on `fuzz-run`, with a run id and a `Done` line per
target, read at every cut.

---

## §8 — float the nightly, lock everything else, make the two failures look different

### Pinning the nightly is not worth its machinery yet

A dated nightly needs three things: a single declaration, a currency step like
exit criterion 10, and a change to `toolchain-declarations`, which deliberately
exempts jobs that run `cargo +<name>`. For a nightly, each fits badly. A dated
nightly is stale within a day. A threshold in minor versions does not apply to
it, and a threshold in days would fire at nearly every cut, which trains
readers to ignore it — E-037's silent staleness, rebuilt with more parts.

**The evidence for drift is weak, and I say so.** No failure in the retrievable
history was nightly-caused — but the harness never compiled, so the nightly
never had a chance to break it. The absence says little. It justifies starting
unpinned, not staying unpinned.

**The cost of floating is also real.** `fuzz-build` runs on push, so a nightly
regression can turn a push run red with no change to the tree.

### Floating, then: how a reader tells nightly drift from a real crash

**By which step failed.** `gh run view --json jobs` reports each step's
conclusion, and the two failures cannot land in the same step:

| Step | Nightly drift | A real crash |
|---|---|---|
| *Install nightly and cargo-fuzz* — also prints `rustc +nightly -vV`, the observed toolchain (RFC-0.30-003) | install failure | — |
| *Build fuzz targets* | **compile error, no artifact** | — |
| *Replay committed seeds* / *Fuzz `<target>` for 300 seconds* | — | **`SUMMARY: libFuzzer: deadly signal`, a `crash-<sha>` artifact uploaded** |

R7 records the failing step's name beside a red `fuzz-build` or `fuzz-run`.

### Everything that can be locked, is

- **cargo-fuzz:** `cargo install cargo-fuzz --locked --version 0.13.2`. A
  version bump becomes a reviewed edit.
- **`fuzz/Cargo.lock` is committed.** `fuzz/` is outside the root workspace, so
  the root `Cargo.lock` covers neither `libfuzzer-sys` nor the decoders'
  dependencies as resolved for the fuzz crate.
- **`apt-get install rustup` is removed.** The runner already has rustup;
  RFC-0.31-002's composite action calls it directly on `ubuntu-24.04`.

### When to revisit

**At the first failure whose failing step is *Build fuzz targets* on an
unchanged tree.** That is observed drift, and it is the moment to pin a dated
nightly and pay for all three pieces of machinery. `toolchain-declarations`
stays as it is until then: its `cargo +<name>` exemption is correct for a
floating nightly.
