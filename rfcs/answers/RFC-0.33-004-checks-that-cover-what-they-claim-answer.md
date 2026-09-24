# RFC-0.33-004 R1 and §A–§E: four instruments, each seen refusing what it used to pass

**Governing RFC:** [../accepted/RFC-0.33-004-checks-that-cover-what-they-claim.md](../accepted/RFC-0.33-004-checks-that-cover-what-they-claim.md)
**Handoff:** [../handoffs/RFC-0.33-004-checks-that-cover-what-they-claim/implementation-handoff.md](../handoffs/RFC-0.33-004-checks-that-cover-what-they-claim/implementation-handoff.md)

Written after R1 and before the first code change, for all four errata, as the
handoff orders. Every absence is probed with `/usr/bin/grep -a` and a control.
Tip at R1: `0c7fbb8` (RFC-0.33-003's last commit; nothing of this line is in it).

---

## R1 — the four re-derivations, at this tip

### E-049 — the hand list, and the crates it misses

| Probe | Result | Control |
|---|---|---|
| The original probe: `-p fjell-…` **occurrences** in `ci.yml` | **101**, on **88** lines, naming **73** distinct crates | the same pattern finds exactly 1 for a crate known to be listed (`-p fjell-fleet-format`) |
| Workspace crates with a **lib target** that **no `-p` entry names** | **14**: `fjell-bundle-format`, `fjell-cap-manifest`, `fjell-config-sync`, `fjell-consistency-check`, `fjell-dev-harness`, `fjell-dtb-validate`, `fjell-fleet-sync`, `fjell-os`, `fjell-replay-cache`, `fjell-sdk`, `fjell-semantic-toolkit`, `fjell-sig-ed25519`, **and `fjell-canon`, `fjell-schema`** | `cargo metadata` lists 96 workspace packages, 55 with a lib target; the same script reports `fjell-fleet-format` as named |
| Crates named with `--lib` that **have no lib target** | **32** (every service, plus tools) | the script names them from `cargo metadata`, not from a guess |

**The count is neither 85 nor 101** — the handoff forbids quoting either, and both
were measured with a definition. The figure in this line's evidence is the pair
above, at this tip: **101 occurrences / 88 lines / 73 crates, 14 lib crates
unnamed.** Two of the fourteen are **this line's neighbour's own new crates**
(`fjell-canon`, `fjell-schema`, RFC-0.33-003): I added them to `Cargo.toml`'s
members and default-members and to no CI list, so `fjell-canon`'s 7 unit tests run
in CI **nowhere**. That is the finding reproducing itself in the commit before this
one, which is the argument for D2.

`cargo test --workspace --lib --exclude fjell-proptest --features
fjell-sxt-crypto/crypto-profile-development` today: **54 lib crates, 706 tests,
exit 0** (the RFC measured 49 / 580; the workspace grew).

### E-052 — the citations, and how many

| Probe | Result | Control |
|---|---|---|
| Links in `docs/src/**/*.md` whose target, resolved against the page, **leaves `docs/src`** | **34 links in 3 files**: `compliance/standards-mapping.md` **32** (on **23** lines, **15** distinct targets), `releasing/v0-release-cycle.md` **1**, `releasing/v1-limitations.md` **1** | in `standards-mapping.md` the same script finds **34** links that resolve *inside* the book (`](../x…`), so it distinguishes the two |

The register says **eleven** (2026-09-16), the RFC says **23 + 1** (lines), and this
tip has **34 link occurrences** in **three** files. The RFC's "+1" is the release
cycle page; **the third file, `v1-limitations.md`, is new** (a cited evidence log).
All three numbers are counts of the same thing at different dates or by different
definitions; the E-052 closure will say so rather than replace one with another.

### E-056 — the enum blindness, by adding and removing a variant

Scratch worktree at the tip, `fjell-abi-snapshot --verify` (exit status in each):

| Edit to `crates/fjell-abi/src/syscall.rs` | `Added` / `Removed` / `Changed sig` | Result |
|---|---|---|
| none (baseline) | 0 / 0 / 0 | PASS |
| **add** `ProbeAddedVariant = 250` to `SyscallNumber` | **0 / 0 / 0** | **PASS** |
| **remove** `Exit = 1` from `SyscallNumber` | **0 / 0 / 0** | **PASS** |
| *control:* add `pub fn probe_added_fn()` | **1** / 0 / 0 (`+ fjell-abi::syscall fn probe_added_fn`) | FAIL, as designed |

Adding and removing a syscall number registered as nothing; a new function is caught.
(`syscall-surface` is the separate check that covers that one enum.)

### E-057 — the split, in the code that reads the profile

`qemu_run.rs::parse_list` on `["bootctl: health FAILED on the last confirmed slot, not resetting", "b"]`:

```
COMMA   ["\"bootctl: health FAILED on the last confirmed slot", "not resetting\"", "b"]
CONTROL ["plain one", "plain two"]
```

One marker becomes two, **with a stray quote left on each half** (so neither half
would match what the author wrote). The bracket half is in `load_profile`, not
`parse_list`: it accumulates lines "until one *contains* `]`", so a marker line such
as `"[INTENT][Normal] demo",` closes the array early. `semantic.toml` documents it
costing that tier 2 of 4 markers; `health-fail.toml` still carries the comma
workaround. The unit tests written first in this line reproduce both halves against
the unmodified reader before it is changed.

---

## §A — Does the lib run replace the three `-p` jobs, or join them? **Replace.**

**One job, `ci-test-lib`, running `cargo xtask host-lib-tests`** — the shared
definition `cargo_metadata::host_lib_test_argv()`, used by `test-all` tier 1, by
that subcommand, and therefore by CI, exactly as `host_bin_test_argv()` already is
for `ci-host-bins`. It replaces `ci-test-host`, `ci-test-services`' test step (its
`cargo check` step stays: that is a check) and `ci-test-v07-formats`. `ci-proptest`
keeps `cargo test -p fjell-proptest` (the one package the derived runs exclude *by
name*, because it runs in its own job) and drops the two `--lib` lines that the new
job now covers.

**The feature is passed explicitly**, as `host_bin_test_argv` does:
`--features fjell-sxt-crypto/crypto-profile-development`. Not optional, for the
reason in the RFC's Risks.

**What feature unification hides — measured, not asserted.** I ran every one of the
**54** lib crates *alone* (`cargo test -p <crate> --lib`, no feature, exit status
each): **53 pass; one fails — `fjell-sxt-crypto`**, on its own `compile_error!`
guard. It is also the **only** `compile_error!` in the workspace (`grep -a -rn`,
control: it finds that one). So the answer to "is any crate's own guard now
untested": **no other guard exists, and the one that does is kept exercised by
passing its feature explicitly**; the *failure* path of that guard (the crate alone,
feature absent) is compile-time and is exercised by nothing — I record that as a
survivor rather than write a test that builds the crate wrongly on purpose. And no
other crate passes only because of what a neighbour enables: 53 of 54 pass alone.

**What is lost by replacing:** per-job parallelism and the per-job failure label.
**Wall-clock, measured cold** (fresh target dir, this machine, `Finished … in`):
the derived run **4.8 s**; the three jobs it replaces **2.7 s + 0.6 s + 0.4 s**
(run in parallel in CI, so ≈ 2.7 s of wall-clock). Each CI job also pays its own
checkout and toolchain install, which dwarfs both figures, so one job is *cheaper*
in runner-minutes and slower in critical-path time by seconds. I state both and
choose the single job because the goal is no crate untested, not fewer jobs — and
the v0.7-formats job's isolation was only ever that flag, which is kept. **A caveat
on the figures:** they are this machine's, not a CI runner's.

## §B — What does D2 forbid? **A `-p` on a `cargo test` command in a CI job.**

Test jobs only; `ci-check`, `ci-cross-check`, the fuzz, miri and verus jobs
legitimately name packages (a subset is the point of a cross-check or a miri run).
The subcheck is a new consistency-check subcheck, **`ci-test-jobs`**, and its
message states the rule itself: *"a CI job that runs `cargo test` must not name
packages with `-p`: a hand-written list is what left ten crates untested (E-049).
Use `cargo xtask host-lib-tests` / `host-bin-tests`, which derive the set from the
workspace."* One allowance, recorded in the subcheck and its message:
`-p fjell-proptest`, which the derived runs exclude by name. Comment lines are not
commands (a comment quoting `cargo test -p` must not trip it — that is a test).
Demonstrated by reintroducing a `-p` line and running it.

## §C — What does the ABI re-record's revealed drift get? **Every change named; a removed variant escalated.**

The handoff's rule, applied as written: `--verify`'s output **before** regenerating
is the evidence (the new hash makes every enum's entry differ, by construction, so
*before* is "N enums changed hash"); the **content** of what changed is named by
running the *new* scanner over the source at the commit that last recorded the
baseline and over the tip, and diffing the variant lists item by item. Additive
variants are expected; **a variant removed** since the baseline is a finding and is
escalated, not absorbed. The commit message groups: variants added, variants removed
(with the commit that removed each), signature changes. I do **not** extend the same
treatment to `struct` bodies and `trait` items in this line (see *Beyond the RFC*).

## §D — Does D4 belong to those subchecks' owners? **Yes — changed here, tests stated.**

`standards-mapping` and `evidence` both resolve a citation as a filesystem path.
Both change, through **one** small shared function
(`resolve_repository_url(link, repository_url) -> Option<repo-relative path>`), so
"what the repository URL is" has one definition, read from `docs/book.toml`'s
`git-repository-url` (the value the book already publishes). A link starting
`{repository_url}/blob/main/` or `/tree/main/` is turned into its repo-relative path
and then checked **exactly as a relative citation is** (exists on disk; for
`evidence`, has a provenance sidecar; and the reverse direction counts it as
cited). What their tests now assert:

- a converted citation is **accepted** (both subchecks);
- a URL whose path **resolves nowhere is still refused**, naming the path;
- an absolute URL **not** under the repository is not silently accepted;
- a relative citation still works (nothing is removed);
- `evidence`'s orphan check counts a log cited *only* by URL as cited.

The 34 links are converted to
`https://github.com/nabbisen/fjell-os/blob/main/<path>`, the spelling 14 other
pages already use. **The site is the test:** after publishing, the converted
citations are followed from the served page (RFC-0.32-003 R8's shape); I cannot
publish, so the site check is stated as **owed at the push**, with the command, and
not claimed.

## §E — Order: **E-057 → E-056 → E-052 → E-049**, as the RFC leans and the handoff requires.

Each is a separate commit, with its own demonstration (D7), so a red gate is always
attributable. E-049 last because it touches every test job.

---

## Beyond the RFC — findings I will file rather than absorb

- **Struct and trait bodies are as blind as enum bodies.** The scanner hashes the
  declaration line of a braced `struct` and a `trait`; adding a field to
  `AuditRecordBin`, or a method to a trait, registers as nothing. This is E-056's
  class, and the RFC (D5) covers enums. Extending the hash costs one condition, but
  re-recording every struct and trait with named drift is a review the RFC did not
  size, and the demonstration for it needs its own baseline reading. I will
  measure it (add and remove a field, control) and file it as its own erratum, not
  fix it silently inside D5.
- `fjell-sxt-crypto`'s guard failure path is exercised by nothing (§A).

## Decisions the RFC did not specify

1. **A new subcheck `ci-test-jobs`** rather than extending `toolchain-declarations`
   (the register's wording says "inversion of"): a different property, and stuffing
   it into a subcheck about toolchain versions would make both harder to read.
2. **`host-lib-tests` as a subcommand**, mirroring `host-bin-tests`, so the shared
   definition is a function both consumers call and a test asserts.
3. **`-p fjell-proptest` is the one named allowance** in D2.
