# Developer Handoff — RFC-0.42-003

**Governing RFC:** [RFC-0.42-003](../../done/RFC-0.42-003-storaged-writes-where-it-is-told.md)
**Milestone:** 0.42 · first line of the milestone, no dependency on 0.42-001 or 0.42-002
**Status:** inherited from the governing RFC (Implemented, 0.42.0)
**Audience:** implementation model

---

## 1. Read the RFC first

It carries the requirements (R1–R6) and the non-goals. This document is the mechanics, the traps,
and what the review request must show. **Where the two disagree, the RFC governs** — say so in the
review request rather than choosing.

## 2. The one-line summary of the defect, so you can hold it in mind

`storaged`'s `recv_call` returns `(tag, w0, w1, w2, w3)` and its only caller binds `w0` to
`_badge` and discards it, so every field is read one word late: the LBA becomes `lba_hi` (always 0)
and every chunk loses a word and gains a hard-coded zero. `init` never notices because it discards
seventeen of its eighteen replies.

## 3. Mechanics and the traps that apply

- **Two prebuilts move, not one.** R4 changes `init`, R1–R3 change `storaged`. So
  `cargo xtask build-services` **then** `build`, and **re-record the repro baseline in the same
  commit** — `rm tests/repro/baseline-digests.txt` then
  `cargo run -p fjell-repro-check -- --skip-build --record-baseline`. Expect **two** moved lines
  (`fjell-storaged.bin`, `fjell-init.bin`) and account for any others. This trap has gone red
  unnoticed before.
- **A source change needs both build steps.** `qemu-run` boots the committed prebuilt, so `build`
  alone will not pick up a service change.
- **Do not regenerate `releases/trust-report.txt`.** It records each prebuilt's byte size, so two
  lines will drift; release artefacts are reverted mid-line and regenerated at the cut.
- **`/usr/bin/grep -a`** for anything you grep in a `.bin` — the shell's `grep` skips
  NUL-containing files and reports a confident zero.
- **One `test-all` at a time.** Nothing else may touch `fjell-disk.img` while it runs.
- **Stage by explicit path** and check with `git diff --cached --name-only`. Not `git add -A`, and
  not `git add -A <dir>` either.
- **The shell starts in the repository.** No `cd` is needed; if you use one, use a relative path.

## 4. Evidence the review request must carry

1. **A test that fails against the current binding** (R5), shown red before it is green. A test that
   only passes afterwards does not establish that it would have caught this.
2. **The throttled control's line** (R6), quoted: `storaged: io incomplete after … lba=0x000000c1 …`
   for `init`'s LBA 193. **If `lba` is still `0x00000000`, R1–R2 are not done** regardless of what
   the unit test says.
3. **The baseline `--numstat`**, with both moved lines named.
4. `cargo xtask build-services && build` with **0 warning lines** — criterion 2's standard, and the
   0.41.0 cut was held up by a warning nobody read.
5. **Full `cargo xtask test-all`**, tier counts reported. `store` stays NOT-GATED; that is expected
   and is a non-goal, not a regression.
6. `consistency-check --all` (expect 21 subchecks), `fmt --check`, mmio-audit, unsafe-audit,
   `abi-snapshot --verify`.

**If a tier goes red, report which and do not re-run to green.** Two cases have answers already:
`storaged: io incomplete` is **E-104**, closed on a cause and disclosed as limitations row 19 — a
known intermittent on host I/O latency, so quote it and hand me the serial log. And per the RFC's
Risks, **R4 may turn a previously silent reply failure into a boot failure**; if that happens the
reply value is the finding. Stop and hand it to me rather than softening the check.

## 5. What is not yours

`rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md` (row 17 is the architect's), the
`io incomplete` line's text or fields, `do_io`'s budget, `READ_CHUNK`, the `store` profile's
`release_gated` flag, and the tag. **Criterion 8 is not yours** — if you believe a limitations or
standards-mapping row is false, say so in the review request and do not edit it.

## 6. Escalate rather than implement badly

Three of the last four escalations on this project were right and each cost me a withdrawn
requirement. In particular: if R4's checked ACKs reveal that `storaged` does not actually reply to
`WRITE_BEGIN` or `WRITE_CHUNK` at all, **stop** — that is a protocol gap rather than a caller bug,
and its shape is mine to rule.
