# RFC-0.35-004 R1–R8: two digests move onto `Canon`, against goldens captured first

**Governing RFC:** [../done/RFC-0.35-004-two-digests-that-describe-themselves.md](../done/RFC-0.35-004-two-digests-that-describe-themselves.md)
**Handoff:** [../handoffs/RFC-0.35-004-two-digests-that-describe-themselves/implementation-handoff.md](../handoffs/RFC-0.35-004-two-digests-that-describe-themselves/implementation-handoff.md)

Written in the order the handoff requires: goldens from the unmodified
encoders before any edit, the chain digest moved before the bundle digest,
everything after ordinary.

---

## R1 — Re-derivation: both claims confirmed, nothing escalated

**`compute_chain_digest`** (`event.rs:133`, unchanged at this tip): ten
parts — `CHAIN_DOMAIN` (20 bytes), `u16` LE schema version, `u64` LE seq,
three `u8` enum discriminants, a 32-byte subject digest, a one-byte
presence flag, a **fixed** 32-byte metadata slot (`[0u8; 32]` when
absent — the array is always written, only its contents differ), and the
32-byte previous digest. **Confirmed: no new `Canon` primitive needed.**
The metadata slot is a presence byte plus `bytes`, not `optional` —
`optional`'s body is written only when present, and this stream's width
never depends on that.

**`compute_bundle_digest`** (`lib.rs:204`, unchanged at this tip): seven
parts — `BUNDLE_DOMAIN` (15 bytes), an unlengthed `name`, three `u32`
**big-endian** (`to_be_bytes()`, confirmed by reading the body, not
assumed), a 16-byte manifest digest, a 32-byte binary hash. **Confirmed:
the one gap is big-endian `u32`.**

Both claims held at the current tip; no escalation needed.

## R2 — Goldens, from the unmodified encoders, first

Added to each crate's own test module, before touching either encoder:

- **Chain**: a metadata-absent and a metadata-present digest (same
  seq/kind/source/subject/subject-digest/previous, differing only in
  `metadata_digest`), plus the raw 130-byte stream for the absent case,
  reconstructed by hand and asserted to hash to the same digest.
- **Bundle**: a short name (`"fjell-example"`) and a name at exactly
  `SERVICE_NAME_MAX` (the unlengthed-name argument's own stress case), plus
  the raw 88-byte stream for the short case.

All four digests and both raw streams captured with `eprintln!`/forced
assertion failures against a plain scratch tool build, then pinned as the
real test assertions. Both crates' full suites pass with the goldens in
place, **before** either encoder moved.

## R3 — `Canon` gains exactly one primitive; 18 unchanged, 20 total

`u32_be`, with the same treatment RFC-0.34-003's five additions got: a
default (`self.bytes(name, &v.to_be_bytes())`) that writes exactly what
every existing big-endian write already wrote, so byte sinks need no
override. Only the recording sink is overridden, to record `"u32 BE"`
rather than an undifferentiated `u8 [4]` — the whole point of a named
primitive instead of a byte array nobody can tell apart from four
arbitrary bytes. `cargo test -p fjell-schema --test frozen`: **16/16
pass**, including `every_committed_file_is_what_its_encoder_generates` —
**the existing eighteen files are untouched** by adding the method.

Both encoders then moved onto `Canon` via a private `write_*_canonical`
function and a fixed-capacity `BufSink`, with `compute_chain_digest`/
`compute_bundle_digest`'s own public signatures **unchanged** (no caller
outside either crate sees a difference). Both crates' full test suites,
goldens included, pass byte-for-byte after the move:

```
fjell-measure-format: 12 passed, 0 failed
fjell-bundle-format:  13 passed, 0 failed
```

`fjell-canon` gained one new test (`u32_be_writes_big_endian_unlike_u32`)
proving the default differs from plain `u32`'s little-endian write.

## R4 — The unlengthed `name`, written down

`compute_bundle_digest`'s `name` carries no length prefix. The argument,
now in the code (`write_bundle_canonical`'s own doc comment) and in the
generated file itself, not merely in this document:

> every field after `name` is fixed-width (three `u32`, a 16-byte manifest
> digest, a 32-byte binary hash), so the total stream length determines
> where `name` ends. That is safe *because* of this fixed trailer, not in
> general — a future variable-width field added after `name` would
> reintroduce the ambiguity a length prefix exists to remove.

Written via `c.var_bytes("name", name, "unlengthed")` (the type renders as
`u8 [unlengthed]`, visibly distinct from a length-prefixed field) plus a
`c.note("name_length", …)` carrying the full argument, which the generated
file prints immediately beside the field — not in a comment a reader of
the `.frozen` file would never see.

## R5 — Both files say what they do not cover

The caveat every *decoded* format already states (*"the decoder is a
second, hand-written reading and is not described"*) does not apply here —
there is no decoder. The honest, stronger caveat is the opposite:
**nothing reads either stream back**, so the file and the encoder are the
only description a second implementation has. Both registry entries carry
exactly this, word for word adapted to each (`chain-digest-v1`:
*"the measurement chain's whole point is a digest, not a decoded
record"*; `service-bundle-digest-v1`: *"`verify_bundle` only recomputes and
compares, it does not decode a reader's independent reading"*).

## R6 — The format's own version, or the honest absence of one

**Chain digest:** `format_version: Some((|| fjell_measure_format::SCHEMA_VERSION, "chain"))`
— the header reads `# version: v1 chain (first generated in v0.35.0)`,
the format's own constant, which is literally the second field the stream
writes.

**Bundle digest:** `compute_bundle_digest` never reads
`BUNDLE_SCHEMA_VERSION` (it exists in the crate, written into the bundle's
on-wire *header*, but not into *this* digest's bytes). `format_version:
None` — and, per R6's own instruction not to leave the header merely
silent about it, a `note` stating the absence explicitly, printed in the
generated file:

```
# note: this stream carries no version field of its own: `BUNDLE_SCHEMA_VERSION`
  exists in the crate and is written into the bundle header, but
  `compute_bundle_digest` never reads it …
```

## R7 — E-065 CLOSED, survivors down to E-073's two

`rfcs/ERRATA.md`'s E-065 entry carries a dated CLOSED blockquote
confirming R1's re-derivation, the goldens, the one primitive, the
unchanged eighteen, R4's argument, R5's caveats and R6's version handling,
and the survivor count. `docs/src/releasing/v1-limitations.md` in the same
commit, including the stale "eighteen (seventeen when this was closed…)"
sentence corrected to twenty, naming which two lines added the nineteenth
and twentieth. `fjell_schema::registry::EXCLUSIONS` no longer lists
`fjell-bundle-format` or `fjell-measure-format`; `fjell-audit-format` and
`fjell-net-format` (both **E-073**) are the only two `Survivor` entries
left — confirmed by count, not assumed, and `frozen.rs`'s own four
bookkeeping tests (every crate described or excused; a survivor names a
real erratum; a survivor's probe still finds a producer; the per-format
round trip) all re-run clean.

## R8 — The gates, each by its own exit status, as the required set

| | |
|---|---|
| `cargo fmt --all --check` | **exit 0** |
| `abi-snapshot --verify` | **PASS**, exit 0 — 453 → 454 (regenerated: one addition, `fjell-bundle-format::fn write_bundle_canonical`, the one newly-public function the census needed to reach; `write_chain_canonical` is in a crate `abi-snapshot` does not scan) |
| `release-rehearsal` | **exit 0 — ALL MECHANICAL GATES PASS** |
| `consistency-check --all` | **exit 0 — PASS, 20 subchecks** (unchanged count — no subcheck added or removed this line); `errata-limitations: PASS (10 ACCEPTED)` |
| `cargo test -p fjell-schema --test frozen` | **16/16**, including the four bookkeeping tests the move touches |
| `fjell-canon`, `fjell-measure-format`, `fjell-bundle-format` host tests | **11, 12, 13 passed, 0 failed** |
| `test-all` | **30 PASS, 0 FAIL, 1 NOT-GATED** (tier 26, QEMU negative: store — pre-existing, documented in v1-limitations.md, not introduced by this line) |
| `repro-check` | `--skip-build` **PASS, 32 artefacts identical** — no guest code touched this line (every change is in host-side format/schema crates), no rebuild, no baseline re-record |
| CI run id | none — nothing pushed |

## Decisions I am making

1. **`write_chain_canonical`/`write_bundle_canonical` are new `pub fn`s**,
   not `pub(crate)`, because the schema census (`fjell-schema`, a different
   crate) must call them to record a description. This is the same
   shape every other moved format already uses (`write_canonical` methods
   and free `write_*_canonical` functions alike are `pub`).
2. **The bundle digest's `BufSink` is sized to `SERVICE_NAME_MAX`, not a
   larger margin.** `BufSink`'s own documented contract is that overrunning
   its declared capacity is a bug, not a truncation to tolerate — sizing to
   the system's own declared maximum is the correct application of that
   contract, not a risk to pad around.
3. **No sample struct added for "a chain event" or "a service bundle"
   beyond what `samples.rs` already models.** Both sample functions return
   a real `MeasurementEvent` / `ServiceBundle` built through each crate's
   own public constructor (`MeasurementEvent::new`, `build_bundle`),
   matching the file's own stated purpose — "built through each crate's
   public API" — rather than a bespoke tuple just for the registry.

## Afterword

The two version claims in the register's own text — "big-endian integers"
(plural) and an implied endianness system — were the first thing R1 found
wrong, before any code changed: both encoders needed exactly one method,
not a mechanism. Re-deriving before writing, as every line in this family
has required, is what kept this the smallest of the three lines that have
used this method, as the governing RFC itself predicted it would be.
