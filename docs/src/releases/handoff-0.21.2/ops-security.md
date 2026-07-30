# Fjell OS — Ops, Release & Security (Cross-Cutting Handoff)

*Compact operational + security handoff. Version: v0.21.2.*

## 1. Release and packaging essentials

| Item | Rule / command | Evidence |
|---|---|---|
| Versioning | Strict patch/minor/major scoping; version in workspace `Cargo.toml` | `CHANGELOG.md` |
| Build artifact | `cargo xtask package-release` → `fjell-os-v{version}.tar.gz` | `crates/fjell-tools/src/package_release.rs` |
| Archive layout | Unpacks to `fjell-os-v{version}/` — no intermediate parent dir | verified per release |
| Reproducibility | Two-build SHA-256 comparison over the artefact set | `tests/repro/`, `fjell-repro-check` |
| Pre-publication | All 11 release-rehearsal gates pass + Gate 9 manual sign-off | `cargo xtask release-rehearsal` |
| Publication control | **v1.0.0 must not be tagged/published/announced without explicit owner (nabbisen) confirmation** | architect review v0.20.0 |

The archive excludes `target/`, `.git/`, `*.img`, prior `fjell-os-v*.tar.gz`,
`tests/runs/`, `tests/qemu/artifacts/`, and `provision/` (ships unprovisioned;
each operator provisions explicitly).

## 2. CI / gate essentials

The eleven mechanical gates run via `cargo xtask release-rehearsal`:

| Gate | What it checks | Blocking? |
|---|---|---|
| 1 | Host test suite | yes |
| 2 | Unsafe audit (0 missing SAFETY) | yes |
| 3 | MMIO audit (0 missing MMIO-ORDER) | yes |
| 4 | ABI snapshot (no removals) | yes |
| 5 | Readiness matrix | yes |
| 6 | Trust report | yes |
| 7 | ERRATA register (0 OPEN) | yes |
| 8 | Validation drills | yes |
| 9 | **Manual** limitations sign-off | yes (human) |
| 10 | Verus release-required proofs (capability + lease) | yes |
| 11 | Callsite conformance audit | yes |

Gate 10 requires `verus` on PATH; without it the gate fails (the other ten
still run). All gates except 9 are mechanical.

## 3. Environment and configuration essentials

- **Toolchain:** Rust 1.91; target `riscv64gc-unknown-none-elf`; `ld.lld`;
  `qemu-system-riscv64`. Verus (separate 1.95 toolchain) only for Gate 10.
- **No external `qemu-img`** — disk image is created in pure Rust.
- **Per-distro packages and full setup:** `docs/src/internals/local-development.md`.
- **Verus install:** `docs/src/verification/verus-setup.md`;
  `verification/verus/TOOLCHAIN.lock` pins the version.
- **Secrets:** none required for build or host tests. Trust-anchor provisioning
  (dev/QEMU) writes a dev key only when `--allow-tofu-provision` is passed; the
  key material is not stored in the repo or release archive.
- **Network:** build pulls crates from crates.io; no runtime network assumptions
  for the QEMU profile.

## 4. Security-critical decisions

| ID | Decision | Why it matters | Evidence / owner |
|---|---|---|---|
| SEC-01 | `forbid(unsafe_code)` outside kernel/arch; every unsafe site categorised with `// SAFETY:` | Bounds and audits the TCB | `docs/src/verification/unsafe-charter.md`; Gate 2 |
| SEC-02 | No silent TOFU; provisioning requires `--allow-tofu-provision` | Prevents accidental trust-on-first-use | RFC-v0.17-001; `cargo xtask provision-dev` |
| SEC-03 | Signed bundles verified (Ed25519/RFC 8032) before execution | Authenticity of deployed binaries | `fjell-sig-ed25519`, `fjell-bundle-format` |
| SEC-04 | Crypto relies on audited primitives (dalek Ed25519, Argon2id, AES-256-GCM) | Avoids custom crypto | dependency manifest |
| SEC-05 | Default-deny capability model; no ambient authority | A task cannot touch what it does not hold | `crates/fjell-kernel/src/trap/syscall.rs` |
| SEC-06 | No plaintext secrets in repo or release archive | Supply-chain hygiene | `provision/` excluded from archives |

## 5. Operational risks

| Risk | Impact | Mitigation | Owner |
|---|---|---|---|
| Gate 10 advisory if Verus absent | False sense of proof coverage | Gate fails closed when verus missing; documented | QA |
| Repro baseline must be re-recorded after kernel changes | Spurious repro FAIL | Documented in testing handoff §5 | Implementer |
| Trust-anchor signing-side coupling is operational, not enforced | Operator could sign with a non-matching key | Documented as v1.1 requirement | Architect |
| Store/upgrade negative paths not gated | No runtime evidence for two security paths | Documented as non-gated; v1.1 target | Implementer |

## 6. Minimal incident / rollback note

This is a pre-1.0 OS project, not a deployed service.

- **Identify a bad release:** a release whose `cargo xtask release-rehearsal`
  does not show all mechanical gates passing, or whose archive fails to unpack
  to a single `fjell-os-v{version}/` root.
- **Supersede:** cut the next patch version with the fix and a CHANGELOG entry;
  the previous archive remains for reference. There is no registry to yank from.
- **Emergency fixes:** approved by the owner (nabbisen) only.
- **Downstream notification:** none for v1.0 (no published downstream
  consumers); record the supersession in `CHANGELOG.md` and the relevant RFC.
