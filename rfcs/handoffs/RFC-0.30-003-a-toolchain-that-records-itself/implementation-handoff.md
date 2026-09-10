# Developer Handoff — RFC-0.30-003

**Governing RFC:** [RFC-0.30-003](../../done/RFC-0.30-003-a-toolchain-that-records-itself.md)
**Milestone:** 0.30
**Status:** inherited from the governing RFC (Implemented, 0.30.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This is the last of the 0.30 instrument arc, and the only one with a hole under it

RFC-0.30-001 closed E-036 and left a residual in its own words: each CI run is
one machine building twice, so **nothing compares digests across two machines or
two toolchains**. That residual is E-037, and it is not closable while no
artefact records what compiled it.

The failure is not hypothetical and it is one day old. On 2026-09-09
`rust-toolchain.toml` was removed; local builds silently moved from **1.91.1 to
1.98.1**; all 24 committed prebuilts changed and `repro-check` went red. **The
detector fired and could say nothing about why.** It was diagnosed only because
someone happened to be checking the removal at that moment. Build the
attribution that was missing.

## 0.1 Re-derive the counts first, and expect to correct me

The RFC's Findings 1-4 are all measured, this week, by me. **Nine consecutive
lines have corrected at least one of my figures**, and E-037's own numbers were
wrong twice over until yesterday (five places when it is twenty-two; two checks
when it is one). Start here:

```
grep -c "apt-get install -y rustc-1.91" .github/workflows/ci.yml
grep -rn "1\.91" --include='*.md' --include='*.toml' --include='*.yml' . | grep -v '^\./target'
rustc -vV
head -3 tests/repro/baseline-digests.txt
cat "$(find tests/evidence -name '*.provenance.txt' | head -1)"
```

Report every figure that disagrees with the RFC's, **in either direction** —
including any where I over-counted. A corrected number is a finding, not an
embarrassment.

## 0.2 Design decisions settled — do not re-open

1. **All three artefact records carry the toolchain** (D1):
   `tests/repro/baseline-digests.txt`, `tests/evidence/**/*.provenance.txt`,
   `docs/release/trust-report.txt`.
2. **Record the observed toolchain, never the declared one** (D2). Run
   `rustc -vV` at production time and record `release`, `commit-hash`, `host`
   and `LLVM version` — all four. **Reading `rust-toolchain.toml` and writing
   its channel down is the defect, not the fix.**
3. **Live declarations and historical records are different things** (D3), by
   rule, not by an exclusion list.
4. **Correct E-037's text, do not merely close it** (D4).
5. **Do not pin the floating channel** (D5) — argue it if you conclude it should
   be, then stop.

---

## 1. Order

**Re-derive the counts → D1/D2 (the recording) → §7 answered → §7 built → both
demonstrated failing → E-037 resolved honestly → evidence.**

The recording comes **first and independently**. It is the half with a
downstream dependency, it is decided, and it must not end up hostage to how §7
lands.

## 2. D2 is the whole point — read this twice

The tempting implementation is to parse `rust-toolchain.toml` and write
`channel` into the three headers. It is less code and it looks identical on a
passing run.

**It would have recorded `1.91` on every artefact throughout the 1.98.1
incident, while being wrong about all of them.** That is proxy attestation —
mode 2 of the defect class this entire milestone has been removing, and the same
shape as Gate 6 counting a stale trust-report's sections without checking the
regeneration succeeded (RFC-0.29-002 R2).

The declaration says `1.91`. The truth today is
`1.91.1 / ed61e7d7e / x86_64-unknown-linux-gnu / LLVM 21.1.2`. Record the truth.

## 3. R2's migration has a live gate under it

`.provenance.txt` gains a required field, and `evidence`'s
`REQUIRED_PROVENANCE_FIELDS` is what validates that file today. Seven evidence
files already exist. **Decide deliberately** whether to backfill them (from
what? their `commit_sha` does not tell you the toolchain — say so if it cannot
be recovered) or to require the field only for new files, and **argue the
choice**. Getting this wrong turns Gate 12 red on files nobody touched.

If a historical file's toolchain genuinely cannot be recovered, record that it
cannot — an honest `unknown (predates this field; not recoverable)` is a true
record. A guessed `1.91` is not.

## 4. §7 — and my lean is the part I most want attacked

**What is the single source of truth, given CI structurally cannot read
`rust-toolchain.toml`?** CI installs rustc from apt and symlinks it into `PATH`,
bypassing rustup entirely, and ubuntu-24.04's apt carries no current rustc — so
"just make CI read the file" is a change of install method, not of a number.
Solve that before proposing it.

**I lean to shape 3** (a drift gate over the live declaration sites), with shape
1 (rustup in CI) named as its successor line.

**Attack this.** My own reasoning against myself, stated in the RFC: three
consecutive lines in this milestone have now chosen *"make the instrument able
to see"* over *"fix the thing being measured,"* and at some point that stops
being a principle and becomes avoidance. **If shape 1 is right, this is the line
to say so** — and you are better placed than I am to judge, because you will
have just measured how much of `ci.yml` is duplication.

Answer the second question with it: **does E-037 close on the shape you pick,
or does its closure bar move?** The bar it set for itself is *"one declaration,
an exact pin, and a record of which toolchain produced each artefact."* Shape 3
meets one of three. That may still be the right call for this line — but say so
in the erratum's own words rather than marking it CLOSED and leaving the reader
to notice.

## 5. Prohibited shortcuts

- **Do not record the declared toolchain.** See §2.
- Do not close E-037 to tidy the register. **A partial close with named
  survivors is a better record than a clean one that is not true** — RFC-0.29-002
  did exactly this for E-014 and it was the right call.
- Do not bump the version. Do not pin the channel.
- Do not "fix" the historical `1.91` mentions in release notes, handoffs or the
  changelog. They are correct as written.
- Do not touch the Verus (`1.95.0`) or nightly toolchains beyond naming them.
- Do not touch the kernel, the ABI surface, or any service.
- Do not take on E-014's survivors, E-034 or E-039.

## 6. Required evidence

1. **Every count re-derived** (§0.1), with each disagreement against the RFC
   reported in either direction.
2. **The three records carrying the observed toolchain**, with a before/after of
   each header.
3. **The R2 migration choice argued**, and `evidence` green on the existing
   seven files.
4. **§7 answered in writing**, with the E-037 closure-bar question answered
   alongside it.
5. **Demonstrated failing** (D6), on genuinely wrong input: a recorded toolchain
   disagreeing with the running one, and — if §7 built a drift gate — a
   declaration site left behind at a bump. Show that the same input passed
   before.
6. **E-037 resolved honestly** — `CLOSED`, or `ACCEPTED` with every surviving
   instance named — text corrected either way, register and
   `docs/release/v1-limitations.md` in the same commit.
7. `release-rehearsal` green; `test-all` all tiers; `consistency-check --all`;
   `syscall-surface` 35/29/6; `callsite-audit` 5 checks.
8. `cargo fmt --all --check`.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Your §7 answer**, and specifically whether you think shape 3 is the right
  call or an avoidance. I want the disagreement if you have one.
- **Whether E-037 genuinely closes.** If it does not, I would rather read two
  named survivors than a CLOSED I have to re-open.
- Anything that turned out **not** to have the defect.
- Any count of mine you re-derived and found different.
