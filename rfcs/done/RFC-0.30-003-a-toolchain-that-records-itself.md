# RFC-0.30-003: The toolchain is declared in twenty-two places, and no artefact records which one built it

**Status:** Implemented (0.30.0) — accepted 2026-09-10
**Milestone:** 0.30
**Tracks.** **E-037** — the last remaining piece of the 0.30 instrument arc, and
the one RFC-0.30-001 explicitly left open when it closed E-036 ("each CI run is
itself one machine building twice; nothing compares digests across two different
machines/toolchains").
**Touches.** `rust-toolchain.toml`, `.github/workflows/ci.yml`,
`tools/fjell-repro-check`, `crates/fjell-tools` (provenance/trust-report
writers), `docs/src/internals/local-development.md`,
`docs/src/tutorials/quick-start.md`, `docs/release/release-checklist.md`.
**Does not touch the kernel, the ABI surface, or any service.**
**Relates to:** RFC-0.30-001 (E-036; this is its named residual);
RFC-0.27-004 (the `.provenance.txt` format this extends); RFC-0.24-003.

## Summary

E-037 says the toolchain is declared in **five** places. **Measured, it is
twenty-two**, and the erratum is wrong in three further particulars. All figures
below were re-derived for this RFC; none is inherited.

### Finding 1 — seventeen of the twenty-two are the same block, copied

| Site | What it says | Count |
|---|---|---|
| `.github/workflows/ci.yml` | `apt-get install -y rustc-1.91 cargo-1.91 …` + two `ln -sf` lines into `$HOME/.local/bin` | **17 jobs** of 19 |
| `rust-toolchain.toml` | `channel = "1.91"` | 1 |
| `Cargo.toml` | `rust-version = "1.91"` | 1 |
| `docs/src/internals/local-development.md:7,21` | prerequisite table + `rustup toolchain install 1.91` | 1 |
| `docs/src/tutorials/quick-start.md:10-11` | `apt install rustc-1.91 cargo-1.91 rust-1.91-src …` | 1 |
| `docs/release/release-checklist.md:25` | `rustc --version \| grep "1.91"` | 1 |

Only `ci-docs` and `ci-fuzz-nightly` lack the install block. **A version bump is
seventeen identical hand-edits plus five different ones** — not "an edit", and
not five. E-037's own claim that bumping is "a scoped piece of work, not an
edit" is right; its number is not.

`docs/src/tutorials/quick-start.md` is the one E-037 missed entirely, and it is
the one a **new user actually follows**. It also names `rust-1.91-src` where CI
installs `rust-src` — a second, quieter disagreement inside the same family.

### Finding 2 — one of them is a check, not two

E-037 says "two of those are checks that would go on asserting 1.91 after a
bump." **One is:** `docs/release/release-checklist.md:25`'s
`rustc --version | grep "1.91"`. The rest are declarations or prose. The
distinction matters because a check that asserts a stale number is worse than a
document that does — it fails a correct build.

### Finding 3 — most occurrences of `1.91` must NOT move, and a naive gate would break them

`grep -rn "1\.91"` finds it in `CHANGELOG.md`, `docs/src/releases/v0.7-release-
notes.md`, `docs/src/releases/v0.1.0-scope.md`, `handoff-v0.17-v0.18.md`, three
files under `handoff-0.21.2/`, and the Verus review records. **Every one of
those is a historical record of what was true at that release and is correct as
written.** Any consolidation or any "all sites agree" gate that cannot tell a
live declaration from a historical one will either flag correct files or force
someone to falsify a release note. This is the specific trap in this erratum,
and it is why "one declaration" is harder here than it sounds.

### Finding 4 — there are three toolchains, and only one is declared in the pinned file

| Toolchain | For | Declared in |
|---|---|---|
| `1.91` (floating within `1.91.x`) | kernel, services, tools | `rust-toolchain.toml` + the 21 sites above |
| `1.95.0-x86_64-unknown-linux-gnu` | the Verus prover (Gate 10) | `ci.yml:577`, `docs/src/internals/local-development.md:165`, `docs/src/verification/verus-setup.md:18` |
| `nightly` | `cargo-fuzz` (`ci-fuzz-nightly`) | `ci.yml:549` only |

`rust-toolchain.toml` describes one of the three. The other two are declared
only where they are used.

### Finding 5 — nothing records the toolchain alongside anything it produced

Checked directly, all three artefact records:

| Record | Header fields | Toolchain? |
|---|---|---|
| `tests/repro/baseline-digests.txt` | `# algo: sha256` — that is the entire header | **no** |
| `tests/evidence/**/*.provenance.txt` | `run_id`, `profile`, `commit_sha`, `command`, `instrumented` | **no** |
| `docs/release/trust-report.txt` | `Generated`, `Version`, `Mode` | **no** |

So a digest mismatch on another machine is **indistinguishable from a real
reproducibility failure**, which is precisely the claim RFC-0.30-001 declined to
make and recorded as this erratum's residual.

**This is not hypothetical.** On 2026-09-09 `rust-toolchain.toml` was removed;
local builds silently moved from **1.91.1 to 1.98.1**; all 24 committed
prebuilts changed and `repro-check` went red. The check fired — it could not say
**why**, and the cause was found only because someone happened to be checking
the removal at the time. The detector exists; the attribution does not.

### Finding 6 — CI structurally cannot follow `rust-toolchain.toml`

CI installs rustc from **apt** and symlinks it into `PATH`, bypassing rustup
entirely, so `rust-toolchain.toml` governs local builds only. And apt on
`ubuntu-24.04` does not carry a current rustc — **bumping CI means changing the
install method, not editing a number.** Any answer to §7 that assumes CI can
just read the file has to solve this first.

## The settled part

**D1 — Every artefact-producing path records the toolchain that produced it.**
`tests/repro/baseline-digests.txt`, `tests/evidence/**/*.provenance.txt`, and
`docs/release/trust-report.txt`. This is the half of E-037 that RFC-0.30-001's
residual depends on and it lands in this line regardless of how §7 is decided.

**D2 — Record the *observed* toolchain, never the declared one.** The value
written must come from running `rustc -vV` at the moment of production —
`release`, `commit-hash`, `host`, and `LLVM version`, all four. Reading
`rust-toolchain.toml` and writing its channel down is **proxy attestation**, the
defect family this whole arc has been removing: it would have recorded `1.91`
throughout the 1.98.1 incident, on every artefact, while being wrong about all
of them. `rustc -vV` on this machine today yields
`1.91.1 / ed61e7d7e / x86_64-unknown-linux-gnu / LLVM 21.1.2`; the declaration
yields `1.91`, which is four of those four fields short.

**D3 — Live declarations and historical records are different things** (Finding
3). Whatever is built must distinguish them explicitly and by rule, not by an
exclusion list that will go stale — this project has already learned that
lesson twice (E-014, E-031).

**D4 — Correct E-037's text, do not merely close it.** Twenty-two places, not
five; one check, not two; `quick-start.md` named. Same requirement, same reason,
as RFC-0.30-002's R5 for E-038.

**D5 — The floating channel is a separate decision from this line.**
`channel = "1.91"` matches any `1.91.x`, and a patch bump moves digests. Pinning
to an exact patch is a behaviour change; it may be right, but it must be argued
on its own and not smuggled in alongside a recording change. If this line
concludes it should be pinned, say so and stop — do not pin.

**D6 — Demonstrated failing** (RFC-v0.22-001). Whatever is built must be shown
red on an input that is genuinely wrong: a recorded toolchain that disagrees
with the running one, or a declaration site left behind at a bump. A passing run
is not evidence.

## The open question — §7

**What is the single source of truth, given CI structurally cannot read
`rust-toolchain.toml` (Finding 6)?**

1. **Make CI use rustup.** Replace the 17 apt+symlink blocks with one composite
   action that runs rustup, which honours `rust-toolchain.toml` by construction.
   One declaration for real. Cost: a rustup install per job (runtime, cache
   design), and the Verus and nightly jobs still declare their own toolchains
   separately. Also the largest CI change this project has made.
2. **Keep apt; generate the blocks.** A tool reads `rust-toolchain.toml` and
   checks (or regenerates) `ci.yml`'s install steps, gated in Gate 12. Keeps
   CI's fast install; adds a generator and a second format to keep valid.
3. **Consolidate nothing; make drift impossible to miss.** A new
   `consistency-check` subcheck parses every **live** declaration site and fails
   when they disagree, with D3's live-vs-historical rule as its core. Cheapest,
   demonstrable, and squarely this project's established pattern — but it leaves
   twenty-two sites to edit at a bump, and **it does not meet E-037's own stated
   closure bar** ("one declaration, an exact pin, and a record of which
   toolchain produced each artefact").

**Answer in writing before implementing**, and answer the second question with
it: **does E-037 close on the shape you pick, or does its closure bar move?** If
shape 3, the bar has to move and the erratum must say why in its own words —
that is a legitimate outcome, but it must be argued, not left implicit by
marking the entry CLOSED.

**I lean to 3 for this line, with 1 named as its successor**, and I want that
lean attacked rather than adopted. My reasoning: D1 is the part with a
downstream dependency (RFC-0.30-001's residual), it is independent of the
consolidation shape, and a drift gate is what makes any later consolidation
safe to attempt. My reasoning's weakness: three consecutive lines in this
milestone have now chosen "make the instrument able to see" over "fix the thing
being measured," and at some point that pattern stops being a principle and
starts being an avoidance. If shape 1 is right, this is the line to say so.

## Requirements

**R1 — Re-derive the counts** before designing anything, and report every figure
that disagrees with this RFC's, in either direction. Six consecutive lines have
corrected at least one of my numbers; I would be more surprised by none than by
another.

**R2 — D1 built**: all three artefact records carry the observed toolchain,
per D2. `.provenance.txt` gains a required field, which means the `evidence`
subcheck's `REQUIRED_PROVENANCE_FIELDS` moves — existing files must be handled
deliberately (backfilled from their `commit_sha`, or the field made required
only for new ones, with the choice argued).

**R3 — §7 answered in writing**, with the E-037 closure-bar question answered
alongside it, and built accordingly.

**R4 — Both demonstrated failing** (D6).

**R5 — E-037's text corrected** (D4), and its Resolution set to whatever §7's
answer justifies — `CLOSED`, or `ACCEPTED` with the surviving instances named,
as RFC-0.29-002 did for E-014. **Do not close it to tidy the register.** A
partial close with two named survivors is a better record than a clean one that
is not true.

**R6 — If any instance survives, `docs/release/v1-limitations.md` says so** in
the reader's terms, not the register's.

### Non-goals

- **Pinning the channel to an exact patch** (D5) — argue it, do not do it.
- **Bumping the version.** This line makes a bump tractable and visible; it does
  not perform one. `rust-version = "1.91"` stays a floor, and it is worth saying
  explicitly whether a floor should move on a toolchain bump at all — it
  generally should not.
- **The Verus and nightly toolchains' consolidation** (Finding 4). Naming them
  is in scope; unifying them is not. Verus's prover version is already recorded
  in its own review records, which is more than the build toolchain manages.
- Any kernel, ABI-surface, or service change.
- E-014's two survivors, E-034, E-039.

## Risks

**Shape 1 is the largest CI change this project has made**, and CI is the only
thing standing between a bad merge and `main`. If §7 picks it, it needs its own
demonstration that every one of the 19 jobs still does what it did — not just
that CI is green, since a job that silently stops running its check is also
green. That is E-036's exact shape, one layer up.

**D1 changes a committed file format** (`.provenance.txt`) that a gate already
validates. Getting R2's migration wrong turns Gate 12 red on files nobody
touched.

**The temptation here is to close E-037 by writing the toolchain into three
headers and calling the declaration problem someone else's.** That may in fact
be the right split — but if so it is a deliberate, argued partial close, not a
silent one, and the register has to read that way afterwards.
