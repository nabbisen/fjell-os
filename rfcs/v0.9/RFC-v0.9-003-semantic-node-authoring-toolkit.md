# RFC-v0.9-003: Semantic Node Authoring Toolkit

## Status

Draft (revised, supersedes pack v0.9-003 draft)

## Target Version

`v0.9.0`.

## Phase

Developer Service Platform — Epic C (Semantic Toolkit).

## Related Work

- v0.5 RFC 004 — semantic catalog v1 (the schema this toolkit targets).
- v0.9 RFCs 001/002 — SDK + manifest.
- v0.6 RFC 003 — semantic schema fuzzing (the toolkit's outputs feed into
  fuzz corpora).

---

## 1. Summary

Introduce a small toolkit for service authors who emit semantic intents:

- a **typed Emitter API** in `fjell-sdk::semantic` that performs
  compile-time validation against catalog v1;
- a **catalog-doc** generator that renders human-readable docs from the
  catalog plus narrative author-notes;
- a **fixture generator** that produces representative encoded records
  for every catalog entry — for tests, fuzz seed corpora, and proxy-text
  rendering snapshots.

The toolkit changes no runtime path. It is purely build-time and test-
time tooling, but it dramatically reduces the cost of adding correct
semantic emission to a new service.

---

## 2. Motivation

By v0.9 the catalog has dozens of intents. A new service author writing
their first emission today must:

- find the right tag in the catalog file;
- learn the field schema by reading source;
- assemble the right encoded form by hand.

The result: subtle bugs (wrong field order, missing field, miscategorised
intent) that the schema-compat fuzz catches but only post-hoc. A typed
API + generator surface catches them at edit time.

---

## 3. Goals

```text
- Compile-time typed emitter: each catalog entry has a generated struct;
  using the wrong fields is a compile error.
- Author-notes alongside the catalog file: prose context that doesn't
  fit in the schema table.
- Doc renderer: produces docs/src/semantic/catalog.md with notes inline.
- Fixture generator: emits one valid encoded sample per catalog entry +
  one minimal sample + a small set of malformed samples for fuzz seed
  corpora.
- Round-trip: every generated fixture parses, decodes, re-encodes
  identically.
```

## 4. Non-Goals

```text
- No introduction of new intents in this RFC. Toolkit operates over the
  existing catalog.
- No runtime introspection — Fjell does not ship the catalog metadata at
  runtime beyond what the encoder needs.
- No multi-version catalog support (only v1; v2 in a future RFC).
- No UI; CLI only.
```

---

## 5. External Design

### 5.1 Typed Emitter API

For each catalog entry, a build-script generates a typed struct + helper:

```rust
// Generated; do not hand-edit. Source: catalog v1, entry 0x0100.
pub struct UpdateStagingStarted {
    pub candidate_id: [u8; 8],
    pub channel_id:   [u8; 8],
    pub counter:      u64,
}

impl UpdateStagingStarted {
    pub const TAG: u16 = 0x0100;
    pub const NAME: &'static str = "UPDATE.STAGING_STARTED";
    pub fn emit(&self, ctx: &mut SemanticCtx<'_>) -> Result<(), SemanticError> { ... }
}
```

The author writes:

```rust
UpdateStagingStarted {
    candidate_id: cand_id,
    channel_id:   ch_id,
    counter:      63,
}.emit(&mut ctx)?;
```

Mistyping a field, omitting a field, or supplying the wrong type fails
to compile.

### 5.2 Catalog source files

```text
crates/fjell-semantic-v1/
   catalog.toml                   — table of entries (machine-checked)
   notes/
      0x0100-update-staging-started.md   — author note per entry
      ...
   build.rs                       — generates Rust + docs from above
```

Each note file is plain markdown with a header front-matter:

```markdown
---
tag: 0x0100
canonical_name: UPDATE.STAGING_STARTED
since: v0.4.0
---

# UPDATE.STAGING_STARTED

Emitted when `upgraded` enters the `Fetching` state for a release
candidate. The candidate_id identifies the persisted StagingRecord
(RFC v0.4-004). Operators see this intent immediately preceding the
first network fetch.

## Examples

(prose / minimal field example)

## Related

- UPDATE.STAGING_ADVANCED
- UPDATE.STAGING_FAILED
```

### 5.3 CLI

```text
$ fjell-tools semantic doc-render              # writes docs/src/semantic/catalog.md
$ fjell-tools semantic fixtures gen            # writes test/fixtures/semantic/*.bin
$ fjell-tools semantic check                   # cross-checks notes vs catalog
$ fjell-tools semantic emit-typed --intent 0x0100 --candidate hex --channel hex --counter 63
```

---

## 6. Data Model

### 6.1 Catalog TOML

```toml
[[entry]]
tag = 0x0100
name = "UPDATE.STAGING_STARTED"
since = "v0.4.0"
critical = false
[[entry.field]]
name = "candidate_id"
kind = "AsciiStr8"
required = true
[[entry.field]]
name = "channel_id"
kind = "AsciiStr8"
required = true
[[entry.field]]
name = "counter"
kind = "U64"
required = true
```

The TOML is the single source of truth. Generated code, generated docs,
and generated fixtures all derive from it.

### 6.2 Fixture file shape

```text
fixtures/semantic/
   valid-min/
      0x0100.bin     — minimum valid record (all required fields, zero values)
      0x0101.bin
      ...
   valid-rich/
      0x0100.bin     — populated values
      ...
   malformed/
      0x0100-truncated.bin
      0x0100-bad-field-kind.bin
      0x0100-unknown-trailing.bin
      ...
```

The malformed set is curated; each file's name encodes what made it
malformed. Fuzz corpora in RFC v0.6-003 copy these as seeds.

---

## 7. Internal Design

### 7.1 build.rs flow

```text
1. read crates/fjell-semantic-v1/catalog.toml
2. for each entry:
     - generate <Entry>Struct + impl block
     - generate emit() helper
     - register the entry in CATALOG_V1 const slice
3. read notes/*.md; for each file:
     - parse front-matter; verify tag matches catalog entry
4. emit docs/src/semantic/catalog.md (renderer pass)
5. emit fixtures/semantic/* (fixture pass)
```

### 7.2 Doc renderer

Renders a Markdown table with rows for every catalog entry:

```markdown
| Tag    | Name                              | Since   | Notes |
|--------|-----------------------------------|---------|-------|
| 0x0100 | UPDATE.STAGING_STARTED            | v0.4.0  | [link] |
| 0x0101 | UPDATE.STAGING_ADVANCED           | v0.4.0  | [link] |
```

Followed by an expanded section per entry that embeds the note prose.

### 7.3 Fixture generator

For each entry:

- Build a minimum-valid struct by zeroing every required field;
- Encode with the catalog encoder; write to `valid-min/`.
- Build a populated struct (deterministic seed); encode; write to
  `valid-rich/`.
- Build a truncated copy of valid-min and write to `malformed/`.
- Build a copy with one field's kind byte replaced; write to
  `malformed/`.

All fixtures regenerate from source; checked-in copies are baseline.

### 7.4 Cross-checks

```text
- Every catalog entry has a notes file. (Or a note marked "no notes
  yet" with a TODO tag — limited to ≤ 5 entries at any time.)
- Every notes file references a real catalog entry.
- Every fixture parses successfully (or is in malformed/).
- Round-trip: encode(decode(fixture)) == fixture for valid-*.
```

---

## 8. Security Design

This RFC introduces no runtime path. Security considerations:

```text
- The toolkit cannot inject new intent emissions that bypass the
  catalog freeze; only entries in catalog.toml can be emitted.
- Notes are markdown text; the rendered docs are static HTML.
- Fixtures are test artefacts; not loaded at runtime.
```

---

## 9. Memory / Resource Design

Build-time only.

---

## 10. Compatibility and Migration

- Existing emission sites are migrated to the typed API in a coordinated
  PR. The legacy `emit_intent(tag, &[fields])` API is retained but
  deprecated; CI lint warns and the migration ADR plans removal at
  v0.10.

---

## 11. Test Strategy

### 11.1 Build-script tests

```text
- catalog_parses_clean
- generated_struct_matches_schema
- generated_emit_round_trips_valid_min
- generated_emit_round_trips_valid_rich
- notes_present_for_every_entry_or_listed_todo
- doc_render_matches_baseline                   (snapshot test)
- fixtures_valid_min_decode_ok
- fixtures_valid_rich_decode_ok
- fixtures_malformed_decode_fails_with_defined_error
```

### 11.2 Workspace cross-checks

```text
- compile-time: a deprecated free-form emit fails build under "deny
  legacy emit" feature flag.
```

### 11.3 Negative

| Marker                                                  | Profile     |
|---------------------------------------------------------|-------------|
| `NEG:SEMTOOLKIT:NOTE_MISSING_TAG_REJECTED`              | sem-toolkit |
| `NEG:SEMTOOLKIT:CATALOG_MISMATCH_REJECTED`              | sem-toolkit |
| `NEG:SEMTOOLKIT:FIXTURE_ROUND_TRIP_FAIL`                | sem-toolkit |
| `NEG:SEMTOOLKIT:DOC_RENDER_DRIFT`                       | sem-toolkit |

(All CI-only.)

---

## 12. Acceptance Criteria

```text
- Toolkit ships; catalog.toml is the single source of truth.
- All entries have notes files (or TODO entries ≤ 5).
- Typed emitter generated and used by at least 3 services in the
  workspace.
- Fixture set regenerates clean.
- ADR-v0.9-003 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/semantic/catalog.md          — generated
docs/src/semantic/author-guide.md
docs/src/development/semantic-toolkit.md
docs/src/adr/v0.9-003-typed-emitter.md
docs/src/adr/v0.9-003-notes-alongside-catalog.md
```

---

## 14. Open Questions

1. **i18n** — notes are English-only. If translations are needed in
   v1.x, add a `notes/<locale>/` tree; out of scope for v0.9.
2. **Cross-references** — notes "Related" sections are author-curated.
   A doc-renderer enhancement could derive these from the catalog's
   shared domain prefix; deferred.
3. **Inline previews in proxy-text** — could proxy-text show truncated
   notes for unknown intents? Out of scope; proxy-text uses tag form.

---

## 15. Release Gate (RFC-local)

```text
- Toolkit shipped.
- Generated docs + fixtures regenerate clean.
- ADRs Accepted.
```
