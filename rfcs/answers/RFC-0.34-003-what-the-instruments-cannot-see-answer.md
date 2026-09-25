# RFC-0.34-003 R1 and §A–§E: what the wire codec's description can and cannot say

**Governing RFC:** [../accepted/RFC-0.34-003-what-the-instruments-cannot-see.md](../accepted/RFC-0.34-003-what-the-instruments-cannot-see.md)
**Handoff:** [../handoffs/RFC-0.34-003-what-the-instruments-cannot-see/implementation-handoff.md](../handoffs/RFC-0.34-003-what-the-instruments-cannot-see/implementation-handoff.md)

Written after R1 and before any code. Tip at R1: `f0c4b02` plus the neg-test fix
(RFC-0.34-002). Absences probed with `/usr/bin/grep -a` and a control.

---

## R1 — what the tree says

### E-067

| Probe | Result | Control |
|---|---|---|
| Items in `tests/abi/snapshot.json` | **442** — 262 `const`, 126 `fn`, **33 `struct`**, **19 `enum`**, **1 `trait`**, 1 `type` — the file's own declared `count` is **442** | the kinds sum to the declared count |
| **The RFC and the handoff say 443. It is 442.** | one item off; the six kinds the RFC lists are right, and their sum is 442 | the declared `count` in the file |
| Structs per crate | `fjell-cap` 8, `fjell-semantic-v1` 7, `fjell-abi` 6, `fjell-audit-format` 4, `fjell-sdk` 3, `fjell-bundle-format` 2, `fjell-service-api` 2, `fjell-syscall` 1 | sums to 33 |
| The one trait | `fjell-cap` | — |
| The test that asserts the blindness | `a_braced_structs_fields_are_still_outside_its_hash` in `tools/fjell-abi-snapshot/src/main.rs`, which this line inverts | it passes today: two structs differing by a field hash alike |

### E-065's first survivor

| Probe | Result | Control |
|---|---|---|
| `crates/formats/fjell-semantic-format/schema/` | **absent** | `crates/formats/fjell-upgrade-format/schema/` exists and lists `boot-control-block.frozen`, `release-metadata-v1.frozen` |
| `wire.rs` | **981 lines** (lines 1–617 code, the rest tests; 17 `#[test]`) | — |
| Which functions **produce bytes** | `encode` (public), and the private `put_text`, `put_token`, `put_opt_u64`, `put_cap`, `put_action`, `put_intent`, `put_fact_value`, `put_state`, `put_event`, over a `Writer` whose methods are `bytes/u8/u16/u32/u64/i64` (all little-endian) | `decode`, `decode_exact` and the `get_*` functions **consume** bytes and produce none |
| Who links it | `fjell-init`, `fjell-sample-service`, `fjell-semantic-stream`, `fjell-proxy-text`, `fjell-proxy-braille`, and `fjell-service-api`'s `receive_framing` test | — |
| **The RFC's and the handoff's description of the codec is partly wrong.** They say it has *chunked frames … and a sentinel* | **`wire.rs` contains neither** (`grep -a -c -i 'sentinel\|chunk'` → **0**). Chunked framing is `fjell-service-api`'s transport layer (`chunked`, `FrameError`; `grep -a -rl -i chunk` finds it in that crate's `lib.rs`); a trailing sentinel is `fjell-semantic-v1`'s codec, converted in RFC-0.33-003. What this codec *does* have: **a tagged payload (three arms), optional fields (a presence byte), a tagged union with five arms (`FactValue`), nested reusable tokens, and counted groups.** | control: both words are real in the two other crates |
| Does `SemanticEnvelope` have `PartialEq`? | **No** (`Clone, Copy, Debug`) | — so a round trip is checked by re-encoding: `encode(decode(encode(x)))` must equal `encode(x)`, plus `Debug` equality |

---

## §A — what describes a decoder? **Nothing does; the file says so, and a round trip is the check.**

The `.frozen` file describes the **encoder**: the sequence of bytes `encode` writes. The
decoder (`decode`, `decode_exact`, the `get_*` functions) is a **second, hand-written
reading of the same layout**, and a decoder that reads different fields than the encoder
writes is the class E-045 hid. So:

1. **A round trip per sample**, in Gate 1: `decode_exact(encode(x))` succeeds, and
   `encode` of the result is **byte-identical** (and `Debug` output equal). It checks the
   decoder against the encoder on every sample; it is a check, not a description.
2. **The file's header states what it does not cover**, so a reader cannot infer the
   file blesses the decoder: *"Describes what `encode` writes. **Not described:** what
   `decode` accepts or refuses — the tag tables it maps back, the count limits it
   enforces, the stream/payload agreement it demands (`StreamPayloadMismatch`), the
   trailing-byte rule of `decode_exact`. A decoder that read a different layout would
   not be caught by this file; the round-trip test is the only check."* The same words go
   in the crate's doc.

## §B — can `Canon` express this codec? **Nearly; here is what it gains and the gap I name.**

The nine converted formats plus the semantic-v1 codec were flat field lists with counted
groups. `wire` adds four shapes, and `Canon` gains a method for each — **with a default
implementation that writes exactly what the codec always wrote**, so the byte sinks
(`BufSink`, `SliceSink`, the snapshot's `DigestWriter`) are untouched and every existing
description is byte-identical:

| Shape in `wire` | `Canon` gains | What it writes |
|---|---|---|
| a reusable nested value (`BoundedText`, `TextToken`, …) at many places | `scope(name, f)` | nothing; **names** its fields (`title.id`, `title.fallback.len`) |
| a presence byte then a body (`Option<u64>`, `Option<CapabilityRequirement>`) | `optional(name, present, f)` | the byte `0`/`1`, then the body if present |
| a tag byte then the arm it selects (the payload, `FactValue`) | `variant(name, tag, label, f)` | the tag byte, then the arm |
| `i64` | `i64(name, v)` | 8 little-endian bytes |

**The recorder must see every arm to describe it, and one encoder run shows only one.**
A description of a tagged union is the *union of what each arm writes*, so a format with
such shapes is recorded from a **set of samples** and their recordings are **merged**
(fixed nodes must be identical across samples — a mismatch is a generation error, as an
element-shape mismatch already is; optional and variant arms are unioned). The sample set
must exercise every arm: an arm no sample reaches is **absent from the file**. Two guards
against that: (i) an `optional` whose body was never observed is a generation error;
(ii) the samples select arms through **exhaustive `match` helpers**, so adding a
`FactValue` or payload variant to the model is a compile error in the sample set rather
than a silent hole in the description.

**Where the description stops — named, not papered over:**

1. **The values behind a tag.** A tag byte is a `u8` with its label and value (`tag 1
   intent`); an *enum's* tag table (which byte means `Severity::Critical`) lives in the
   `tagged!` tables and appears in the file only as a `u8` field. The same limit as every
   ordinary field's value (RFC-0.33-003, ADR-v0.6-003): names, types, widths, order,
   capacities — not what a byte means. The goldens hold the values.
2. **The encoder's refusals.** `encode` returns `BufferTooSmall` and `TooLong` with a
   defined precedence (whichever happens first in write order). That is behaviour, not
   layout, and is not described; it is *preserved* by the rewrite and held by the
   existing error tests plus the goldens.
3. **A count and its group are related by adjacency**, as in the ten existing formats
   (`action_count` then `group actions`); the file does not encode that the count *is*
   the group's length.

I am **not** inventing a second notation for any of it. If a shape had resisted `Canon`,
the description would stop there and be named a survivor; none did.

## §C — hashing a struct's body without making Gate 4 noisy. **The rules, stated in the tool.**

They are in the tool's module doc (a table), in the order the handoff lists:

| | | why |
|---|---|---|
| comments, doc comments, whitespace, layout | **out** | cannot change the ABI; a hash that moves on a reflow is regenerated unread |
| `#[repr(...)]` | **in** | it *is* the layout |
| `#[non_exhaustive]` | **in** | changes what a downstream `match` may assume |
| `#[derive(...)]` and other item attributes | **out** | trait impls and lints, not shape; extended and reordered as routine. **Cost stated:** removing `Copy` is not seen |
| a member's own attributes | **in** (part of its text) | rare, and on the member |
| **private fields** | **in** | they set the size and layout of a `#[repr(C)]` struct |
| **field / variant / item order** | **in** | field order is layout; variant order renumbers |
| a trait item's **default body** | **out** | behaviour, not surface — the signature before `{` is in |

**The control the handoff demands** — *a reflow that does not move the hash* — is a test:
the same struct written one-line, multi-line, with doc comments, trailing comments,
different alignment, and with a `#[derive]` and `#[must_use]` added hashes **identically**;
and the mirror: `#[repr(C)]` added, a field added, removed, retyped, renamed, reordered or
made private each **move** it. **One consequence to say now:** including the `repr`
attribute *also* moves the hash of every enum that has a `#[repr(...)]` (RFC-0.33-004 D5
hashed enums by variants only), so the enums are re-recorded again, in this line's single
re-record. That is by construction, not drift, and R4 reads the real drift separately.

## §D — order. **E-067 first, as the handoff says.**

E-067 (the hash, the inverted test, the re-record read by the tag walk) → then the
codec: goldens captured from the **unmodified** encoder → the `Canon` extension and the
recorder merge → the rewrite → the generated file, the round trip, Gate 1 → the
demonstrations → counts → errata. With E-067 in place, the codec's own ABI additions
(`write_canonical` in `fjell-semantic-format`) are visible to Gate 4 by body.

## §E — the version. **A field like any other, and the header says the wire version.**

`wire` carries **two** versions: `WIRE_VERSION` (the encoding's, refused if unknown by the
decoder) and the model's `schema_version`. Both are written as ordinary `u16` fields and
appear in the file as such, and the header states the **wire version** the way the disk
structures' headers state their on-disk version (RFC-0.33-003 D11):
`# version: v1 wire (first generated in v0.34.0)`, read from `WIRE_VERSION`. The file
does not bump it: **no byte moves**, which the goldens enforce.

---

## Order and what I will not do

E-067 → codec, each step green before the next. **Not touched:** E-065's other four
survivors; the wire format's design, version or error precedence; the kernel's raw
`AuditRecordBin` write; the number of crates the snapshot scans. **E-065 is updated, not
closed**: its first survivor resolved, four remain.

## Decisions the RFC did not specify

1. The corrected item count (442, not 443) and the corrected description of the codec (no
   chunking or sentinel in `wire.rs`).
2. `--dump-enums` becomes `--dump-members`, covering enums, structs and traits (the old
   name stays as an alias).
3. Tag arms are named by a **label** (`intent`, `state`, `event`; `bool`, `u64`, `i64`,
   `text`, `ratio`) so the file reads, and merging keys on it.
