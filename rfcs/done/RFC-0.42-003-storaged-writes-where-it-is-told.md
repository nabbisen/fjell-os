# RFC-0.42-003: `storaged` writes where it is told, and the success it reports is the write that was asked for

**Status:** Accepted — accepted 2026-11-09
**Milestone:** 0.42
**Tracks.** **E-105** — `storaged` discards the first data word of every message, so every write
lands at sector 0 with part of each chunk lost and 8 bytes of every 32 zeroed, while `init`
reports success.
**Touches** *(indicative)*: `crates/services/fjell-storaged/src/main.rs`,
`crates/services/fjell-init/src/main.rs`, `crates/fjell-kernel/prebuilt/fjell-storaged.bin`,
`crates/fjell-kernel/prebuilt/fjell-init.bin`, `tests/repro/baseline-digests.txt`,
`rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md` (row 17 — the architect's).
**Relates to:** **RFC-0.41-006**, whose three natural captures are the evidence below and whose
`io incomplete` line is this line's cheapest verification; **E-100** and **RFC-0.40-003**, the
precedent that tests inside a `no_std` binary are not coverage a gate can run; **E-106**, which
owns `do_io`'s budget and is not touched here.

## What is wrong

**It is a wire-format disagreement between two services, not a bug in one.**

`init` documents the protocol and sends it that way
(`crates/services/fjell-init/src/main.rs:78-80`):

```
WRITE_BEGIN(lba_lo, lba_hi)   → ACK
WRITE_CHUNK(w0..w3) ×16       → ACK each  (16 × 32B = 512B)
WRITE_COMMIT                  → WRITE_OK or WRITE_ERR
```

and at `:85` sends `ipc_call(ep, WRITE_BEGIN, lba as usize, (lba >> 32) as usize, 0, 0)` — the low
half in the **first** data word.

`storaged`'s own `recv_call` says what it returns (`fjell-storaged/src/main.rs:352`):
*"Returns (tag, w0, w1, w2, w3)."* **Its only caller, at `:462`, binds that second value — a data
word — to `_badge` and throws it away:**

```rust
let (tag_packed, _badge, w1, w2, w3) = recv_call();
```

So every field is read one word late, and the file contradicts itself four lines apart.

**The two consequences:**

1. **Every write lands at sector 0.** `WRITE_BEGIN` computes `lba = (w1 as u64) | ((w2 as u64) << 32)`
   (`:467`), where its `w1` is `init`'s **`lba_hi`** and its `w2` is `init`'s third word, a literal
   `0`. For every LBA below 2³² — which is every LBA this project uses — `lba_hi` is 0, so
   **`lba = 0`**.
2. **Each 32-byte chunk loses 8 bytes and gains 8 zeroes.** `WRITE_CHUNK` copies
   `[w1, w2, w3, w4]` (`:478`) where `let w4 = 0usize` (`:463`) is a hard-coded word. So `init`'s
   `words[0]` is dropped and the fourth word is always zero.

**The evidence is already in the tree, from 0.41.** All three natural E-104 captures print
**`lba=0x00000000`** — every write this project has direct evidence of went to sector 0, exactly as
predicted. `init` asks for LBA **193** (`init/main.rs:723`) and `LBA_SUPERBLOCK_A` (`:741`).

**Nothing noticed, and that is a third defect.** `init` discards the ACK of **every** `WRITE_BEGIN`
and all sixteen `WRITE_CHUNK` calls — `let _ = ipc_call(…)` at `:85` and `:98` — and checks only
`WRITE_COMMIT`'s reply (`:101-102`). A protocol documenting `→ ACK` whose caller writes `let _ =`
is a protocol in name only.

**And the success report is truthful about the wrong operation.** `WRITE_COMMIT` returns
`WRITE_OK` because the write `storaged` performed did succeed — a 512-byte buffer written to sector
0. The falsehood is not in the reply; it is that nobody asked for that write. That is a sharper
thing than a wrong message, and it is why this is in scope now rather than left latent.

**Latent, and why that is not a defence.** Nothing reads the store back — `READ_CHUNK` is a
placeholder — and the `store` tier carries four markers with **no emitting scenario**
(`tests/qemu/profiles/store.toml`, `release_gated = false`). So no gate observes the damage. The
data is still wrong on the disk.

## Requirements

**R1 — `storaged` reads the four data words as four data words.** Fix the binding at `:462`, not
the protocol: `recv_call` already returns `w0` and already says so. **The word must be named for
what it is**; a rename to `_w0` that still discards it does not satisfy this.

**R2 — `WRITE_BEGIN` computes the LBA from the caller's first two words**, so an LBA of 193
arrives as 193.

**R3 — `WRITE_CHUNK` writes the caller's four words, and `let w4 = 0usize` is deleted**, not
renamed or set from somewhere else. Of every 32 bytes the caller sends, 32 must be written.

**R4 — `init` stops discarding the ACKs its own protocol documents.** The `let _ =` at `:85` and
`:98` become checked, and a failed `WRITE_BEGIN` or `WRITE_CHUNK` fails `storaged_write` rather
than being overwritten by a `WRITE_COMMIT` that cannot know. **This is why nothing caught R1-R3 for
as long as it stood**, so fixing the writes without fixing the blindness would leave the next such
defect equally invisible.

**R5 — the decode becomes testable by a gate that can run.** The word-to-`(lba, words)` decode is
pure, and today it lives inside a `no_std` binary where `#[cfg(test)]` is coverage no gate can
execute — **exactly E-100's shape, closed by RFC-0.40-003 by moving the tests somewhere a gate
runs.** Follow that precedent rather than inventing one. A test must exist that fails against the
current `_badge` binding.

**R6 — the fix is shown on a real boot, using 0.41's own instrument.** Run the throttled control
(`boot-loop --drive-extra ,throttling.bps-total=50`) and quote the `storaged: io incomplete …`
line: `lba` must be the LBA `init` asked for — **`0x000000c1` for 193** — and no longer
`0x00000000`. This costs nothing to run and turns E-104's diagnostic line into E-105's acceptance
test.

### Non-goals

- **Implementing `READ_CHUNK` or a read-back round trip.** That is a feature, and it is what the
  `store` tier would need to stop being NOT-GATED. **Not in this line**, and the tier stays
  `release_gated = false`; R5 and R6 are the verification instead.
- **Making the `store` tier gated.** Downstream of the above.
- **`do_io`'s 500,000-round budget**, which belongs to E-106 and 0.42-002. Do not touch the poll,
  the doorbell re-kick or the bound.
- **The `io incomplete` line's text or fields.** They are correct as of 0.41 and R6 depends on them
  being unchanged.

## Risks

- **`init.bin` moves as well as `storaged.bin`** because R4 changes `init`. Two prebuilts, and the
  repro baseline is re-recorded **in the same commit** — the trap that has gone red unnoticed
  before.
- **R4 can turn a previously silent failure into a boot failure.** If any `WRITE_BEGIN` or
  `WRITE_CHUNK` is replying with something other than an ACK today, `init` has never noticed and
  will now exit. **That is the correct outcome and must not be softened** — but it should be
  expected, and if it happens the reply value is the finding, not the check.
- **Writes now land where asked, which is new ground for the disk image.** Sector 0 has absorbed
  every write to date; LBA 193 and the superblock mirrors have not been written correctly before.
  A tier that passed over a store nobody wrote properly may behave differently.
