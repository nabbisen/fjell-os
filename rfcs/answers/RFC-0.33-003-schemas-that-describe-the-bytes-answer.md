# RFC-0.33-003 R1 and §A–§E: the layout lives in the encoder, not the struct

**Governing RFC:** [../accepted/RFC-0.33-003-schemas-that-describe-the-bytes.md](../accepted/RFC-0.33-003-schemas-that-describe-the-bytes.md)
**Handoff:** [../handoffs/RFC-0.33-003-schemas-that-describe-the-bytes/implementation-handoff.md](../handoffs/RFC-0.33-003-schemas-that-describe-the-bytes/implementation-handoff.md)

Written after R1 and before any code, in the handoff's order. Every absence is
probed with `/usr/bin/grep -a` and a control. **One honest ordering note first:**
R1 asks for the drift *per format, field by field, for all eleven*. I measured the
formats I could read by hand (below) and found the drift pervasive and of two
different kinds; the field-level table for all eleven is produced by the
instrument this line builds (its diff of generated against committed, taken
**before** D3 regenerates anything) and is appended to this document under
*R1 addendum* when it exists. It is not decided by, or designed around, before the
design below is written.

---

## R1 — what the tree says

| Probe | Result | Control |
|---|---|---|
| The eleven `.frozen` files | **eleven**, paths in `ci-schema-gate` match exactly | `find crates -name '*.frozen'` |
| `fjell-tools schema dump` exists | **no** — `main.rs` has no `schema` arm | the same probe finds `Some("test-all")` (1) |
| Anything reads a `.frozen` file | **only `ci.yml` and documents mention them**; no `.rs` file does | `/usr/bin/grep -a -rIln '\.frozen'` over the tree, excluding runs and history, lists 14 files, none Rust |
| `ci-schema-gate` | checks the eleven paths exist and are non-empty; its own comment says it "cannot detect drift" | read in place, `ci.yml:511–550` |
| The four E-055 sites | `fjell-init/src/main.rs` lines **596, 613, 633, 652**: `StoreSuperblock` (twice), `RecordHeader`, `BootControlBlock` | the kernel has 2, `fjell-init` 4, and **no other crate outside the kernel** has any (`grep -a -rn from_raw_parts crates`) |
| Anything reads a format back from disk | **no** — see below | the tree's only reader of block data would be `storaged`'s `READ_CHUNK` |

**Read-back, per format** (§E). `storaged`'s `READ_CHUNK` arm is
`reply(READ_CHUNK); // placeholder; READ not used in M7`, and **nothing calls it**
(`grep -a` for `READ_CHUNK`/`READ_BEGIN`/`storaged_read` outside `fjell-storaged`
finds only a comment in `fjell-init`). `bootctl` builds its block in memory. So for
`StoreSuperblock`, `RecordHeader` and `BootControlBlock` — the three that reach a
disk — **the answer is: nothing reads them back**, and D5's byte change is free
today. The eleven `.frozen` formats are not written to disk at all by anything I
found; they are **digest inputs and one codec** (next section), so "read back from
disk" does not apply to them and I say so per format rather than in general.

### What "the bytes" are, for the eleven — the finding that decides §A

I expected eleven wire formats. They are three different things:

| Kind | Formats | How the bytes are produced |
|---|---|---|
| **A digest input stream** | rollback record, release metadata, attestation v2, diag bundle, identity, board, keyring snapshot, measurement summary, release summary, snapshot envelope | a `compute_digest`/`*_digest` function feeding `Digest32::of_parts(&[…])`, or a local `w_u8!/w_u16!/…` macro writer into a stack buffer, or a streaming `DigestWriter` — **six different mechanisms** |
| **A real codec** | semantic intent v1 (`fjell-semantic-v1`) | `codec::encode`/`decode` with a catalogue |
| — | (`fjell-diag-format` has no encoder other than its digest writer) | |

So a frozen file for most of these is a description of **the canonical byte stream
that a hash is taken over** — which *is* a wire format in the sense that matters
(change it and every stored digest and signature stops verifying), but it is
produced by a function, and the *struct* is not what defines it.

### The drift — read by hand for the formats I could compare

| Format | Frozen file vs the code |
|---|---|
| **`rollback-record-v1`** | `channel u8[16]` is `channel_id [u8; 8]`; `min_counter u32` is `u64`; `updated_tick` is `last_advance_tick`; **absent from the file**: `last_advance_source` (`u8`) and the 32-byte zero placeholder for `record_digest` that the stream really contains. The RFC's table, confirmed. |
| **`release-metadata-v1`** | **not a stale description of `ReleaseMetadata` — a different structure.** The file has `release_id`, `channel [16]`, `counter u32`, `min_counter u32`, three digests and a signature; the code's stream has `channel_id [8]`, `release_counter u64`, `embedded_min_counter u64`, `release_manifest_digest`, `signing_anchor_epoch`, `trust_provider_id`, `measurement_at_stage`, `created_tick`, the provenance pair and a 32-byte placeholder. Nothing in the file corresponds to the signature it lists. |
| **`attestation v2`** | the file lists **17** fields; the stream has **~50** parts, including whole groups the file does not mention — keyring, boot, verification, snapshot, health, rollback, freshness, provenance. |
| **`diag bundle-v1`** | the file has `source_service`, `entry_count`, `entries[].{tag,tick,len,body}`; the stream has a `"FJELL-DIAG-V1"` prefix, `created_tick`, `measurement_head`, `last_attestation` and two counted groups of `{seq, tag, code, tick}`. Different. |
| **`keyring snapshot-v1`** | the file has `purpose_count` and `purpose[0..6].{purpose, epoch, anchor_bytes[64]}`; the stream has a `"SNAP-V1"` tag, a presence marker (`+`/`-`) per slot, and a per-anchor scratch of purpose, algorithm, authority and epoch, then `key_len` and the key bytes. Different. |
| **`identity` (`node-identity-v1`)** | **agrees** with the code and with the doc comment above it. |
| **`measurement-summary-v1`** | **agrees** with the code. |
| **`snapshot-v2`** | agrees in shape; the domain byte is **conditional on `schema_version >= 2`**, which a flat file cannot say without picking a version. |
| others (board, release-summary, semantic intent) | to be measured by the instrument; not asserted here. |

Two consequences. **Drift is not uniform**: some files agree, some lag, some
describe something else, and nothing says which — which is exactly what a
comparison test exists to say. And **a stream can be conditional or
data-dependent** (`schema_version`-gated bytes, counted groups, presence
markers), so a recorder must be able to run the encoder on a *representative
value* and not on "the struct".

---

## §A — How does the generator learn a layout? **By running the encoder.**

**The honest answer to the handoff's §0.1 is that it cannot learn it from the
struct.** A struct says what a value *holds*; the bytes a format *produces* are
decided by a function — which fields, in which order, at which width, gated by
which version. A generator that read struct fields would describe the wrong thing
(the digest stream for `RollbackRecord` contains a 32-byte zero placeholder that
is a field of nothing, and the release-metadata stream has parts the struct holds
under other names), and one that hard-coded fields would be E-045 again with more
steps. The alternative the
handoff sanctions — *the frozen file describes the encoder's output* — is not a
fallback here: it is the only description that can be true.

**The mechanism:** each format's byte-producing function is rewritten to write
through one small trait, `Canon`, whose calls carry the field's **name and type**:

```rust
fn write_canonical(r: &RollbackRecord, c: &mut impl Canon) {
    c.domain(ROLLBACK_RECORD_DOMAIN);
    c.u16("schema_version", r.schema_version);
    c.bytes("channel_id", &r.channel_id);
    c.u64("min_counter", r.min_counter);
    …
}
```

There are two sinks for the **same function**:

* a **hashing sink** collects the bytes and takes the digest — exactly what the
  format did before, checked bit for bit (below);
* a **recording sink** runs the function on a representative value and emits the
  sequence of `(name, type, width)` — which *is* the frozen file's body.

The description is therefore the encoder, read a second way. There is no second
list of fields to keep in step, because there is no second list. Change a field in
`write_canonical` and the recorder output changes with it; change the struct
without touching the encoder and **nothing about the bytes changed**, which is the
truth.

**The safety net for the refactor itself:** before touching any function I capture
**golden digests** from the *current* code for a fixed set of values per format;
each rewritten function must reproduce them exactly. A refactor that changes a
digest is a wire-format change, and this line is not allowed to make one by
accident.

**Where things live.** `fjell-canon` — a no_std, dependency-free leaf: the trait
and a fixed-buffer sink. The format crates depend on it. `fjell-schema` — a host
library, in `default-members` so **Gate 1 runs it**: the recording sink, the
registry of formats, the generator, and the comparison. It depends on every format
crate, which is why it cannot be a dev-test of `fjell-canon` (a crate testing its
own dependents sees two copies of itself). `fjell-tools schema dump` is the
deliberate way to regenerate; the test is what makes drift fail. Neither is a
kernel dependency.

## §B — Is the current DSL the right output? **Yes; and compare text.**

The `.frozen` grammar is line-oriented and readable, and nothing needs to parse it:
the test compares **generated text to committed text**. To *name the field* when
they differ, the comparison splits both into lines and reports by the field name
that starts each (`field <name> …`) — a split on whitespace, not a grammar. The
DSL grows only where the recorder needs it: a `note` line for the dated D3 record,
and the conditional/counted shapes the eleven really contain.

## §C — The formats with no file. **Add the three that reach a disk, and record the rest.**

Finding 4 named two disk structures; the tree has **three**: `StoreSuperblock`,
`RecordHeader` and `BootControlBlock` (E-055's four sites are those three, the
superblock twice). All three get a generated file. For every other crate under
`crates/formats/` (22) and the two outside it that define formats (`fjell-keyring`,
`fjell-semantic-v1`), the registry records either **a generated file** or **a
reason** — and a test fails if a format crate appears that the registry does not
mention, so "nobody wrote one" cannot recur. Whether each *reason* is true is part
of R6's evidence, each with a control; where a crate has a byte-producing function
and I cannot honestly exclude it, I say so in the review request rather than
recording a reason that is convenient.

## §D — One commit or two? **E-055 first, then generate — as the handoff orders.**

The disk structures' new serialisation (`write_canonical`, named fields, no
padding) is the first change; their frozen files are then generated from it. The
generator never describes a layout that is about to move. Because the disk
structures gain their encoder through the same `Canon` trait, "the function that
writes the sector" and "the description of the sector" are the same function from
the first commit.

## §E — Does anything read these from disk? **No — and it is free exactly once.**

Per format, above. `READ_CHUNK` is a placeholder nothing calls; the block is built
in memory. D5 changes the on-disk bytes of three structures that nothing reads, so
it is not a migration. **The window closes at the first read-back**, which is why
this line goes before persistence (RFC-0.33-001 §A). R1 re-derived that at this
tip rather than trusting the RFC's note.

---

## Order of work

`R1` (this document; the field-level table follows from the instrument) → capture
**golden digests** for every format → **D5/R5**: `StoreSuperblock`, `RecordHeader`,
`BootControlBlock` serialise named fields through `Canon`, `init` stops
reinterpreting struct memory, and the indeterminate-byte test → `fjell-canon` and
the eleven digest functions rewritten against the goldens → `fjell-schema`: the
recorder, the registry, the comparison → **the drift table, measured before
anything is regenerated** → **D3/D4**: regenerate toward the code, each file with a
dated note of what it used to claim → D6 coverage → **D7**'s three demonstrations →
R8 (`ci-schema-gate` retired) → R9 (ADR-v0.6-003, E-045 and E-055).

## Decisions I am making that the RFC did not specify

1. **The recorder runs the encoder** (§A) — this changes eleven digest functions'
   *shape* (not their output), which the *Touches* list does not name. The alternative,
   describing structs, would be false; I say so rather than doing it.
2. **Two new crates**, `fjell-canon` and `fjell-schema`, for the reason in §A.
3. **The dated note (D3) is hand-written text in the registry.** It is documentation
   of what a file used to claim, not a description of a layout, so it is not a second
   description of the thing being frozen.
4. **`schema_version` moves only where bytes moved** (D4) — and because the goldens
   hold the digest streams still, that is the three disk structures, and nothing
   else.
