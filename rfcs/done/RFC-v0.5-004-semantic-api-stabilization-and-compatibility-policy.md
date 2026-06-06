# RFC-v0.5-004: Semantic API Stabilization and Compatibility Policy

**Status.** Implemented (v0.5.0)

## Status

Draft (revised, supersedes pack v0.5-004 draft)

## Target Version

`v0.5.0`.

## Phase

Platform Surface and Semantic Stabilization — Epic D (Semantic API).

## Related Work

- v0.2 semantic-stream design (see RFC 045 in sequential history).
- v0.4 RFC 005 — DiagnosticBundle (consumer of stable intent tags).
- v0.6 RFC 003 — semantic schema compatibility (fuzzing).
- v0.9 RFC 003 — semantic node authoring toolkit (consumer).

---

## 1. Summary

Lift the semantic-stream surface from "internal-may-change" to **stable v1
contract**:

- A frozen catalog of `SemanticIntent` tags (the public taxonomy).
- A frozen field schema per intent.
- A canonical binary encoding and JSON projection.
- A compatibility policy: additive-only changes within a major version,
  breaking only at major bumps.
- A version-negotiation handshake between writers (services) and readers
  (`proxy-text`, `diagnosticsd`, future SDK consumers).

After this RFC, services and tools may depend on the v1 catalog with the
guarantee that a v1.x build never silently changes the meaning of any tag.

---

## 2. Motivation

Until now the semantic stream has been a moving target — fields added at
each phase. v0.4's `DiagnosticBundle` projection (RFC v0.4-005) and v0.7's
fleet-state aggregation both need a stable contract. Without one, every
downstream consumer either pins to a build hash or breaks.

Locking the v1 catalog at v0.5.0 is the right moment: v0.3 hardware-trust
and v0.4 networking are the last large semantic surfaces; v0.5 is the
natural quiescence point before fleet work.

---

## 3. Goals

```text
- Frozen v1 catalog file under crates/fjell-semantic-v1/catalog.rs.
- Frozen v1 schema per intent under fjell-semantic-v1/schema.rs.
- Canonical binary encoding (compact, deterministic).
- JSON projection (advisory).
- Version-negotiation: reader requests catalog version; writer either
  serves v1 or refuses.
- Tests prove every catalog entry parses, projects, and round-trips.
```

## 4. Non-Goals

```text
- No new intents in this RFC; this is a freeze, not a feature.
- No backwards-incompatible field renames; renames trigger v2.
- No support for streaming aggregation beyond what semantic-stream already
  provides.
```

---

## 5. External Design

### 5.1 Catalog shape

Catalog is a `const` table:

```rust
pub const CATALOG_V1: &[IntentEntry] = &[
    IntentEntry { tag: 0x0100, name: "UPDATE.STAGING_STARTED",
                  schema: &SCHEMA_UPDATE_STAGING_STARTED },
    IntentEntry { tag: 0x0101, name: "UPDATE.STAGING_ADVANCED",
                  schema: &SCHEMA_UPDATE_STAGING_ADVANCED },
    // ... full table
];

pub struct IntentEntry {
    pub tag:    u16,
    pub name:   &'static str,
    pub schema: &'static IntentSchema,
}
```

### 5.2 Schema shape

```rust
pub struct IntentSchema {
    pub field_count: u8,
    pub fields:      &'static [FieldSpec],
}

pub struct FieldSpec {
    pub name:   &'static str,
    pub kind:   FieldKind,
    pub required: bool,
}

#[repr(u8)]
pub enum FieldKind {
    U8 = 1, U16, U32, U64,
    Digest32,
    AsciiStr8, AsciiStr16, AsciiStr32, AsciiStr64,
    TagU16,
}
```

### 5.3 Encoding

```text
canonical bytes:
   "FJSI-V1" (7 B) ||
   intent_tag u16 LE ||
   created_tick u64 LE ||
   for each field in schema order:
     present u8 (always 1 for required, 0/1 for optional) ||
     if present: field bytes in fixed encoding
   trailing u8 = 0xFF (sentinel)
```

JSON projection is fixed-key-order, ASCII, no nesting.

---

## 6. Data Model

### 6.1 Catalog membership (v1, frozen)

(Full table abbreviated; each tag has a single schema.)

```text
Update domain:
  0x0100 UPDATE.STAGING_STARTED          { candidate_id, channel_id, counter }
  0x0101 UPDATE.STAGING_ADVANCED         { candidate_id, from_state, to_state }
  0x0102 UPDATE.STAGING_FAILED           { candidate_id, error_code }
  0x0103 UPDATE.STAGING_CONFIRMED        { candidate_id, counter, slot }
  0x0110 UPDATE.ROLLBACK_BLOCKED         { channel_id, attempted, min_counter }
  0x0111 UPDATE.ROLLBACK_TO_PREVIOUS_SLOT { previous_slot, reason_code }

Attestation domain:
  0x0120 ATTEST.RECORD_SIGNED            { record_id, profile, provider_id }
  0x0121 ATTEST.RECORD_VERIFY_FAILED     { record_id, error_code }

Security boundary domain:
  0x0130 SECURITY.REGISTRY_ENFORCING     { providers_registered }
  0x0131 SECURITY.PROVIDER_FAULTED       { provider_id, reason_code }
  0x0132 SECURITY.KEYRING_EPOCH_ADVANCED { purpose, old_epoch, new_epoch }

Net domain:
  0x0140 NET.LINK_UP                     { device_id, mtu }
  0x0141 NET.LINK_DOWN                   { device_id, reason_code }
  0x0142 NET.SXT_CHANNEL_OPENED          { channel_id, kind, server_name }
  0x0143 NET.SXT_CHANNEL_CLOSED          { channel_id, reason_code }

Recovery domain:
  0x0150 RECOVERY.ENTERED                { reason_code }
  0x0151 RECOVERY.EXITED                 { outcome_code }

Platform domain:
  0x0160 PLATFORM.PROFILES_READY         { platform_digest, board_digest }

Health domain:
  0x0170 HEALTH.TARGET_REACHED           { target_id, status }
  0x0171 HEALTH.TARGET_FAILED            { target_id, reason_code }

Future-reserved (won't be used in v1):
  0x0200..=0x02FF — FLEET domain (RFCs v0.7, v0.8)
  0x0300..=0x03FF — SDK domain (RFC v0.9)
```

### 6.2 Compatibility policy

```text
v1.x:
  - new intents may be added (with tags in unallocated subranges) provided
    they are *additive* (no consumer breakage).
  - new optional fields may be added at the end of an existing schema.
  - required fields cannot change kind or order.
  - tags cannot be renamed or repurposed.

v2.x:
  - introduces a new catalog file `catalog_v2.rs`. v1 continues to exist
    for a deprecation period of one minor cycle.
  - reader/writer negotiate `CatalogVersion` at handshake; writer may
    serve both.

v2 within a v1 deprecation period:
  - readers must accept both; writers may emit either.
  - after deprecation expires, writers may drop v1 support; emission of a
    v1 record by a v2-only writer is a build error.
```

### 6.3 Version negotiation

```rust
pub struct CatalogVersion { pub major: u8, pub minor: u8 }

pub trait SemanticReader {
    fn requested_versions(&self) -> &[CatalogVersion];
}

pub trait SemanticWriter {
    fn supported_versions(&self) -> &[CatalogVersion];
    fn emit(&self, version: CatalogVersion, tag: u16, fields: &[FieldValue])
        -> Result<(), SemanticError>;
}
```

`semantic-stream` selects the highest common version on handshake.

---

## 7. Internal Design

### 7.1 Encoder / decoder

```rust
pub fn encode(version: CatalogVersion, tag: u16, fields: &[FieldValue], out: &mut [u8])
    -> Result<usize, SemanticError>;

pub fn decode(bytes: &[u8])
    -> Result<DecodedIntent, SemanticError>;
```

Both are pure functions; no allocator.

### 7.2 schema-test macro

For every catalog entry, a generated host test asserts:

- encode → decode → round-trips field values;
- field count matches the schema;
- unknown trailing bytes are rejected;
- truncated buffers are rejected at well-defined error codes.

### 7.3 Doc-gen

`fjell-tools semantic catalog` prints the canonical v1 table. CI compares
against a checked-in markdown table to catch silent drift.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-140: A compromised service emits an unknown tag claiming an alarm
              intent.
Mitigation:  reader rejects unknown tags with SemanticError::UnknownTag;
              audit records this as a security event.

Threat T-141: Field-kind mismatch (e.g., writer emits a u32 where schema
              expects u16, hoping the reader misparses).
Mitigation:  field-kind tag in the encoded form is checked against schema
              before parsing the value.

Threat T-142: Cross-version confusion (v2 record decoded as v1).
Mitigation:  the "FJSI-V1" magic distinguishes versions; v2 will use
              "FJSI-V2".
```

### 8.2 Audit emission

```text
SemanticUnknownTag        { observed_tag }
SemanticFieldMismatch     { intent_tag, field_index, expected_kind, actual_kind }
SemanticEncodeFailure     { intent_tag, error_code }
SemanticVersionMismatch   { reader_versions, writer_versions }
```

---

## 9. Memory / Resource Design

- Catalog tables and schemas live in `.rodata` (≈ 4 KiB).
- Encoder uses an external buffer supplied by the caller; no internal alloc.

---

## 10. Compatibility and Migration

- All v0.2/v0.3/v0.4 semantic events listed above. Any event in the wild not
  on the list is renamed or retired *before* v0.5.0; the migration ADR
  enumerates every renaming.
- `proxy-text` and `diagnosticsd` switch to the v1 catalog as their single
  source of truth.

---

## 11. Test Strategy

### 11.1 Host unit tests (generated)

For every entry in `CATALOG_V1`:

```text
- <name>_round_trip_encode_decode
- <name>_field_count_matches_schema
- <name>_truncation_rejected
- <name>_unknown_field_rejected
```

### 11.2 Catalog audit

```text
- catalog_unique_tags
- catalog_schemas_match_doc_table        (compare against docs/src/semantic/catalog.md)
- catalog_no_renamed_field_within_v1     (CI checks against the previous-tag commit)
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:SEM:UNKNOWN_TAG_REJECTED`                          | semantic |
| `NEG:SEM:FIELD_KIND_MISMATCH_REJECTED`                  | semantic |
| `NEG:SEM:VERSION_MISMATCH_REJECTED`                     | semantic |
| `NEG:SEM:TRUNCATED_RECORD_REJECTED`                     | semantic |
| `NEG:SEM:UNALLOCATED_TAG_REJECTED`                      | semantic |

---

## 12. Acceptance Criteria

```text
- fjell-semantic-v1 crate lands with frozen catalog.
- Catalog matches docs/src/semantic/catalog.md (CI check).
- All generated round-trip tests green.
- proxy-text and diagnosticsd consume v1 catalog.
- 5 NEG markers green.
- ADR-v0.5-004 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/semantic/catalog.md                — canonical table (machine-checked)
docs/src/architecture/v0.5-004-semantic-stabilization.md
docs/src/development/v0.5-004-semantic-stabilization.md
docs/src/adr/v0.5-004-semantic-v1-freeze.md
docs/src/adr/v0.5-004-compat-policy.md
```

---

## 14. Open Questions

1. **Variable-length string fields** — currently fixed-size ASCII fields
   only. Variable-length is rejected because it complicates fuzzing and
   redaction; if needed in v0.6+, introduce as an opaque-bytes kind with a
   max-len bound.
2. **Catalog version pinning by consumer** — should a consumer be able to
   ask only for a *subset* of v1? Not in v0.5; a consumer takes all of v1
   or none. Subset-pinning is an SDK feature.

---

## 15. Release Gate (RFC-local)

```text
- Catalog frozen.
- All generated tests green.
- proxy-text + diagnosticsd switched over.
- 5 NEG markers green.
- ADRs Accepted.
```
