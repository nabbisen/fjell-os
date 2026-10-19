# Developer Handoff — RFC-0.38-001

**Governing RFC:** [RFC-0.38-001](../../accepted/RFC-0.38-001-the-measurement-chain-measures-something.md)
**Milestone:** 0.38
**Status:** inherited from the governing RFC (Accepted, 0.38)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does the measurement chain contain a digest of something that actually booted?**
Today it contains `0x0011` and `0xAB`, each followed by twenty-four zero bytes,
because nothing in the tree computes a digest of anything. When a recorded
measurement equals a digest computed independently from the same bytes — and
something asserts that equality — this line is done.

**Not the measure:** that the recorded value is non-zero, or that it changed. A
different placeholder is still a placeholder.

## 0.1 Two things to settle before writing much

1. **R2's design choice, and escalate if it needs a new syscall or a `BootInfo`
   field.** The kernel holds the bytes; `measuredd` is userspace and the kernel
   does not speak its protocol. How the digest travels is yours to decide and
   **state**, but an ABI addition is not an implementation detail — it lands in the
   snapshot and the cut checks `Added: 0`.
2. **R5's demonstration needs a temporary capability.** Nothing can call
   `GET_HEAD` — no caller exists and slot 4 is 0.39's. Use the technique you used
   for E-083: scratch install, call it, capture the transcript, revert, and **prove
   the revert** with a marker `grep` on the committed tree. Plan for it rather than
   discovering at the end that the central change cannot be shown.

## 1. Order

1. **R1** — re-derive with controls, **and sweep for other placeholder digests**.
   `[0xBB; 32]` and `[0xCC; 32]` in `init` are two I know of and have not chased;
   escalate if one feeds something the RFC does not name.
2. **R2** — the digest is computed and reaches the chain. Escalate first if it
   needs ABI.
3. **R3** — the boot-time cost, measured on a real run.
4. **R4** — E-085: four words, the digest whole, `latest_seq`/`dropped` to their
   own tag, the caller stops discarding `w2`/`w3`.
5. **R5** — the scratch-capability demonstration, then the revert, then the proof.
6. **R6** — the comparison instrument: a recorded digest against one derived
   independently.
7. **R7** — nothing retro-claims the old events as measurements.
8. **R8** — E-086 and E-085 closed; flag still `false`; `remote-diag` unchanged
   and passing; E-087 untouched.
9. **R9** — gates, rebuild, baseline **with the binaries**, CI.

## 2. Traps specific to this line

1. **Do not install slot 4.** The owner scoped it to 0.39 so that this line proves
   the chain is true before anything reports it. A visible measurement is 0.39's
   result, not this line's.
2. **Do not use `EXPORT_LOG` as a chunking model** — it is a stub that sends no
   content (**E-087**), and the architect's own E-085 ruling cited it wrongly. If
   you ever need real chunking, `storaged`'s `WRITE_BEGIN`/`WRITE_CHUNK`/
   `WRITE_COMMIT` is the working one. For this line you need **no** chunking: a
   reply carries four words, which is exactly 32 bytes.
3. **A non-zero digest is not evidence.** R6 is equality against an independently
   computed value. If that turns out to be impossible inside a QEMU tier, say so
   and propose a host check — do not downgrade the assertion to "not all zeros".
4. **The frozen chain-digest description is a gate.** `compute_chain_digest`'s byte
   stream has a generated file (RFC-0.35-004) with goldens. If a real subject
   digest changes that stream, **stop** — it should not, since the field was always
   32 bytes, but verify rather than assume.
5. **Longer boots are a finding, not a timeout change.** R3 asks for the number.
6. **Rebuild, re-record, and commit the binaries in the same commit.** The 0.37.0
   cut committed a baseline without the five binaries it was hashed against and
   `repro-check` reported PASS throughout, because it reads the working tree rather
   than git. Run `git status` before the next commit rather than assuming.

## 3. What to put in the review request

- R1's probes and controls, and the placeholder sweep's result — including
  "nothing else" with the control that makes it credible.
- R2's design choice, stated, with the reason; and whether it needed ABI (and if
  so, the escalation that preceded it).
- R3's boot-time figures, before and after.
- R4's diff, and the new tag for `latest_seq`/`dropped`.
- R5's transcript: the whole digest arriving, then the revert proven.
- R6's instrument, and the equality it asserts.
- E-086 and E-085 closed; `measurement_head_available` still `false`;
  `remote-diag` still passing unchanged; E-087 visibly untouched.
- Each gate by its own exit status, as the required set; the rebuild, the baseline
  **and the binaries**; a CI run id.

Hand back one path: your review request. Do not tag anything.
