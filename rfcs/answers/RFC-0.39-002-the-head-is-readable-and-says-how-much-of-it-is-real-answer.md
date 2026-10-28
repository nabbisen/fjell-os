# RFC-0.39-002 R1–R7: the head is readable, and says how much of it is real

**Governing RFC:** [../done/RFC-0.39-002-the-head-is-readable-and-says-how-much-of-it-is-real.md](../done/RFC-0.39-002-the-head-is-readable-and-says-how-much-of-it-is-real.md)
**Handoff:** [../handoffs/RFC-0.39-002-the-head-is-readable-and-says-how-much-of-it-is-real/implementation-handoff.md](../handoffs/RFC-0.39-002-the-head-is-readable-and-says-how-much-of-it-is-real/implementation-handoff.md)

Written against base `7de0fcd` (RFC-0.39-003's tip). Order followed: R1, the tier red (R5), R3, R2, R4,
the tier green, R6, R7. The install and the disclosure are **one commit**, as the handoff's trap 1 requires.

---

## R1 — re-derived, with controls

* **RFC-0.39-001 landed first and was accepted:** `b9a38b5`, `8f65db8`, `2283250`; accepted 2026-10-27.
* **Slot 4 was declared and uninstalled, and the waiver named this line:** `capability_topology.rs`'s
  `DIAGNOSTICSD` slot 4 waiver, whose own text said installing it "belongs to the line that flips the flag".
  `spawn.rs`'s `DIAGNOSTICSD` block installed slot 1 only.
* **`measurement_head_available` is derived, not hardcoded:** `build_bundle` took it from
  `query_measurement_head`'s `(Digest32, bool)`, `true` only when a real `HEAD_REPLY` was received.
* **`remote-diag` asserted `measurement_head_available=false`** (and the three other flags `false`).
* **The encoder-zeroing pattern** is in `fjell-diag-format/src/bundle.rs::write_canonical`: for each
  availability flag, a fixed-width field written whether or not available and zeroed by the encoder.

## R5 — the tier, red first

`remote-diag.toml` was rewritten to assert the new truth **before** any implementation, and run against the
unchanged tree: `measurement_head_available=true` missing; `measurement_event_count=4` missing;
`measurement_events_kernel_digested=1` missing; and the chain-head check failed, the host recomputing
`59ea8820…52f3f` (for the images as they then were) with no line to compare against. Exit 1.

The head assertion is **equality with an independent derivation**, not "non-zero". New
`crates/fjell-tools/src/chain_head.rs`: external `sha2`, the preimage assembled by hand from the documented
layout, no import of `fjell-measure-format` (a dev-dependency imports it only to prove agreement), and the
four events a boot appends written out in the module — kernel image set, then three `Init` placeholders. It
is checked against the in-tree chain implementation, and against the provenance it encodes (the head
changes if the placeholders' source reverts to `Verifyd`). The shared comparison is one helper
(`judge_hex_prefixes`) used by both the image-set check and this one, so they cannot judge differently.

**Green after:** `measurement_head_available=true`, head `bb4f8174254869c005304d70096562c8866d42d6bcdd485edb49d5fbae60daa5`
**equal** to the host's derivation, `measurement_event_count=4`, `measurement_events_kernel_digested=1`; the
other three flags `false`. The tier asserts 11 markers and the equality; PASS (final `test-all`,
`tests/runs/20261027-225557`).

## R3 — what the chain reports, and the name

`measuredd` keeps one counter, `kernel_digested`, incremented on a successful `APPEND_IMAGE_SET`. The
reply to `GET_HEAD_STATS` now carries it in the **spare fourth word**: `[latest_seq, dropped, last_kind,
kernel_digested]`. **Spare word, not a new op:** a reply carries exactly four words and this one was unused;
a second op would add a round trip for nothing, and no new protocol constant was needed.

The field is named for what it measures: **`measurement_events_kernel_digested`** — events whose digest
arrived by the path where the kernel produced it. It is a *structural* count. `measuredd` cannot look at 32
bytes and know whether they are a hash or a literal, so no name here says "real", "verified" or
"trustworthy". The doc comment on the field says that the remainder
(`measurement_event_count − measurement_events_kernel_digested`) are events whose digest the appending
service supplied, that today those are `init`'s literal `0xAB` (E-095), and that a reader who takes the field
as "how much of the chain is trustworthy" has read more into it than it says.

## R2 — slot 4

`spawn.rs`'s `DIAGNOSTICSD` block installs slot 4 with `CapRights::CALL` only, to measuredd's endpoint
object, now a named kernel-private constant (`MEASUREDD_EP_OBJECT`) used by both the per-image table and
this install. A `fjell-abi` constant would have been an ABI addition. **The waiver is removed**, not
reworded; its place in the file records that a satisfied waiver is not a waiver with new text.
`capability-topology`: **5 installed, 1 waived, 6 checked** (was 4 installed, 2 waived).

**What the capability does not do:** it does not stop `diagnosticsd` sending an append — `CALL` is a right to
send to an endpoint, not to an operation. What stops it is `measuredd`'s refusal of appenders it cannot
attest (RFC-0.39-001). My first draft of the kernel comment said the opposite; it was wrong and is
corrected. **Shown, not argued:** with a scratch edit (reverted) `diagnosticsd` sent `APPEND_EVENT` and
`APPEND_IMAGE_SET` through the new capability; both returned `ERR 6` (PermissionDenied), the chain kept
exactly its four events, and the head still equalled the host's derivation. Revert proved: marker absent
from the tree and the prebuilts (controls present), all 33 prebuilts byte-identical to before the scratch.

## R4 — the bundle

`DIAG_BUNDLE_VERSION` 3 → 4. Two `u32` written straight after `measurement_head`, **zeroed by the encoder**
while the head is unavailable (two raw values hash identically; an unavailable section hashes as an all-zero
one). Beyond the brief:

* **The measurement section is one value.** `BundleBuilder::new` takes `Option<MeasurementReport>` instead of
  `(head, available)`. `None` is unavailable with every field zero; there is no way to pass a head with
  "unavailable", or a count with no head. `MeasurementReport::new` refuses
  `kernel_digested > event_count`, which describes no possible chain.
* **A consistent snapshot.** The head and the counts come from separate calls. `diagnosticsd` reads the
  statistics before and after `GET_HEAD` and must see them identical, trying again a few times before
  reporting the section unavailable, so a head is never paired with another moment's counts.
* **`bundle-v1.frozen` was regenerated by `cargo xtask schema dump`**, not edited; `schema check` then
  reports 21 files matching. Only `bundle-v1.frozen` changed — **`chain-digest-v1.frozen` is unchanged** and
  `the_chain_digest_stream_is_untouched` passes.
* **The golden digest move is proved, not re-taken.** `golden.rs`'s header forbids regenerating a stored
  digest to make a change pass. `diag_bundle_v4_is_v3_plus_two_counts` rewinds the v4 stream (delete bytes
  73..81, version word back to 3) and recovers the **previous** golden `06332d60…4548`. The new constant is
  `eee02b62…feaa`. So the only change in the stream is the two counts and the version.

## R6 — no retro-claim

Neither forbidden sentence is written. Row 13 now says publishing the head in the bundle does **not** make
attestation real (`last_attestation_available` stays `false`); row 14 says the bundle carries the head and
the composition, 4 and 1, and that the second cannot describe the other three; row 15 says the head is now
published *which is why the provenance had to be attested first*. The history bullets that said the flag
reads `false` carry a dated update rather than a rewrite. E-095 gets a note that the bundle discloses its
condition by number without closing it.

## R7 — gates

See the review request.
