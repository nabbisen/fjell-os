# RFC-0.38-001 R1–R9: the measurement chain measures something

**Governing RFC:** [../accepted/RFC-0.38-001-the-measurement-chain-measures-something.md](../accepted/RFC-0.38-001-the-measurement-chain-measures-something.md)
**Handoff:** [../handoffs/RFC-0.38-001-the-measurement-chain-measures-something/implementation-handoff.md](../handoffs/RFC-0.38-001-the-measurement-chain-measures-something/implementation-handoff.md)
**Design ruling followed:** the RFC's "Design ruling (R2 escalation), 2026-10-22", recorded in `.git-exclude/reviewed/review-record-rfc-0.38-001-r2-escalation.md`.

Written against base `de7b91f`. The R1 re-derivation and the placeholder sweep were done
before any code (they are in the review request, which carries the two escalations); this
document records what was built and the evidence for each requirement. Gate results are
in the review request.

---

## R1 — re-derived at `de7b91f`, with controls

Probes run with `git show` / `git grep` against the commit, not the working tree. The
kernel probe has a positive control: the same pattern is found in the format crate.

| Claim | Found at `de7b91f` |
|---|---|
| `init` appends four events with literal subjects | `fjell-init/src/main.rs:955` (boot evidence, literal `0x0011`) and `:970` (a loop of three, literal `0xAB`) — four events from two call sites |
| `init` prints a literal sequence | `emit_measurement_status(4, 0)` at `:1025`, after the comment "approximate: seq = number of APPEND_EVENT calls made = 4" |
| `measuredd` replies 8 of 32 bytes | `GET_HEAD` at `fjell-measuredd/src/main.rs:211`; `APPEND_OK` carried `seq` and `cd_lo` (`:206`) |
| The kernel computed no digest | `git grep -c 'Digest32::of'` over `crates/fjell-kernel/src` at `de7b91f`: **no match**. Control: the same pattern over `fjell-measure-format/src` — 5 in `digest.rs`, 2 in `event.rs` |
| `GET_HEAD` sites | declaration (`fjell-service-api`), `diagnosticsd:144` (a one-way send), `measuredd:211` (the handler). Three code sites, as the RFC's correction says |

The sweep for other placeholder digests, and the finding that `attestd`'s signed record is
built from constants (E-094), are in the review request; the architect ruled on both.

## R2 — what measures what, and how it travels (per the ruling)

**Where it is computed:** in the kernel, at boot, from the bytes it embeds with
`include_bytes!`. `kernel/src/boot_info.rs::image_set()` scans image ids `0..=u16::MAX`,
asks `image_bytes(ImageId(id))` for each, takes `SHA-256` of the whole flat binary, and
calls `fjell_measure_format::compute_image_set_digest`.

**The digest** (`fjell-measure-format/src/image_set.rs`, frozen description
`schema/image-set-digest-v1.frozen`):

```
digest_i = SHA-256(bytes_i)
set      = SHA-256( "FJELL-IMAGE-SET-V1" ‖ u16 count ‖ Σ ( u16 image_id ‖ digest_i ) )   ascending id, ≤ 64
```

The count is in the preimage (ruling 2). The domain string and layout are a named constant
and a documented encoding, written through `Canon` like the chain digest. Empty, too many,
and non-ascending sets are refused. `compute_chain_digest`'s frozen stream is untouched:
all twenty existing `.frozen` files are byte-identical and only the new one is added.

**How it travels (ruling 1, overruling the entry-register option):** `BootInfo` v2.
The kernel allocates a frame (`FrameOwner::UserData`), writes `BootInfo::to_bytes()` into
it, maps it read-only user (`VmRegionKind::UserRodata`) at `BOOT_INFO_VA` (one guard page
above the stack top), and passes the VA in `a1`. `BootInfo`'s contract is its 56-byte wire
encoding — little-endian, declaration order, no padding; `#[repr(C)]` is removed because
the in-memory layout is not what is transferred. `from_bytes` accepts `version >= 2`, refuses
`Short` and older versions. The five legacy capability fields are kept so the prefix does
not move and hold `ABSENT` (0xFFFF); no kernel ever filled them.

`init` reads the page once (`copy_nonoverlapping`), then **exits 1 loudly** if it is missing
or malformed, if the count is 0, or if the append fails — a boot that cannot measure does
not report one.

**Why a new op rather than widening `APPEND_EVENT`:** a request carries four words and one
is the kind word, so a 32-byte digest cannot ride beside it. `APPEND_IMAGE_SET` (0x30A)
carries the four words of the digest and nothing else; `measuredd` supplies the event kind
(`BootEvidenceImported`), source (`Kernel`) and subject (`BootEvidence`). `init` can therefore
not label the event as anything else. This needed `fjell_syscall::sys_ipc_call4`, a
wrapper over the existing `IpcCall` syscall that returns four reply words (a new snapshot
item, no new kernel syscall).

## R3 — boot cost, measured

`rdtime` is not usable from S-mode here (the M-mode shim never sets `mcounteren`), so the
cost was measured from the host: timestamping serial lines as they arrive, 25 boots each.

| | span containing the hash | boot to init exit |
|---|---|---|
| with the hash | 3.43 ms | ≈ 452 ms |
| control: hash removed, everything else identical | 0.25 ms | ≈ 478 ms |

The hash costs **about 3.2 ms** for the embedded images. *(Architect, 2026-10-24: this said "for 154,972 bytes"; the 33 prebuilts total **158,660** bytes at the tip — 154,972 was the total before this line rebuilt three of the images it measures. The conclusion is unaffected.)* The end-to-end difference is
inside the noise (the control was slower). No tier's timeout was moved; the new tier has a
60 s timeout and the `m8` tier is unchanged.

## R4 — E-085

* `reply()` in `measuredd` carries four words; `HEAD_REPLY` carries the whole chain digest
  in w0..w3. `digest_to_words` / `words_to_digest` in `fjell-service-api` are the one
  conversion (with a `usize::BITS == 64` assertion and a round-trip test).
* `GET_HEAD_STATS` (0x30B) / `HEAD_STATS_REPLY` (0x30C) carry `(latest_seq, dropped,
  last kind)` on their own tag. 0x30A–0x30C were confirmed free tree-wide.
* `APPEND_OK` carries only `seq`.
* `diagnosticsd::query_measurement_head` is now a real `sys_ipc_call4` and reads all four
  words. The old code sent one way and then `recv`'d on measuredd's own endpoint, which
  could never have worked.
* No chunking. `EXPORT_LOG` (E-087) is untouched apart from its helper's arity.

`init` no longer prints a literal: `emit_measurement_status` takes the sequence and dropped
count from `GET_HEAD_STATS`, and the sequence passed to `GENERATE` is the same value.

## R5 — the whole digest arriving, with a scratch capability

Slot 4 was installed in a scratch change (a `CALL` capability for `diagnosticsd` to
`measuredd`), the `remote-diag` tier run, and everything reverted.

* `measuredd` sent chain head `248b098422c551b9f2577387755d91f10a41c3f7c1dcde4ba3fddc2846ec2a9c`.
* `diagnosticsd` received the identical 64 hex digits (all four words).
* A Python replay of the four events (`chainhead.py`, standalone, using only `hashlib` and
  the stream documented in `chain-digest-v1.frozen`) produces the same head.

**The revert, proved:** the scratch marker grep on the committed tree finds nothing, with a
positive control on a string that is present; the 33 prebuilt hashes and the kernel hash are
identical to their pre-scratch values; `remote-diag` passes unchanged with
`measurement_head_available = false`. Slot 4 is out of the committed tree.

## R6 — the comparison instrument

`cargo xtask qemu-run` profiles gain `expect_image_set_digest = [ "<prefix>", … ]`. For each
prefix the serial log must contain a line where the prefix is followed by **exactly the 64
hex digits the host recomputes** from `crates/fjell-kernel/prebuilt/*.bin` (equality, and not
followed by more hex). It writes `runs/<id>/image-set-digest.txt` and counts as a case, so a
profile using it is not a placeholder.

**Independence from the code under test.** The host side (`fjell-tools/src/image_set.rs`)
uses the external `sha2` crate, assembles the preimage by hand without importing the format
crate, and maps file → image id from the `fjell-<x>.bin` → `ImageId::<X>` naming and
`fjell-abi/src/service.rs`, not from the kernel's `image.rs`. A unit test checks that it
agrees with the in-tree implementation on the real prebuilts — the in-tree digest is checked
against the independent one, not against a second in-tree copy.

New tier `measurement` (`tests/qemu/profiles/measurement.toml`) asserts the `M8:` markers and
both digest lines (the kernel's `boot: service image set: N images, digest sha256:…` and
`measuredd: image-set digest …`, which measuredd prints from the *stored* event).

**Four derivations of one value, 33 images:**
`c6af3c85d3b1dfa0df830f9157a0288a24cecbb6598a747ed4337ee28815b720` — the kernel's line, the
`measuredd` line, the host `sha2` recomputation, and coreutils `sha256sum` plus a Python
reconstruction (`crosscheck.py`).

**It fails when it should — three failures shown:**

1. *Pre-fix tree.* A worktree at `de7b91f` (created and removed through git) run with the new
   tooling: **red**, no digest lines at all.
2. *Image changed after the kernel was built.* One byte of `fjell-powerd.bin` altered, kernel
   not rebuilt: both lines **MISMATCH** against the new host value `535d63db…`, though a
   well-formed non-zero digest is present — exactly what "merely non-zero" would have
   passed. The file was restored byte-identically.
3. *Corruption in transit.* A scratch `init` zeroed word 3 of the digest before sending: the
   kernel line equals the host value, the `measuredd` line **MISMATCH** (last 8 bytes zero).
   Reverted; all 33 prebuilt hashes identical.

## R7 — the history is not retro-claimed

* Of the four events, **one is real** (the image-set digest). The other three (`0xAB`) are
  still placeholders and `init` labels them so (`append_placeholder_event`).
* `docs/src/architecture/measurement-and-attestation.md` is rewritten to say: only the
  service images are measured; one event of four is real; the attestation record does not
  capture the chain (E-094); the signature is the dev keyed-SHA-256 provider.
* `v1-limitations.md` has a new **row 14** saying what the chain measures and what it does
  not, and nothing in this line implies attestation became real.

## R8 — E-085 and E-086 closed

* ERRATA: E-085 and E-086 carry dated closure blockquotes (originals left as written);
  both Summary rows read `RFC-0.38-001 | CLOSED`. E-087 is untouched (ACCEPTED).
* `v1-limitations.md`: the E-085 and E-086 bullets are marked CLOSED with the old text kept.
* `measurement_head_available` is still `false`; `remote-diag.toml` changed by a comment
  only and still passes.

The E-086 closure names what survives it (three placeholders, service images only, relay not
attested, E-094, flag false). The architect may prefer those split into a new erratum; I
named them in the closure rather than leave them implied.

## R9 — gates and binaries

See the review request for each gate by exit status. The three prebuilts that moved
(`fjell-init`, `fjell-measuredd`, `fjell-diagnosticsd`) were rebuilt first and the baseline
re-recorded afterwards; exactly three baseline lines changed and the three `.bin` files are
in the same commit.

## Left alone, and noticed

* `BootInfo::SLOT_*` constants have wrong values (0..3) and nothing uses them; left as they
  were, noted in the type.
* `decode_source(0x01)` maps `Kernel` to `Measuredd` on the legacy path, now unreachable.
* `tests/qemu/profiles/README.md` is stale; `docs/src/abi/v0.1-inventory.md` is frozen history.
* Counts such as "32 tiers" in docs and READMEs will need updating at the cut.
* Nothing in this line measures the kernel image itself; `BootEvidence::kernel_digest` still
  has zero read sites.
