# Developer Handoff — RFC-0.24-002

**Governing RFC:** [RFC-0.24-002](../../proposed/RFC-0.24-002-instrument-repairs.md)
**Milestone:** 0.24 — blocks the cut
**Status:** inherited from the governing RFC (Proposed — accepted for implementation 2026-08-03)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This is the opposite line from the last one

RFC-0.24-001 forbade fixing. This RFC is only fixing — **seven named repairs and
nothing else.**

That inversion is the main risk. You spent four passes recording 37 findings
without touching them, and 30 of those are still open, still one edit away, and
several are two-line changes you already know how to make. **Do not make them.**
The seven are enumerated in the RFC; the non-goals name the rest explicitly.

If a slice's fix seems to naturally pull in an eighth thing, that is R2 firing.
Stop and escalate.

## 0.1 What "done" means for one slice

1. **The instrument observed failing** on a deliberately broken input — *before*
   the fix, reproducing the audit's finding, and *after* the fix, confirming it
   now catches what it missed.
2. The fix.
3. The register row in `docs/verification/instrument-audit.md` moved from
   `finding` to `sound`, citing the demonstration.

**Step 1 before step 2, always.** RFC-v0.22-001's governing principle is that a
gate never observed failing is a gate with no evidence it works. A fix whose
demonstration was only ever run green proves nothing.

## 0.2 Design decisions settled — do not re-open

1. **Seven slices, independently revertible.** Commit per slice. If one turns
   out to be wrong, it comes out without disturbing the others.
2. **Submit once, at the end**, not per slice — unlike the audit. These are
   small and interdependent only through Slices 1 and 3. One review request.
3. **Slice order is not free.** See §2.

---

## 1. Change scope

**In scope, by file:**

| File | Slice |
|---|---|
| `crates/fjell-tools/src/release_rehearsal.rs` | 1 |
| `crates/fjell-tools/src/smoke.rs` | 2 |
| `tools/fjell-unsafe-audit/src/main.rs` | 3 |
| `.github/workflows/ci.yml` | 3, 6, 7 |
| `examples/three-node-fleet/fjell-hello/src/main.rs` | 3 — **after** the demonstration |
| `crates/fjell-tools/src/negative.rs` | 4 |
| `tools/fjell-abi-snapshot/src/main.rs` | 5 |
| `tests/abi/snapshot.json` | 5 — only if the format changes |
| `docs/verification/instrument-audit.md` | all — row updates |

**Explicitly NOT in scope** — the full list is in the RFC's non-goals; the ones
you are most likely to reach for:

- Gates 3–8's string-matching predicates. Slice 1 touches Gates 1 and 2 **only**.
- The `FORBIDDEN` `"TEST:FAIL"` miss, the TOML bracket truncation, the stale
  `KNOWN_*_CATEGORIES` lists.
- The 19 crates never named in `ci.yml`; the missing `semantic` negative
  category in CI's matrix; `v0.6-verification`.
- **Adding any instrument** — no schema drift check, no `release-rehearsal`
  proptest gate, no README link checker. All three are tempting and all three
  are 0.25 candidates.
- E-013's fix.
- Gate 12 `syscall-surface` must stay **35/26/9**.

## 2. Slice order — this one matters

**Slice 3 first, and its demonstration before its fix.**

The tree is **red right now** on real committed input, and you will only see it
if you look before you touch anything:

```
$ cargo run -p fjell-unsafe-audit -- --workspace . --check
  total unsafe sites : 284
  with valid category tag: 283
  MISSING/UNKNOWN category: 1
  missing comment    : 0
exit=0
```

`examples/three-node-fleet/fjell-hello/src/main.rs:46` is tagged
`category=asm-instruction`, which is not one of the seven valid categories.
Sequence:

1. Capture the above on the untouched tree.
2. Make `--check` exit non-zero when `missing_cats > 0`. **Run it — it now
   fails on real committed input.** Capture that. This is the demonstration, and
   it is the only one in the whole audit that required constructing nothing.
3. *Then* correct the tag to `csr-asm`. Confirm green.

**Do not correct the tag first.** If you do, the demonstration is gone and
cannot be recovered without faking it.

**Then Slice 1**, because Slice 3's enforcement does not reach Gate 2 until
Gate 2 reads exit status. Verify that ordering holds: after Slice 3 and before
Slice 1, Gate 2 should still report `PASS` on a category violation — confirm
that, because it is the evidence that Slice 1 is load-bearing rather than
cosmetic.

Slices 2, 4, 5, 6, 7 are independent. Any order.

## 3. Per-slice demonstrations

| Slice | Break | Instrument must |
|---|---|---|
| 1 | Syntax error in `fjell-store-model` (not a `fjell-tools` dependency, so the rehearsal binary still builds) | Gate 1 → `FAIL` (Pass 1 recorded `PASS`) |
| 2 | `cargo xtask qemu-test m8-typo` | Fail naming the unknown milestone, not run `m8` |
| 3 | none — the tree is already broken | `--check` → exit 1, then green after the tag fix |
| 4 | none — `lease` has no profile | `cargo xtask qemu-negative lease` → fail naming the missing profile |
| 5 | `jq -c . tests/abi/snapshot.json`; separately, truncate mid-array | `--verify` → `FAIL` in **both** cases |
| 6 | Run the corrected command; then break one property | 24 tests reported, not 0; then job fails |
| 7 | Delete a `.frozen` file | Script → `FAIL` (currently exits 0) |

Revert every constructed break and confirm `git diff --stat` clean before moving
on. Slices 3 and 4 need no revert — nothing was constructed.

## 4. Two places the RFC corrects itself — read them

- **Slice 2** no longer includes `fjell-unsafe-audit`'s `_ => Self::Unknown`
  catch-all. The Pass 4 review record folded it in as "same shape, same fix";
  it is the same shape but **not** the same fix. `Unknown` is a legitimate value
  to compute — the defect is that nothing enforces it, which is Slice 3. **The
  arm stays.**
- **Slice 5** is stated as a *property*, not an implementation: a snapshot that
  does not parse completely must fail, never read as empty or partial. Both the
  total case (zero items) and the **partial** case (some items, not all) must be
  caught. A zero-check alone would miss the partial case, which is the more
  insidious one. Route is yours; if it changes the committed artifact's format,
  regenerate it in the same slice and say so in the review request.

## 5. Prohibited shortcuts

- Do not fix a finding outside the seven. Thirty are open by design.
- Do not add an instrument, however small, however obviously good.
- Do not weaken a predicate to get a green tree. If Slice 1 fires on something
  real and pre-existing, **stop and escalate** — that is R1, and it is the gate
  working.
- Do not correct the `asm-instruction` tag before capturing the failure.
- Do not run a demonstration only after the fix. Failing-first is the point.
- Do not mark unexecuted commands as passed.

## 6. Required evidence

1. **Fourteen demonstration captures** — each slice failing before, passing
   after. Slice 3's "before" is on the untouched tree.
2. `cargo xtask release-rehearsal` green; Gate 12 still **35/26/9**.
3. `cargo xtask test-all` — all 19 tiers. Tier 2 unaffected; the ten negative
   categories unaffected by Slice 4.
4. `cargo fmt --all --check` clean. *(Named explicitly because omitting it from
   RFC-v0.23-002's evidence list let a fmt failure surface at release prep.)*
5. `docs/verification/instrument-audit.md` — seven rows moved `finding` →
   `sound`, each citing its demonstration.

## 7. Review request

Standard format, in `.git-exclude/review-request/`. **One request for all seven
slices.**

Flag for focused review:

- Any slice where the "before" demonstration was harder to produce than expected
  — that usually means the finding was characterised imprecisely.
- Slice 5's chosen route, and whether the partial-parse case is genuinely
  covered rather than covered in principle.
- Anything you were tempted to fix and did not. Name it; that list is useful to
  the close-out.
