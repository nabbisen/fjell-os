# v1.0 Limitations — Gate 9 Reference

*The single authoritative list for release-rehearsal Gate 9 ("confirm the
v1.0 limitations section"). Each item links to its governing record. Changes
require updating the governing record first, then this page.*

| # | Limitation | Governing record |
|---|------------|------------------|
| 1 | **Hardware** — no validated real-hardware deployment; the VisionFive 2 profile is provisional and was never booted on silicon | Errata **E-004** (ACCEPTED); `docs/src/deployment/starfive-visionfive2.md` TODOs |
| 2 | **Multi-hart** — the kernel runs single-hart; SMP scheduling, per-hart locking (e.g. the console spinlock), and IPIs are deferred to the multi-hart milestone | v1.0 design decision; `crates/fjell-kernel/src/console.rs` invariant note |
| 3 | **POSIX** — no POSIX compatibility surface (descriptors, fork, signals, ttys) | Non-goal **N1** |
| 4 | **Kernel-IPC for the SDK reference service** — the SDK reference service does not operate over live kernel-mediated IPC | Non-goal **N21** |
| 5 | **ZeroizeOnDrop** — no independently verified byte-level key-erasure guarantee | Non-goal **N23** |
| 6 | **Trust-anchor provisioning** — TOFU with `--allow-tofu-provision` flag (dev/QEMU), factory station (v1.1), hardware-anchored (v2+). Flag implemented (`cargo xtask provision-dev --allow-tofu-provision`) in v0.20.0. | **RFC-v0.17-001** (Accepted, 2026-06-04) |
| 7 | **`cap_install` rights validation does not execute** — `sys_cap_install`'s and `sys_cap_install_with_rights`'s doc-comments claim the kernel validates `rights ⊆ installer authority`; no such check runs, because the `CapInstall` syscall has no dispatch arm at all. The path fails closed (`UnknownSyscall`) rather than granting excess rights — not a live security hole — but the documented behaviour is not shipped. Disposition of `CapInstall` and the other **5** declared-but-undispatched syscalls (`PlatformReboot`, `TaskKill`, `MmioUnmap`, `DmaShare`, `Reboot`) was deferred to v0.22 and **did not happen**; it remains open. *Corrected at the 0.27.0 cut: this read "the other 8 … deferred to v0.22", a count that had moved (35 declared / 29 dispatched / 6 undispatched) and a deferral to a milestone that shipped without it. The current figure is printed by `syscall-surface` at every release rehearsal.* | Errata **E-011** (ACCEPTED); **RFC-v0.21.3-001** §M2 |
| 8 | **Accessibility and inclusion** — the goal is stated, the delivery is partial: two presentations exist, both text on a serial console and both output-only — `proxy-text`, and `proxy-braille`, which writes braille cells (the stream a braille display driver would consume) that no braille reader has read and no device has displayed; no speech or simplified presentation; no way for a person to answer the node through any presentation; nothing tested against a standard or with a person. A missing or crashed presentation no longer stalls a publisher (E-058, **closed**), with survivors named. The one action a person could take is now authorised on the kernel-attested sender's own policy row rather than a word it sent (E-059, **closed**), though it still executes nothing. The presentation boundary is now in the threat model (E-060, **closed**; T21–T23) — including a residual with **no defence**: nothing lets the attending operator cross-check what a presentation shows against the node's own record. See [the section below](#accessibility-and-inclusion--what-does-not-exist-yet) | Errata **E-054**, **E-058** (closed), **E-059** (closed), **E-060** (closed); **RFC-0.33-002** D6; **RFC-0.34-001**; **RFC-0.34-004**; **RFC-0.34-005**; ADR-v0.5-005 |
| 9 | **A lease binding could be replaced** — `sys_cap_bind_lease` overwrote an existing `LeaseBinding`, so a `LeaseAdmin` holder could move a capability off the lease governing it and escape a revocation. `RFC-v0.7.4-003` had removed parent-based cascading revocation on the promise that lease revocation invalidates every copy sharing a binding, which that overwrite undercut. Ruled 2026-10-22: a binding is set once. **Closed by RFC-0.38-004** (2026-10-22): a second bind is refused with `BadState`, to the same lease or another, and revoking the first lease still kills the capability; a no-op for every current caller. What remains is row 10 (E-091), not this | Errata **E-090**; **RFC-v0.7.4-003**; **RFC-0.38-004** |
| 10 | **Nothing limits which lease a holder may bind to** — the `LEASE_BIND` right and lease-scope check `RFC-v0.7.4-003` specified were never built, so the right that gates *creating* a lease also gates *binding* one, and a holder of `LeaseAdmin` + `LEASE_CREATE` may bind an unbound capability to any `Active` lease, including one it does not own. Three tasks hold such a capability. Not an escalation path for an unprivileged task — that was E-088, closed by RFC-0.38-002 — and not an escape from a binding, which is E-090 | Errata **E-091** (ACCEPTED); `RFC-v0.7.4-003` |
| 11 | **Lease revocation is audited under a code nothing decodes** — the kernel appends `AuditKindInternal::LeaseRevoked = 40` on every lease revocation (`crates/fjell-kernel/src/lease/mod.rs:343`), while the published decoder has `LeaseRevoked = 16` and falls back `_ => Internal`, with no arm for 40. So every revocation record is read as `Internal` and the ABI's own `LeaseRevoked` variant is unreachable. No authority consequence; an observability one, on the event class the lease mechanism exists to record | Errata **E-092** (ACCEPTED) |
| 12 | **Cascading revocation covers only explicitly bound capabilities** — `RFC-v0.7.4-003` removed kernel-local recursive revocation (`parent = None`) on the stated premise that "every meaningful cap MUST have a lease", which was never enforced: bootstrap capabilities carry `lease: None` and a copy inherits that. So a copied capability that was never lease-bound has no recursive revocation at all — neither parent links, which were removed, nor a lease. In the shipped tree only the test services bind leases | Errata **E-093** (ACCEPTED); `RFC-v0.7.4-003` |
| 13 | **The signed attestation record is constants** — `attestd` always signs a record built by `AttestationRecordV2::dev` from literals (`kernel_digest` `0x11`, release/rootfs/policy `0x22`/`0x33`/`0x44`, `chain_digest` `0x55`, `head_seq: 1`), and it asserts `release_verified`, `rootfs_verified` and `policy_verified` **true** while nothing verifies. The sequence number passed to `sign_v2` is never read, and `attestd` has no endpoint to `measuredd`, so the measurement chain does not reach the record — repairing the chain (RFC-0.38-001) does not make attestation real. Development profile only; the signer is the keyed-SHA-256 dev provider (row for E-028) | Errata **E-094** (ACCEPTED) |
| 14 | **What the measurement chain measures, and what it does not** — one of its four recorded events is a real measurement: the digest of the **33 service images** the kernel embeds (RFC-0.38-001). The other three (release, rootfs and policy "verified") still record the literal `0xAB`, because nothing in the tree verifies a release, a rootfs or a policy bundle. **The kernel's own image, configuration and runtime state are not measured.** `measuredd` takes the digest on `init`'s word (the kernel's, via `BootInfo`); the relay is not attested. `measurement_head_available` is `false` in every bundle. **None of this is attestation** — see row 13: the signed record is independent of the chain. Boots before RFC-0.38-001 recorded placeholders for all four events | Errata **E-095** (ACCEPTED — the three placeholders); **E-086** (CLOSED, remaining survivors named in its closure), **E-085** (CLOSED); **E-094** (row 13); `tests/qemu/profiles/measurement.toml` |
| 15 | **The chain's provenance is declared, not attested** — `MeasurementSource` means "which service produced the event", and `measuredd`'s `APPEND_EVENT` takes it from the caller's word while discarding the kernel-attested sender it already receives. Three events per boot declare `Verifyd` although `init` sends them and never consults `verifyd`. No consumer reads the chain head yet (slot 4 uninstalled, `measurement_head_available` false), which is the only reason this is not live | Errata **E-096** (ACCEPTED) |

## Accessibility and inclusion — what does not exist yet

*Written first, before the pages that state the goal (RFC-0.33-002 D6): it is the
condition on them. Everything below was checked against the tree at the commit
that added it, and is kept true at each cut (the third of the readiness matrix's
inclusion rows). It describes what a person who needs speech, braille or a
simplified presentation cannot do with Fjell **today**; it is not a promise of
when that changes.*

Fjell's design rests on presentation being a proxy's job, not the OS core's
(requirements §2.6, §4.5). That is an **architectural** property, and it is the
only thing this project claims here. Nothing on this page has been delivered to a
person, and the list is the reason no page says otherwise.

1. **Two presentations exist, both text, both on a serial console.**
   `fjell-proxy-text` renders the intent stream as text; `fjell-proxy-braille`
   renders the same envelopes as lines of uncontracted braille cells — the
   stream a braille display driver would consume — and is asserted **by content**
   in the `semantic-braille` tier, from a committed vector. There is no speech
   presentation and no simplified presentation. The statement that a screen and
   an assistive device would run the same core, differing only in the proxy, has
   now been exercised with **two** proxies, one decoder (`wire::decode_exact`
   behind the same reassembler; no decoding logic is copied) and one envelope
   rendered twice in one run. That is a demonstrated boundary between two
   presentations *of text on a console*. It is not a demonstration with a person,
   a device or a screen reader, and **the braille is not a braille standard**: it
   is a small, documented, uncontracted rule set (`crates/fjell-braille`) that
   **no braille reader has read**. What it leaves out is item 11.
2. **Nothing has been driven on assistive hardware, and the validated platform
   cannot.** QEMU `virt` has no audio device and no braille display, and no
   hardware profile has ever booted on silicon (**E-004**). A future speech or
   braille proxy can, on the validated platform, only *emit the stream a
   synthesiser or a display driver would consume* — not be heard or felt. The
   braille presentation of item 1 does exactly that, to the console; no display
   was driven.
3. **There is no way in.** A person cannot answer, choose, confirm or refuse
   through any presentation. `proxy-text` never reads input, **by decision**
   (ADR-v0.5-005: an input path through the proxy would bypass capability
   policy). The ADR names the alternative — operator input through `fjell-tools`
   over a separate capability-gated path — and **that path is not built**: no
   `fjell-tools` command delivers anything to a running node, and the one input
   that does reach a node (UART receive) arrives at `init` and is used for a test
   hook. An *input request* can be represented in the intent stream
   (FR-SEM-001); nothing can answer one. A return leg from the proxy to the
   stream exists, but it is driven by the proxy's own code under a demonstration
   capability, executes nothing, and carries its rights as a payload word the
   stream cannot verify. How input should reach the system is an open design
   decision — an ADR is a readiness-matrix row, not yet written.
4. **A missing or crashed presentation no longer stalls the node's
   publishers — with survivors** (Errata **E-058**, **CLOSED** by RFC-0.34-001).
   It used to: `semantic-stream` forwarded each envelope to the proxy with a
   blocking call before replying to the publisher, and with `proxy-text` not
   started the `semantic` profile fell from 314 lines to 124 and `init` never
   reached its last phase (measured; `rfcs/answers/RFC-0.34-001-…-answer.md`).
   The kernel has no non-blocking send, a server holds one reply edge, and a dead
   receiver never releases a blocked sender, so the fix is not a reordering: the
   stream **never initiates a blocking call to a presentation**. Each presentation
   *asks* the stream and is answered by reply, and is woken through an endpoint
   of its own. Both presentations ask; `proxy-text` was converted to (RFC-0.34-001
   D9) after a relay task that asked on its behalf was built and then judged a
   permanent shim for ten lines of code, and removed.
   The `semantic-absent` tier starts the node without `proxy-text` and `init`
   reaches its last phase (270 lines in the tier, with the braille presentation
   still running; the same absence stopped the node at 124 lines before), and the
   stream says on its own output that the presentation has not asked for
   anything. The other case E-058 measured — a presentation **faulting mid-run**
   — is a committed tier too, `semantic-crash`: a test-only presentation registers
   with the stream, is woken, takes its first message and faults (the kernel
   reports the fault in its own task); `init` then publishes twelve real
   envelopes and **all twelve are answered** (the old behaviour was `init`
   stopping at its next publish); the stream reports the presentation as no
   longer asking; and the text and braille presentations keep rendering what was
   published after it died. **What survives**, named:
   (a) a presentation that is *alive but never asks again* is not detected as
   dead — there is no wall-clock timer, only the stream's own activity to count.
   While it has work queued it is reported (`has stopped asking`) after 64 of the
   stream's calls and at each doubling; a silent presentation with **nothing**
   queued is idle by design and never reported;
   (b) a presentation that faults in the two instructions between being told
   "parked" and reaching `recv` would still block the stream on its next wake —
   the one place the stream can wait on one, and it is code this project wrote;
   (c) envelopes a presentation cannot take are queued to **8 KiB and then
   dropped**, newest first, so a presentation that starts late or falls further
   behind shows a **gap**, and the gap is reported on the node's own console line,
   **not through the presentation** — a person reading only the braille is not
   told that something was skipped;
   (d) nothing restarts a stuck presentation;
   (e) a rendered line over the kernel's **160-byte console-line bound** is
   split — visibly, since Errata **E-062**'s close (`[cont]` marks each half)
   rather than silently — but the bound itself is unchanged, and nothing stops
   a longer title or description from being the first committed line to cross
   it (the longest measured so far is 153 bytes of text, 135 of braille). All
   five are now also in the threat model, named rather than implied: **T22**.
5. **Little of the node's state reaches a presentation.** Only `init` and one
   SDK sample service publish to the stream. Running services do not report
   their live state through it, so what a presentation shows today is largely
   boot-time narration, not the state of a running node.
6. **There are no applications.** No application model exists; the only
   application-shaped service is an SDK reference sample. That is the founding
   requirements' own position at v1 (§4.1: not a desktop, not an application
   ecosystem), not an oversight.
7. **Nothing adapts to a person.** No Personal Proxy exists (FR-SEM-003 is a
   *Could* in the requirements). The intent stream is presentation-agnostic; no
   measurement or per-person optimisation is built on it.
8. **No standard is claimed, and none is tested.** Fjell makes no claim of EN 301
   549, Section 508, WCAG or any other accessibility standard, has not been
   evaluated against one, and has not been tested with people or with assistive
   technology. The operator tooling is a command-line program that has not been
   evaluated with assistive technology either; nothing here says it is, or is
   not, usable with it.
9. **The presentation boundary is now in the threat model — was absent until
   RFC-0.34-005, which closed E-060.** Three entries, **T21–T23**: the
   authority-claim instance of T17's adjacent class (closed by RFC-0.34-004);
   an absent or crashed presentation stalling the node's narration (closed by
   RFC-0.34-001, with five named survivors — item 4 above); and a compromised
   presentation showing false state to the attending operator, for which
   **no defence exists** — named as a residual, not asserted away. The model
   is explicit that it covers only a presentation on the **same node**: a
   presentation on a separate or personal device is named and excluded, and
   still needs its own amendment before it is described as safe — the
   condition E-060 closed on, carried forward rather than met.
10. **The one action a person could take is authorised on a payload word the
   authoriser cannot check — was true until RFC-0.34-004, which closed
   E-059.** The presentation's return leg sent `DISPATCH_ACTION` carrying the
   rights it holds as an ordinary payload word, and `semantic-stream` compared
   the action's required rights against that word; nothing on the receiving
   side could confirm the word came from the proxy's own `sys_cap_inspect`
   read rather than anywhere else. `w2` is now never read: the stream
   authorises from a policy row keyed by the kernel-attested sender identity
   (`CapRights::PROXY_TEXT_DEMO`, the same constant `spawn.rs` installs), not
   from anything a presentation sends. **This does not make the action real.**
   Today a permitted action still **executes nothing** (the dispatch returns
   `Ok` or `Denied` as a message) — the fix changes what the authority decision
   is based on, not what it does, and it is still what an input path (item 3)
   would be built on.

11. **The braille presentation shows the producer's English sentence, and leaves
   out what a person deciding whether to act would need.** A `TextToken` carries
   an id and a fallback string; no catalogue exists, so the renderer **ignores the
   id** and uses the fallback. The architecture's claim that *meaning, not text,
   crosses the boundary* therefore holds for the structure around the text — kind,
   severity, the set of actions, the capability an action needs — and **not for
   the text itself**: a person who needs simplified language gets the same
   sentence, in cells. The rendering also omits each action's required
   capability, reversibility and confirmation policy, an intent's expiry, a
   state's facts beyond their count, and an event's subject and audit sequence.
   Nothing can be acted on through any presentation (item 3), which is why the
   omission is not a live risk today and is exactly what an input path would make
   one.

**What this section does not say.** It does not say Fjell is "accessible" or
"inclusive" as a verdict about the product — there is no such verdict to give.
It does not date any of the above: a second presentation modality **ships in
0.33.0** (the 0.34 line, cut inside that release) and has been driven on no device,
the adaptive Personal Proxy is unscheduled, and the readiness matrix carries two
of the three inclusion rows as work in progress.

Additional operational notes (not Gate 9 items, listed for completeness):

- **`test-all` tier 1 used to never run the tests of any package without a
  library target — including the verification tooling's own** (Errata
  **E-013**, **CLOSED** by RFC-0.29-001). Tier 1 ("Host library tests") ran
  `cargo test --workspace --lib`, which silently skipped any package with no
  library target. Measured at the time: **40 of 89 manifests had no lib
  target, and 10 of those carried 166 `#[test]` functions `--lib` never
  reached** — the count grew steadily every release after (RFC-0.29-001's
  own re-derivation found 41/288 before its fix, and the two prior figures
  it corrected — 305 and 285/20 — had themselves already gone stale by the
  time they were used, since every instrument this project adds lands
  inside this count).

  **Fixed by a new tier**, not by widening `--lib`: `cargo test --workspace
  --bins --tests` reaches every crate above except `fjell-kernel`, but
  cannot be added blindly — `fjell-kernel` and every crate under
  `crates/services/`/`crates/drivers/` are `#![no_std]` binaries with their
  own `panic_impl`, and either flag tries to build all of them for the host,
  a compile error rather than a test failure. `crates/fjell-tools/src/
  cargo_metadata.rs` derives the exclude set from `cargo metadata` instead
  of a name list. Demonstrated failing first (RFC-v0.22-001): a register
  deliberately removed from a real `SYSCALL-CALLSITE-002`-guarded `asm!`
  block in `fjell-syscall` was caught by the new tier, reverted, and shown
  passing again.

  **`fjell-kernel`'s tests remain unreachable** — the real target is
  bare-metal with no libtest harness, so no host invocation reaches
  `lease/mod.rs` (one half of a Verus release-required target),
  `mm/frame_alloc.rs`, `mm/user_ptr.rs`, `task/scheduler.rs`, or
  `trap/dispatch.rs`'s tests (including the RFC-v0.23-002 milestone
  markers) either. Making the kernel host-testable was this line's
  explicit non-goal and, per this entry's own original framing, was never
  this erratum's core claim ("this is not 'kernel unit tests do not run'
  but 'the verification tooling's own tests do not run'") — tracked
  separately as a distinct, still-open architectural question.

  **The CI half is fixed too**, not just `test-all`'s: a new `ci-host-bins`
  job runs the identical derived command (`cargo xtask host-bin-tests`), so
  the gate tools' own tests — previously never named in any
  `.github/workflows/ci.yml` job at all — now run on every push and PR, not
  only locally.

- **Several verification instruments used to decide by matching a fixed
  string** (Errata **E-014**, ACCEPTED, tracked **RFC-0.29-002** — six of
  seven instances fixed, one survives). Gate 5 used to count rows containing
  `**OPEN**`, so a row marked `**BLOCKED**` was counted in none of its four
  buckets — fixed by reading the underlying tool's real exit status
  (`fjell-readiness-check` already computed the right answer; nothing
  needed re-deriving from its text). Gate 6 used to count the literals
  `§1`..`§6` and discard its regeneration's exit status — fixed, and
  demonstrated on a regeneration that fails while a stale six-section
  report remains on disk (it now fails, not passes). Gate 7 used to count
  the literal `"| OPEN |"`, missing an annotated `"| OPEN (blocked on X) |"`
  — the shape the register already uses for `ACCEPTED`; fixed by parsing
  the table's actual status cells (RFC-0.29-002 §7's shared parser). The
  negative harness's `FORBIDDEN` list used to match `"TEST:FAIL"`, not a
  substring of the real message `TEST:M7:FAIL (init did not exit cleanly)`
  — fixed with a structural `TEST:<token>:FAIL` scan. `errata-limitations`
  used to require only that an erratum's bare *ID string* appear anywhere
  in this file — fixed to require the file's own established `**E-NNN**`
  bold-reference convention, so an incidental, unformatted mention no
  longer counts as disclosure. `fjell-unsafe-audit`'s category extractor
  used to split on whitespace and commas only, so `category=csr-asm;
  <explanation>` silently read as `Unknown` — fixed by adding `;` to the
  delimiter set. **The shared TOML array parser survives, unfixed**: it
  still closes an array at the first line *containing* `]`, not the first
  unquoted one, so a marker string with a literal `]` (e.g. an
  `"[INTENT] ..."`-style marker) still truncates the array early —
  confirmed still live, not this line's scope (five specific instruments
  were named; this was not one of them).

- **Instrument scopes were hand-enumerated and had drifted from reality**
  (Errata **E-015**, **CLOSED** by **RFC-0.29-002**). Three of four
  historical instances were fixed by RFC-0.29-001: **21 of 91 workspace
  crates are never named in any `ci.yml` job** — the six gate-tool crates
  (E-013, above) and the three crates backing Gate 8's validation drills
  (`fjell-sig-ed25519`, `fjell-fleet-sync`, `fjell-config-sync`) now all run
  via the `ci-host-bins` job, closing this entry's own "possibly
  intentional; nothing in the workflow says so" — it was not intentional,
  and their tests run in ordinary CI now (Gate 8's drill *markers* stay
  rehearsal-only by design; that mechanism is separate and untouched).
  `ci-qemu-negative`'s matrix, `NEG_CATEGORIES`, and the
  `KNOWN_V01X_CATEGORIES`/`KNOWN_V02_CATEGORIES` lists were all **removed**
  — `qemu_run::discover_negative_categories`, derived from
  `tests/qemu/profiles/*.toml`, is the one answer every call site uses.
  **The fourth, surviving instance is now fixed too**: `smoke.rs`'s
  `v0.6-verification` milestone — mapped to a marker
  (`TEST:V0.6-VERIFY:PASS`) no kernel or service code has ever emitted, and
  invoked by nothing, anywhere — is deleted rather than wired up (there is
  no v0.6 functionality behind it to wire up to); `cargo xtask qemu-test
  v0.6-verification` now correctly reports `unknown milestone`.
  `fjell-driver-uart`, `fjell-svc-fault`, and `fjell-svc-timeout` remain
  absent from `ci-cross-check`'s crate list — a `cargo check` coverage gap,
  not a test gap (all three have zero `#[test]`s today), outside this
  erratum's own test-execution framing; disclosed, not part of its closure.

- **No instrument verifies any document link, index, or count** (Errata
  **E-016**, **CLOSED** by RFC-0.27-001). `rfcs/README.md` — the repository's
  RFC index under RFC 000 — had zero instrument coverage; thirteen (measured
  again for this RFC: fourteen) relative links in tracked documentation were
  broken; the instrument audit's own totals table had stated a population of
  56 while summing to 54 (already corrected under RFC-0.24-003); and the
  index's "Shipped" column names a release for roughly 150 historical rows as
  `v0.3.0`, `v0.22.0` and so on — tags that never carried a `v` prefix,
  left as historical rows rather than retroactively renamed. RFC-0.27-001
  built the missing coverage as three new `fjell-consistency-check`
  subchecks: `errata-tracking` (the tracking-column defect this same RFC
  found, below), `doc-links` (every relative link in a tracked `.md` file
  must resolve; 12 of 14 broken links fixed mechanically, 2 recorded in
  `tests/doc-links/known-broken.txt` pending an ADR-renumbering decision
  outside this line's scope), and `doc-counts` (`rfcs/README.md`'s five
  folder-count assertions checked against the tree). The index's stale
  release-tag rows are unchanged — renaming ~150 historical files was out of
  scope here as it was in 2026-08-03, for the same reason (it would break the
  links commits and release records point at).

- **The instrument audit's `sound` verdicts were not all demonstration-backed**
  (Errata **E-017**, **CLOSED** by **RFC-0.29-002**). RFC-0.24-001 requires
  every instrument claimed `sound` to carry a committed demonstration of it
  failing, and records `UNAUDITED` otherwise. Two rows (`ci-proptest`, `Gate
  4`) were found violating that in the 0.24 review and repaired then; the
  re-derivation of the remaining rows was left incomplete, and the register's
  own recount (2026-09-09) itself undercounted — 21, not 22 — by missing
  `ci-verus`'s differently-bolded `**sound (by explicit design)**` heading.
  All 22 rows are now re-derived or corrected: 4 rows that cited only a
  tool's own unit suite (Gate 2, Gate 11, Gate 12, `syscall/expected.toml`)
  had their citations corrected to the real, specific demonstrations that
  already existed (a live category violation for Gate 2; the
  `SYSCALL-CALLSITE-001`/`-002` regression tests for Gate 11; one named
  failing-test per subcheck for Gate 12's then-ten subchecks — eleven since
  RFC-0.30-003); 4 rows with no
  demonstration at all (`fjell-abi-snapshot` ×2, `repro/baseline-digests.txt`,
  `ci-arm64-check`) each got one produced live against real committed data
  (a reverted-and-restored scanner regression, a corrupted digest byte, a
  broken cross-compile). **No instance survives.** This is why RFC-0.24-001
  shipped `Implemented-with-Errata` rather than `Implemented`.

- **The `ipc` negative profile's blocked-recv scenario assumed an
  unsynchronised scheduling order** (Errata **E-019**, **CLOSED** by
  **RFC-0.28-003**). `fjell-neg-test`'s `test_ipc_blocked_recv` documented,
  in its own source comment, an assumption about *relative* task-scheduling
  order — "sample-service immediately calls `sys_ipc_recv` and blocks before
  the scheduler returns to neg-test" — that RFC-0.26-001's fairness fix no
  longer guarantees. (`fjell-sample-service`'s startup intent emission was
  originally recorded here too; it is a service rather than a harness and
  was split out as **E-020**, closed separately by RFC-0.26-004 — see
  below.) A predecessor line, RFC-0.26-003, concluded no signal existed to
  wait on and none could be built — **false**: the kernel is the authority
  on a task's blocked state (`sys_task_status` already dispatched, already
  imported by `fjell-neg-test`) and can be polled; the real gap was
  *addressing* (`sample-service` isn't a task `neg-test` spawned itself, so
  it had no `TaskId` to poll). RFC-0.28-003 closed the addressing gap with
  a one-way, kernel-attested identity exchange on their already-dedicated
  endpoint (RFC 042's object 6, not the contested shared object 0) and
  replaced the single defensive `sys_yield()` with a bounded poll, failing
  closed with a new marker on exhaustion. Demonstrated live, both
  directions: with the wait removed, `sys_task_status` reads `Runnable`,
  not `Blocked`, and the profile now correctly reports FAIL; with the real
  poll, `sample-service` reaches `Blocked` after exactly 2 iterations,
  measured repeatedly — precisely characterising what the old single-yield
  comment was, in fact if not by contract, relying on.

- **The ABDD live path runs again** (Errata **E-020**, **CLOSED** by
  RFC-0.26-004). RFC-v0.23-001 shipped this project's distinguishing
  architectural bet in `0.23.0` — `sample-service` emits an intent,
  `semantic-stream` routes it, and a *separate* `proxy-text` task renders it,
  with the capability-checked refusal demonstrated — and created
  `tests/qemu/profiles/semantic.toml` in the same RFC as a fail-closed guard.
  RFC-0.26-001's scheduler fix removed the priority asymmetry the path had
  been silently relying on, and `sample-service` called `emit_sample_intent()`
  on the bare assertion that its peers were *"already spawned and ready by
  this point"* rather than synchronising on it.
  RFC-0.26-004 replaced the assertion with a real wait —
  `emit_sample_intent()`'s transport is a blocking `sys_ipc_call`, which
  queues and blocks the caller until `semantic-stream` actually reaches its
  receive loop and replies — made safe by establishing that
  `semantic-stream`'s and `proxy-text`'s endpoints each have exactly one
  receiver (see E-021 below), so the call can never be delivered to, and
  dropped by, anyone else. All four `semantic.toml` markers pass, with a
  causal ordering in the serial log (not just marker presence) confirming the
  wait actually executed. See
  `rfcs/answers/RFC-0.26-004-readiness-channel-answer.md`.

- **`init` no longer receives on another task's endpoint** (Errata **E-021**,
  **CLOSED** by RFC-0.26-004, cleanly, no residual hazard). `fjell-init`'s
  `wait_ready_exact` used to loop on a blocking receive and discard any
  message whose tag did not match the one it wanted — no reply, no re-queue,
  no log — while holding receive-capable capabilities to endpoint objects 7
  and 8, which are also `semantic-stream`'s and `proxy-text`'s own endpoints
  and carry ordinary protocol traffic: two tasks received on one queue with
  nothing arbitrating between them. RFC-0.26-004 removed `wait_ready_exact`
  entirely rather than patching its missing `else` — there is no code path
  left in `init` that can receive on either endpoint. `init`'s capability to
  object 7 is narrowed to `CALL` only (kernel-enforced: a future `sys_ipc_recv`
  there would fail the rights check); its capability to object 8 is removed
  outright. **Invariant established: a service's endpoint has exactly one
  receiver — the service itself.** See
  `rfcs/answers/RFC-0.26-004-readiness-channel-answer.md`.

- **The one-way send wrapper's name and doc-comment described a primitive
  the kernel never implemented** (Errata **E-022**, **CLOSED** by
  RFC-0.27-002). `sys_ipc_try_send` was named as though it tries and
  documented as a non-blocking, fire-and-forget contract; the kernel
  implements one-way send as coherent **rendezvous** IPC, symmetric with
  two-way call/reply — `sendq`/`recvq` are waiter queues, not message
  buffers, and the kernel correctly blocks the caller when no receiver is
  waiting. **The kernel was correct; the wrapper's name and documentation
  were not**, which RFC-0.27-002 fixed by renaming it to `sys_ipc_send` and
  rewriting its doc-comment, plus the three normative docs describing the
  old contract. First found live while implementing RFC-0.26-004: once
  `init` was correctly removed as `semantic-stream`'s/`proxy-text`'s
  accidental co-receiver (E-021), each service's pre-existing
  `send_ready()` call (announcing into its own endpoint, before reaching
  its own receive loop) blocked itself permanently under the corrected
  understanding of the contract — those two calls were dead code under the
  new invariant regardless, and were deleted. **RFC-0.27-002's required
  audit found this shape recurs in five more services**
  (`fjell-measuredd`, `fjell-attestd`, `fjell-recoveryd`, `fjell-storaged`,
  each via raw inline `asm!`, not the wrapper). None **self-deadlocks**:
  each sender genuinely **blocks** when it finds no receiver waiting (the
  kernel's own audit ring confirms this live — `fjell-attestd`'s own
  `send_ready()` recorded `Queued`, not `Delivered`) and is **woken** once
  `init`'s `wait_service_ready`/`wait_storaged_ready` reaches that
  endpoint — the exact masking arrangement already removed for
  `semantic-stream`/`proxy-text`, intact here only because nothing has
  asked `init` to stop. Filed as **E-024**, below. Whether a genuinely
  non-blocking one-way send
  should exist is answered as **a real, recurring need, not decided by
  this line** — an ABI addition requires escalation this RFC's scope does
  not authorise. See
  `rfcs/answers/RFC-0.27-002-one-way-send-contract-answer.md` for the full
  audit and reasoning.

- **The release tool's `RELEASE.md` generation and consistency checks were never
  built** (Errata **E-023**, **CLOSED** by RFC-0.27-001). RFC-v0.7.1-001,
  marked `Implemented (v0.7.1)`, specified five behaviours for the release
  tool; one shipped. `package_release.rs` read the workspace version and
  tarred the repository root — it did not generate a `RELEASE.md`, did not
  produce a digest manifest of `crates/fjell-kernel/prebuilt/`, did not exit
  non-zero on inconsistency, and **did not grep for stale version mentions
  outside `CHANGELOG.md`** — the second row, and the one that mattered: it is
  exactly what would have caught `README.md` sitting at `0.21.3` with five
  wrong counts through five releases, found only when the owner asked. The
  root `RELEASE.md` — a ten-line signpost carrying none of the specified
  contents — was removed 2026-08-27. RFC-0.27-001 built the specified
  check, scoped as `version-currency` (a new `fjell-consistency-check`
  subcheck, not a `package-release` change): `README.md` — the one document
  whose purpose is "what is Fjell OS right now" — must not assert a version
  other than the current workspace version. A tree-wide sweep was tried and
  rejected as unbuildable (over 800 legitimate historical version mentions
  across RFC files and `ROADMAP.md`); see the subcheck's own design note for
  why `README.md` is the right scope. `RELEASE.md`-file generation and a
  prebuilt-artefact digest manifest remain unbuilt — out of this line's
  scope, and not what caused the incident this erratum records.

- **v1.0 release-checklist Step 9 references a build output that does not
  exist** (Errata **E-012**, ACCEPTED). Step 9 signs
  `target/release-bundles/*.bundle`; nothing in `crates/` or `tools/` writes
  that path, and `package-release` produces a single tarball. Steps 9–10 are
  the signing steps, so the v1.0 checklist cannot currently be executed to
  completion. Deliberately not investigated in v0.22 (owner decision,
  2026-07-30 — v1.0 is not in view); must be resolved before v1.0 preparation
  begins.

- **`init` no longer receives on any service's own endpoint** (Errata
  **E-024**, **CLOSED** by RFC-0.28-001). `storaged`, `measuredd`, `attestd`
  and `recoveryd` used to announce readiness into the same endpoint object
  they later receive protocol traffic on, rescued from self-deadlock only by
  `init` also holding receive rights there and reaching each wait in a fixed
  boot sequence — the same missing-`else`-shaped hazard as E-021, on four
  more objects. RFC-0.28-001 gave service-manager its own dedicated
  readiness endpoint (distinct from the "shared" object 0, which `auditd`
  and `bootctl` independently default to and were found, live, to be racing
  service-manager for the same messages), narrowed `init`'s capability on
  all four remaining objects to `CALL` only, and replaced the direct
  per-service waits with a relay through service-manager. The invariant "a
  service's endpoint has exactly one receiver" now holds structurally, not
  by convention, on every object it names.

- **Two tools walked untracked scratch trees, with disagreeing exclusion
  lists** (Errata **E-025**, **CLOSED** by RFC-0.28-005). `trust-report`'s
  cap-manifest scan and `fjell-unsafe-audit`'s walk each hand-maintained a
  different set of skipped directories, and neither excluded
  `.git-exclude/` — the directory this project's own conventions use for
  scratch work. A checkout there took the cap-manifest count from 1 to 2
  and doubled the reported unsafe-site inventory (demonstrated live against
  a fresh clean-clone checkout: 284/284 to 568/568 — the RFC's own cited
  311/622 had already gone stale relative to `HEAD` by the time this was
  fixed, an unrelated drift checked and reported rather than assumed). Both
  readings were internally consistent, so nothing in the output said which
  repository it described. Fixed by deriving each tool's scope from `git`
  itself rather than an enumerated name list: `fjell-unsafe-audit` (which
  must still see a developer's uncommitted work) now uses
  `git ls-files --cached --others --exclude-standard`; `trust-report`'s
  cap-manifest scan (which reports on the repository as shipped) now uses
  `git ls-files` alone. Neither hand-lists `.git-exclude` or any other
  scratch-directory name; a scratch checkout is excluded because
  `.gitignore` already says so, not because a person remembered to add it.


- **No QEMU serial log had ever been committed alongside the document that
  cites it** (Errata **E-026**, **CLOSED** by RFC-0.27-004). E-013 leaves
  nothing kernel-side host-testable, so QEMU logs are the only evidence for
  kernel behaviour, and `.gitignore:28` (`*.log`) kept all of them out of the
  tree; `tests/qemu/artifacts/` was overwritten by the next run, and
  `test-all`'s per-tier logs under `tests/runs/` capture build stdout rather
  than the serial transcript. `tests/evidence/` (a narrow, deliberate
  `.gitignore` exception) plus `cargo xtask evidence promote` now let a
  citation be committed with mandatory provenance — including whether the
  build was instrumented, since a provenance block naming only a commit sha
  would let a reader assume a reproducibility the log may not have. Gate 12's
  `evidence` subcheck holds this both ways: every citation must resolve with
  valid, ancestor-checked provenance, and every promoted file must be cited
  by something. **Of the citations this project had already made, 1 of 3 was
  resolvable** and was promoted (RFC-0.27-002's); the other 2 were not — see
  **E-029**, below.

- **Nothing checks that the threat model's threats cite RFCs** (Errata
  **E-027**, ACCEPTED). The v0.9–v0.15 handoff asserted a "threat-model gate"
  enforcing it; no such gate exists in any commit on any branch. The property
  holds as of 2026-08-31 — all 20 `Tn` sections in
  `docs/src/security/threat-model-v1.md` cite an RFC, and the 20 in-scope / 8
  out-of-scope counts are correct — but it is held by hand, and a regression
  would be reported by nothing.

- **`fjell-sxt-crypto`'s doc-comment cites two documentation files that do
  not exist** (Errata **E-028**, ACCEPTED). RFC-v0.7.3-002 (Implemented,
  v0.7.1) specified `docs/src/security/crypto-profile.md` and
  `docs/src/security/crypto-roadmap.md` as deliverables — the latter is
  named in that RFC's own acceptance criteria — but neither exists anywhere
  in the tree, while the crate's live doc-comment still points at both.
  The underlying disclosure (development-only crypto, not for production
  use, documented cache-timing leak) is not lost — it is in the
  doc-comment itself — but the dedicated write-up is missing. Found while
  building the CRA/IEC standards mapping (RFC-0.27-003) and tracing its
  confidentiality-clause evidence into this crate.

- **Two historical QEMU-log citations were unresolvable** (Errata
  **E-029**, **CLOSED** by RFC-0.28-005 R3). RFC-0.27-004's R6
  reconciliation found the `tests/qemu/artifacts/semantic/serial.log`
  content cited by `RFC-0.26-004-readiness-channel-answer.md` and by the
  archived `RFC-0.26-002-abdd-path-synchronisation.md` no longer existed —
  overwritten by later runs before this RFC's per-run retention existed.
  Not re-run to stand in for the originals (D4); the first was annotated
  in place, the second (already `Superseded`) was left as the point-in-time
  record it already was. Fixed by re-running the `semantic` profile fresh
  (commit `6582d04`, clean tree) and promoting it with real provenance
  (`tests/evidence/RFC-0.28-005/semantic-fresh-2026-09-08.log`); the
  readiness-channel answer document now cites the fresh run **alongside**
  the historical annotation, which stays rather than being deleted once
  superseded. The archived RFC-0.26-002 citation is left as-is, per this
  project's practice of not rewriting archived records.

- **Nothing checked that the project's two version strings agree** (Errata
  **E-030**, **CLOSED** by RFC-0.28-005). `[workspace.package] version` and
  `crates/fjell-os/Cargo.toml`'s `fjell-abi` version pin must match at every
  release and are maintained by hand; `version-currency` checked `README.md`
  only, not this pair — the gap that cost the 0.27.0 cut its first command.
  `version-currency` now also compares the pair directly, demonstrated
  failing on the exact mismatch the 0.27.0 cut hit (`0.26.0` pin against a
  `0.27.0` workspace version). The check states its own limit in its
  failure message: the mismatch was already fail-closed (`cargo metadata`
  cannot resolve, so every gate, tier, and build fails with it regardless),
  so this buys the time of a named failure rather than adding correctness a
  passing run didn't already have.

- **RFC 058's readiness tracking now completes** (Errata **E-031**,
  **CLOSED** by RFC-0.28-001). `service-manager` used to emit
  `NEG:SVC:READY_ACCEPTED:PASS` only once 10 services reported ready — a
  threshold re-derivation found unreachable by an even wider margin than
  first thought (3 of 14 images could reliably reach service-manager under
  the old topology, not "at most 5": its own receiving object was also
  `auditd`'s and `bootctl`'s default, and live-verified to lose messages to
  both). RFC-0.28-001 gave service-manager a dedicated, uncontested
  endpoint and re-derived the threshold to **8** — the number of images
  that actually send `tags::SERVICE_READY` today, confirmed live and
  reproducible over repeated runs, not lowered to force the marker.

- **35 hand-rolled syscall `asm!` blocks across 14 crates carried three
  register-contract bugs** (Errata **E-032**, **CLOSED** by **RFC-0.28-002**).
  Twelve omitted the `a6` clobber (`IpcRecv`); eighteen declared `a0` as a
  plain input where the kernel writes status (`IpcRecv`/`IpcReply`); three
  `IpcCall` sites didn't declare the reply-word registers `sys_ipc_reply`
  overwrites unconditionally on completion. RFC-0.28-001 hit the first bug
  in two blocks it wrote — a non-deterministic permanent hang that debug
  prints made disappear — and fixed those two; RFC-0.28-002 deleted the
  other 28 hand-rolled blocks (every syscall they issued already had an
  audited wrapper) and fixed the register contract of the 7 kept, where no
  existing wrapper covers the shape needed (4-word `IpcCall`, worded
  `IpcReply`). Closed structurally: Gate 11's `SYSCALL-CALLSITE-001` refuses
  any raw syscall-issuing block outside `fjell-syscall` unless it is on an
  explicit, guard-owned allowlist naming exactly those 7 sites, each still
  checked for correct clobbers. A related gap found during the same audit —
  `fjell-syscall`'s own `sys_ipc_recv`/`sys_cap_inspect`/`sys_ipc_call_words`
  carried the identical defect class internally — is **E-033**, closed by
  RFC-0.28-004 (below).

- **Four `send` helpers used to accept a payload word that was never
  transmitted** (Errata **E-034**, **CLOSED** by **RFC-0.34-004**).
  `sxt_send`, two `send_tag`s and `send_sxt` took a data word; none packed a
  word count into the tag, so the kernel copied zero words and the payload
  never reached a receiver. The parameter is now gone from all four
  signatures. One caller's dropped word was not merely unread: `upgraded`'s
  `channel_id` for `UpdateMetadataFetch`/`Close` genuinely reaches a receiver
  that reads it and always finds `0` — `fetch_update_index` has always
  returned `None` against a real `secure-transportd`. Pre-existing,
  independent of this fix, and named rather than repaired: carrying the word
  for real is an ABI change to the tag packing, its own line. Also found and
  named: `secure-transportd`'s receive loop polls only one of its three
  client endpoints, so `attestd`'s and `diagnosticsd`'s requests are
  unreachable regardless; and `fjell-auditd` never implements the
  `AUDIT_QUERY` protocol `diagnosticsd` speaks to it over IPC at all.

  > **Amended 2026-10-15, exit criterion 8 of the 0.35.0 cut — three present-tense
  > claims above are history now.** They were true when written at RFC-0.34-004 and
  > 0.35 changed all three. **The words are carried**: RFC-0.35-001 sends both the
  > channel kind and the channel id for real through `sys_ipc_send_words`, and made
  > channel id `0` invalid by construction. **`fetch_update_index` still returns
  > `None`**, but no longer because a receiver reads `0` — the TLS handshake fails
  > closed against an unprovisioned trust anchor, a separate limitation recorded
  > elsewhere on this page (E-070, **CLOSED** as to the vanishing message, not as to
  > the fetch). **The three unserved endpoints are gone**: RFC-0.35-002 deleted
  > `CAP_DIAG_EP` and `CAP_ATTEST_EP` and corrected the round-robin comment (E-069),
  > and **retracted the `AUDIT_QUERY` sub-protocol entirely** — `fjell-auditd` still
  > does not implement it, and nothing asks it to any more (E-068). This bullet is
  > kept as written because it records what RFC-0.34-004 found; the correction is
  > here rather than in its text for that reason.

- **`fjell-syscall`'s own helpers carried the register-contract defect the
  crate exists to protect everyone else from** (Errata **E-033**, **CLOSED**
  by **RFC-0.28-004**). `sys_ipc_recv` lost the delivery's words and attested
  sender identity; `sys_ipc_call_words` never declared `a5` at all (a third
  instance, found extending the guard into this crate, not in the original
  scoping). `sys_cap_inspect`'s actual defect was more precise than first
  scoped: the second call it issued was `CapRevoke` (13), not `CapInspect`
  (14) — a stale literal, not a scheduling race. Demonstrated live: the one
  real caller (`fjell-proxy-text`) currently gets the *correct* rights value
  only because the inspected capability correctly lacks `REVOKE`, so the
  wrong-numbered call fails closed without touching the registers the first
  (correct) call had already written — undefined behaviour appearing correct
  by coincidence, not a guaranteed contract. All three now issue one
  correctly-declared syscall each; `SYSCALL-CALLSITE-002` (Gate 11) enforces
  it inside `fjell-syscall` going forward. `sys_ipc_recv`'s long-term future
  (repair vs. remove) remains an open escalation, not resolved by this fix.

- **The ABI baseline used to never be enforced current** (Errata **E-035**,
  **CLOSED** by **RFC-0.30-002**). `tests/abi/snapshot.json` used to hold 413
  items against a tree of 418; Gate 4 reported `Added: 5 (additive — OK)` and
  passed, correctly under the old rule. Removals and signature changes were
  still caught — but the baseline no longer described any shipped release, and
  a future regeneration would have absorbed every accumulated addition in one
  unreviewed step.

  **Fixed:** `fjell-abi-snapshot --verify` now fails whenever `Added != 0`, not
  only on `Removed`/`Changed sig` — the same gate, a stricter pass condition.
  Argued on measured cost, not assumed: `tests/abi/snapshot.json` has been
  touched in 8 commits across this project's entire 176-RFC history, so the
  gate is red only on the rare line that actually touches the stable surface,
  and only until that same line runs `--generate`. Demonstrated failing on a
  deliberately un-regenerated baseline (3 real current items held back via a
  scratch `--snapshot` copy, no tracked file touched): `Added: 3`, `Result:
  FAIL`, naming all three. `docs/src/releasing/v0-release-cycle.md`'s cut-time
  step is now a confirmation that the per-line discipline held, not a task.

- **The two-build reproducibility check used to never run, and could not fail
  if it were** (Errata **E-036**, **CLOSED** by **RFC-0.30-001**).
  `fjell-repro-check`'s two-build mode used to run `cargo xtask build` twice
  with no clean and no separate target directory, so the second build was an
  incremental no-op and the comparison was between a file and itself —
  measured at 0.41s and 0.40s per "build". Every call in the tree passed
  `--skip-build`, a staleness check on committed artefacts, not a
  reproducibility check, and neither mode named the kernel binary anywhere
  even though the two-build path already collected it.

  **Fixed:** a scoped `cargo clean --release --target
  riscv64gc-unknown-none-elf` now precedes each of the two builds, so the
  second cannot reuse the first's output. Measured, not assumed: a genuine
  two-build run costs **4–7 seconds total** (this project's crates are small
  — one kernel, 29 services, `opt-level = "s"`), not the "doubles CI build
  time" the RFC worried about before anyone had timed it. It now runs in CI
  on every push (`ci-repro-check`, `cargo xtask two-build-check`), not
  nowhere.

  *Corrected 2026-09-12 (E-041): that job has never succeeded. Every CI job that builds services fails at `-Z build-std` because Ubuntu's apt `rust-src` ships no `library/Cargo.lock`; the workflow has had one green run in 152, on 2026-05-05.* The check is real and its demonstrations were
  local; it has not yet run anywhere else.

  *Corrected again 2026-09-12, later the same day (RFC-0.31-002): it runs on
  CI now, and this is the first sentence about it here written from a run
  rather than from `ci.yml`. `ci-repro-check` is green on run `34674356238`,
  job `103501524504`; `cargo xtask two-build-check` ran 04:59:10Z-04:59:37Z,
  27 seconds to build the product twice and compare all 30 artefacts. The
  claim in the paragraph above — "it now runs in CI on every push" — is true
  from `ca1dcd5` onward and was false for the six months before it.*

  **The build is, as measured, reproducible.** Two independent runs (each
  with its own clean) produced bit-for-bit identical output across all 30
  artefacts — the kernel ELF plus all 29 service prebuilts, both counts
  re-derived and confirmed — checked twice on 2026-09-09. Building the
  identical commit from a different absolute checkout path also reproduced
  identically, ruling out the classic embedded-build-path hazard for this
  toolchain/profile. This is **same-machine** reproducibility only —
  cross-machine remains untested and unclaimed — **this project makes no
  cross-machine claim**, and every digest it has ever recorded was produced
  on one machine. *(Stated on its own from 2026-09-12. This sentence used to
  cite E-037's surviving instances; E-037 is now CLOSED, and this limitation
  is not one of the things that closed with it — it was never a contradicted
  claim, only an unmade one. Decoupled deliberately rather than allowed to
  retire alongside the erratum that happened to be carrying it.)*

  **The check's sensitivity was demonstrated, not assumed to exist:** forcing
  a differing `-C metadata` value scoped to the `riscv64gc-unknown-none-elf`
  target only (no kernel/service source touched) reproduced this project's
  own prior incident — `-C metadata` moving digests on a version bump
  (RFC-v0.16-005 H-04) — and the actual fixed comparison correctly reported
  `FAIL` on 14 of 29 service prebuilts.

  **Kernel coverage was already there, just unstated:** the two-build path's
  `DEFAULT_TARGETS` always included the kernel ELF alongside `prebuilt/` (30
  artefacts total); this is now said out loud (T20, `ERRATA.md`, here).
  `--skip-build`'s committed baseline stays 29, services only, by design —
  the kernel is not a committed artefact, so there is no baseline digest of
  it to record. Two checks, two honestly different scopes.


- **The toolchain used to be declared in twenty-two places, and recorded
  nowhere** (Errata **E-037**, ~~ACCEPTED~~ **CLOSED** 2026-09-12, by
  **RFC-0.30-003**, **RFC-0.31-002** and **RFC-0.31-003** — all three of its
  own closure conditions, *"one declaration, an exact pin, and a record"*,
  are met). *Updated 2026-09-12: the consolidation survivor is CLOSED. CI installs through
  rustup from `rust-toolchain.toml` via one composite action, so the count
  is five, not twenty-two — the declaration itself plus the four
  documentation sites a human types by hand, all four still gated. The
  surviving limitation is the second one below: the channel floats within
  `1.91.x`, unpinned, and cross-machine reproducibility is still compared
  nowhere. Pinning is now a free decision: it used to collide with the drift
  gate, because Ubuntu's apt has no `rustc-1.91.1` package, and `ci.yml`
  names no apt package any more.* `rust-toolchain.toml` (channel, `rust-src`, the
  RISC-V target), `.github/workflows/ci.yml` (`apt-get install
  rustc-1.91`, copied into **17 of 19** jobs), `docs/src/releasing/
  release-checklist.md`, `docs/src/internals/local-development.md`,
  `docs/src/tutorials/quick-start.md` and `Cargo.toml`'s `rust-version`
  all state `1.91` independently; one of them (`release-checklist.md`) is
  a check that would have kept asserting `1.91` after a bump. Nothing
  recorded which toolchain produced the repro baseline, so a digest
  mismatch was indistinguishable from a real reproducibility failure.

  **Fixed:** all three artefact-producing paths (`tests/repro/
  baseline-digests.txt`, `tests/evidence/**/*.provenance.txt`,
  `releases/trust-report.txt`) now record the toolchain **observed**
  at production time (`rustc -vV`'s release/commit-hash/host/LLVM
  version) — never the declared channel, which would have kept saying
  `1.91` throughout the 1.91→1.98.1 incident while being wrong the whole
  time. A new `toolchain-declarations` consistency-check compares
  `rust-toolchain.toml`'s channel against the other 21 live sites (not
  the historical release notes and handoffs the same grep also finds —
  those are correct as written) and fails naming exactly which site was
  left behind at a bump.

  **What survives, in plain terms:** a version bump is still 22 manual
  edits, not one — this makes a forgotten site loud instead of making it
  impossible. And the channel still floats within `1.91.x`, unpinned;
  pinning was argued (probably worth it eventually, cost not yet weighed)
  and deliberately not done there.

  **One survivor now** (updated 2026-09-12). Consolidation is **closed**:
  RFC-0.31-002 put CI on rustup reading `rust-toolchain.toml`, and
  twenty-two declaration sites became five, four of them documentation a
  human types. *The argument recorded here previously — that CI installing
  via apt was the safer choice, because its independence from
  `rust-toolchain.toml` contained the 1.98.1 drift to local builds — did not
  survive contact with E-041: apt's `rust-src` ships no `library/Cargo.lock`,
  so that "independence" was coincident with CI being unable to build the
  product at all.*

  **Nothing remains (updated 2026-09-12, RFC-0.31-003).** The channel is
  pinned exactly at **1.98.1** — current stable, not the ten-month-old
  compiler `1.91` resolved to, because pinning where we happened to be would
  have frozen a gap nobody had chosen. Verified by a 24-tier QEMU pass on the
  rebuilt tree and a fully green CI run (`34695574958`, 34 jobs), not by
  compilation alone: 24 of the 29 prebuilts changed, and a digest diff cannot
  tell an expected total change from a regression hiding inside one.

  Before the pin could be taken, `docs/src/tutorials/quick-start.md` had to
  move off apt — it was still telling every first-time reader to
  `apt install rustc-1.91`, which is at once the path that cannot
  `-Z build-std` (E-041's cause, still live in the tutorial months after
  E-041 was closed on CI going green) and the last thing in the tree that
  would have collided with an exact pin. RFC-0.31-002's review had recorded
  that collision as gone; it had moved from `ci.yml` to the tutorial, and the
  test certifying it gone used an apt command that has never been runnable.

  **What this closure does not include**, stated so it is not lost with the
  erratum: cross-machine reproducibility is still compared nowhere (above,
  now stated independently), and the true MSRV minimum is still undetermined
  — the floor is now *verified* at exactly `1.91.0` rather than assumed, but
  verified is not bisected. **And the pin's own cost is instrumented**: an
  exact pin turns silent drift into silent staleness, so release-cycle exit
  criterion 10 records the pin, current stable and the gap at every cut, and
  more than three minor versions behind blocks the tag or takes an
  accepted-risk statement.

  *Corrected 2026-09-10. This bullet previously said `rust-toolchain.toml` had
  been removed and that no `rust-version` field existed. Both were true for one
  day: the file was removed on 2026-09-09 (`4cebbc4`), restored the same day
  after the removal was found to have silently moved local builds from 1.91.1
  to 1.98.1 and changed 24 of the 29 committed prebuilts, and `rust-version = "1.91"`
  was added to `[workspace.package]` at the same time. The bullet was not
  updated then.*

- **Four subchecks used to fail without a result line naming themselves**
  (Errata **E-038**, **CLOSED** by **RFC-0.30-002**). With an RFC lifecycle
  folder absent, `rfc-status-folder`, `handoff-status`, `errata-tracking` and
  `doc-counts` all used to fail with a bare `consistency-check: cannot read …`
  line — no subject, so the run ended `consistency-check: FAIL` with no way to
  tell which of ten checks broke. (`handoff-status` was not actually silent, a
  correction from this line's own diagnosis: depending on which RFCs currently
  had handoffs, it either produced the same unnamed message or — worse —
  **passed incorrectly**, blind to the missing folder because it only ever
  touched lifecycle folders incidentally, through whichever RFC a handoff
  happened to cite.)

  **Fixed:** `read_file`/`read_dir_named` (`tools/fjell-consistency-check`) now
  take the calling subcheck's own name and print `<name>: FAIL — <reason>` on
  any I/O failure — applied everywhere that shape appeared in the crate, not
  only these four — the sweep reached all ten subchecks that existed at the
  time, the other six being `doc-links`, `version-currency`,
  `syscall-surface`, `evidence`, `standards-mapping` and
  `errata-limitations`; the eleventh, `toolchain-declarations`
  (RFC-0.30-003), was built on the same named helpers.
  `handoff-status` additionally now enumerates `rfcs/{proposed,accepted,done}`
  directly before touching any handoff, closing the blind-pass gap. Keeper
  files still exist in all four lifecycle folders, unrelated and unchanged —
  this fix is for when one goes missing anyway.


- **`qemu-test` used to accept six milestones nothing can pass** (Errata
  **E-040**, **CLOSED** by **RFC-0.31-001**). `cargo xtask qemu-test m1`…`m6`
  were accepted by the harness, but the kernel emits a PASS marker for only
  `m7`, `m8`, `v0.4-net`, `v0.5-platform` and `v0.7-sync`. Running one of the
  six booted QEMU, waited out the full 60-second timeout and reported `FAIL`,
  indistinguishable from a real regression, rather than saying the milestone
  is not implemented. Found 2026-09-10; E-015's closure had named only one
  such milestone and six more survived it.

  *Corrected 2026-09-12, while closing this erratum: this bullet said "none
  of the six is gated by `test-all` or CI, so nothing that decides a release
  is affected." **`test-all` is right; CI is wrong.** `.github/workflows/
  ci.yml`'s `ci-qemu-smoke` job runs a matrix of `[m1, m2, m3, m4, m5, m6,
  m7, m8]` and invokes `cargo xtask qemu-test` on each. Six of the eight name
  milestones nothing can pass. Reported rather than repaired here: what CI
  gates is adjacent to this line's "do not change what `test-all` gates"
  non-goal and `ci.yml` is outside its Touches.* *Corrected again at review,
  2026-09-12: the submission said those six "have been booting QEMU, timing
  out and failing on every push" — read from the YAML, not from a run. On CI
  none of the eight boots QEMU; all fail at the service build, as does every
  other service-building job, and always has. See **E-041**.*

  **Fixed:** the six are deleted and now reach the fail-closed `unknown
  milestone` path, which costs no QEMU boot — `qemu-test m5` answers in 0.36s
  and lists the five names that can actually pass. They were not reconnected:
  `TEST:M7:PASS` is the cumulative pass for everything M1–M6 ever checked, and
  re-adding user-space markers would recreate the concurrent-UART garbling
  that moved marker emission into the kernel in the first place. **And the
  general question is now asked by a check rather than per instance** — a test
  reads the kernel's own `trap/dispatch.rs` and fails if any accepted
  milestone is not emitted there, if anything emitted is not accepted, or if a
  marker is constructed in a way the check cannot read (so "invisible" can no
  longer pass for "absent"). That last part is what makes this closure
  different from E-015's, which named one instance and left six.

- **CI had never built this product** (Errata **E-041**, **CLOSED**
  2026-09-12 by **RFC-0.31-002**, on run `34692058308`). The workflow has had one successful run in 152, on
  2026-05-05, before any QEMU-building job existed. Every job that builds a
  service — the smoke and negative matrices, the v0.7 smokes, the two-build
  reproducibility check, `cross-check`, `test-services` — fails at
  `-Z build-std` because Ubuntu's apt `rust-src` ships no `library/Cargo.lock`;
  `test-v07-formats` fails on a feature guard RFC-0.29-001 named and left;
  `proptest`'s CI command does not compile at all. Every gate that
  decides a release in this project runs locally and always has, so nothing
  shipped on a claim CI made; but every sentence in this project that said a
  check "runs in CI on every push" was written from `ci.yml`'s text and was
  never true. The badge at the top of `README.md` has been red the whole time.
  Repair needs rustup in CI (E-037's shape 1, now a precondition rather than a
  successor) and a release-cycle step that records the observed CI conclusion.

  **Repaired 2026-09-12 (RFC-0.31-002), and observed: run `34674794847`,
  33 jobs green, 1 red, 1 skipped.** CI installs the toolchain through rustup
  from `rust-toolchain.toml` via `.github/actions/toolchain`, which fails
  closed if that file is absent (demonstrated red on run `34675044074`, job
  `103503283586`, at the assert step with the build step skipped). The kernel
  boots under QEMU on CI; the two-build reproducibility check runs there; the
  dead `m1`-`m6` matrix entries are gone; and the release cycle gained exit
  criterion 9, which records the release commit's run id and per-job
  conclusions. Two of this paragraph's three named causes turned out to be
  one — `proptest` compiled and ran 24 passing property tests all along, and
  died afterwards in its doc-test phase because the apt shim covered `rustc`
  and `cargo` but not `rustdoc`.

  **Closed the same day, on a fully green run.** The last red job,
  `test-services`, was red because `fjell-identityd` had never compiled for
  `riscv64gc-unknown-none-elf` (**E-042**) — a service source defect that
  RFC-0.31-002's scope forbade it to fix, and that review found deeper than
  the two-line repair it looked like. The owner's decision was to delete the
  dead crate; run **`34692058308`** is then **34 green, 0 red, 1 skipped**,
  confirmed again on the next commit (`34692184045`). This workflow has three
  successful runs in 165: one on 2026-05-05, before any job built for RISC-V,
  and these two. They are the first in which CI compiled the kernel, booted
  it under QEMU, ran all fourteen negative profiles, and compared two
  independent builds of the product.

  *Corrected 2026-09-15: "fully green" held for every job CI runs **on
  push**. The weekly scheduled fuzz job was listed as skipped in both runs and
  has never succeeded — see **E-043**.*

- **One service crate had never compiled for its own target** (Errata
  **E-042**, **CLOSED** 2026-09-12 — deleted).
  `fjell-identityd` imported `fjell_cap` and `fjell_service_api` without
  declaring either as a dependency, imported `Decision` and
  `NodeIdentityBuilder` from `fjell_identity_format`'s root after both had
  moved into submodules — and, behind those, called `store_read`/`store_append`
  from `fjell-service-api/src/storaged.rs`, an orphan file no `mod`
  declaration ever included, whose functions were skeletons returning
  `ServiceUnavailable`. It was never a two-line fix: the service was written
  against an API that was never compiled and a manifest ordering that never
  shipped. `ci-test-services`, the only job that compiled it, had never got
  past installing its toolchain (E-041) and so had never reported it.

  **Deleted** (owner decision): the daemon and the orphan module. The node
  identity *design* is untouched and live — `fjell-identity-format`
  implements it and four other crates depend on it; what went was the
  daemon that was never built, never spawned, and could not have worked. It
  failed identically locally and on CI (run `34674794847`, job
  `103502643804`); the other nineteen service crates in that job cross-checked
  clean. It was invisible because the only job that compiled it had never got
  past installing its toolchain — which is E-041's cost, stated concretely: an
  instrument reporting nothing is not neutral, it is cover. **It was in no
  release artefact** (`cargo xtask build` never built it and no `prebuilt/`
  entry came from it), so nothing shipped depended on it. How long it had been
  broken is answerable after all: since it was written, because the API it
  imported was in a file no `mod` declaration ever included.

- **Fuzzing covers six decoders, and only one of them sits on a live
  cross-service boundary** (Errata **E-043**, **CLOSED** 2026-09-15 by
  RFC-0.32-001). The fuzz harness had never run: five of its eight targets
  called functions that never existed. It now has one target per byte decoder
  a host fuzz crate can reach — the semantic envelope wire format, semantic
  intent records, revocation records, audit records, the device-tree
  validator (whose target also fuzzes the kernel's header check), capability
  manifests — and all of them were fuzzed for 300 seconds each at the
  0.32.0 cut (dispatch run `35089305545`). *(This bullet said "six decoders,
  and none on a live boundary" until the 0.32.0 cut review, then "seven"
  when RFC-0.32-002 added the wire decoder; **it is six again** since
  RFC-0.33-005 deleted `fjell-dtb-derive` and its target — the same number as
  the original, and **not** the same six. The changed `dtb_validate` target has not
  yet run on CI: a dispatched `fuzz-run` is owed after the push.)* Every push builds the targets and replays every
  committed seed. What this does not cover, stated plainly:
  - **Most fuzzed decoders are not on live untrusted paths.** Only the audit
    decoder and the semantic envelope decoder have runtime callers. *(Updated
    2026-09-16, RFC-0.32-002: the live cross-service byte path was
    `fjell_service_api::chunked::reassemble`, left unfuzzed because it was
    unsound by construction. It has been replaced by
    `fjell_semantic_format::wire::decode`, safe code returning `Result`, and
    the `semantic_envelope_wire_decode` target fuzzes it — the first fuzzed
    decoder on a live cross-service path.)*
  - **Some decoders cannot be reached from a fuzz crate at all** — the key-file
    and signature-manifest parsers inside the `fjell-tools` binary, and the
    word and MMIO decoders inside bare-metal services and drivers.
  - **New-input crashes are found weekly, not before merge.** Push and pull
    request runs replay known inputs only.
  - **The fuzzing nightly floats**, so a nightly regression can turn the job
    red with no change to the tree; it fails at the build step, which is how
    it is told apart from a crash.
  - One format, `fjell-verify-format`, still has no tests at all.
    *(`fjell-store-format` gained its first three on 2026-09-16 with
    RFC-0.32-002's checksum change.)*

- **One device-tree parser never worked on a real device tree, and was deleted**
  (Errata **E-048**, **CLOSED** 2026-09-25 by RFC-0.33-005, with survivors).
  `fjell-dtb-derive` returned `MissingPlic` on QEMU's own `virt` tree, nothing used
  it, and on this board no fix would help: QEMU's eight identical `virtio,mmio` nodes
  cannot be told apart from the tree. It is gone, and the record says why so nobody
  rebuilds it in bring-up by accident. **`fjell-dtb-validate` stays and is now
  exercised in Gate 1** against the committed QEMU tree — which proves *compatible
  strings present*, not that each device is at its declared address. **It still has no
  caller**: the kernel checks only the tree's *header* at boot (through a new
  dependency-free crate, `fjell-fdt-header`), and full boot-time validation, and any
  derivation of a board profile from a tree, belong with hardware bring-up (E-004).
  `fjell-devmgr` builds its board profile in code.

- **A health failure cannot reach a reset in this deployment** (Errata
  **E-044**, **CLOSED** by RFC-0.33-001, with the survivors named here).
  `bootctl` now decides health
  for real — `service-manager` reports a required service's fault, and the
  decision is printed and observed in a QEMU tier. But a genuine health failure
  on the active slot has no rollback target while that slot is also the last
  confirmed one, which is always true here: one kernel image, no slot switching,
  and nothing durable. `bootctl` reports it and does not reset. Reaching a real
  reset from a health decision needs a staged candidate slot **and** a durable
  try counter — without the counter, resetting on an organic failure would loop.
  That is why the failure path is reachable today only through a deliberate
  console trigger in the test profile, and by nothing a production boot can
  encounter on its own. The reset *mechanism* is demonstrated separately.
  **Also surviving:** four syscalls remain undispatched (`CapInstall`,
  `TaskKill`, `MmioUnmap`, `DmaShare`). *(A fifth survivor — that no tier booted
  the machine a second time, because the harness passes `-no-reboot` when it
  judges a reset — is retired: the `reboot-again` tier runs without it, injects the
  trigger once and counts the boots exactly. It exists because running the reset
  that way by hand found the machine did **not** come back — QEMU leaves `satp`
  as the previous boot set it — which the kernel now clears at entry.)*

- **Four test affordances are present in the shipped image**: a console byte,
  `F`, that makes `init` spawn the fault service (RFC-0.33-001 D10); a console
  byte, `R`, that makes `init` send `neg-test` one message so that it runs its
  reboot scenario, with the `Reboot` capability it holds for the purpose
  (D15, D17); a console byte, `P`, that makes `init` spawn a test-only
  presentation which faults on its first message, then publish twelve envelopes
  and report how many were answered (RFC-0.34-001 D11) — the stream carries a
  third, dormant presentation row for it, which queues and reports nothing unless
  it starts; and a switch that makes `init` **not start `proxy-text`** when the
  machine carries a virtio balloon device, saying so on the console
  (RFC-0.34-001 D8, D10). The three bytes are injected once by an external agent and
  are absent on the next boot, which is what makes a *reset* trigger safe: **a
  trigger for a reset must not survive the reset** (D17 — the first version was a
  virtio entropy device, which does, and a machine that has one would reset, boot
  and reset again). The balloon switch is a hardware-presence trigger, accepted
  because its worst case is a presentation that does not start and a console line
  saying why. Each exists so that a decision, a reset or an absence can be
  observed rather than asserted. `svc-fault` and `svc-timeout` are already in the
  image for the same reason.

- **The ABI baseline did not see enum variants** (Errata **E-056**, **CLOSED**
  2026-09-25 by RFC-0.33-004). Gate 4 hashed an enum's declaration line, so adding
  or removing a variant — a syscall number among them — was zero drift. The hash now
  covers the variants, and the re-record was read: across twenty-two releases the only
  variant removed from an existing enum was `Reboot = 120`, deliberately, in
  RFC-0.33-001. A braced struct's fields and a trait's items are
  covered too since RFC-0.34-003 (Errata **E-067**, **CLOSED** 2026-09-25): adding a
  field to `AuditRecordBin` now registers, `#[repr]` is part of the hash, and the
  re-record was read (no field or method removed in twenty-two releases). **Not seen:**
  attributes other than `repr`, `non_exhaustive` and — for the one published crate,
  `fjell-abi` — the sorted `#[derive]` set. Removing `Copy` from a struct in a
  `publish = false` crate is invisible to Gate 4 (the workspace compiler catches it at its use site). The syscall enum is also covered by `syscall-surface`.

- **A QEMU profile's markers could be silently split** (Errata **E-057**,
  **CLOSED** 2026-09-25 by RFC-0.33-004). The profile reader split on every comma
  and closed an array at the first `]`, inside quoted strings too, so a marker could
  be shortened or halved without a word; both failures let a tier pass on less than
  its author wrote. It now respects quoting and **refuses at load** what it cannot
  carry (an unterminated string, a bare word, an empty marker). `semantic` asserts its
  bracketed marker whole again (restored at review); two profiles (`uart-rx`,
  `semantic-braille`) still assert the bracket-free text they were written with to
  avoid the old reader — nothing else has been strengthened.

- **Struct padding was written to disk in four places** (Errata **E-055**,
  **CLOSED** 2026-09-24 by RFC-0.33-003). `fjell-init` filled sector buffers by
  viewing `StoreSuperblock`, `RecordHeader` and `BootControlBlock` as byte slices,
  so uninitialised padding reached the disk image. Each now serialises its named
  fields and nothing else, `fjell-init` zero-fills the rest of the sector
  explicitly, and a test runs the serialiser into a `0x00`-filled and an
  `0xFF`-filled buffer and requires identical output. Nothing reads any of them
  back yet (E-044), so this was not a migration — **and that stops being true at
  the first read-back**. The 0.32.0 CHANGELOG and release record state that no
  such sites remained outside the kernel; that measurement was taken with a `grep`
  that silently skips files containing NUL bytes, and `fjell-init` was the only
  such file.

- **The documentation did not say who Fjell is for** (Errata **E-054**,
  **CLOSED** 2026-09-24 by RFC-0.33-002). Inclusion — the separation of meaning
  from presentation — is a founding pillar, and neither page a reader meets first
  named it while the non-goals and identity pages narrowed the audience to
  headless industrial nodes. The pages now name it a primary goal *as a goal and a
  mechanism, not a delivery*, carry a fourth archetype, and the limits are in
  [the section above](#accessibility-and-inclusion--what-does-not-exist-yet), which
  is the condition on that claim and is kept true at each cut.

- **Thirty-four citations in the published book pointed outside it** (Errata
  **E-052**, **CLOSED** 2026-09-25 by RFC-0.33-004, with one thing owed). They were
  relative paths that leave the book — correct on disk, 404 on the site — in three
  files (the register's earlier counts, eleven and 23 + 1, were at other dates and
  by other definitions). The two subchecks that forced that spelling now accept an
  absolute repository URL, all 34 are converted, and `doc-links` refuses a new one.
  **Owed:** the served site has not been checked, because it is published only on a
  push; the check is to follow a converted citation from the live
  `compliance/standards-mapping` page.

- **The security advisory process is built, and has never been rehearsed**
  (Errata **E-051**, CLOSED by RFC-0.32-004, with that survivor named).
  RFC-v0.15-003 specified an advisory process and a per-advisory register, was
  marked Implemented, and built neither; the release checklist meanwhile
  published an address that could not receive mail beside the working channel.
  **Now built:** one process document, one intake channel — GitHub's private
  security advisory — and a register that is empty, because no vulnerability
  has been reported, and checked while empty. Advisories published against
  every crate in `Cargo.lock` are found by CI on every push and weekly, and the
  first run found two (E-053, fixed). What remains:
  - **The process has never been run end to end.** RFC-v0.15-003 required a
    rehearsal record; none exists, because a real rehearsal needs a private
    advisory to run through. A reporter is promised acknowledgement within 7
    days and a severity decision within 14; nothing has yet tested either.
  - **Every time in the process is a target, not a guarantee.** Fjell OS has
    one maintainer, and the process says so rather than implying a 30-day
    patch can be promised.
  - **The process has never been exercised end to end.** The register's shape
    is checked; intake, triage and coordinated disclosure have only been
    written down.
  - **A clean dependency check says nothing about what Fjell OS ships**, and
    its report says so: the published crates and the kernel have no
    third-party dependencies at all. It covers the tools, tests, benchmarks
    and the development-grade crypto crate.

- **Most of the documentation was not in the documentation** (Errata
  **E-050**, **CLOSED** 2026-09-16 by RFC-0.32-003). The book rendered 59 of
  the 135 Markdown files under its own source root; the other 75 — including
  all 41 architecture decision records — were in the repository but in no
  book, `mdbook build` reported nothing, and nothing published the book in any
  case. Several pages were stubs pointing at the real document outside the
  book, where the site cannot follow them, and one described itself as a
  symlink that does not exist.

  **Corrected.** Every page is in `SUMMARY.md`, the ADRs are a navigable
  section with an index, all prose lives under `docs/src`, the stubs are gone,
  no directory name is used twice, the mdBook version is pinned and checked,
  and the book is published to GitHub Pages from `main`. Five subchecks hold
  it. What this does **not** cover:
  - **Nothing watches the published site between deploys.** The deploy job
    checks its own artifact contains the ADRs; that is a control on the build,
    not a monitor on the site.
  - **Five mdBook 0.5 warnings remain**, all placeholder angle brackets in
    page content read as unclosed HTML tags. They are recorded in
    `docs/MDBOOK.lock` as an expected baseline rather than fixed, because that
    line moved documents and did not edit them.
  - **A page can still be wrong.** These instruments check that a page is
    reachable, is not a pointer, and says whether it is maintained. None of
    them reads it.

- **Fourteen crates' unit tests did not run in CI** (Errata **E-049**, **CLOSED**
  2026-09-25 by RFC-0.33-004, with a CI run owed). CI named test packages in
  hand-written `-p` lists (101 entries when re-measured), and fourteen lib crates
  appeared in none of them — the ten the entry first named, plus two the previous
  line had added; **190 tests in 17 crates** ran in no CI job. CI's lib tests are now
  one workspace-derived job (`cargo xtask host-lib-tests`), the same command
  `test-all` and the release checklist run, so a crate is tested the day it is
  added; a consistency subcheck (`ci-test-jobs`) refuses a hand-written package
  list on a `cargo test` command, and `fjell-ci-coverage` is deleted. **Survivors:**
  the `cargo check` lists are still hand-written; `fjell-sxt-crypto`'s guard failure
  path is exercised by nothing; and **the new job has not yet run on CI** — nothing
  is pushed.

- **A/B boot confirmation and rollback** (Errata **E-044**, **CLOSED** by
  RFC-0.33-001). ADR-0009's state machine now has a runtime: `bootctl` owns the
  boot-control block, `service-manager` reports health, and a reboot syscall is
  dispatched with a capability check. What it does **not** do — no reset from a
  health decision, four syscalls undispatched, nothing durable, a second boot no
  tier observes — is the *A health failure cannot reach a reset* item above.

- **The frozen wire-format schemas were neither frozen nor accurate** (Errata
  **E-045**, **CLOSED** 2026-09-24 by RFC-0.33-003, with survivors). The eleven
  `.frozen` files were hand-written, checked by a CI job that only confirmed they
  were non-empty, and — measured for all eleven — **three agreed with the code, one
  differed by spelling and seven described something other than what is hashed**.
  They are now **generated** by `cargo xtask schema dump` from the same functions
  that produce the bytes, compared with the code in Gate 1 (the comparison names
  the field that drifted), and there are twenty (seventeen when this was closed;
  the semantic wire codec's was the eighteenth, RFC-0.34-003; the measurement
  chain and bundle digests are the nineteenth and twentieth, RFC-0.35-004, closing
  E-065). **What this does and does not
  guarantee:** a change to a covered format's field names, types, widths, order or
  group capacity cannot be committed without its file changing; it does *not* see
  the *values* of ordinary fields (the golden digests hold those for the formats
  they cover, and the three disk structures' own tests
  hold theirs), and a version bump is something the diff makes visible to a
  reviewer, not something a check enforces. **Survivors — Erratum E-073
  (unscheduled):** two format crates produce bytes and still have no generated
  description — `fjell-audit-format` and `fjell-net-format`'s `#[repr(C)]`
  layouts, closing which needs the kernel's own `sys_audit_drain` to move.

- **A fleet roster's digest covered only its first eight members** (Errata
  **E-066**, **CLOSED** 2026-09-24, found and fixed inside RFC-0.33-003). The
  digest stream was built in a 512-byte buffer by a writer that truncates silently,
  so two rosters differing only in their ninth member had the same digest.
  Nothing in the tree builds a roster of more than a few members, so it was never
  live; digests of eight members or fewer, and of every policy, are unchanged.

- **Two code paths treated Rust structs as raw bytes unsoundly** (Errata
  **E-046**, **CLOSED** 2026-09-16 by RFC-0.32-002). The semantic-stream and
  text-proxy services rebuilt a message from bytes another service sent by
  reinterpreting them directly as a Rust type containing an enum, so a
  malformed message from a buggy or compromised sender was undefined behaviour
  rather than a rejected input — Miri named it at
  `.correlation_id.<enum-tag>`. Separately, the boot-control and
  store-superblock checksums were computed over struct memory including
  padding.

  **Corrected.** The envelope now crosses the boundary as a versioned wire
  format decoded by safe code that returns `Result`, both receive loops check
  the length `BEGIN` declared instead of discarding it, semantic-stream
  re-encodes what it forwards rather than passing received bytes through, and
  both checksums are computed over an explicit field serialisation. What this
  does **not** cover, stated plainly:
  - **No QEMU tier drives a malformed envelope.** The `semantic` negative
    profile exercises the live path end to end, not a hostile sender; the five
    framing refusals and the decoder's refusals are host tests.
  - **The kernel's four raw-reinterpretation sites are unchanged** — a
    deliberate non-goal of that line, and a separate boundary (E-044).
  - **The old checksum bytes are not readable.** Nothing has ever read either
    block from disk (E-044), so the version constants moved to 2 rather than
    the encoder preserving a format with no reader. A disk written by an
    earlier build would now read as invalid.

- **The release cut used to be the only work in this project nobody reviews**
  (Errata **E-039**, **CLOSED** at 0.30.0). The cycle's Roles table makes the implementer
  Responsible for verifying exit criteria and producing the release record, with
  the architect Accountable and Consulted. In practice the architect did all of
  it for five consecutive releases (`0.25.0`-`0.29.0`), so those changes landed
  unreviewed — and they included a four-commit staging failure, a
  `rfcs/accepted/` directory that vanished from fresh clones, and document
  corrections nobody checked.

  **Corrected at 0.30.0.** The cause was that the cut had no handoff while every
  RFC has one; `docs/src/releasing/release-handoff.md` is now the standing handoff
  for every cut, and the Roles table's undefined `A`/`R`/`C`/`I` legend is
  stated. The 0.30.0 cut was the first executed by the implementer from that
  handoff and reviewed by the architect like any other line. Exit criterion 8 —
  reading this document, the standards mapping and the threat model against a
  release's real changes — stays with the architect, because a resolving path
  is not a true row and no gate can tell the difference; at this first reviewed
  cut it found the threat model's T20 row still saying the toolchain was
  "not yet recorded" two days after RFC-0.30-003 had recorded it. The cut also
  found that the written order ran exit criterion 6 before the release record
  existed, so a refusal the record's own commit causes could not be seen —
  fixed in the handoff.

- **QEMU negative-test coverage status (v0.19/v0.20).** The nine main
  negative categories now run real QEMU profiles with fail-closed marker
  checking (a wrong error, an unexpected success, or a panic in the serial
  log fails the run). All nine now have every specified marker confirmed
  (capability 8, mmio 3, dma 3, audit 1, user-copy 2, policy 4, harness 1,
  **svc 4/4 — Errata E-024/E-031, closed by RFC-0.28-001**, evidence:
  [`tests/evidence/RFC-0.28-001/svc-ready-accepted-unauthorized-rejected.log`](https://github.com/nabbisen/fjell-os/blob/main/tests/evidence/RFC-0.28-001/svc-ready-accepted-unauthorized-rejected.log));
  the ipc profile is restored to 3/3 in v0.20.0 after fixing the IPC words ABI and the
  reply-edge cancellation path.

  > **Amended 2026-10-17, exit criterion 8 of the 0.36.0 cut: `svc` is 5/5, not
  > 4/4.** The counts above are the **v0.19/v0.20** ones this bullet records, and
  > the word *"now"* in front of them stopped being true when RFC-0.36-001 added
  > `NEG:SVC:FORMERLY_SILENT_READY_OBSERVED:PASS` — `tests/qemu/profiles/svc.toml`
  > lists **five** `expected_markers` and the tier reports
  > `PASS (5 marker(s) matched)`. The historical figures keep their text, as this
  > bullet's heading says they are v0.19/v0.20's; what is corrected is the present
  > tense. **Reported by the implementer's own sweep** under §6 of the cut handoff,
  > as a candidate and without editing — the boundary that sweep was given, and it
  > held. The `store` and `upgrade` negative profiles
  exist as marker specifications but have **no emitting scenarios yet** and
  are explicitly **not v1 release-gated**; running them manually fails
  honestly rather than placeholder-passing.

  > **Amended 2026-10-15, exit criterion 8 of the 0.35.0 cut: `upgrade` is no
  > longer one of them.** RFC-0.35-001 gave its four markers a real emitting
  > scenario — each asserted at the deciding service's own output (`verifyd`'s
  > `VERIFY_FAIL` arm, `bootctl`'s active-slot refusal and its `NoFallback` health
  > verdict) as well as by `neg-test`'s own independent read — and flipped
  > `release_gated` to `true`. **`store` is now the only tier in the project that is
  > not release-gated**, and its four markers are the remaining four of the eight:
  > closing them needs a store that can *reject*, because `fjell-storaged` has no
  > CRC, superblock, mirror or recovery code to exercise (E-065's own amendment
  > records why that is a feature line and not a test line).

- **The measurement chain is fed placeholder literals on every boot, because
  nothing measures anything** (Erratum **E-086**, ~~ACCEPTED 2026-10-19,
  unscheduled~~ **CLOSED 2026-10-24** by **RFC-0.38-001**, with survivors in
  row 14). **Closed:** the kernel now hashes the 33 service images it embeds and
  the chain's first event records that digest; the `measurement` tier requires it
  to equal a digest the host recomputes independently. **Three of the chain's four
  recorded events are still placeholders, and only the service images are
  measured — row 14.** *What follows is the entry as it stood before that line,
  kept as the record; its present tense is not the present.* **This one is live,
  not latent.** `fjell-init` appends four
  measurement events per boot whose "subject digest" is a literal — `0x0011` for
  boot evidence, `0xAB` for each of three verification results — with the remaining
  words zero, and `measuredd` rebuilds only `sd[0..16]`, leaving the rest zero. Each
  recorded measurement is a magic number followed by twenty-four zero bytes.
  **Nothing computes a real digest**: `init`'s other digest fields are
  `Digest32([0xBB; 32])` and `[0xCC; 32]`, and the kernel embeds every service
  binary with `include_bytes!` without ever hashing one. The chain is the node's
  attestation root; `attestd` takes its provider measurement from that head and a
  diagnostic bundle would report it in a **signed** artefact. Every step is
  faithful and the value is a constant. **This also fixes E-085's ordering**:
  closing that entry alone would transmit the complete placeholder and set
  `measurement_head_available: true` on it, which is worse than today's honest
  `false`. Closing E-086 means **something must measure something** — the obvious
  candidate being the kernel hashing the images it already embeds — which is a
  feature, not a wiring fix.
- **`measuredd`'s `EXPORT_LOG` exports nothing** (Erratum **E-087**, ACCEPTED
  2026-10-19, unscheduled). The handler replies `EXPORT_CHUNK` with a sequence
  number and zeros, then `EXPORT_DONE` with zeros: no loop, no chunk sequence, no
  log content, and no caller has ever existed. **E-085's own resolution — and the
  bullet below — cited this as the working chunking pattern to follow**, from the
  constants' existence in the shared API without reading the handler. The genuinely
  working pattern is `fjell-storaged`'s `WRITE_BEGIN`/`WRITE_CHUNK`/`WRITE_COMMIT`,
  which `fjell-service-api`'s own ABDD note already names as the model. Corrected
  below.
- **`fjell-measuredd`'s `GET_HEAD` has never been able to answer with the chain
  digest it names** (Erratum **E-085**, ~~ACCEPTED 2026-10-18, unscheduled~~
  **CLOSED 2026-10-24** by **RFC-0.38-001**). **Closed:** `GET_HEAD` now replies
  with the whole 32-byte digest in four words, and the caller (`diagnosticsd`) makes
  a real call and reads all four — demonstrated with a temporary capability, since
  reverted. `measurement_head_available` still reads `false`, now only because slot 4
  stays uninstalled by the owner's scoping (the next milestone). *What follows is the
  entry as it stood before that line.* Its
  handler replies `reply(HEAD_REPLY, latest_seq, cd_lo, dropped)` where `cd_lo` is
  `chain_digest.0[0..8]` — **eight of thirty-two bytes** — in a three-word `reply`,
  one word of which carries digest content. **The protocol cannot return a chain
  digest, for any consumer.** `GET_HEAD` has two occurrences tree-wide, its
  declaration and that arm, so no caller has ever existed to notice. **The trap**:
  fixing E-083's tags alone would make a real reply arrive and land `latest_seq` in
  a `Digest32`'s first eight bytes, reported `measurement_head_available: true` —
  passing both RFC-0.36-001's zeroing test and RFC-0.37-001's trap 3. **Ruled: the
  digest travels whole or not at all.** A partial digest is refused (a `Digest32`
  has no partial presence), and so is squeezing 32 bytes into four words by dropping
  `latest_seq` or `dropped`. The mechanism this project already uses for oversized
  payloads is chunking — ~~`measuredd`'s own
  `EXPORT_LOG`/`EXPORT_CHUNK`/`EXPORT_DONE`~~ **`fjell-storaged`'s
  `WRITE_BEGIN`/`WRITE_CHUNK`/`WRITE_COMMIT`** *(corrected 2026-10-19: the
  `measuredd` export named here is a stub that sends no content — **E-087** — and in
  any case no chunking is needed, because the kernel's `sys_ipc_reply` copies four
  words, exactly one `Digest32`)*.
  Until then **`measurement_head_available` stays `false`**, which is correct and is
  what that flag was built for.
- **`fjell-diagnosticsd` invented tag numbers for a conversation the server already
  spoke, and two of them were already taken** — Erratum **E-083**, **CLOSED** by
  **RFC-0.37-001**. `fjell-measuredd` serves the shared `measuredd::GET_HEAD`
  (`0x303`) and replies `HEAD_REPLY` (`0x304`), with a real head digest.
  `fjell-diagnosticsd` used to send its own locally declared `MEASUREDD_HEAD_QUERY`
  (`0x320`), expecting `0x321` — so the query would have matched no arm **even with
  the capability installed**, demonstrated failing exactly that way before the fix.
  And `0x320`/`0x321` were already `recoveryd::READY`/`LIST_SNAPSHOTS` in the shared
  API. Five `SXT_*` constants in the same file were hand-copied from
  `secure-transportd` too — the shape RFC-0.36-001 removed for `verifyd`.
  `diagnosticsd` now reads the shared `measuredd::{GET_HEAD, HEAD_REPLY}` and
  `sxt::{OPEN_CHANNEL, OPENED, DIAG_PUSH, DIAG_ACK, CLOSE}`, the latter the single
  declaration `secure-transportd`'s own `SxtTag` enum now also reads. **Whoever
  scheduled "install slot 4 and get a real measurement head" was this very line,
  and it did not close E-083 and stop there**: it found, before installing
  anything, that the fix alone would have landed a sequence number in a `Digest32`
  and reported it available (**E-085**, filed and ruled) — slot 4 stays
  uninstalled, and `measurement_head_available` reads `false` for that reason now,
  not E-083's.
- **Two services' event loops did not run — one was a comment, one was
  unreachable.** **`fjell-fleetd` had no event loop at all** — Erratum **E-081**,
  **CLOSED** by **RFC-0.37-001**: it ran format self-checks, printed `fleetd:
  ready`, and called `sys_exit(0)` directly beneath the section header
  `// ── IPC event loop (v0.8.1 …) ───`. `loop {` appeared **zero** times in that
  file and it declared no capability slots — the service terminated, and was not
  among the services listed below as exiting by design; it had, in fact, never
  even been cross-compiled as a bootable image (no `_start` symbol, absent from
  the kernel's image table). It now has a real event loop, asking `diagnosticsd`
  for a bundle on a console-byte trigger, demonstrated alive by
  `tests/qemu/profiles/remote-diag.toml`.
  **`fjell-netd` had a complete IPC event loop that could never execute** — Erratum
  **E-084**, split from E-081 on 2026-10-18, **CLOSED** by **RFC-0.37-002**: the
  loop was real, finished code (receiving messages, handling `LinkUp`,
  pre-allocating an update-metadata session for `secure-transportd`), statically
  unreachable after `sys_exit(0)` (`sys_exit` is `-> !`), and hidden from the
  compiler's own `unreachable_code` lint by a crate-level `#![allow(...)]` — see
  E-082, below. Deleting a working implementation would have been a real loss, so
  it was kept, extracted into `run_event_loop`, a free function `service_main`
  never calls; a reader now meets it as `dead_code` the compiler names explicitly
  ("function is never used") rather than as a code path sitting, unremarked, after
  a diverging call. **This is also why E-078 understated itself**, at the time
  E-084 was split out from this entry: that entry called `fleetd` and
  `diagnosticsd` "two stubs facing each other", and one of them had exited, so
  E-078's named asker could not ask because it was not running. **Both are now
  closed** — E-078 by RFC-0.37-001 (above), E-082 and this entry by RFC-0.37-002.
- **Exit criterion 2's "0 warning lines" was partly bought by lint suppression** —
  Erratum **E-082**, **CLOSED** by **RFC-0.37-002**. At `226544d`, **13 crates**
  carried a crate-level `#![allow(...)]`; the suppressed lints included
  **`unreachable_code` in three** — `fjell-netd`, `fjell-driver-virtio-net` and
  `fjell-secure-transportd`. Measured rather than assumed: in `netd` the allow hid
  E-081's (E-084's) unreachable event loop, the lint doing exactly its job with the
  switch off; in the other two it hid **nothing at all** — their loops were
  reachable and their only `sys_exit` was the panic handler's — copied boilerplate
  that would have silently absorbed the next instance. Re-derived at the tip
  (not inherited) and fixed: **9** of the 13 allows suppressed nothing and were
  deleted outright (including `fjell-driver-virtio-net`'s, whose own top comment
  had independently and falsely claimed an early exit "by design" — the same
  copied sentence `fjell-secure-transportd` and `fjell-proxy-text` carried before
  earlier lines corrected it, a third instance); **3** survive, narrowed to
  item/enum-level `#[allow]`s with a stated reason (`fjell-attestd`,
  `fjell-secure-transportd`, and `fjell-storaged` — the last kept crate-level,
  deliberately, for 13 single-purpose register constants scattered through a
  hardware map); and `fjell-netd`'s — the 13th — is E-084, above. Exit criterion 2
  now also reports a count, as a record figure rather than a gate that fails
  (`cargo xtask build` / `cargo xtask lint-allow-report`,
  `crates/fjell-tools/src/lint_allow_report.rs`): **1 crate-level allow survives
  tree-wide** today.
- Several services in the QEMU image are **smoke-test stubs** that signal
  ready and exit by design (`fjell-netd`,
  `fjell-driver-virtio-blk`, `fjell-powerd`, among others). Their full
  implementations are tracked on the post-v1.0 roadmap; their early-exit
  pattern is intentional. **`fjell-secure-transportd` is not one of these**
  (removed from this list by RFC-0.35-001): it never exited early, and since
  that line installed the capability the kernel never had for its
  `upgraded` channel (E-072), it now genuinely exchanges messages rather
  than busy-spinning on an empty slot. What it still does not do is real
  certificate verification — `fjell-sxt-crypto`'s own doc comment — and it
  does not serve `diagnosticsd` or `attestd` (E-069, RFC-0.35-002).
  `fjell-proxy-text` is also not an early-exit stub (RFC-0.34-001 converted
  it to ask-and-park); left in a prior cut's list and not corrected here,
  outside this line's scope. **`fjell-driver-virtio-net` is also not one of
  these** (removed from this list by RFC-0.37-002, E-082): it has no
  `sys_exit(0)` anywhere, and `service_main` reaches a real
  `loop { sys_irq_wait(...) }` after binding its IRQ — the same early-exit
  sentence was copied, verbatim, from `netd`'s own stub comment into this
  crate's top comment and was never true of this code. A third instance of
  the same copied claim, found while re-deriving this crate's lint
  inventory for E-082 rather than trusting the earlier two corrections to
  have caught every copy.
- The repro-check baseline (`tests/repro/baseline-digests.txt`) tracks the
  committed `prebuilt/*.bin` artefacts and must be re-recorded whenever the
  prebuilt service binaries are rebuilt — see `tools/fjell-repro-check`.

- **The console a person reads: what is fixed and what is bounded** (Errata **E-061**,
  **E-062**, **E-063**, all **CLOSED** — 2026-09-25 by RFC-0.34-002, E-062 at its review
  2026-09-29).
  - **Fixed:** the kernel now resolves a task's console line when the task leaves — a
    task that exits or faults mid-line has its last words shown, ending `[cut]`, instead
    of lost; an index outside the console table fails the write instead of aliasing
    another task's buffer; `M6: storaged ready` has one writer, and a harness check
    (`once_markers`) fails if a second appears; a full task table returns
    `SysError::TaskTableFull` and `init` prints `init: cannot spawn image N: <reason>`,
    naming `AlreadyMapped` distinctly from `NoMemory` where the kernel's own mapping
    step is what failed. **What E-062's eight non-printing bytes actually were:** measured
    on an instrumented kernel, the line buffer was empty at all 277 task departures across
    24 tier logs, and no slot is ever reused (`TaskTable::remove` has no caller) — *a dead
    task's bytes in front of the next task's line*, the mechanism this entry first named,
    **has never happened**. The bytes were **`storaged`'s own diagnostic probe writes**
    (`0x90 + device id`, one per virtio slot scanned; two more probes per disk write) —
    debug residue with no reader, not the lifetime defect, and now **deleted**. A tier
    (`semantic.toml`) asserts the console clean — valid UTF-8, no control character other
    than `\n`, `\r`, `\t` — demonstrated against the unmodified `storaged` (fails, an
    invalid byte) and the fixed one (passes).
  - **Bounded — and a reader of a braille presentation through the console needs this:**
    the console line buffer is **160 bytes**. A longer line is **cut**, and says so
    (`[cont]` ends a chunk and begins the next), but it *is* cut. The longest line any
    committed log carries is 153 bytes and the longest braille line 135, so nothing has
    been split yet; a presentation must wrap deliberately, not discover the limit.
    Sizing the buffer from the wire format (4,624 bytes × 40 tasks = 185 KB of kernel
    `.bss`) was declined. `sys_debug_write` is still one byte per `ecall` (D7); marker
    assertions still match substrings.
    *(The earlier text of this entry said every tier asserting the M6 marker passes on
    `init`'s line alone; no specification asserted it, and the search's control was found
    in twelve files, not four.)*
- **The kernel finds and reserves firmware's device tree, and reads nothing else
  from it** (Erratum **E-064**, **CLOSED** by RFC-0.33-001 D22). The boot shim used
  to overwrite the DTB pointer in `a1`, so `kmain` received `__bss_end`, the reserve
  that keeps the tree out of the free pool failed on its first frame with its error
  discarded, and the real page was allocatable. The pointer now survives, the
  header's magic and size are checked before anything is stored or reserved, the
  tree's real extent is reserved (5,044 bytes at `0x87e00000` under QEMU: two
  frames), and a failure is printed. **What survives:** only the header is read.
  `platform::detect` still ignores the tree and returns a hard-coded `qemu-virt`
  profile, and `fjell-dtb-validate`'s full validation is still not wired into boot
  (E-048; hardware bring-up, E-004) — so no path here has run on a real board's
  tree, and this line says nothing about one.
- **Two digest streams that produced bytes and had no generated description**
  was Erratum **E-065**, **CLOSED** by **RFC-0.35-004**. `fjell-measure-format`'s
  `compute_chain_digest` and `fjell-bundle-format`'s `compute_bundle_digest` now
  write through `Canon`, which gained one new method — a big-endian `u32`, with
  the same "default writes what the encoder already wrote" treatment
  RFC-0.34-003's five additions got, so **the existing eighteen descriptions are
  unchanged**. Goldens captured from the unmodified encoders first (a
  metadata-absent and a metadata-present case for the chain; a short and a
  `SERVICE_NAME_MAX`-length name for the bundle) are reproduced byte for byte by
  the moved encoders. The bundle digest's `name` carries no length prefix — the
  argument for why that is safe here (every field after it is fixed-width, so the
  total stream length determines where `name` ends) is written into the generated
  file itself, beside the field, rather than left for a reader to assume. Both
  files say what they do not cover: **nothing reads either stream back**, so the
  file and the encoder are the only description a second implementation has — the
  opposite and stronger version of the caveat every decoded format already states.
  **Two survivors remain, named and typed, not a gap nobody noticed**:
  `fjell-audit-format` and `fjell-net-format` (Erratum **E-073**, unscheduled),
  whose `#[repr(C)]` layouts *are* the description and one of which is written raw
  by the kernel — closing them needs the kernel's own audit write to move, which
  is its own line. The set is **managed, not merely listed**:
  `fjell_schema::registry::EXCLUSIONS` types each crate `InMemoryOnly` or
  `Survivor { erratum, what }`, and `crates/fjell-schema/tests/frozen.rs` fails when
  a formats crate is neither described nor excused, when a survivor names an
  erratum the register does not contain, or when a survivor's probe stops finding a
  producer — so a third undescribed format cannot appear silently. **What none of
  the twenty generated files cover — a reader must not infer more:** each
  describes what its encoder *writes*. A decoded format's reader is a second,
  hand-written reading of the same layout and is not described (a decoder that
  read different fields would not be caught by the file; a round trip per sample
  is the only check), nor are the values behind a tag byte or an encoder's
  refusals; the two digest streams have no decoder at all, which is the stronger
  caveat stated above.
- **An instrument that reports on less than it claims, twice over** was Erratum
  **E-071**, **CLOSED** by **RFC-0.35-003** (filed 2026-10-13 at RFC-0.34-004's
  review). `fjell-repro-check` used to **silently ignore an unrecognised flag**
  and fall through to its full two-build mode — so the natural way to ask for
  usage, `--help`, started a build that could race a concurrent `test-all`. It had
  happened twice. **Demonstrated both ways**, in a scratch tree: the unmodified
  tool given `--help` runs two real builds and reports `PASS (33 artefacts
  identical)`, never printing a usage line; the fixed tool given the same flag
  prints usage and exits 0. An unrecognised argument now exits non-zero with the
  usage; neither it nor `--help` ever builds. The sweep this line's R4 required
  (`crates/fjell-tools/` and `tools/`, for the same shape — no `--help`, no
  rejection) found and fixed two more: `fjell-unsafe-audit`, `fjell-mmio-audit`.
  **Fixing `fjell-unsafe-audit` surfaced a real bug its old silence had hidden**:
  four call sites (`release-rehearsal` Gate 2, `test-all` tier 3, the trust
  report's §5, CI's `ci-unsafe-audit` job) and three documentation pages all
  passed `--workspace .` — `fjell-mmio-audit`'s flag, not this tool's own
  (`--root`) — silently accepted and ignored, harmless only because the ignored
  value happened to equal `root`'s own default either way. All corrected to
  `--root`, each command run for real before being written down.

  **The second half of this entry — the readiness-summary gap — had no erratum
  number of its own** (the governing RFC's own words: found and corrected by hand
  twice, nothing checked it). `docs/src/releasing/v1-readiness.md`'s own *"Summary
  at …"* block disagreed with its own rows in three ways on 2026-10-13, and the
  correction note itself went stale by 2026-10-14 — **twice, with Gate 5 green
  throughout**, because that gate only ever checks `open == 0` and never compares
  the document's claims with its contents. `readiness-summary`, Gate 12's 20th
  subcheck, is the comparison: the table's four counts against what the rows
  actually produce, the prose against the table, and the heading's version against
  the workspace's. It **fails** against a fixture reconstructing the 2026-10-12
  disagreement and **passes** against the tree. `fjell-upgraded`'s update-index
  fetch and the two request paths formerly listed here (`diagnosticsd`'s audit
  query, `secure-transportd`'s claimed round-robin) are **closed** — see below.
- **A benchmark flag that fell open to "just print" instead of its named mode**
  was Erratum **E-077**, **CLOSED** by **RFC-0.36-002** (filed 2026-10-14
  during RFC-0.35-003's own sweep for E-071's shape; a different defect from
  E-071's own — silent loss of the caller's intent, not a wrong *expensive*
  mode, since every path already ran the one genuinely slow step — named
  rather than repaired on that line, per its own "report, fix the same
  defect, file the rest"). A typo'd or misremembered `--baseline`/`--check`
  matched neither flag and `cmd_bench` silently ran the cheapest default
  (print only): a caller who meant to write a baseline or check tolerance
  believed one had happened when neither did. **Demonstrated both ways**:
  the unmodified tool given `--basline` ran the full benches and exited 0;
  the fixed tool given the same flag exits **2** with its usage, confirmed
  at the shell (`$? == 2`) rather than only from the function's own return
  value — `cmd_bench` returns its `ExitCode` to `fjell-tools`' own
  dispatcher, unlike the three standalone-binary precedents, and that
  dispatcher was shown to preserve it. `--help`/`-h` print the usage and
  exit 0 without running a benchmark; every flag it names exists. `1` is
  unchanged: a metric exceeding tolerance or a failed baseline write — the
  fourth application of E-071's remedy, with the three precedents' own
  exit-code mistake (`1` vs `2`) not repeated.
- **Two protocols retracted, not answered** were Errata **E-068** and **E-069**,
  both **CLOSED** by **RFC-0.35-002**. `diagnosticsd`'s `AUDIT_QUERY` sub-protocol
  (three constants in `fjell-service-api`, `collect_audit_events`, the declared
  `CAP_AUDITD_EP`) is deleted outright — `fjell-auditd` never implemented it, and
  the wait for a reply that could never come did not merely return nothing: it
  **hung the service** (E-068's consequence, corrected in its own entry above;
  demonstrated in a scratch build, shown gone after). `secure-transportd`'s
  declared endpoints for `diagnosticsd` and `attestd`, and the comment claiming a
  round-robin over them, are deleted the same way (E-069). Neither is implemented
  under a different name: a `DiagnosticBundle` now carries `audit_available: bool`
  so a caller can tell "no records" from "not implemented," and `v1-limitations.md`
  — here — is where the capability question implementing the query for real would
  require (who may read the audit log, which right, revocable how) is named for
  whoever takes that line next.
- **`fjell-upgraded`'s update-index fetch has never completed** was Erratum
  **E-070**, **CLOSED** by **RFC-0.35-001**. The uncarried channel-kind word was a
  symptom, not the cause: the capability `secure-transportd` received on did not
  exist until E-072 (below) closed, so no word — carried or not — was ever going
  anywhere. With the capability installed and both the channel-kind and channel-id
  words now carried for real (`sys_ipc_send_words`), the fetch reaches a real,
  listening `secure-transportd`, which **correctly fails the handshake closed**
  (no simulated trust — v0.7.3's real certificate verification is still a stub) —
  `fetch_update_index` still returns `None`, now for that honest, already-documented
  reason, not a silent IPC failure. The `upgrade` QEMU tier is release-gated as of
  this line, with a real emitting scenario; `store`'s is a separate, still-open
  candidate, because `fjell-storaged` has no CRC, superblock, mirror or recovery
  code to exercise and that tier's four markers need a store that can reject, not a
  test.
- **Five services address peers through CSpace slots the kernel never installs, and
  two of them busy-spin on the absence for the lifetime of every boot** was Erratum
  **E-072**, **CLOSED** by **RFC-0.35-001**, with **`fjell-attestd` and `fjell-netd`
  named as survivors, re-derived individually rather than assumed** — neither
  busy-spins. `fjell-attestd`'s declared slot 5 (`CAP_SXT_EP`) is addressed only by
  `push_attestation`, which has **no caller anywhere in the tree**: a dead `pub fn`,
  not a live loop, so the missing capability's error path is never reached at
  runtime and `attestd`'s own main loop receives on slot 0 instead. `fjell-netd`'s
  declared slot 1 (`CAP_DRV_EP`) has **no reference outside its own declaration**
  — `recv_msg` hardcodes slot 0, never this constant. Both are unreachable dead
  declarations, a materially better outcome than the two services below, named
  rather than rounded in. `fjell-secure-transportd`,
  `fjell-diagnosticsd`, `fjell-upgraded`, `fjell-attestd` and `fjell-netd` declared
  named capability slots — eleven of them — and `crates/fjell-kernel/src/task/spawn.rs`
  installed, for every image, only slot 0 and `SERVICE_READY_SEND_SLOT`; its
  per-image install blocks covered nine other images and **none of these five**.
  `sys_ipc_recv` returned the capability error immediately rather than blocking,
  both services mapped it to a zero tag that was in neither protocol's vocabulary,
  and both loops re-entered at once: **`secure-transportd` and `fjell-diagnosticsd`
  spun**, each under a comment describing the work it was doing, and 27 recorded
  boots showed `secure-transportd` printing `ready` and then none of its three
  per-outcome lines, ever (re-derived live for this line: 4.38 million further
  iterations, still none). This line installs the two slots the update path needs
  — `secure-transportd`'s slot 2 and `upgraded`'s slot 6, onto one shared endpoint —
  and left the other nine declared-but-uninstalled slots to RFC-0.35-002, **named,
  not silently dropped**, by the new `capability-topology` consistency-check gate,
  which compares the two independent declarations this erratum found and fails on
  any non-waived divergence — **of the shape every declaration in this tree
  takes**: a `const` whose slot is a decimal literal, in any `.rs` file under a
  service's `src/`. A slot whose number is a named constant, or a binding that is
  not such a `const`, is **not** discovered; the check says so in its own module
  doc rather than implying a reach it does not have (architect probes, 2026-10-14 —
  the `channel.rs` case was a real miss and is fixed, the constant-expression case
  is a stated limit of a text scanner). **RFC-0.35-002 retired five of the nine**
  by deleting the declaration outright, closing E-068/E-069/E-074 (below): the gate
  now reports `2 installed, 4 waived, 6 checked`, not eleven. The four still
  waived — `secure-transportd`'s smgr-readiness slot, `diagnosticsd`'s tools,
  secure-transportd and measuredd slots — remain RFC-0.35-002's own residual
  decisions, each still named with its reason. A recv error is also no longer a
  tight loop in either service, independent of which slot caused it —
  `fjell-diagnosticsd`'s own `CAP_TOOLS_EP` remains unfixed (still RFC-0.35-002's
  decision not to install it; this line's own non-goals forbid it too) and will
  keep erroring on every boot, but it now yields and discloses once, rather than
  spinning or erroring silently (RFC-0.35-002 R5). Two readiness notifications were
  still wrong — one sends into an empty slot, one into the service's **own** shared
  endpoint instead of `service-manager`'s — untouched by either line. *(Closed by
  RFC-0.36-001, 2026-10-15, below under E-075: both now send through
  `SERVICE_READY_SEND_SLOT`, `secure-transportd`'s smgr-readiness waiver is retired
  with its declaration, and the gate reports `2 installed, 3 waived, 5 checked`.)*
- **Two services announced their readiness to nobody, each under a comment saying
  otherwise** was Erratum **E-075**, **CLOSED** by **RFC-0.36-001** (carried out of
  E-072 at its closure, where it was named in the body and then left behind).
  `fjell-secure-transportd` sent readiness on **slot 1**, which nothing installs, and
  `fjell-diagnosticsd` sent it on **slot 0**, its **own** identity endpoint on the
  shared object — neither ever reached `service-manager`. Both now send through
  `fjell_service_api::ready::SERVICE_READY_SEND_SLOT`, matching the nine services
  that already did; their own `CAP_SMGR_EP` constants are gone, and
  `capability-topology`'s `secure-transportd` smgr-readiness waiver is retired with
  the declaration it named. The harder half — something asserting `service-manager`
  actually observes them — is a new per-service observable,
  `NEG:SVC:FORMERLY_SILENT_READY_OBSERVED:PASS`, set only once `service-manager`'s
  own endpoint has received `SERVICE_READY` from **both** services, asserted in
  `tests/qemu/profiles/svc.toml` and confirmed live. `READY_ACCEPTED_THRESHOLD`
  raised **8 → 10** in the same commit, since the true count of images sending
  `tags::SERVICE_READY` is now 10 and leaving the threshold at 8 would itself have
  become a stale claim.
- **`fjell-diagnosticsd` could not be asked for anything, and every path inside it
  was dead** was Erratum **E-078**, **CLOSED** by **RFC-0.37-001**. It could not
  receive — `CAP_TOOLS_EP` (slot 1) was declared but `spawn.rs` installed it for no
  image, so its loop errored every iteration and yielded. **Nothing could ask it**:
  `BUILD_BUNDLE` and `PUSH` each had exactly two sites tree-wide, the declaration
  and the service's own comparison. It could not reach `measuredd` (**E-076**,
  closed earlier — the slot stayed declared, not installed) or push (slot 2,
  failing closed and replying `PUSH_FAULT`, which was honest). **This was the
  premise E-075 and E-076 were latent because of**. The asker specified by
  `RFC-v0.8-005` — `fjell-fleetd`, the node-side endpoint — turned out not to be
  running at all: no event loop (**E-081**), and, found while wiring this line, no
  `_start` symbol in its own runtime and absent from the kernel's image table and
  build list — it had never been cross-compiled and linked as a bootable image.
  `fleetd` now has a real event loop that asks `diagnosticsd` for a bundle on a
  console-byte trigger (not a real `RemoteDiagRequest` over the network — out of
  scope), `diagnosticsd`'s `CAP_TOOLS_EP` is installed, and the whole round trip is
  demonstrated live (`tests/qemu/profiles/remote-diag.toml`): `diagnosticsd` prints
  each of the bundle's four availability flags by name, all `false`, each for a
  stated reason. **No threat-model entry was needed**: `push_bundle` still
  addresses slot 2, still uninstalled, so no bundle leaves the node — the export
  boundary this entry anticipated does not yet exist to carry one.
- **The diagnostic bundle's signed digest did not cover two fields its own
  specification lists** was Erratum **E-080**, **CLOSED** by **RFC-0.36-001**.
  `RFC-v0.4-005` §6.4's formula includes `provider_id u32 LE ||
  keyring_anchor_epoch u32 LE`; `DiagnosticBundle::write_canonical` wrote
  **neither**, though both exist as struct fields — the trust provider a signed
  bundle claims to come from, and the key epoch it claims to be anchored at, were
  **outside the signature** and could be altered without invalidating it. The test
  §8 names, `bundle_digest_covers_all_fields`, now exists and was demonstrated both
  ways: stashed against the unmodified encoder, two bundles differing only in
  `provider_id` produced identical digests (the bug, reproduced); restored, the
  same test passes. `write_canonical` now writes both fields, in the
  specification's own order, right after `created_tick` and before
  `measurement_head`; `bundle-v1.frozen` regenerated, header reading `v3 wire`.
  Compounded with **E-079** (also closed): `keyring_anchor_epoch` was both a
  fabricated literal and unauthenticated — E-079 removed the fabrication by
  deleting the literal's only path in; this entry closes the authentication half
  for whatever value the field legitimately carries once one exists.
- **The same bundle carried an attestation digest, a key epoch, and a semantic
  intent — one never queried, one never queried, one invented** was Erratum
  **E-079**, **CLOSED** by **RFC-0.36-001**. Found by doing the sweep **E-076**'s own
  entry called for, and widened once more during implementation (escalated, then
  ruled by the architect): `build_bundle` passed `Digest32([0u8; 32])` as
  `last_attestation` and the literal `1` as `keyring_anchor_epoch` — both **literal,
  never queried at all**, where the measurement head at least asked and failed. And
  it called `add_intent` with literal arguments, so the bundle's semantic-intents
  section was **one hardcoded record asserting an update staging was confirmed two
  seconds ago**, whether or not anything of the kind happened. **The intent was a
  worse class than the two literals**: absence dressed as data can be spotted by a
  reader who knows an all-zero digest is suspicious, but this was **fabricated**
  data — well-formed, plausible, and inside an artefact the builder **signs**. All
  three were latent only because of **E-078**, closed by RFC-0.37-001 above. The fix: a
  permanently-unavailable flag for the epoch and the attestation digest (neither
  has ever had a real source, so `BundleBuilder::new` no longer accepts either as a
  parameter — a literal cannot be fabricated into a constructor that does not take
  one), and outright **deletion** for the intent — a bundle with no intents is
  honest and one with an invented intent is not.
- **A diagnostic bundle reported a measurement head of zeros when it could not ask
  for one** was Erratum **E-076**, **CLOSED** by **RFC-0.36-001**.
  `fjell-diagnosticsd`'s `query_measurement_head` addresses **slot 4, which nothing
  installs**, and used to return `Digest32([0u8; 32])` on failure — thirty-two zero
  bytes in the field that attests what the system measured, **indistinguishable
  from a real digest**, with no flag saying the value was never obtained. This was
  the mechanism E-074's entry predicted one line before it closed. `DiagnosticBundle`
  now carries `measurement_head_available`, adjacent to the field it qualifies, set
  `true` only by `query_measurement_head` itself when a real `MEASUREDD_HEAD_REPLY`
  arrived — the code that obtains the value is the only code that may say so —
  and `write_canonical` zeroes the field in the digest whenever the flag is `0`,
  regardless of what the struct holds. `DIAG_BUNDLE_VERSION` bumped 2 → 3;
  `bundle-v1.frozen` regenerated; `golden.rs`'s stored digest updated with a note
  citing the bump. `push_bundle` still shares the shape on slot 2 but fails
  **closed**, replying `PUSH_FAULT`, which was already honest. The sweep this entry
  asked for ("the same question applies to every other field filled from a slot
  nobody installs") is what found E-079 and, through it, E-080.
- **Two capability-addressing functions that no caller reaches** was Erratum
  **E-074**, **CLOSED** by **RFC-0.35-002** (split from E-072 at its closure, so a
  closed entry does not carry live residuals). The two survivors above were a
  **different defect with a different remedy**: E-072 was *declared but not
  installed*, fixed by installing or waiving; this was *built and never wired to a
  caller*, fixed by calling it or deleting it. Retracted, as ruled: `attestd`'s
  whole "Remote attestation push" section (`push_attestation`, `CAP_SXT_EP`, the
  SXT helpers only it called) and `netd`'s `CAP_DRV_EP` are deleted. Found in
  passing: `push_attestation`'s own write to a `static mut` nonce cache would have
  page-faulted the moment it ran — a service's image is mapped `R | X | U`, never
  `W` — a second, previously-latent defect this deletion removes rather than needing
  to fix.
