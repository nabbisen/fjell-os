# RFC-0.39-001 R1–R8: the chain records who actually said it

**Governing RFC:** [../done/RFC-0.39-001-the-chain-records-who-actually-said-it.md](../done/RFC-0.39-001-the-chain-records-who-actually-said-it.md)
**Handoff:** [../handoffs/RFC-0.39-001-the-chain-records-who-actually-said-it/implementation-handoff.md](../handoffs/RFC-0.39-001-the-chain-records-who-actually-said-it/implementation-handoff.md)
**Rulings followed:** the RFC's "Design ruling (R1 escalation), 2026-10-27"
(`.git-exclude/reviewed/review-record-rfc-0.39-001-r1-escalation.md`).

Written against base `aeb07db`, in the handoff's order: R1 (done in the escalation), R5's tier
red against the unchanged tree, a scratch demonstration of the same on the unchanged tree, R2,
R3, R4, green, then R6 and R8.

---

## R1 — re-derived, with controls

Done at `75cd536` and accepted in the escalation; the facts did not move at `aeb07db`. One
addition from the ruling: `decode_kind` / `decode_subject` fail *upward* (`_ =>
BootEvidenceImported` / `BootEvidence`), so they are in scope (ruling 4).

## R5, pre-change — the declaration was believed

**The tier, red.** Stored-event lines were added to `measuredd` (`measuredd: event seq=N kind=K
source=S subject=J`, read back out of the chain, not the request) and the `measurement` profile
asserts the producer of each event. Against the unchanged logic the run failed on all three:

```
measuredd: event seq=1 kind=1 source=1  subject=1     (image set -> Kernel)
measuredd: event seq=2 kind=2 source=2  subject=2     (Verifyd -- init sent it)
measuredd: event seq=3 kind=3 source=2  subject=3
measuredd: event seq=4 kind=4 source=2  subject=4
[xtask] once-marker check FAILED — `measuredd: event seq=2 kind=2 source=13 subject=2` ...
```

Control: the `seq=1` marker, which the unchanged logic does produce, matched.

**The over-IPC halves, with a scratch capability** (0.37/0.38 R5 technique; never committed).
A `CALL` capability to `measuredd` was installed for `verifyd` (a mapped service) and `neg-test`
(an unmapped one), each making four calls. Against the unchanged logic **every call was accepted**:

| Caller | Call | Result |
|---|---|---|
| `neg-test` | `APPEND_EVENT`, source byte 0 | `APPEND_OK`; recorded `source=12` (**Measuredd**) |
| `neg-test` / `verifyd` | `APPEND_EVENT`, declared source `0x0B` (Attestd) | `APPEND_OK`; recorded `source=11` |
| `neg-test` / `verifyd` | `APPEND_EVENT`, kind `0x55` (no such kind) | `APPEND_OK`; recorded `kind=1` (**BootEvidenceImported**) |
| `neg-test` / `verifyd` | `APPEND_IMAGE_SET`, digest `0x77…` | `APPEND_OK`; recorded `kind=1 source=1` (**Kernel**) |

So: a task that is not `init` could append, label itself as any service, promote an invalid
kind to the chain's most load-bearing one, and obtain a `Kernel`-labelled event.

## R2 — the mapping, and what happens on an unknown id

`fjell_measure_format::source_for_sender(image_id)` (`src/attest.rs`):

| Attested sender | Recorded source |
|---|---|
| `INIT` | `Init` (**new**, `0x0D`) |
| `MEASUREDD` `VERIFYD` `ROOTFSD` `CONFIGD` `CAP_BROKER` `SERVICE_MANAGER` `SNAPSHOTD` `BOOTCTL` `UPGRADED` `RECOVERYD` `ATTESTD` | itself |
| **anything else** | **refused**, `PermissionDenied` — no `_ =>` default |

`Kernel` has no image: it is reached only through `APPEND_IMAGE_SET`. `decode_source` is
deleted. The function lives in the format crate (which gained a dependency on the zero-dependency
`fjell-abi`) so it is a pure function tested on the host: `every_image_id_is_mapped_or_refused`
sweeps all 65 536 ids against the table; `no_service_can_record_as_the_kernel` asserts it;
`the_services_that_must_be_refused_are` names the neg-test, drivers, `storaged`, `auditd`,
`devmgr`, a presentation service and `diagnosticsd`.

`MeasurementSource`'s doc comment now carries the ruled definition: *who produced the measured
fact*, determined by `measuredd` from the operation plus the attested sender.

## R3 — the reserved byte

`decode_append_word` refuses a non-zero `source` byte with **`InvalidSource` (0x02)**: the
caller supplied a value for a field that is no longer theirs, and the error says which field. An
unmapped appender is **`PermissionDenied` (0x06)**: its source is not invalid, it is not a
recognised appender. Order: sender, source byte, kind, subject — so an unmapped appender learns
nothing about the word (`an_unmapped_sender_is_refused_before_the_word_is_read`). These are the
first real codes on a path that had only ever replied `Internal`.

**Widening the digest is not free.** Four words minus the kind word is 24 bytes against 32.
Not done; the dependency is carried on E-095 (see R6).

## R4 — the three events

`init` no longer passes a source (`append_placeholder_event(ep, kind, subj)`), so the wire byte is
zero. They record `source=13` (`Init`). They are not routed through `verifyd`.

## R5, post-change — refused, and the positive cases

The tier is green (7 markers): events 2–4 read back `source=13`. The same scratch, against the
changed logic:

| Caller | Call | Result |
|---|---|---|
| `neg-test` (unmapped) | `APPEND_EVENT` source 0 | `ERR` **6** (PermissionDenied) |
| `verifyd` (mapped) | `APPEND_EVENT` source 0 | `APPEND_OK`; recorded `source=2` (**itself**) |
| `verifyd` | declared source `0x0B` | `ERR` **2** (InvalidSource) |
| `neg-test` | declared source `0x0B` | `ERR` **6** (refused as an appender first) |
| `verifyd` | kind `0x55` | `ERR` **9** (InvalidKind) |
| `neg-test` / `verifyd` | `APPEND_IMAGE_SET` | `ERR` **6** (PermissionDenied) |

The scratch was reverted and the revert proved (review request).

## Kind and subject (ruling 4)

`MeasurementKind::from_u8` / `MeasurementSubject::from_u8` return `None` for a byte naming none;
`measuredd` refuses with two new codes, `InvalidKind` (0x09) and `InvalidSubject` (0x0A) —
`MeasurementError` had nothing honest to say for these and `InvalidSource` would have misled.
That is a judgement call, flagged in the review request.

## R6 — E-096 closed

`rfcs/ERRATA.md`: closure note (original left as written), Summary row `CLOSED`.
`v1-limitations.md` row 15 rewritten. **The 16-of-32-byte `APPEND_EVENT` digest dependency was
recorded only in E-096**, so closing it would have dropped it; it is now carried on E-095 as a
dated note. `docs/src/architecture/measurement-and-attestation.md` gained "What an event's
`source` says".

## R7 — no retro-claim

The prose says the old `Verifyd` source "was never attested, it was what `init` put in the
request", that chains recorded before this line carry it, and nowhere calls it a relay.

## R8 — gates

See the review request for each by exit status.
