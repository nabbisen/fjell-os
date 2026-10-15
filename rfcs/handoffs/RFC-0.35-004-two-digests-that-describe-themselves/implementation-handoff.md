# Developer Handoff — RFC-0.35-004

**Governing RFC:** [RFC-0.35-004](../../done/RFC-0.35-004-two-digests-that-describe-themselves.md)
**Milestone:** 0.35
**Status:** inherited from the governing RFC (Implemented, 0.35.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can a second implementation of the measurement chain produce the same digest
from a written description?** Today it cannot: there is no description, and
because you cannot decode a hash, there is also no round trip that would catch a
drift. When both streams have a generated file that reproduces digests captured
from the unmodified encoders, this line is done.

**Not the measure:** that E-065's row says CLOSED. The row is a consequence.

## 0.1 Take the goldens before you touch anything

This is the whole safety argument and it can only be done once. For each of
`compute_chain_digest` and `compute_bundle_digest`, from the encoder **as it
stands**: record the digest for several inputs — include the metadata-absent and
metadata-present cases for the chain, and a short and a long `name` for the
bundle — and, where you can reach it, the concatenated byte stream itself. Commit
them as test fixtures in the same commit as the first edit, so the moved encoder
is checked against bytes recorded before it moved.

**If any digest changes afterwards, stop.** A changed digest here is a format
change in the attestation path, and this line does not make one.

## 1. Order

1. **R1 re-derivation** — confirm or refute the RFC's two size claims. Escalate
   before writing if `Canon` needs more than one primitive.
2. **R2 goldens**, from the unmodified encoders.
3. **R3** — `Canon` gains the big-endian `u32`, with a default that writes what
   is written today. Show the eighteen existing files unchanged.
4. **Move `compute_chain_digest`** onto `Canon`. No new primitive needed; the
   metadata slot is a presence byte plus fixed 32 bytes — **not** `optional`.
5. **Move `compute_bundle_digest`** onto `Canon`, using the new primitive.
6. **R4** — the unlengthed-`name` argument, written down or refuted.
7. **R5/R6** — both files' caveats and their version headers.
8. **R7** — E-065 closed, limitations in the same commit, survivors down to
   E-073's two.
9. **R8** — the gates, as the required set.

## 2. Traps specific to this line

1. **`Canon::optional` is the wrong primitive for the chain digest** and it is the
   one you will reach for. It writes its body only when present; the chain digest
   writes 32 bytes either way. Using it would generate a description of a format
   this project does not have — the exact defect the census exists to prevent.
2. **Do not add a length prefix to the bundle digest's `name`.** R4 wants the
   safety argument written down. A prefix changes the digest.
3. **A new `Canon` primitive can move an existing description** if its default
   differs from what encoders already write. RFC-0.34-003 added five without
   moving any of the seventeen then-existing files; match that, and state the
   count.
4. **`frozen.rs` has four tests that will object** if a survivor's bookkeeping is
   half-moved: every formats crate described or excused, a survivor naming an
   erratum the register contains, a survivor's probe still finding a producer, and
   the per-format round trip. Moving two entries out of `EXCLUSIONS` touches all
   four — run them before you believe the move is done.
5. **E-073 is not yours.** Its two `#[repr(C)]` layouts stay `Survivor`, and its
   erratum reference in `EXCLUSIONS` stays `E-073`. If the survivor count does not
   land on exactly two, something was swept.
6. **No guest code changes here**, so no prebuilt should move. If one does,
   re-record the repro baseline in the **same commit** and say which binary moved
   and why — but first check whether you changed something you did not mean to.
7. **`fjell-repro-check` exits 2 for a usage error and 1 for a real failure** as of
   RFC-0.35-003. Pass only the flags its `--help` names.

## 3. What to put in the review request

- R1's answer: did both size claims hold?
- The goldens, and the moved encoders reproducing them byte for byte, for every
  case including metadata-absent/present and short/long `name`.
- The `Canon` primitive, and the count of existing descriptions that moved (expect
  **zero**).
- Both generated files, with their caveats and version headers.
- R4's argument about the unlengthed `name`.
- The survivor count after the move (**two**, E-073's), and E-065 CLOSED with
  limitations in the same commit.
- Each gate by its own exit status, **as the required set, failures included**:
  `release-rehearsal`, `abi-snapshot --verify`, `consistency-check --all`,
  `cargo fmt --all --check`, `test-all`, `repro-check`.

Hand back one path: your review request. Do not tag anything.
