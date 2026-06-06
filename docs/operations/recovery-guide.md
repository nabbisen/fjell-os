# Fjell OS — Operator Recovery Guide

*Governed by RFC-v0.15-004. Authoritative for v1.0.0.*

---

## Quick-start triage

```
A node is not booting           → §3.1  Boot failures
A bundle deployment failed      → §3.2  Rollout failures
A node has been quarantined     → §3.3  Node quarantine
The coordinator is down         → §3.4  Coordinator loss
A key is compromised            → §3.5  Key compromise
The fleet is partitioned        → §3.6  Partition handling
Audit storage is corrupted      → §3.7  Local audit recovery
None of the above               → §4    General diagnostic flow
```

---

## §3.1 Boot failures

**Symptoms:** No serial output after firmware banner, or:
```
FJELL-BOOT-FAIL: DTB R4 <mmio_address>
FJELL-BOOT-FAIL: DTB R1_BAD_MAGIC
FJELL-BOOT-FAIL: DTB R6_NO_IRQCTRL
```

**Diagnosis:**
```bash
# Capture serial log
screen /dev/ttyUSB0 115200
```

| Diagnostic | Cause | Resolution |
|------------|-------|------------|
| No output | Image not loaded | Re-flash SD card per RFC-v0.12-005 |
| `FJELL-BOOT-FAIL: DTB R4` | Required device missing from DTB | Update `board-profile.toml` or firmware |
| `FJELL-BOOT-FAIL: DTB R1` | Wrong firmware version | Flash OpenSBI ≥ 1.5 |
| `FJELL-BOOT-FAIL: DTB R6` | No PLIC node in DTB | Update board profile; file issue with board_digest |
| Boots to `init: ready` then hangs | Service spawn failure | Capture audit ring; check `service-manager` |

---

## §3.2 Rollout failures

**Symptoms:**
```
FLEET.ROLLOUT_PAUSED { failures=3, total=10, threshold=10 }
BUNDLE.ROLLBACK_TRIGGERED { reason_code=2 }
```

**Diagnosis:**
```bash
cargo xtask fleet rollout status --plan-id <id>
cargo xtask fleet rollout logs   --plan-id <id> --node <bad-node>
```

**Recovery:**
1. If `reason_code=2` (health-check timeout): examine service logs for `BUNDLE_HEALTH:FAILED`.
2. Fix root cause in the bundle or service, build and publish new version.
3. Resume rollout:
```bash
cargo xtask fleet rollout rollback --plan-id <id>
# or if investigating:
cargo xtask fleet rollout pause --plan-id <id>
```

---

## §3.3 Node quarantine

**Symptoms:**
```
FLEET.NODE_QUARANTINED { node_id=<id> }
SECURITY.ATTEST_REPLAY_REFUSED
```

**Diagnosis:**
```bash
cargo xtask fleet quarantine status --node <id>
```

**Recovery (key rotation, if attestation key compromised):**
```bash
cargo xtask fleet re-enrol --node <id>
# Generates new attestation key, distributes via ReconcileManifest
```

**Recovery (false positive):**
```bash
# If the quarantine was triggered incorrectly:
cargo xtask fleet quarantine lift --node <id>
# Emits FLEET.QUARANTINE_LIFTED audit record
```

---

## §3.4 Coordinator loss (DR1)

**Symptoms:** All members show `Partitioned, no coordinator`.

**Prerequisites:** `TrustAnchorRoot` key available offline.

**Recovery:**
```bash
# Identify surviving members
cargo xtask fleet status

# Promote a surviving member to coordinator
# (requires TrustAnchorRoot passphrase)
cargo xtask fleet promote --node <surviving-node-id>
# Expected: FLEET.COORDINATOR_PROMOTED audit record on all surviving nodes
```

**Verification:**
```bash
cargo xtask fleet status
# Expected: coordinator = <new-node-id>, state = Healthy
```

---

## §3.5 Key compromise (DR2 + DR5)

Follow the playbook in `docs/operations/key-compromise.md` (RFC-v0.13-003).

Quick reference:

```bash
# Revoke the compromised key
cargo xtask key revoke --key-id <hex16> --reason compromised

# Generate and distribute replacement
cargo xtask key gen    --epoch <N+1> --out replacement.key
cargo xtask fleet reattest --trigger rotation
```

---

## §3.6 Partition handling

**Symptoms:** Coordinator shows `FLEET.PARTITION_DETECTED`.

**Diagnostic:**
```bash
cargo xtask fleet status
# Shows: state=Partitioned, partitioned_since=<ns>
```

**Recovery (link restored automatically):**
The coordinator initiates reconciliation when link is restored.
Monitor with:
```bash
cargo xtask fleet status --watch
# Expected progression: Suspect → Partitioned → Reconciling → Healthy
```

**Recovery (manual reconcile if link restoration does not trigger):**
```bash
cargo xtask fleet reconcile --force
```

---

## §3.7 Local audit storage corruption (DR5)

**Symptoms:**
```
CONSISTENCY.SUMMARY_REJECTED { check=SyncSeqRegression }
```

**Diagnosis:**
```bash
cargo run -p fjell-summary-check -- \
    --seq <new> --epoch 1 --boot 1 --lifecycle 4 \
    --prev-seq <prev> --known-bundle <hex32>
# Identifies which consistency check is failing
```

**Recovery:**
```bash
# Quarantine the node; collect evidence
cargo xtask fleet quarantine --node <id>

# After investigation, re-enrol with fresh state
cargo xtask fleet re-enrol --node <id>
```

---

## §4 General diagnostic flow

1. **Collect:** `cargo xtask trust-report`
2. **Inspect §6** (CI evidence): is the test gate green?
3. **Inspect §5** (unsafe inventory): is `missing_comment = 0`?
4. **Inspect §1** (capability inventory): are caps as expected?
5. If `cargo xtask fleet status` shows anything other than `Healthy`, jump to §3.

---

## Failure modes catalogue

| Symptom | Section | Severity | Est. MTTR |
|---------|---------|----------|-----------|
| `FJELL-BOOT-FAIL: DTB` | §3.1 | High | 15 min |
| `BUNDLE_HEALTH:FAILED` | §3.2 | Medium | 30 min |
| `FLEET.ROLLOUT_PAUSED` | §3.2 | Medium | 1 hr |
| `FLEET.NODE_QUARANTINED` | §3.3 | High | 1–2 hr |
| `Partitioned, no coordinator` | §3.4 | Critical | 30 min |
| `SECURITY.ATTEST_REPLAY_REFUSED` | §3.3+§3.5 | High | per playbook |
| `CONSISTENCY.SUMMARY_REJECTED` | §3.7 | Low | 1–2 hr |
| `FLEET.PARTITION_DETECTED` | §3.6 | Medium | auto |

---

*Full DR scenarios (DR1–DR8): `docs/operations/disaster-recovery.md` (RFC-v0.13-005).*
*Key compromise playbook: `docs/operations/key-compromise.md` (RFC-v0.13-003).*
