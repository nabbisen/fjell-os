# RFC-v0.13-003 — Key Compromise Recovery Playbook

**Status:** Implemented (v0.13.0)
**Target version:** v0.13.0
**Parent:** v0.13-001.
**Cross-refs:** RFC-v0.11-004 (rotation), v0.11-005 (replay).

## 1. Problem

v0.11-004 makes key rotation possible. It does not say how an operator
*runs* a rotation against a deployed fleet under stress — when a key
is believed compromised, the operator needs a single document that
walks through the response without inventing steps.

This RFC ships that playbook, backed by automation that turns the
playbook into mechanical actions wherever possible. The operator's
job is to make policy decisions; the tool runs the steps.

## 2. Threat scenarios covered

The playbook explicitly addresses:

- **S1.** Suspected compromise of the *bundle signing key* (the v0.11
  signer key used by `cargo xtask sign-bundle`).
- **S2.** Suspected compromise of an *attestation key* on a node.
- **S3.** Suspected compromise of the *trust-anchor distribution* path
  (i.e. an attacker may have substituted trust anchors at install
  time).
- **S4.** Lost key material (operator-side loss, not adversarial).
- **S5.** Routine rotation (no compromise, scheduled rollover).

Out of scope: compromise of the underlying cryptographic primitive
(would require a Fjell-wide algorithm migration, deferred to a
post-v1.0 hybrid-mode RFC).

## 3. The playbook (`docs/operations/key-compromise.md`)

For each scenario the document specifies:

```text
1. Triage (what to observe before acting)
2. Containment (steps that bound damage)
3. Rotation (the actual procedure)
4. Verification (how to confirm recovery)
5. Post-incident (evidence to retain, what to attest)
```

Worked examples:

### S1. Bundle signing key compromise

1. **Triage.** Detect unexpected bundles in the audit chain
   (`SECURITY.SIGNED_BUNDLE_OBSERVED` records not corresponding to a
   release entry).
2. **Containment.** Operator runs `cargo xtask key revoke
   --key-id <compromised> --reason compromised`. The resulting
   `RevocationRecord` is signed by the revocation authority key
   (RFC-v0.11-004 §3) and distributed to all nodes.
3. **Rotation.** Generate a new signing key at epoch N+1, distribute
   its trust anchor, sign a tombstone bundle that revokes the previous
   key in the kernel's persistent trust anchor store.
4. **Verification.** Each node emits `FLEET.TRUST_ANCHOR_UPDATED`;
   coordinator collects until all enrolled nodes have acknowledged
   the rotation. Any node not acknowledging within the configured
   window enters `Stale-trust-anchors` posture (RFC-v0.11-004 §6).
5. **Post-incident.** Operator retains the audit chain segment from
   first observation through verification as immutable evidence;
   attests its contents under the new signing key.

### S2. Per-node attestation key compromise

Similar shape; differs in that the response is scoped to the affected
node. The node is quarantined (its attestations refused) and its key
rotated locally via the v0.11-004 procedure. The fleet emits
`FLEET.NODE_QUARANTINED` and `FLEET.NODE_REKEY_COMPLETED`.

### S3. Trust-anchor distribution compromise

The hardest scenario. The playbook covers:

- How to distinguish "anchors are subtly wrong" from "anchors are
  correct but key is compromised."
- Out-of-band re-pinning: a signed re-pin manifest signed by a
  designated higher-authority key (a v0.13 addition: the
  `TrustAnchorRoot` key).
- The implications for nodes that have already accepted the
  compromised anchors — they enter quarantine until physically
  re-attested.

### S4. Lost key material

No revocation can be signed (the revocation authority key has the
same shape problem). The playbook covers the *break-glass* procedure:
documented procedure for re-establishing trust from a hardware-rooted
source if available (v0.12 secure-element work) or from the operator's
attested workstation if not. The result is a new keyring with new
epochs.

### S5. Routine rotation

The simple case: schedule, generate, distribute, retire, revoke. Used
as the calibration scenario for the reference fleet in CI — runs
every CI cycle as `test-all` rotation drill.

## 4. Automation surface

New xtask subcommands:

```
cargo xtask key revoke    --key-id <hex> --reason <code>
cargo xtask key rotate    --next-epoch <n>
cargo xtask key tombstone --key-id <hex>
cargo xtask fleet quarantine --node <id>
cargo xtask fleet re-enrol   --node <id>
```

Each command:

- Produces a signed manifest committed to the audit chain.
- Updates the fleet roster.
- Emits the appropriate catalog intents.
- Refuses to run without a passphrase prompt for any key-bearing
  operation.

## 5. The new `TrustAnchorRoot` authority

S3 motivates a higher-authority key whose only purpose is signing
re-pin manifests for the trust-anchor distribution path. The
`TrustAnchorRoot` key:

- Is stored fully offline (operator's preferred cold-storage method;
  the playbook recommends but does not enforce a specific medium).
- Has its own KeyEpoch and revocation procedure.
- Signs nothing other than `TrustAnchorRepinManifest` records.
- Its public part is committed to every node at provisioning time and
  cannot be replaced via the normal trust-anchor channel.

If the `TrustAnchorRoot` is itself compromised, the only recovery is
to re-provision every node from physical media — a fact the playbook
states plainly.

## 6. Partition-aware key rotation

If a partition is active during rotation:

- Nodes on `PartitionedAway` side do not advance KeyEpoch (RFC-v0.13-002
  §3).
- On reconciliation the new trust anchors flow through the
  `ReconcileManifest`.
- Until reconciliation, partitioned nodes continue to honour the
  pre-partition anchors; a new bundle signed under the new key is
  refused with `EpochUnknown` on the partitioned side. This is the
  intended fail-closed behaviour.

## 7. CI drills

`tests/qemu/profiles/rotation-drill.toml` exercises:

- S1 against the three-node fleet (signing key compromise).
- S5 routine rotation.
- S2 single-node attestation key compromise.

Markers: `DRILL:S1:PASS`, `DRILL:S2:PASS`, `DRILL:S5:PASS`.

S3 and S4 are not CI-feasible (S3 requires substituted anchors at
install time; S4 requires destroying real key material). They are
exercised by manual operator walkthrough at landing time, attested in
`docs/operations/drill-attestation-v0.13.md`.

## 8. Acceptance criteria

1. `docs/operations/key-compromise.md` exists and covers S1–S5.
2. The five new xtask commands exist and work end-to-end against the
   reference fleet.
3. `TrustAnchorRoot` is defined, provisioned in the reference fleet
   demo, and exercised once at fleet bring-up.
4. The CI drill profile passes for S1, S2, S5.
5. S3 and S4 walkthroughs are attested in the doc at landing.
6. The Trust Report's "Recovery posture" section enumerates the five
   scenarios with the timestamp of the last drill.

## 9. Out of scope

- Multi-operator quorum on revocation (v1.x).
- Forensic incident-response tooling beyond evidence preservation.
- Coordination with external CSIRTs / disclosure processes.
- Cryptographic algorithm migration playbook.
