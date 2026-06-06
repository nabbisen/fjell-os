# RFC-v0.4-004: Remote Update Metadata Fetch and Staged upgraded Integration

**Status.** Implemented (v0.4.0)

## Status

Draft (revised, supersedes pack v0.4-004 draft)

## Target Version

`v0.4.0`.

## Phase

Minimal Secure Control-Plane Networking — Epic D (Remote Update).

## Related Work

- v0.3 RFC 003 — `ReleaseMetadata`, `RollbackRecord`; consumed unchanged.
- v0.4 RFC 003 — `secure-transportd` UpdateMetadata channel.
- v0.7 RFC 003 — release-summary sync (consumes this RFC's update index).
- v0.8 RFC 003 — staged-rollout governance (consumes the staging steps).

---

## 1. Summary

Add a remote update-metadata fetch path to `upgraded` that:

1. asks `secure-transportd` to fetch an **`UpdateIndex`** document from a
   pinned server;
2. selects an eligible `ReleaseMetadata` from the index that passes local
   anti-rollback and channel-id constraints (RFC v0.3-003);
3. fetches the release manifest and the release blob through the same
   channel;
4. **stages** the release into the inactive A/B slot through a *staged
   pipeline* (`Fetched → Verified → Committed → Confirmed`), with rollback
   on any step failure;
5. reports staging progress as semantic events.

The kernel is unchanged. `verifyd`, `keyring`, and `storaged` are unchanged.
`upgraded` gains the staged pipeline state machine and the remote fetch
client. `bootctl` is unchanged.

---

## 2. Motivation

Until now Fjell has accepted release bundles only from a locally provided
path. To make the OS field-updatable, the fetch path must:

- run *after* the security boundary is in `Enforcing` phase;
- use *only* the `secure-transportd` UpdateMetadata channel;
- preserve anti-rollback;
- be observable and abortable at every step.

The staged pipeline makes "abortable" first-class: any failure (network drop,
signature mismatch, rollback violation, health-probe fail) lands the system
in a defined state with no half-applied artefacts.

---

## 3. Goals

```text
- Fetch UpdateIndex from a single pinned server per channel.
- Parse UpdateIndex with strict, no-alloc JSON-subset parser.
- Select the highest counter that passes local rollback/channel checks.
- Stage manifest + blob in a 4-state pipeline:
    Fetched → Verified → Committed → Confirmed
- Each transition produces an audit event and a semantic intent.
- Aborting from any state is safe and idempotent.
- Health-probe failure auto-rolls back from Confirmed; the v0.2 rollback path
  is reused.
- All artefacts deleted on abort.
```

## 4. Non-Goals

```text
- No multi-server failover. One server per channel.
- No P2P or torrent style fetch.
- No delta updates (full-blob only in v0.4.0).
- No background fetch; staging is operator-initiated.
- No automatic update policy; the operator (or a future v0.8 fleet manager)
  triggers the update.
- No release-author-time metadata signing beyond what v0.3-003 specifies.
```

---

## 5. External Design

### 5.1 Operator workflow

```text
$ fjell-tools update check          # ask upgraded to fetch latest index
$ fjell-tools update list           # show eligible candidates
$ fjell-tools update stage <id>     # fetch+verify into inactive slot
$ fjell-tools update commit <id>    # write to disk; reboot pending
$ fjell-tools update confirm        # after first boot succeeds health
$ fjell-tools update abort <id>     # discard at any pre-confirmed state
```

Each command runs through `cap-broker` and requires a specific right.

### 5.2 UpdateIndex JSON-subset

A strict JSON-subset (no nested objects in candidate entries, ASCII only,
length-bounded):

```json
{
  "schema": 1,
  "channel": "stable--",
  "issued_at_tick": 0,
  "index_digest": "<sha256-hex-or-null>",
  "candidates": [
    { "id": "R000058", "counter": 58, "anchor_epoch": 3,
      "manifest_url": "/meta/R000058.manifest",
      "manifest_digest": "<sha256-hex>",
      "blob_url": "/blob/R000058.blob",
      "blob_digest": "<sha256-hex>",
      "size_bytes": 12345 },
    { "id": "R000063", "counter": 63, "anchor_epoch": 3, ... }
  ]
}
```

`index_digest` is computed over the canonical encoding of the candidates list
(see §6.3). If `index_digest` is present, the parser verifies it after parse.

### 5.3 Staged pipeline states

```rust
#[repr(u8)]
pub enum StagedState {
    Idle             = 0,
    Fetching         = 1,
    Fetched          = 2,
    Verifying        = 3,
    Verified         = 4,
    Committing       = 5,
    Committed        = 6,
    AwaitingReboot   = 7,
    BootedPending    = 8,    // booted, awaiting health probe
    HealthChecking   = 9,
    Confirmed        = 10,
    Failed           = 11,   // terminal; rollback triggered or pending abort
    Aborting         = 12,
    Aborted          = 13,
}
```

State transitions are observable in the semantic stream as
`UpdateStateChanged { from, to, candidate_id }`.

---

## 6. Data Model

### 6.1 Persisted staging record

```rust
pub struct StagingRecord {
    pub schema_version:     u16,           // = 1
    pub candidate_id:       [u8; 8],
    pub channel_id:         [u8; 8],
    pub counter:            u64,
    pub state:              StagedState,
    pub manifest_digest:    Digest32,
    pub blob_digest:        Digest32,
    pub size_bytes:         u64,
    pub fetched_at_tick:    u64,
    pub verified_at_tick:   u64,
    pub committed_at_tick:  u64,
    pub confirmed_at_tick:  u64,
    pub failure_code:       u16,           // 0 = none
    pub record_digest:      Digest32,
}
```

Persisted to `storaged` on every state transition. Recovery scan picks the
*latest* record per candidate_id; the latest with a non-terminal state is
"in progress" and resumed (or aborted) by `upgraded` at boot.

### 6.2 Update fetch caps

```rust
// Rights inside ChannelCap (already minted by secure-transportd; here are the
// upgraded-side rights).
pub const UPGRADE_FETCH_INDEX:    CapRights = CapRights(1 << 0);
pub const UPGRADE_FETCH_MANIFEST: CapRights = CapRights(1 << 1);
pub const UPGRADE_FETCH_BLOB:     CapRights = CapRights(1 << 2);
pub const UPGRADE_STAGE:          CapRights = CapRights(1 << 3);
pub const UPGRADE_COMMIT:         CapRights = CapRights(1 << 4);
pub const UPGRADE_CONFIRM:        CapRights = CapRights(1 << 5);
pub const UPGRADE_ABORT:          CapRights = CapRights(1 << 6);
```

### 6.3 Canonical index digest

```text
index_digest = SHA256(
    "FJELL-UPDATE-INDEX-V1" ||
    schema u16 LE ||
    channel_id 8 B ||
    issued_at_tick u64 LE ||
    candidate_count u32 LE ||
    for each candidate (in ascending counter order):
        candidate_id 8 B ||
        counter u64 LE ||
        anchor_epoch u32 LE ||
        manifest_digest 32 B ||
        blob_digest 32 B ||
        size_bytes u64 LE
)
```

`manifest_url` and `blob_url` are *not* covered by the digest — they are
addresses, not content. If they change between fetches, the digest still
verifies as long as the content does.

---

## 7. Internal Design

### 7.1 Pipeline implementation

```text
on operator stage(candidate_id):
  cand = persist.lookup(candidate_id) or persist.create(candidate_id)
  if cand.state != Idle:
      return Err(BadTransition)

  cand.state = Fetching; persist.write(cand)
  manifest_bytes = sxt.update_metadata_fetch(channel, manifest_url)?
  if sha256(manifest_bytes) != cand.manifest_digest: fail(ManifestDigestMismatch)
  blob_bytes = sxt.update_metadata_fetch(channel, blob_url)?    // streamed
  if sha256(blob_bytes) != cand.blob_digest: fail(BlobDigestMismatch)
  cand.state = Fetched; persist.write(cand)

  cand.state = Verifying; persist.write(cand)
  metadata = verifyd.parse_release_metadata(&manifest_bytes)?
  if metadata.channel_id != cand.channel_id: fail(ChannelMismatch)
  if metadata.release_counter != cand.counter: fail(CounterMismatch)
  persisted = storaged.latest_rollback_record(cand.channel_id)?
  if metadata.release_counter < persisted.min_counter: fail(RollbackRejected)
  verifyd.verify_signature(metadata, keyring)?                  // RFC v0.3-002
  cand.state = Verified; persist.write(cand)

  // Commit = write blob into inactive slot
  cand.state = Committing; persist.write(cand)
  target_slot = bootctl.inactive_slot()
  rootfsd.write_slot(target_slot, blob_bytes, metadata.rootfs_digest)?
  bootctl.set_pending(target_slot, metadata.metadata_digest)?
  cand.state = Committed; persist.write(cand)

  cand.state = AwaitingReboot; persist.write(cand)
```

```text
on first boot from new slot:
  bootctl.mark_booted(slot)
  upgraded.observe_boot()
  cand.state = BootedPending; persist.write(cand)

on operator confirm:
  cand.state = HealthChecking; persist.write(cand)
  if !health_probe.passes(): fail(HealthCheckFailed)
  bootctl.confirm_slot(slot)
  storaged.append_rollback_record(
    channel=cand.channel_id, min=cand.counter, source=UpgradedConfirmation)
  cand.state = Confirmed; persist.write(cand)
```

```text
on operator abort(candidate_id) [allowed from Fetched..Committed]:
  cand.state = Aborting; persist.write(cand)
  rootfsd.zeroize_slot(target_slot)        // if Committing/Committed
  bootctl.clear_pending()                  // if pending
  cand.state = Aborted; persist.write(cand)
```

```text
on failure(code):
  cand.state = Failed; cand.failure_code = code; persist.write(cand)
  emit_audit; emit_semantic(UpdateFailed)
  do not roll back rollback_record; do not boot the slot.
```

### 7.2 Rollback after BootedPending

If the new slot reaches `BootedPending` but health probe fails (or any other
issue during `HealthChecking`), upgraded:

- marks the slot Failed;
- requests bootctl rollback to the previous slot (existing v0.2 path);
- the next boot is into the previous slot;
- `persisted_min_counter` for the channel is *not* advanced (the failed
  candidate never reached `Confirmed`).

### 7.3 Strict JSON-subset parser

```rust
pub fn parse_update_index(bytes: &[u8]) -> Result<UpdateIndex, IndexError>;
```

Rules:

- ASCII-only;
- nesting depth ≤ 3;
- string length ≤ 256 B;
- numeric values fit in u64;
- candidate count ≤ MAX_INDEX_CANDIDATES = 64;
- order: keys appear in canonical order within an object (verified, not
  inferred);
- whitespace ignored only between tokens.

The parser is fuzz-tested (deferred to v0.6 RFC 003).

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-90: Server delivers an UpdateIndex whose candidates point to
             attacker-controlled URLs.
Mitigation:  URLs only resolve through the same secure-transportd channel
             (same SNI, same anchor); attacker-controlled URL cannot escape
             the pinned scope.

Threat T-91: Server delivers a valid signed release whose counter is below
             the local minimum.
Mitigation:  Rollback check in Verifying step rejects with RollbackRejected.

Threat T-92: Operator confirms a slot that did not pass health probe.
Mitigation:  upgraded.confirm requires HealthChecking state; HealthChecking
             only enters if probe passes.

Threat T-93: Partial fetch leaves a half-written blob on disk and a crash
             recovery later "completes" it with bogus content.
Mitigation:  rootfsd writes are content-addressed; on resume, upgraded
             re-verifies the entire blob digest before any transition past
             Fetched.

Threat T-94: storaged StagingRecord is rewritten to claim Confirmed.
Mitigation:  record_digest covers the state; staged transitions are also
             emitted to the audit ring with sequence numbers; replay
             reconstructs the highest legitimate state.

Threat T-95: Long-running fetch ties up secure-transportd channel.
Mitigation:  per-channel byte-budget; per-step deadline; abort on timeout
             with audit emission.
```

### 8.2 Audit emission

```text
UpdateIndexFetched          { channel_id, candidate_count, index_digest }
UpdateIndexParseFailed      { error_code }
UpdateCandidateSelected     { candidate_id, counter }
UpdateStateTransition       { candidate_id, from, to }
UpdateRollbackRejected      { candidate_id, counter, min_counter }
UpdateHealthCheckFailed     { candidate_id, reason_code }
UpdateAborted               { candidate_id, prior_state }
```

### 8.3 Semantic intents

```text
UPDATE.INDEX_FETCHED
UPDATE.CANDIDATE_SELECTED
UPDATE.STAGING_STARTED
UPDATE.STAGING_ADVANCED      (with from/to state)
UPDATE.STAGING_FAILED
UPDATE.STAGING_CONFIRMED
UPDATE.STAGING_ABORTED
UPDATE.ROLLBACK_TO_PREVIOUS_SLOT
```

---

## 9. Memory / Resource Design

- `UpdateIndex` parsed structure: 64 candidates × ~120 B ≈ 7.7 KiB.
- StagingRecord ≈ 144 B; persisted per transition.
- Blob stream: chunked to MTU-bound packets through `secure-transportd`
  channel.

---

## 10. Compatibility and Migration

### 10.1 Compatibility with v0.3 rollback

- `RollbackRecord` schema unchanged.
- Local-only update path from v0.2/v0.3 still works (it skips Fetching).

### 10.2 Migration plan

| Step | Action |
|------|--------|
| 1    | Add `StagingRecord` to `fjell-upgrade-format`. |
| 2    | Add JSON-subset parser to a new internal crate `fjell-update-index`. |
| 3    | Extend `upgraded` with the pipeline state machine. |
| 4    | Extend `fjell-tools` CLI subcommands. |
| 5    | Add operator-facing semantic intents to `proxy-text`. |

---

## 11. Test Strategy

### 11.1 Host unit tests (`fjell-update-index`)

```text
- parse_minimal_index_ok
- parse_unknown_key_rejected_strict
- parse_unicode_rejected
- parse_nested_too_deep_rejected
- parse_string_too_long_rejected
- parse_canonical_order_violation_rejected
- index_digest_matches_after_parse
- index_digest_mismatch_rejected
- parser_never_panics_on_random_input   (lightweight property test)
```

### 11.2 Host unit tests (pipeline)

```text
- pipeline_idle_to_fetched_to_verified_to_committed_happy_path
- pipeline_abort_from_fetched
- pipeline_abort_from_committed
- pipeline_resume_from_fetched_after_crash
- pipeline_rollback_rejected_at_verifying
- pipeline_channel_mismatch_rejected_at_verifying
- pipeline_health_check_failure_does_not_confirm
- pipeline_health_check_failure_does_not_advance_min_counter
- pipeline_record_digest_covers_state
```

### 11.3 QEMU smoke tests

```text
- SMOKE:UPDATE:INDEX_FETCH        — fetch and parse index
- SMOKE:UPDATE:FULL_PIPELINE      — fetch → verify → commit → confirm
- SMOKE:UPDATE:ABORT_MID_STAGE    — operator aborts during Committing
```

### 11.4 QEMU negative tests

| Marker                                                      | Profile |
|-------------------------------------------------------------|---------|
| `NEG:UPDATE:INDEX_DIGEST_MISMATCH_REJECTED`                 | update  |
| `NEG:UPDATE:MANIFEST_DIGEST_MISMATCH_REJECTED`              | update  |
| `NEG:UPDATE:BLOB_DIGEST_MISMATCH_REJECTED`                  | update  |
| `NEG:UPDATE:ROLLBACK_AT_VERIFY_REJECTED`                    | update  |
| `NEG:UPDATE:CHANNEL_MISMATCH_AT_VERIFY_REJECTED`            | update  |
| `NEG:UPDATE:HEALTH_FAILURE_DOES_NOT_CONFIRM`                | update  |
| `NEG:UPDATE:HEALTH_FAILURE_DOES_NOT_ADVANCE_MIN_COUNTER`    | update  |
| `NEG:UPDATE:RESUME_AFTER_CRASH_REVERIFIES`                  | update  |
| `NEG:UPDATE:ABORT_FROM_COMMITTED_ZEROIZES_SLOT`             | update  |
| `NEG:UPDATE:UNAUTHORISED_RIGHT_REJECTED`                    | update  |

---

## 12. Acceptance Criteria

```text
- fjell-update-index crate exists with host tests.
- upgraded extended; pipeline tests green.
- 3 SMOKE + 10 NEG markers green.
- Operator CLI subcommands available.
- Rollback to previous slot demonstrated when health probe fails.
- ADR-v0.4-004 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.4-004-staged-upgrade.md
docs/src/development/v0.4-004-staged-upgrade.md
docs/src/verification/v0.4-004-staged-upgrade-invariants.md
docs/src/format/update-index.md
docs/src/operator/update-cli.md
docs/src/adr/v0.4-004-staged-upgrade-states.md
```

---

## 14. Open Questions

1. **Where does the index_digest come from?** It's optional in v0.4.0
   because the manifest signature already binds counter and digests. The
   index_digest is a future-fleet convenience for batched approvals. Keep
   the field but mark it optional in canonical encoding.
2. **Resume granularity** — current design resumes at the *start* of the
   crashed state and re-fetches. If blobs grow large, byte-range resume is
   useful. Deferred to a v0.4.x RFC.
3. **Background fetch** — explicitly out of scope. Could be added safely in
   v0.8 with fleet management.

---

## 15. Release Gate (RFC-local)

```text
- Code merged.
- 3 SMOKE + 10 NEG markers green.
- Operator CLI works in QEMU run.
- ADR Accepted.
- CHANGELOG entries filed.
```
