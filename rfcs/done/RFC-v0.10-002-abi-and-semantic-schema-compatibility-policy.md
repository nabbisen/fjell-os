# RFC-v0.10-002 — ABI and Semantic Schema Compatibility Policy

**Status:** Implemented (v0.10.0)
**Target version:** v0.10.0
**Parent:** RFC 061 §9 (ABI freeze scope).
**Affects:** `fjell-sdk`, `fjell-syscall`, `fjell-service-api`,
    `fjell-semantic-v1`, `fjell-audit-format`, `fjell-bundle-format`,
    CI gates.

## 1. Problem

`SDK_API_REV = 1` exists (RFC v0.9-001) but no tooling enforces what
"stable" means. v1.0 cannot promise compatibility without:

- a written rule for what is in the stable surface,
- a CI check that flags breakage on PR,
- a documented migration story for the rare necessary break.

RFC 061 §9 sets the *scope*; this RFC sets the *policy*.

## 2. Stable surface

The surface frozen at v1.0 is exactly the items marked "Yes" in
RFC 061 §9. Restated here for completeness:

```
S1. Syscall ABI               — fjell-syscall:: pub fn sys_* + SysError
S2. Capability kinds & rights — fjell-cap::{CapKind, CapRights}
S3. Lease semantics           — fjell-abi::lease::{LeaseId, LeaseEpoch}
S4. Semantic catalog v1       — tag numbers, schema, owner mapping
S5. Audit record format       — AuditPersistRecord wire bytes
S6. Bundle format             — ServiceBundleHeader + bundle_digest
S7. Service IPC tags v0_7     — fjell_service_api::v0_7::*
S8. Boot control block        — BCB on-disk format
S9. CapManifest TOML grammar  — keys, types, lint rules
```

## 3. Stability tiers

Each item on the stable surface carries one of three tiers:

- **STABLE.** Removal, renaming, or signature change requires a v2.0
  release and at least one minor-release deprecation cycle.
- **PROVISIONAL.** Frozen for v1.0 but signature may change on
  v0.(x+1).0 boundaries before v1.0. Marked clearly in docs.
- **DEPRECATED.** Scheduled for removal at the next major. Replacement
  path documented per item.

The mapping from S1–S9 above to tiers is published at
`docs/abi/stability.md` and updated when items move.

## 4. Breakage rules

A change is "breaking" if it falls into any of:

- Removing a `pub` item from a STABLE surface.
- Changing a function signature, struct layout, enum variant set, or
  trait method set in a STABLE surface.
- Changing the wire bytes of S5, S6, S8.
- Renumbering or repurposing a catalog tag in S4.
- Removing a CapKind or CapRight bit in S2.
- Renumbering an IPC tag in S7.

Breaking changes between v1.0 and v2.0 are disallowed without a
deprecation cycle. Within v0.x, breaking changes are allowed but must
have a CHANGELOG entry under a `### Breaking` heading.

## 5. Schema migration: catalog v1 → vN

Semantic catalog migrations follow the v0.5-004 freeze policy plus:

- A new catalog version may not reuse a v1 tag for a different intent.
- A new catalog version may add tags in unused ranges.
- A new catalog version may **not** silently change a v1 schema.
- Bundles signed against catalog v1 remain valid under catalog vN.

## 6. CI enforcement

The following CI gate runs on every PR (`ci-abi-check`):

1. Build a snapshot of each stable surface (`pub` item names, function
   signatures, struct field lists, enum variant lists).
2. Diff against the snapshot checked into the repository
   (`tests/abi/snapshot.json`).
3. PR fails if any STABLE item disappears or changes signature.
4. New items are allowed without snapshot update (additive).
5. To intentionally change a STABLE item, update the snapshot in the
   same PR with a CHANGELOG `### Breaking` entry.

For binary formats (S5, S6, S8) the gate computes a digest of the
canonical encoding of a reference record and compares against a pinned
value.

The snapshot tool lives in `tools/fjell-abi-snapshot/`.

## 7. Acceptance criteria

1. `tools/fjell-abi-snapshot/` exists and produces a deterministic
   `snapshot.json`.
2. CI job `ci-abi-check` is wired into `.github/workflows/ci.yml`.
3. `docs/abi/policy.md` and `docs/abi/stability.md` exist; every
   STABLE surface item is listed with its tier.
4. A deliberate breaking change in a test branch is detected by the
   gate (proven by red CI on a fixture PR).
5. The current state of `fjell-sdk` re-exports is annotated with tiers
   matching RFC v0.9-001's `tier` module.

## 8. Out of scope

- Tooling for *automatic* migration of dependent code.
- ABI for non-Rust language bindings (deferred to v0.14 if pursued).
- Wire-level binary compat between different processor architectures
  (the audit/bundle formats are explicitly little-endian, defined in
  the relevant format crates; no further policy needed).
