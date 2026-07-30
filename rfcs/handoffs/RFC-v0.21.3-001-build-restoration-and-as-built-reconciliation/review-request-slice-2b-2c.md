# Review Request — RFC-v0.21.3-001, Slices 2b & 2c, and Finding C

**Governing RFC:** [RFC-v0.21.3-001](../../proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md)
**Handoff:** [implementation-handoff.md](./implementation-handoff.md) §3b, §3c
**Prior review:** [review-record-slice-1-2.md](./review-record-slice-1-2.md)
**Submitted by:** implementation model
**Status:** Slices 2b and 2c complete and committed, both authorized fixes
verified green. Finding C (raised by the prior review) is characterized
below; §4.3 remains blocked on it, as anticipated. Slice 3 not started.

Branch: `docs/v0.21.3-rfc-and-design-baseline`
Commits: `e54a700` (Slice 2b), `c3e24f1` (Slice 2c)

---

## 1. Implementation summary

Executed both fixes exactly as authorized, in the order and separation the
review record specified (§5): Slice 2b (comment relocation, Gate 2) then
Slice 2c (path fix + regeneration, Gate 4), each a standalone commit, neither
touching Slice 2's `c0f5b9c`. Then characterized Finding C per §6 of the
review record, as required before Slice 3 §4.3 may proceed.

## 2. Addressed RFC sections / review-record rulings

- §3 (Finding A ruling) — Slice 2b.
- §4 (Finding B ruling) — Slice 2c.
- §5 (slice placement) — followed: two separate commits, before Slice 3.
- §6 (Finding C) — characterized; see §7 below. Not fixed (no ruling
  authorized a fix; §6 asks only for characterization or an explicit
  blocked statement).
- §8 (fold `Write` import fix into 2c) — done.

## 3. Changed files, by slice

### Slice 2b — commit `e54a700`

| File | Change |
|---|---|
| `crates/fjell-kernel/src/main.rs` | `// SAFETY:` comment moved from above the `ks_init!`/`ks_get!` macro arm pattern to immediately above the `unsafe { ... }` inside the arm body. Comment-only; no token the compiler sees changed. |
| `crates/services/fjell-recoveryd/src/main.rs` | Same move for `send_ready()` and `reply()` — comment relocated from above `fn ... {` to immediately above `unsafe { ... }` inside the body, matching the pre-existing pattern in `recv_call()` in the same file. |

### Slice 2c — commit `c3e24f1`

| File | Change |
|---|---|
| `tools/fjell-abi-snapshot/src/main.rs` | `STABLE_CRATES`: `crates/fjell-audit-format/src` → `crates/formats/fjell-audit-format/src`; `crates/fjell-bundle-format/src` → `crates/formats/fjell-bundle-format/src`. Removed unused `Write` import (line 32). |
| `tests/abi/snapshot.json` | Regenerated (`--generate`): 401 → 404 items. |

## 4. Important implementation decisions

- Verified the review record's §2 provenance claim independently rather
  than taking it as given: checked out `f3519dc` into an isolated
  `git worktree`, applied only the path fix, ran `--verify`, and got the
  same result the architect reported (401→404, Added 3, Removed 0,
  **Changed sig 0**, PASS). Recorded in the Slice 2c commit message.
  Worktree removed after use; no trace left in the tree.
- Did not attempt to fix or explain away Finding C. Characterized it with
  the minimum evidence the review asked for, then stopped — a build-layout
  non-determinism issue is outside anything authorized so far and would be
  a scope decision, not an implementation one.
- Cleaned up gate-run side effects after each verification pass
  (`docs/release/trust-report.txt` reverted, `tests/repro/`, `tests/runs/`
  removed) rather than let them accumulate as noise ahead of the commits
  the review will actually look at.

## 5. Differences from the handoff/RFC

None. Both fixes match the review record's rulings exactly (comment-only
relocation; two-line path fix plus conditional regeneration).

## 6. Executed commands and real output

**Slice 2b:**
```
$ cargo run -p fjell-unsafe-audit        (before fix)  → 270/274, 4 missing
$ cargo run -p fjell-unsafe-audit        (after fix)   → 274/274, 0 missing
$ cargo fmt --all && git diff --stat     (workspace-wide) → no output (no diff)
$ cargo xtask build                      → 0 warnings
```

**Slice 2c:**
```
$ cargo build -p fjell-abi-snapshot      → 0 warnings (Write import gone)
$ cargo fmt --all && git status --short  → only main.rs (fmt's own line-wrap
                                            of the two corrected tuples; a
                                            second run produces no further diff)

$ cargo run -p fjell-abi-snapshot -- --verify     (post-2b, pre-regenerate)
  Baseline items : 401
  Current items  : 404
  Added          : 3 (additive — OK)
  Removed        : 0
  Changed sig    : 163

$ cargo run -p fjell-abi-snapshot -- --generate
  fjell-abi-snapshot: wrote 404 items to tests/abi/snapshot.json

$ cargo run -p fjell-abi-snapshot -- --verify     (post-regenerate)
  Baseline items : 404
  Current items  : 404
  Added          : 0 (additive — OK)
  Removed        : 0
  Changed sig    : 0
  Result         : PASS

# Independent re-derivation of the review record's provenance claim,
# isolated worktree at f3519dc + path fix only:
  Baseline items : 401
  Current items  : 404
  Added          : 3 (additive — OK)
  Removed        : 0
  Changed sig    : 0
  Result         : PASS
```

**Combined gate re-run**, `cargo xtask release-rehearsal` at `c3e24f1`:
```
  [PASS] Gate 1  Host test suite (0 failures)
  [PASS] Gate 2  Unsafe audit (0 missing)
  [PASS] Gate 3  MMIO audit (0 missing)
  [PASS] Gate 4  ABI snapshot verify
  [PASS] Gate 5  Readiness matrix (0 OPEN)
  [PASS] Gate 6  Trust report (6 sections)
  [PASS] Gate 7  ERRATA register (0 OPEN)
  [PASS] Gate 8  Validation drills (markers)
  [ -- ] Gate 9  Release-notes limitations    MANUAL (unchanged, out of scope)
  [FAIL] Gate 10 Verus release-required proofs   verus not on PATH → CONFORMANCE-ONLY
  [PASS] Gate 11 Callsite conformance
RELEASE-REHEARSAL: ONE OR MORE GATES FAILED   (Gate 10 only; Gate 9 manual)
```
Gate 10 reported as failure, not conformance-only-as-pass, per standing
instruction — Verus is not installed in this environment. Capability (6)
and lease (10) conformance tests passed.

## 7. Finding C — characterization

**Reaches executable content. Not confined to a volatile metadata region.
§4.3 remains blocked**, as the review record anticipated (§6).

Method: extracted the pre-outage committed binary and the Slice-1 rebuilt
binary for two of the 11 changed services (`fjell-attestd`, 6465 bytes;
`fjell-init`, 18160 bytes, the largest of the 11) via `git show <rev>:<path>`,
then `cmp -l` for the byte-offset distribution, then decoded the differing
regions as little-endian 16-bit values to check for structure.

**`fjell-attestd`** — 68 differing bytes, spanning offsets 261–5818 of 6465
(the file's data-bearing range, immediately following what is visibly RISC-V
instruction encoding in a hex dump around the first difference — not a
header or trailer). Decoded as 16-bit pairs, the deltas are overwhelmingly
one of two constants:

```
old=0x5b40 new=0x3a60 delta=-8416   old=0x1f40 new=0x3fa0 delta=+8288
old=0x58e0 new=0x3800 delta=-8416   old=0xf3a0 new=0x1400 delta=-57248
old=0x5660 new=0x3580 delta=-8416   old=0xdaa0 new=0xdb20 delta=+128
... (26 of 35 pairs are exactly -8416 or +8288; a handful are +128)
```

A dominant, repeated, non-random delta (`-8416` / `+8288`, related by exactly
128) across two-thirds of the differing sites is the signature of an
address- or offset-encoding immediate that moved by a fixed amount between
the two builds — not the signature of a timestamp, build path string, or
build-ID (those would show as differing ASCII bytes or a single contiguous
blob, not scattered 16-bit arithmetic deltas).

**`fjell-init`** — 702 differing bytes (3.9% of 18160), offsets 1349–14653 —
i.e. spanning nearly the entire file. Deltas here do **not** cluster around
one or two constants; they are widely scattered (the full histogram is in
the raw evidence). This is consistent with the same underlying
phenomenon — reference/address immediates shifting due to a layout or
ordering difference — but affecting a larger, less uniform binary (more
functions, more embedded string data) more pervasively.

**Conclusion:** both sampled binaries show differences embedded in
instruction-adjacent content, distributed across most of the file rather
than confined to a small trailing or leading region. This does not look
like non-determinism in a metadata field; it looks like non-deterministic
layout or symbol/codegen-unit ordering between the two build invocations,
which is a known category of Rust build irreproducibility (not something
this handoff's scope — host tool fixes only — authorizes investigating
further, e.g. `codegen-units`, incremental compilation, or linker input
ordering).

I have not attempted to fix this. Per the review record §6, recording a
repro baseline over this would bake an unexplained delta in as "correct" —
exactly what §4.3's fail-closed change exists to prevent. **§4.3 is
blocked** pending a decision on how (or whether, for v0.21.3) to pursue
build-layout determinism.

Raw evidence (extracted binaries, offset lists, delta histograms) is in my
working scratch space, not committed; it can be re-derived in under a
minute from the two commands above (`git show ac96c1f:<path>` vs
`git show f3519dc:<path>`, `cmp -l`) if the architect wants to re-verify
independently, which is how I checked the prior review's own claims.

## 8. Unresolved issues and blocked items

- **Slice 3 §4.3** (repro baseline recording) is blocked on Finding C.
  Everything else in Slice 3 (§4.1 syscall docs, §4.2 index/link/stamp
  drift) does not depend on it and could proceed.
- Gate 10 (Verus) still unavailable in this environment; unchanged from
  the prior review request.

## 9. Known limitations

None new. Carried forward: Gate 9 manual, Gate 10 environment-dependent,
the ABI gate's formatting-sensitivity design trade-off (recorded in the
Slice 2c commit message, deferred to v0.22 per the prior review §4).

## 10. Requested review focus

1. Confirm Slices 2b/2c are accepted as executed (both gates independently
   re-verifiable from the commits alone).
2. Disposition of Finding C: is a build-layout/determinism investigation
   in scope for v0.21.3, deferred to v0.22 like the ABI gate's formatting
   sensitivity, or something that needs its own RFC given it touches
   codegen/link configuration rather than a single host tool?
3. Whether Slice 3 should proceed now on §4.1/§4.2 alone, with §4.3
   explicitly marked blocked in the same submission, or whether the whole
   of Slice 3 should wait for a Finding C ruling.
