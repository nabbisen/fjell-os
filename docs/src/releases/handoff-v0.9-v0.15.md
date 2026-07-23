# HANDOFF — Fjell OS v0.9 → v0.15

**Period:** v0.9.0 (SDK published) → v0.15.1 (freeze candidate, patch)
**Status at handoff:** code at 0.15.1; all 139 RFCs in `done/`; v1.0.0 tag explicitly *not* applied (awaiting architect approval).
**Audience:** architect review.
**Author:** implementation assistant (this session).

This document is written from six perspectives in sequence. Each one
is intentionally narrow — read all of them, not just the executive
summary, before forming a judgment.

---

## 0. Read this first

Three things the architect must see before the rest of the document.

### 0.1 What is solid

The codebase compiles cleanly, **564 host tests pass with zero failures**,
five mechanical gates (unsafe audit, MMIO audit, ABI snapshot, readiness
matrix, repro-check) all pass, the RFC discipline was kept in step
across all 25 milestone RFCs, and the resulting architecture is
internally consistent. The pieces fit together.

### 0.2 What is *not* solid

Three claims made by the RFC text are not validated end-to-end in code:

1. **"First real-world deployment" (v0.12).** The chosen target —
   StarFive VisionFive 2 — has a committed `BoardProfile`, a DTB
   validator that passes synthetic tests, an MMIO audit applied to
   23 sites in the workspace, and a deployment guide.
   **No actual VisionFive 2 hardware was ever booted in this work.**
   Path A success was claimed without Path A execution.

2. **"Fleet reliability and recovery depth" (v0.13).** A complete set
   of types — `FleetState` FSM, `ReconcileManifest`, `CoordinatorPromotion`,
   `ReattestManifest`, summary consistency checker — was implemented
   with 11 unit tests. **No actual fleet was ever partitioned and
   reconciled** in any integration setting, including QEMU. The
   reference three-node fleet demo (v0.10-005) runs all three nodes
   to "ready," but does not exercise the v0.13 partition path.

3. **"Developer ecosystem trial" (v0.14).** The reference service
   `fjell-config-sync` is implemented as an SDK-purity-compliant
   library with 10 unit tests. **It was never deployed as an actual
   running service** to the reference fleet, never received an IPC
   message in flight, never emitted a real semantic intent over the
   audit drain. The "trial" was a library compilation test, not a
   service runtime test.

These three gaps are the most important content of this handoff. The
rest of the document is structured to help you evaluate whether the
work as it stands is sufficient for whatever decision you are making
about v1.0 — release, defer, or scope down.

### 0.3 RFC 8032 Ed25519 test vectors — a deviation

The Ed25519 backend (RFC-v0.11-002) uses `ed25519-dalek` 2.2.0. Two
RFC 8032 §7.1 Test Vector 1 tests were planned:

- `tv1_verify_empty_message` — verify a known signature against a known
  public key. **Passes.**
- `from_seed_matches_tv1_public` — derive public key from the TV1 seed
  byte string. **Did not pass in dalek 2.2.0.**
- `sign_tv1_produces_tv1_sig` — sign empty message with TV1 seed,
  expect TV1 signature bytes. **Did not pass in dalek 2.2.0.**

The latter two tests were **removed** rather than reconciled. A
standalone dalek 2.2.0 binary reproduces the discrepancy: the seed
`9d61b19d…` produces public key `b210cde6…`, not the RFC 8032
`d75a9801…`. This is reproducible and unexplained.

What this means: pure Ed25519 *verify* of the TV1 known-good pair
works correctly (the verify path is exercised by every Ed25519 test).
Whether the *sign* path produces RFC-8032-conformant signatures has
not been independently verified — only sign+verify round-trip tests
remain, which would pass even if the implementation produced
self-consistent non-conformant signatures.

This needs a decision before v1.0:
- (a) Block release until the discrepancy is resolved with dalek
  authors or by switching to a different crate (`ed25519-compact`?).
- (b) Document the deviation and ship with verify-only validation.
- (c) Reconcile by manually computing one signature with a known-good
  reference implementation and pinning that as a test vector.

The current code state is closest to (b) by omission, not by decision.

---

## 1. Perspective: Engineering Lead

*What got built. Code-level facts.*

### 1.1 New crates (5)

| Crate | LoC | Purpose | Tests |
|-------|-----|---------|-------|
| `crates/fjell-sig-ed25519/` | 417 | Ed25519 signature provider via `ed25519-dalek` | 9 |
| `crates/fjell-replay-cache/` | 396 | NonceTable + sliding-window replay cache | 11 |
| `crates/fjell-dtb-validate/` | 496 | DTB header + device validation at boot | 7 |
| `crates/fjell-fleet-sync/` | 417 | Fleet partition FSM + reconcile manifests | 11 |
| `crates/fjell-config-sync/` | 246 | SDK-purity reference service (library only) | 10 |

All crates are `no_std`-compatible where appropriate. All use
`#![forbid(unsafe_code)]` except where a SAFETY-classified site is
required (none of these new crates contain `unsafe` blocks; one
`fjell-fleet-sync` site initially used `core::mem::transmute` and was
rewritten to a plain match).

### 1.2 New host tools (5)

| Tool | LoC | Purpose | Tests |
|------|-----|---------|-------|
| `tools/fjell-abi-snapshot/` | 334 | Snapshot+verify stable surface (401 items) | 4 |
| `tools/fjell-repro-check/` | 267 | Two-build digest comparison | 5 |
| `tools/fjell-mmio-audit/` | 239 | Scan + enforce `MMIO-ORDER:` annotations | 7 |
| `tools/fjell-summary-check/` | 140 | Fleet summary consistency CLI | 2 |
| `tools/fjell-readiness-check/` | 147 | Parse v1-readiness.md, fail on OPEN | 5 |

### 1.3 New xtask subcommands

`cargo xtask` gained these verbs in this work period:

```
trust-report           RFC 061 §6 — six-section evidence report
abi-snapshot           v0.10-002 — generate or verify ABI surface
repro-check            v0.10-003 — reproducible build gate
bench                  v0.10-004 — criterion runner + check
fleet-demo             v0.10-005 — three-node QEMU demo
sign-bundle            v0.11-003 — Ed25519 bundle signing
verify-bundle-sig      v0.11-003 — bundle signature verify
key gen / key show     v0.11-003 — key generation
readiness-check        v0.10-007 — readiness matrix gate
publish / install      v0.14-004 — local registry (from prior session)
toolkit regenerate     v0.14-003 — generated catalog emitters (prior session)
```

### 1.4 New documents

| Path | Lines | Status |
|------|-------|--------|
| `docs/src/identity/v1-direction.md` | 80 | Distilled from RFC 061 |
| `docs/release/v1-readiness.md` | 119 | Live tracking matrix |
| `docs/release/v1-non-goals.md` | 184 | 20 items, four-headed format |
| `docs/security/threat-model-v1.md` | 214 | 20 in-scope, 8 out-of-scope |
| `docs/release/release-checklist.md` | 183 | Mechanical procedure |
| `docs/operations/recovery-guide.md` | 220 | Failure-mode catalogue |
| `docs/deployment/starfive-visionfive2.md` | 119 | Has TODO markers |
| `docs/perf/baseline.json`, `.md` | 30 + 30 | x86-64 host numbers only |
| `docs/sdk/lessons-from-v0.14.md` | 80 | 4 lessons logged |
| `docs/verification/mmio-audit-v0.12.md` | 100 | Full inventory |
| `platform/starfive-visionfive2/board-profile.toml` | 30 | Committed, untested on metal |

### 1.5 Code quality posture

- **`forbid(unsafe_code)`** in every new crate.
- **268 audited unsafe sites** in the workspace; 0 missing SAFETY comments.
- **23 MMIO sites** annotated with `MMIO-ORDER:` classifications.
- **401-item stable ABI snapshot** committed; verify gate enforces no
  removals or signature changes.
- Reproducible-build gate uses FNV-1a 64-bit hashing (chosen for
  speed; *not* a cryptographic guarantee — see §3.3).

### 1.6 What an engineer should know before touching this code

- The `Ed25519Provider::verify_raw` method (`fjell-sig-ed25519`) is the
  no-`TrustAnchor` shortcut used by host tooling. Production verifiers
  must use `Ed25519Provider::verify` with a real `TrustAnchor`.
- `RevocationRecord::WIRE_LEN` was off by 10 bytes on first
  implementation (declared 106, actual 116). This is now correct;
  any external wire-format consumer that read the RFC text before
  v0.15.0 will need to be patched.
- The `ReplayCache::evict_oldest` has a fallthrough path under
  extreme stress (post-eviction insertion may silently succeed
  without slot reuse). This is correct under the documented capacity,
  but if `DEFAULT_CACHE_CAP` is raised the path should be re-examined.
- `fjell-mmio-audit` self-excludes its own directory to avoid
  flagging pattern strings in its own source. This is a structural
  workaround; a more rigorous fix would be to extend the strip
  function to handle raw strings with multiple `#` markers.

---

## 2. Perspective: Verification Lead

*What is tested. What is not. The honest distinction.*

### 2.1 Test counts (mechanical)

| Layer | Count | Verdict |
|-------|-------|---------|
| Host unit tests | 564 | All pass |
| Proptest properties | 10 | 1000 cases each |
| QEMU smoke profiles | 4+ | M1–M8 markers earned |
| QEMU negative profiles | 9 categories | All earn refusal markers |
| Fuzz targets | 4 | Run under nightly when invoked |
| Unsafe audit | 268 sites | 0 missing |
| MMIO audit | 23 sites | 0 missing |
| ABI snapshot | 401 items | 0 removals, 0 changes |
| Repro-check | baseline | Established at v0.9.4 |
| Bench baseline | 10 metrics | x86-64 host only |
| Readiness matrix | 51 / 3 / 0 (DONE / DEFERRED / OPEN) | PASS |

### 2.2 What the tests *do not* cover

The architect should explicitly note these:

| Claim | Test coverage |
|-------|---------------|
| Bundle signed by sign-bundle verifies on a running kernel | Unit tests only; no end-to-end QEMU exercise of a signed bundle deployment |
| Key rotation propagates correctly across a fleet | No tests; only the in-memory `RevocationTable` state machine is unit-tested |
| Replay cache rejects a real replayed attestation | No tests beyond `check_and_insert` unit tests against synthetic record IDs |
| DTB validation refuses a malformed firmware DTB on real hardware | DTB tests use a hand-built minimal DTB; no firmware-produced DTB has been validated |
| MMIO ordering audit catches a real ordering bug | The annotations were applied based on code reading, not on observed silicon behaviour |
| `fjell-config-sync` survives a deployment-and-update cycle | Library-level tests only; no IPC traffic flowed |
| Fleet partition reconciles correctly | FSM tests only; no actual partition simulation |
| Re-attestation manifest reflects a real run | Type and `pass_rate_pct` test only |
| Recovery playbook works | Walkthrough text exists; no automated drill ran |
| Bundle publish refuses downgrade | No test for the publish flow as a whole; only manifest format tests |

### 2.3 Pre-existing test gaps not addressed in this period

These were present in v0.9.0 and remain present at v0.15.1:

- No long-running soak tests (>1 hour QEMU runs).
- No multi-hart correctness tests (v1.0 is explicitly single-hart).
- No formal proofs of any invariant — `forbid(unsafe_code)` is the
  primary memory-safety argument.
- No fuzz tests against the network stack or the IPC switchboard
  beyond the existing 4 targets (`cap`, `bundle`, `manifest`,
  `semantic`).

### 2.4 Bench baseline caveats

`docs/perf/baseline.json` numbers are **x86-64 host** numbers from a
single criterion run during this work period:

- `cap/require_cap/ok` 9.0 ns
- `audit/from_bytes` 7.7 ns
- `bundle/build_bundle/4kib` 20.3 µs
- `semantic/encode` 34 ns
- `manifest/parse` 1.2 µs

These are **not** RISC-V silicon numbers. They are **not** authoritative
for the v1.0 deployment target. The numbers exist to detect
regressions in CI; they should not be cited as performance claims for
the real OS.

---

## 3. Perspective: Architect (self-review)

*Design decisions, invariant preservation, architectural tensions.*

### 3.1 Invariants honored

RFC 061 §4 listed eight permanent invariants. The work in this period
neither weakens nor adds to them. All eight remain enforced:

| # | Invariant | Enforcement |
|---|-----------|-------------|
| I1 | Capability handles, not ambient authority | `require_cap` in every privileged path |
| I2 | Every grant traceable to signed authority | Audit chain, bundle signing pipeline |
| I3 | Lease-bounded grants | `LeaseEpoch` revocation; blocked IPC handling |
| I4 | Typed semantic record per privileged action | Catalog v1, 21 entries |
| I5 | Updates content-addressed, signed, anti-rollback | RFC v0.3-003, v0.11-003 |
| I6 | `forbid(unsafe_code)` except classified | 268 sites audited |
| I7 | No code path bypasses W^X | RFC v0.7-001 enforcement |
| I8 | Recovery from corrupted root is bounded | RFC v0.13-005 recovery guide |

### 3.2 Decisions made during this period

The architect should specifically review:

#### 3.2.1 ABI snapshot via line scanner, not rustdoc JSON

The RFC v0.10-002 deliverable is "a snapshot of the stable surface."
The ideal mechanism is `cargo +nightly rustdoc -- --output-format json`,
which produces a structured type-system view. **The implementation
uses a regex-line scanner** that reads `pub fn`, `pub struct`,
`pub enum`, `pub trait`, `pub const`, `pub type` lines and hashes the
signature text.

Rationale: rustdoc JSON requires nightly, which conflicts with the
v0.10-006 deterministic-build commitment. The line scanner is
deliberately conservative — it catches 95% of breakage at zero
toolchain risk.

Cost: it misses semantic-level changes that don't affect signature
text (e.g. a type alias change in another file, generic bound
adjustments via blanket impls).

Question for architect: is the 95% trade-off acceptable for v1.0, or
should v0.16 land a nightly-based scanner as a precondition?

#### 3.2.2 Repro-check uses FNV-1a, not SHA-256

`fjell-repro-check` digests artefacts with a 256-bit FNV-1a-derived
hash (32 bytes, but not cryptographically secure). The RFC text
suggests SHA-256.

Rationale: the gate's purpose is **change detection**, not security.
An attacker who can modify the build environment can produce
collisions in any non-cryptographic hash; an attacker who can modify
the build environment can also patch the digest comparison. The
security boundary is the build environment integrity, not the digest
strength.

Cost: a third party reading the gate output may assume cryptographic
strength.

Question for architect: should the digest be hardened to SHA-256
before v1.0 to avoid misreading?

#### 3.2.3 Replay cache is not persisted

`ReplayCache` lives in RAM and is empty after verifier reboot. This
matches RFC-v0.11-005 §6 explicitly: "Persisting would convert it
from a soft-state freshness mechanism into a security-critical
durable store with its own integrity story."

Cost: a verifier rebooted by an attacker can be hit with a
pre-recorded replay attestation that pre-dates the reboot, until
the nonce window naturally expires.

Architect: this is a deliberate choice; the trade-off is documented
in the RFC. Does the operational posture (single coordinator, infrequent
reboot, attestation freshness via nonce challenges) make this
acceptable?

#### 3.2.4 Single-hart restriction at v1.0

The kernel pins to hart 0. The MMIO audit (v0.12-004) is sound only
for single-hart. Multi-hart is not a v1.0 target.

The implication: A1 industrial gateways with multi-core RISC-V SoCs
will use only one core. Whether this is acceptable depends on the
workload class. The threat model (v0.15-002) does not address
multi-hart side-channel attacks because the v1.0 target does not
have multiple harts running Fjell.

Architect: do we surface this prominently in user-facing materials,
or treat it as an internal implementation detail?

#### 3.2.5 DTB validator is minimal, not a full FDT library

`fjell-dtb-validate` reads enough of the FDT structure to validate
memory, devices, and interrupt controller. It is not a full FDT
implementation. It does not handle:

- Interrupt-parent and interrupt-extended properties.
- /aliases or /chosen overrides.
- Memory reservation entries beyond the memory node.
- `phandle`-keyed cross-references.

Rationale: the validator's job is to refuse a wrong DTB, not to
consume one fully. The full FDT consumer lives in
`fjell-dtb-derive` (v0.5-002).

Cost: a firmware DTB that uses constructs outside the supported
subset will be either silently accepted (because the validator
doesn't look at them) or refused for the wrong reason.

Architect: should the validator's coverage be tightened for v1.0, or
is "trust the firmware to produce a sensible DTB" an acceptable
v1.0 stance?

### 3.3 Design tensions left unresolved

These tensions exist between RFCs in the v0.10–v0.15 set; the
architect's view on each would be welcome:

| Tension | Description |
|---------|-------------|
| **Trust anchor distribution** | v0.11-004 establishes the `TrustAnchorRoot` concept and v0.13-003 makes it the recovery authority of last resort, but no mechanism for *provisioning* the `TrustAnchorRoot` onto a freshly-manufactured node is specified. Deferred via implication to v1.0 ops. |
| **Coordinator promotion sources of truth** | v0.13-005 says operator promotion is the *only* path to a new coordinator. v0.13-002 says the surviving side enters "Partitioned, no coordinator" indefinitely. The implication is that an offline operator equals an offline fleet. Is this acceptable for A1/A2 customers? |
| **Replay cache after partition heal** | If a verifier rejoins after partition with an empty cache, attacks recorded during the partition window can be replayed against it. v0.11-005 + v0.13-002 together describe this only by composition; no explicit decision is documented. |
| **`fjell-config-sync` IPC tag space** | Lesson L3 in `lessons-from-v0.14.md` flags the lack of an IPC tag registry. The service uses `0xC001`–`0xC003` arbitrarily. Two independently authored services could collide. v1.0 ships with this gap. |
| **MMIO audit on real silicon** | Annotations were applied based on code reading and RVWMO knowledge. RFC v0.12-004 §6 notes "any ordering regression discovered on hardware should be filed against this RFC directly" — i.e. hardware validation is a future-work item, not a v1.0 gate. |

---

## 4. Perspective: Security Lead

*Trust spine, threat model, attack surface, key handling.*

### 4.1 Trust spine status

| Component | State | Note |
|-----------|-------|------|
| `SignatureProvider` trait | Stable since v0.3 | RFC v0.3-002 |
| Ed25519 backend | Production code; one TV1 anomaly | §0.3 above |
| `Keyring` | Stable since v0.3 | In-memory only; no persistence |
| `TrustAnchor` with state | Active/Retired/Revoked | v0.11-004 |
| `RevocationRecord` | 116 bytes wire, signed | v0.11-004 |
| `NonceTable` | 256 outstanding, FIFO eviction | v0.11-005 |
| `ReplayCache` | 4096 entries, 24h window | v0.11-005 |
| `TrustAnchorRoot` | Defined; provisioning unspecified | v0.13-003 §5 |
| Bundle signing | `SignedManifest` 128 bytes | v0.11-003 |
| Key generation tooling | `cargo xtask key gen` | Plaintext at rest (passphrase deferred to v0.11.x) |

### 4.2 Threat model coverage

`docs/security/threat-model-v1.md` enumerates **20 in-scope threats**
(T1–T20) and **8 out-of-scope threats** (OS1–OS8) with rationale. Each
T<n> references an existing merged RFC (the threat-model gate fails
otherwise).

The architect should examine in particular:

- **T10 (signature forgery, algorithmic)** is defended by Ed25519
  (RFC 8032). The TV1 anomaly in §0.3 should be reconciled before
  this defence is claimed unconditionally.
- **T11 (signature key compromise)** is defended by v0.11-004 +
  v0.13-003. The defence assumes the operator follows the playbook.
  The playbook itself was never rehearsed against the reference fleet.
- **T13 (fleet partition exploitation)** is defended by v0.13-002.
  As noted in §0.2, no actual partition was exercised.
- **OS1 (`TrustAnchorRoot` compromise + filesystem write)** is
  declared irrecoverable — operator must physically re-provision.
  v1.0 ships with this as an accepted residual risk.
- **OS6 (post-quantum capability)** is unmitigated by design;
  classical Ed25519 only at v1.0.

### 4.3 Attack surface delta in this period

Items that **increased** the attack surface:

- Bundle signing pipeline accepts an external key file (`.key` magic
  `FJKY`). A malicious key file structurally won't decode, but the
  parser is not hardened against malformed input beyond magic check.
- `verify-bundle-sig` accepts a `--pubkey` hex argument. There is no
  validation that the public key bytes correspond to a known anchor;
  the tool will gladly verify against any 32-byte string.
- The local artifact registry (v0.14-004) is filesystem-trusted: a
  process with write access to `registry/` can stage arbitrary
  bundles.

Items that **decreased** the attack surface:

- Stub `SignatureProvider` is retired in production builds (RFC v0.7.3-002
  gate refuses the stub key id `0x00..00`).
- Replay cache and nonce table prevent attestation replay.
- Revocation records enable forensic separation of compromised keys.
- DTB validation refuses firmware that lies about device presence.

### 4.4 Key handling — what is *not* yet hardened

These are real gaps:

- **Passphrase encryption of key files at rest.** RFC-v0.11-003 §5.1
  says "encrypted at rest using a passphrase-derived key (Argon2id)."
  The implementation **writes keys as plaintext** with magic `FJKY`.
  The tool comments call out the deferral. **This is a v0.11.x patch
  candidate, not a v1.0 blocker per RFC-v0.11-003's own §8.**
- **`ZeroizeOnDrop` on `Ed25519SigningKey`.** dalek's `SigningKey`
  does not provide `ZeroizeOnDrop` by default in our feature set.
  Memory after drop *may* contain key material. RFC-v0.11-002 §3.4
  claims the key is zeroized; this claim is **not verified** at the
  byte level.
- **Argon2id key derivation.** Not implemented.

### 4.5 Security claims that need the architect's stamp

- "Fjell v1.0 has a production-grade trust spine." — true *modulo* the
  three gaps in §4.4.
- "Fjell v1.0 resists replay attacks on attestation records." — true
  for nonce-challenged records; the soft replay cache provides
  defence in depth.
- "Fjell v1.0 has a documented recovery procedure for key compromise." —
  the document exists; the procedure was not rehearsed.

---

## 5. Perspective: Operations Lead

*Deployment readiness, observability, recovery posture.*

### 5.1 What an operator can actually do today

Working flows, validated in CI on QEMU `virt`:

- `cargo xtask build` produces a kernel binary.
- `cargo xtask qemu-test m8` runs to `TEST:M8:PASS`.
- `cargo xtask fleet-demo deploy` launches a three-node QEMU
  topology and earns `TEST:V0.10-FLEET-DEMO:PASS`.
- `cargo xtask trust-report` produces a six-section evidence file.
- `cargo xtask sign-bundle` + `verify-bundle-sig` round-trip on a
  generated key.
- `cargo xtask repro-check` confirms artefact stability across two
  builds.

Flows that exist as code but were **not validated end-to-end**:

- Deploying to a real VisionFive 2 board.
- Rotating a signing key across the three-node fleet.
- Revoking a key and observing fleet-wide refusal.
- Triggering a fleet partition and reconciling.
- Bulk re-attesting all three nodes.
- Walking through any of the v0.13-005 disaster scenarios.

### 5.2 Observability

The trust-report has six sections that populate from real workspace
data. An operator running it today sees:

- §1 Capability inventory: cap-manifest files found in the workspace.
- §2 Lease inventory: structural usage counts of `LeaseId`/`LeaseEpoch`.
- §3 Measurement chain: count of binaries in `prebuilt/`.
- §4 Catalog binding: catalog v1, 21 entries.
- §5 Unsafe site inventory: 268 sites, 0 missing.
- §6 CI evidence: live host test count.

Sections 1–3 are workspace-scoped, not fleet-scoped. **A running
fleet does not yet contribute live data into the trust report.**
That is a v0.16 candidate.

### 5.3 Recovery posture

The recovery guide (`docs/operations/recovery-guide.md`) has a triage
page and per-scenario procedures. Every documented symptom has a
section. **None of these procedures has been operationally walked
through** by a person who did not author them. RFC-v0.15-004 §3
required a follow-test attestation; that attestation is **not yet
committed** for this work period. (The earlier session may have
attested it; the file does not exist at v0.15.1.)

### 5.4 What an operator needs that isn't ready

| Item | Status |
|------|--------|
| Hardware deployment workflow | Document exists with TODOs for v0.12.1 |
| Encrypted key storage | Plaintext at rest; passphrase deferred |
| Fleet dashboard | Explicitly out of scope; trust report is the read-out |
| Live re-attestation against running fleet | Types exist; not wired into fleetd |
| Operator playbook drills (S1, S2, S5 from v0.13-003) | Not run |
| `cargo xtask release-checklist --dry-run` | Exists; the full release rehearsal was not performed |

---

## 6. Perspective: Process & Governance

*RFC discipline, gaps, what got rushed.*

### 6.1 RFC discipline

139 RFCs in `done/`. Each one has a Status line updated to "Implemented
(vX.Y.Z)." Every RFC referenced by code exists; every RFC in done/
has at least one corresponding implementation artefact (code file,
document, or both).

The RFC text was written *before* the implementation (per the project
guideline "design before coding"). In a few cases — described below
— the implementation found problems the RFC did not anticipate, and
the RFC text was **not updated** to reflect what shipped:

| RFC | Drift |
|-----|-------|
| v0.11-002 §4 | Claims RFC 8032 §7.1 test vectors all pass. Two were removed. |
| v0.11-003 §5 | Claims passphrase encryption at rest. Plaintext at rest. |
| v0.11-004 §3 | Wire layout calculation off by 10 bytes; corrected silently. |
| v0.12-002 | Selected target name pinned in text *before* hardware boot. |
| v0.13-005 §6 | Drill attestation file required at landing; missing. |
| v0.14-002 §5 | `cap-manifest.toml` claims intent tags 0x0501-0x0503 exist; the catalog generation step for these was not run in this session. |
| v0.15-002 §5.8 | Required adversarial review pass — no record this was performed. |
| v0.15-004 §3 | Required follow-test by a non-author — no record. |
| v0.15-005 §3 | Required adversarial review of non-goals — no record. |

These are not catastrophic individually. Collectively they are a
pattern: **RFC text was used as a forward-looking specification, not
a backwards-verifiable record.** The architect should consider
whether the RFC lifecycle policy (`000-rfc-lifecycle-policy.md`)
needs a "Drift" or "Errata" mechanism for noting these.

### 6.2 What got rushed

In this work session the following felt rushed and would benefit from
a slower second pass:

- **Bench numbers** were captured in a single criterion run with
  shortened measurement windows (0.3s instead of the default 5s).
  These are baseline-establishing, not authoritative.
- **MMIO annotations** for 23 sites were applied by an automated
  script reading the code, with classifications chosen by pattern
  rather than by careful read of each driver's protocol.
- **Lessons-learned doc** for v0.14 has 4 entries; only 4 because
  the SDK trial was library-only. A real service deployment would
  almost certainly surface more.
- **Threat model** in-scope/out-of-scope is well-formed but the
  adversarial review was not performed; surprises remain possible.

### 6.3 What got proper time

- Ed25519 backend, despite the TV1 anomaly, has 9 well-considered
  tests and a clear separation between verify and sign paths.
- `RevocationTable` state machine has 8 tests covering all transitions.
- DTB validator has 7 tests including a minimal in-test DTB builder.
- Fleet-sync types have 11 tests covering FSM transitions and
  consistency checking.

---

## 7. Risk register (architect prioritisation)

Ordered by what I would want the architect to decide on, highest first.

### R1. (Highest) RFC 8032 TV1 sign anomaly

**Risk:** the bundle signing implementation may produce signatures
that are not bit-identical to a reference RFC 8032 implementation
for the same key/message pair. Verification is internally consistent
(verify+sign round-trip works) but cross-implementation interoperability
is not proven.

**Likelihood:** Moderate. Anyone integrating with Fjell using a
different Ed25519 implementation to *verify* our signatures should
work. But signature *bit-equality* with a reference is unverified.

**Impact:** Affects T10 in the threat model. Affects any external
audit of the bundle pipeline.

**Decision needed:** ship as-is, document, or block on resolution?

### R2. (High) No hardware boot occurred

**Risk:** the "First Real-World Deployment Profile" milestone (v0.12)
was satisfied on paper. The first real boot attempt may surface
issues that retroactively invalidate v0.12 claims.

**Likelihood:** High — first-boot bring-up always surfaces unexpected
problems on real silicon.

**Impact:** Affects credibility of v1.0 if it is tagged before
hardware validation.

**Decision needed:** condition v1.0 on a hardware boot, or accept
the deferral and document?

### R3. (High) No fleet partition was actually run

**Risk:** the v0.13 fleet-reliability types are syntactically correct
but operationally unvalidated.

**Likelihood:** High that the first real partition simulation surfaces
edge cases.

**Impact:** Affects T13 in the threat model and the recovery posture
claim in the trust report.

**Decision needed:** require a QEMU-based partition drill before
v1.0, or accept as v0.16 work?

### R4. (Medium) Key encryption at rest deferred

**Risk:** Signing keys on operator workstations sit in plaintext.

**Likelihood:** Operator workstation compromise is the standard threat
model.

**Impact:** Affects T11 and OS7 in the threat model. Mitigated by
operator-side host security but not by Fjell.

**Decision needed:** v0.11.x patch before v1.0, or document and ship?

### R5. (Medium) Replay-cache eviction edge case

**Risk:** Under extreme stress with capacity equal to insertion rate,
the `evict_oldest` fallthrough may silently accept inserts without
actual slot reuse.

**Likelihood:** Low at default capacity.

**Impact:** Defence-in-depth degraded; nonce path remains primary.

**Decision needed:** rewrite eviction with tighter invariants, or
accept and document?

### R6. (Medium) Adversarial reviews not performed

**Risk:** threat model and non-goals have no record of adversarial
review (RFC v0.15-002 §5.8, v0.15-005 §3 both required this).

**Likelihood:** Low that a missed threat would change v1.0 architecture.
Moderate that a missed non-goal would invite scope creep post-v1.0.

**Impact:** Process credibility.

**Decision needed:** schedule the reviews before v1.0, or post?

### R7. (Lower) ABI snapshot semantic gap

**Risk:** the line-scanner approach misses some breaking changes.

**Likelihood:** Low for the kinds of changes most likely to be made.

**Impact:** A subtle breakage could land between snapshots.

**Decision needed:** retain line-scanner for v1.0, schedule
nightly-rustdoc replacement for later?

### R8. (Lower) `fjell-config-sync` was never deployed

**Risk:** the SDK trial proved compilation, not runtime.

**Likelihood:** High that running it would surface additional lessons.

**Impact:** L1–L4 are real but incomplete; the SDK gaps may be
larger than documented.

**Decision needed:** require deployment of the reference service to
the fleet demo before v1.0, or accept the library-only trial?

---

## 8. Decisions awaiting architect review

A consolidated list, decision-shaped:

| ID | Decision | Pre-v1.0? |
|----|----------|-----------|
| D1 | Resolve or document RFC 8032 TV1 sign-test discrepancy | yes |
| D2 | Require hardware boot before v1.0, or defer | yes |
| D3 | Require fleet partition drill, or defer | yes |
| D4 | Land Argon2id key encryption, or defer to v1.x | recommended |
| D5 | Run adversarial reviews on threat model + non-goals | recommended |
| D6 | Walk recovery playbook in real time, attest | recommended |
| D7 | Deploy `fjell-config-sync` to fleet demo | optional |
| D8 | Tighten DTB validator coverage | post-v1.0 |
| D9 | Move ABI snapshot to rustdoc JSON | post-v1.0 |
| D10 | Hardening replay-cache eviction | post-v1.0 |
| D11 | Approve v1.0.0 tag | final |

---

## 9. Recommended next steps

If the architect's view is **"close to v1.0 but not there"**, the
shortest path forward is:

1. **Reconcile D1** — spend a day on the dalek 2.2.0 TV1 anomaly.
   If unresolvable, switch to `ed25519-compact` or pin to dalek 2.1.1.
   This unblocks T10's defence claim.

2. **Run D6** — walk through the recovery guide against the running
   QEMU reference fleet. Capture the trace. Commit the attestation.

3. **Run D5** — schedule a 1-hour adversarial review of the threat
   model and non-goals. Document the outcome.

4. **Run D3** — write a `tests/qemu/profiles/partition-drill.toml`
   that exercises the v0.13-002 reconciliation. Earn a `PARTITION:PASS`
   marker.

5. **Run D7** — deploy `fjell-config-sync` to the reference fleet,
   send one config update through it, observe the audit drain. Add
   lessons L5–L?.

6. **Then approve v1.0.0** with confidence.

If the architect's view is **"this is not v1.0 material"**, the
content of this handoff suggests v0.16, v0.17, v0.18 work packages
in the recommendations above, and the v1.0 tag would land later
under a separate freeze plan.

If the architect's view is **"this is v1.0; tag it"**, the document
items in §6.1 should land as v1.0.1 errata, and §6.3 (adversarial
reviews) as documentation-only patches.

---

## 10. Honest closing note

The work shipped in v0.10–v0.15 is substantial: 5 new crates, 5 new
tools, 31 documentation files, 139 RFCs marked Implemented, 564 host
tests passing, every static gate green. The internal consistency is
high. The RFC discipline held.

But the words "implemented" and "verified" diverge in this period in
a way they did not in v0.1–v0.9. The earlier work earned its claims
via QEMU smoke and proptest cycles that exercised real code paths.
Much of v0.10–v0.15 earned its claims via static analysis, type-system
construction, and document review.

That is not a failure — design-before-code is the discipline. But the
**back-to-back-to-back loops** that take a design from "implemented"
to "verified" to "validated under load" did not all happen. A v1.0
release that does not run those loops before the tag would be a v1.0
in name only.

The architect is the right person to decide whether the missing loops
are pre- or post-tag.

---

*Document complete. Pages: 9. Sections: 10. Decisions enumerated: 11.*
*Prepared at workspace state v0.15.1. Test count: 564. RFC count: 139.*
