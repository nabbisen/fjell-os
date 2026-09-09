# RFC-0.29-001 §6 and R1–R4 — 305 tests, five lists, and what checking each one found

**Governing RFC:** [rfcs/done/RFC-0.29-001-tests-that-run-and-lists-that-derive.md](../../rfcs/done/RFC-0.29-001-tests-that-run-and-lists-that-derive.md)

Order followed per the handoff: **R1 → R2 → §6 → R3 → R4 → close.**

---

## R1 — the flag choice, and the numbers checked, not assumed

**D3 answered directly.** `--bins` reaches unit tests in crates with no
`[lib]` target; `--tests` additionally reaches integration `tests/`
directories (excluding `fjell-proptest`'s, which tier 2 already runs with
`--release`); `--all-targets` reaches both plus benches and examples.

**Neither `--bins` nor `--tests` can be added `--workspace`-wide on their
own — this was the RFC's own framing, and it does not survive contact with
this codebase.** `crates/fjell-kernel`, every crate under
`crates/services/`, and every crate under `crates/drivers/` are
`#![no_std]`/`#![no_main]` binaries with their own `panic_impl`. The
instant `--bins` or `--tests` is added to a `--workspace` invocation,
cargo tries to build every one of them for the host target. This is not a
test failure — it is a hard compile error
(`duplicate lang item 'panic_impl'`, unresolved `alloc::Box`), demonstrated
live against `fjell-kernel` alone and confirmed to hold for every one of
the 31 bare-metal crates. `--tests` has the identical failure mode:
checked directly, not inferred from `--bins`'s behaviour, since `--tests`
also builds a crate's bin targets under the test harness.

**The fix:** `crates/fjell-tools/src/cargo_metadata.rs` (new) derives the
bare-metal exclude set from `cargo metadata`'s `manifest_path`, not a name
list — a new service or driver crate is excluded automatically by where
it lives. `host_bin_test_argv()` builds the one shared invocation:

```
cargo test --workspace --bins --tests --exclude fjell-proptest \
  --features fjell-sxt-crypto/crypto-profile-development \
  --exclude <31 bare-metal crate names, derived>
```

**A necessary addition outside the RFC's literal wording, found building
this, not before:** excluding the bare-metal crates removes
`fjell-secure-transportd` from the build graph — which turns out to be the
dependency edge that was silently enabling `fjell-sxt-crypto`'s
`crypto-profile-development` feature for the whole workspace via Cargo's
feature unification. Without it, `fjell-sxt-crypto`'s own `compile_error!`
guard (RFC-v0.7.3-002: *"not suitable for production cryptographic use,
enable explicitly"*) fires for real, and the tier cannot compile at all.
The explicit `--features` flag above restores the previously-accidental
state deliberately. **Not this line's non-goal to fix elsewhere:**
`ci-test-v07-formats` runs `cargo test -p fjell-sxt-crypto --lib` today
with no `--features` flag and is exposed to the identical masking —
recorded here, not corrected there.

**A finding: the RFC's own cited counts were already stale.**

| | RFC's figure | Re-derived (2026-09-08/09) |
|---|---|---|
| Crates with no `[lib]` target | 40 | **41** |
| Unit tests in those crates | 285 | **288** (258 host + 30 `fjell-kernel`) |
| `fjell-tools`'s own share | 86 | **89** — 3 tests this line itself added to `qemu_run.rs`/`cargo_metadata.rs` while building R1/R3 |
| Integration `tests/`-dir tests (excl. proptest) | 20 | **27** — `fjell-cap` (16), `fjell-upgrade-format` (7), `fjell-config-sync` (2), `fjell-fleet-sync` (2) |
| Total unreachable by tier 1 | 305 | **315** before this fix (288 + 27); **285 now reachable** (258 host + 27 integration); **30 (`fjell-kernel`) remain unreachable, unchanged, non-goal** |

`fjell-kernel`'s 30 were checked directly, not assumed reachable: even
alone, `cargo test -p fjell-kernel --bins` fails to compile on host for
the reasons above. **The RFC's own Non-goals bullet — "its 30 tests…
will start running" — does not happen and cannot, under any flag choice,
without making the kernel host-testable, which is this line's own
explicit non-goal.** Reported rather than quietly reconciled.

**Result, demonstrated live (`--no-qemu`, so this tier alone is
visible):**

```
2  Host bin/integration tests    1.4s  PASS
```

800 tests pass across every host-buildable crate once the tier reaches
them all (not just the 9 gate-tool crates — every crate `--bins`/`--tests`
can now safely reach). Zero failures on first run: an honest surprise, not
a claim — see R2 for why that alone is not evidence the tier works.

## R2 — the tier demonstrated failing, on the case the erratum is about

Per the handoff: break one of the 36 tests behind `SYSCALL-CALLSITE-002`,
not an unrelated fixture. `crates/fjell-syscall/src/lib.rs`'s
`sys_ipc_call_words` had its `lateout("a5") _` register declaration
removed — the exact register this project's own history already
regressed once (RFC-0.28-004's Finding 1).

```
[test-all] Host bin/integration tests............................. FAIL (1.4s)
  callsite_audit::tests::shipped_tree_has_no_fjell_syscall_internal_violations ... FAILED
  test result: FAILED. 93 passed; 1 failed
```

Reverted immediately (`git diff` on the file confirmed empty before
continuing); re-run shows `PASS` again. The tier is load-bearing, not
`ci-proptest`'s shape repeated.

---

## §6 — the authority for "the negative-test categories"

**Answer: shape 3 — the profiles on disk, with the opt-out recorded in
the profile.** The handoff's lean, argued rather than assumed.

Shape 1 (tracked-files-only in spirit — "just the profiles, nothing
else") was rejected for the reason D5 names directly: `store` and
`upgrade` have marker specifications but no emitting scenario yet
(`v1-limitations.md`), and forcing them red by construction conflates "a
disclosed, known gap" with "a regression" — exactly the ambiguity a
derived answer is supposed to remove, not reintroduce.

Shape 2 (one constant, everything else derives from it) was rejected for
the same reason RFC-0.28-005 rejected a single shared exclusion list: it
stops the *five* lists from disagreeing with each other, but the one
remaining list is still a name a person must remember to add — the
`KNOWN_V01X_CATEGORIES`/`KNOWN_V02_CATEGORIES` pair this line removes was
already shape 2, and it went stale (13 entries, one an unused alias,
missing four categories `NEG_CATEGORIES` had).

**Shape 3, built:** `qemu_run::discover_negative_categories()` lists
`tests/qemu/profiles/*.toml`, derives a category from every file, and
excludes exactly the ones whose own `multi_node = true` says they need a
different runner (`fleet-demo` — three simultaneous QEMU instances,
RFC-v0.10-005, delegated to `cargo xtask fleet-demo`, never
`qemu_run::run_profile`). No category name is hand-listed anywhere; a new
profile file is picked up automatically, and `fleet-demo`'s exclusion is
structural, not enumerated.

**The opt-out, recorded in the file itself:** `release_gated = false` plus
a required `not_gated_reason` string — the loader refuses to load a
profile that sets the former without the latter (demonstrated: a
synthetic profile with `release_gated = false` and no reason fails to
load, naming the missing field). `store.toml` and `upgrade.toml` both now
carry this, quoting `v1-limitations.md`'s existing disclosure rather than
inventing new wording.

**A profile that opts out is still derived into scope and still runs**
(D5) — `test_all.rs`'s tier 5 calls `run_tier_not_gated` for these two,
which executes the real command and reports the real PASS/FAIL, but
excludes the result from the tier's own pass/fail tally. Confirmed live:
`store`/`upgrade` both report `FAIL (not release-gated: …)` in the summary
(they have no emitting scenario, exactly as documented) while
`✓ ALL REQUIRED TIERS PASSED` still holds for the run as a whole.

---

## R3 — one derived answer, in three places that used to disagree

1. **`test_all.rs`'s tier 5** calls `qemu_run::discover_negative_categories()`
   directly — no `NEG_CATEGORIES` constant remains.
2. **`negative.rs`**'s `KNOWN_V01X_CATEGORIES`/`KNOWN_V02_CATEGORIES` are
   removed entirely; both the usage message and the "unknown category"
   error now print the same derived list.
3. **`ci.yml`**'s `ci-qemu-negative` matrix cannot itself run a program to
   compute its own entries, so a new job (`ci-negative-matrix`) runs
   `cargo xtask list-negative-categories` (new subcommand — prints
   `[{"category":"...","release_gated":bool}, ...]`) and hands the result
   to `ci-qemu-negative` via `fromJson`. `release_gated: false` categories
   get `continue-on-error: true` at the step level — they still run, and
   their artefacts still upload, but a known gap does not turn CI red.

**D4 satisfied structurally, not by editing the sentence:** `test_all.rs`'s
module doc-comment no longer states a category count at all — it
describes what tier 5 iterates over and points at
`discover_negative_categories`. There is nothing left in that comment for
a future category to go stale against.

**The `KNOWN_*` lists' removal closes the fourth of the RFC's five
disagreeing lists.** The fifth — `test_all.rs`'s own literal
`NEG_CATEGORIES` array — is the third; both are now gone, replaced by the
one call site each of the three consuming places makes.

**Categories, re-derived rather than counted from the RFC's own table:**
`tests/qemu/profiles/` holds 15 files; `fleet-demo` is excluded
structurally, leaving **14** real negative-test categories (12 currently
release-gated, 2 disclosed-and-not: `store`, `upgrade`) — not the RFC's
cited 15 (which counted `fleet-demo` as a category) or 12 (`NEG_CATEGORIES`'s
own count, missing `store`/`upgrade` entirely as candidates at all before
this line).

---

## R4 — CI crate coverage, derived where it could be, disclosed where it couldn't

**Re-derived, and different from the RFC's own count:** `cargo metadata`
lists **91** workspace crates (not 93); **21** are never named literally
in `ci.yml` (not 23) — the same number the 2026-08-27 measurement found,
before the RFC's own "23 of 93" re-derivation. Reported as measured, not
reconciled against either prior figure.

**The `ci-host-bins` job (R1's own fix) closes most of this by
construction, not by naming crates.** `cargo xtask host-bin-tests` runs
every host-buildable crate's `--bins`/`--tests` uniformly; of the 21
crates never named in `ci.yml`'s *text*, this closes real test-coverage
gaps for:

- The 6 gate-tool crates E-013 already named (`fjell-abi-snapshot`,
  `fjell-consistency-check`, `fjell-mmio-audit`, `fjell-readiness-check`,
  `fjell-repro-check`, `fjell-summary-check`).
- **`fjell-sig-ed25519`, `fjell-fleet-sync`, `fjell-config-sync`** — the
  three crates backing Gate 8's validation drills. Their entry in E-015
  says their tests "exist and are reachable, and CI simply never invokes
  them" — an open half of E-015, not a designed exclusion (the drills'
  *markers* are deliberately rehearsal-only; their `#[test]` suites are
  not, and were simply never enumerated). `ci-host-bins` now runs their
  32 unit tests plus 4 integration tests in ordinary CI. **Gate 8's
  marker timing is unaffected** — that mechanism is untouched; this only
  adds continuous unit-test coverage for the crates behind it.

**A methodology note, worth recording so a future audit does not
miscount:** after this line, "named literally in `ci.yml`'s text" is no
longer a valid proxy for "does this crate's test suite run in CI" — the
gate-tool and drill crates above are exercised through
`cargo xtask host-bin-tests`, a single derived command, and never appear
by name in the YAML at all. A grep-based recount (as this RFC's own
figures were derived) will undercount them as still-absent when they are
not.

**What remains genuinely uncovered, disclosed rather than swept in:**

- `fjell-driver-uart`, `fjell-svc-fault`, `fjell-svc-timeout` are still
  absent from `ci-cross-check`'s cross-compilation list — but all three
  have **zero** `#[test]` functions today, so this is a `cargo check`
  coverage gap, not a test-coverage one, and outside E-013/E-015's own
  test-execution framing. Not fixed here (would be a service/driver-facing
  CI change beyond this line's scope); named rather than left silent.
- `fjell-benchmarks` (bench-only, zero `#[test]`s) and `fjell-os` (zero
  `#[test]`s) remain unnamed with nothing lost either way.

## E-015's fourth bullet: not fixed, said so

The pre-existing entry's fourth bullet — `smoke.rs`'s `v0.6-verification`
milestone, accepted by `cmd_qemu_test` but present in neither
`test_all.rs`'s `SMOKE_PROFILES` nor `ci.yml`'s `ci-qemu-v07` matrix — is
**the same shape of defect this line fixes for the negative categories,
in `smoke.rs`, which this RFC does not touch.** Confirmed still true
(`SMOKE_PROFILES` and `ci-qemu-v07`'s matrix both still omit it). Left
alone deliberately: fixing it would mean editing `smoke.rs`, which is
outside `Touches`, and folding it in here is exactly the kind of
scope-widening the handoff's own §0 warns against. **E-015 does not close
to zero remaining instances by this line** — see disposition below.

---

## Dispositions

**E-013 — CLOSED.** Its own text already drew the boundary: *"this is not
'kernel unit tests do not run' but 'the verification tooling's own tests
do not run.'"* That claim is now false — they do. `fjell-kernel`'s
30 tests remain unreachable, exactly as the RFC's own Non-goals section
states will remain true, and are not part of what this erratum's core
claim was about; tracked separately, not swept into this closure.

**E-015 — remains ACCEPTED, not CLOSED.** Three of its four historical
bullets are fixed (the five-lists disagreement, the `KNOWN_*` staleness,
the CI negative-matrix gap, and most of the crate-coverage gap). The
fourth — `smoke.rs`'s vestigial `v0.6-verification` milestone — survives,
named above, left open per R5's explicit instruction rather than quietly
closed around it.
