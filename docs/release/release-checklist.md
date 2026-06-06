# Fjell OS — Release Checklist

*Governed by RFC-v0.15-003. Run this exactly to produce a v1.0 release.*
*Every command must produce the documented output or the step is FAIL.*

---

## Pre-flight (all steps must be PASS before continuing)

```bash run-verified
# 1 — Verify clean working copy
git status --short
# Expected: empty output (no modified or untracked files)
```

```bash run-verified
# 2 — Verify toolchain
rustc --version | grep "1.91"
# Expected: line containing "1.91"
```

---

## Step 1 — Host test suite

```bash run-verified
cargo test --workspace --lib --exclude fjell-proptest
# Expected: test result: ok. N passed; 0 failed
```

---

## Step 2 — Property tests

```bash run-verified
cargo test -p fjell-proptest --release
# Expected: test result: ok.  (≥ 10 properties, 0 failed)
```

---

## Step 3a — Unsafe-audit gate

```bash run-verified
cargo run -p fjell-unsafe-audit -- --workspace . --check
# Expected: missing comment : 0
```

## Step 3b — Reproducible build gate

```bash run-verified
cargo xtask repro-check
# Expected: fjell-repro-check: PASS
```

## Step 3c — MMIO ordering audit gate

```bash run-verified
cargo run -p fjell-mmio-audit -- --workspace . --check
# Expected: missing annotation : 0
```

---

## Step 4 — ABI snapshot verification

```bash run-verified
cargo xtask abi-snapshot --verify
# Expected: Result : PASS
```

---

## Step 5 — Trust Report

```bash run-verified
cargo xtask trust-report
# Expected: [trust-report] written to docs/release/trust-report.txt
# Verify all 6 sections are non-empty:
grep "^§[1-6]\." docs/release/trust-report.txt
```

---

## Step 6 — Documentation build

```bash run-verified
cargo xtask docs build
# Expected: docs build: OK
```

---

## Step 7 — v1.0 Readiness Matrix: zero OPEN cells

```bash run-verified
grep "OPEN" docs/release/v1-readiness.md
# Expected: empty output (no OPEN cells)
```

---

## Step 7b — RFC Errata gate: zero OPEN errata (RFC-v0.16-004)

```bash run-verified
grep -E "\| (OPEN) \|" docs/rfcs/ERRATA.md
# Expected: empty output. ACCEPTED items are permitted but must each
# appear in the release notes limitations section.
```

---

## Step 7c — Validation drill gate (RFC-v0.16-008)

All validation-closure drills must pass and emit their markers:

```bash run-verified
cargo test -p fjell-sig-ed25519 from_seed_matches_tv1_public sign_tv1_produces_tv1_sig
cargo test -p fjell-fleet-sync --test partition_drill -- --nocapture | grep DRILL:
cargo test -p fjell-config-sync --test runtime_trial -- --nocapture | grep DRILL:
# Expected markers:
#   DRILL:FLEET-PARTITION-RECONCILE:PASS
#   DRILL:FLEET-PARTITION-ROLLBACK-REJECTED:PASS
#   DRILL:SDK-CONFIG-SYNC-RUNTIME:PASS
#   DRILL:SDK-CONFIG-SYNC-CONVERGENCE:PASS
```

---

## Step 8 — Release bundle

```bash run-verified
cargo xtask build
# Expected: Finished release [optimized]
```

---

## Step 9 — Sign all bundles

```bash
# Sign with the v1.0 release key (stored offline)
for bundle in target/release-bundles/*.bundle; do
    cargo xtask sign-bundle \
        --bundle "$bundle" \
        --key    /path/to/v1.0-release.key \
        --out    "$bundle.sig"
done
# Expected: sign-bundle: wrote 128 bytes to <bundle>.sig
```

---

## Step 10 — Attest the release manifest

```bash
cargo xtask trust-report --out docs/release/v1.0.0/trust-report.txt
cargo xtask sign-bundle \
    --bundle docs/release/v1.0.0/trust-report.txt \
    --key    /path/to/v1.0-release.key \
    --out    docs/release/v1.0.0/trust-report.txt.sig
```

---

## Step 11 — Tag

```bash
git tag -s v1.0.0 -m "Fjell OS v1.0.0"
git push origin v1.0.0
```

---

## Step 12 — Package

```bash run-verified
cargo xtask release --version v1.0.0
# Expected: release.tar.gz written
```

---

## Security advisory process (RFC-v0.15-003 §3)

### Intake
- Reporter contacts `security@<domain>` (fill in before v1.0 landing).
- Acknowledgement target: 72 hours.

### Severity tiers
- **Critical:** ≤ 30 days to patch.
- **High:** ≤ 60 days.
- **Medium:** ≤ 90 days.
- **Low:** next regular release.

### Advisory format (template)
```
ID:          FSAD-YYYY-NNN
Severity:    Critical | High | Medium | Low
Reported:    YYYY-MM-DD
Disclosed:   YYYY-MM-DD
Affected:    vX.Y.Z–vA.B.C
Fixed in:    vA.B.C
Reporter:    (name or "anonymous")
Description: …
Threat ref:  T<n> (RFC-v0.15-002)
References:  commit hash, RFCs
```

Committed to `docs/security/advisories/FSAD-YYYY-NNN.md`.
