# RFC-0.40 fix-forward, round 2 (G1–G6): the documents stop claiming mechanisms the tree does not have

**Handoff:** `.git-exclude/handoff/fix-forward-0.40-round-2.md` · **Base:** `c52cf26`
One commit, documents and comments only. No behaviour, format, `SCHEMA_VERSION`, ABI, erratum number or
limitations row changed.

| Item | Where | What |
|---|---|---|
| G1 | `crates/services/fjell-attestd/src/main.rs` | the `sign_v2` comment now says `local()` writes flag 0 and value 0, not the encoder; same line count |
| G2 | `crates/formats/fjell-attestation-format/src/v2.rs` | `local()`'s doc gains the fourth bullet (the rollback counter pair: whether the caller's own read **succeeded**, what it **returned**, `false`/`0` otherwise, never `rollback_min`) and its closing paragraph names E-101 beside E-095 |
| G3 | `rfcs/ERRATA.md`, E-101 | one bullet, **as a property**: every flag-and-value pair is written zero by the encoder when its flag is `false`; today the digests and the measurement section are, the counter pair is not; not a `SCHEMA_VERSION` bump (no reachable byte moves; precedent: `v2.frozen`'s own RFC-0.33-003 D3 note); scheduled with the entry, not done now |
| G4 | `rfcs/ERRATA.md`, E-102 | the shared-object paragraph rewritten as the register's own dated correction: 20 images **hold** slot 0 against object 0 (35 `ImageId`s, 15 dedicated arms, `_ => 0`, `ALL_NON_META` ⊇ SEND); exactly **two** receive on it, `auditd` (`src/main.rs:105`) and `netd` (`src/main.rs:79`); nothing sends to object 0; sample-service's send to its own slot 0 is safe because slot 0 is its dedicated object 6; the stale "auditd/bootctl" comment in sample-service recorded as something E-102's fix should clean up, **not edited**. The false "no service sends to its own slot 0" is gone from the **Tree** bullet as well |
| G5 | `rfcs/ERRATA.md`, E-100's Status line | dated, not deleted: row 16 "as the table stood in 2026-10 before this closure; that row was removed when E-100 closed, and row 16 is now E-103" |

The four historical places the handoff names (`RFC-0.40-003`'s text, its handoff, the 0.40-003 answer, the
0.38-002 answer) were not touched.

## What I re-checked against the source rather than copied

The correction to E-102 is the reviewer's enumeration. Re-checked by me: `fjell-auditd/src/main.rs:105`
(`sys_ipc_recv(0u32)`), `fjell-netd/src/main.rs:79` (`sys_ipc_recv_msg(0)`), the sample-service send and its
RFC-0.28-003 comment (dedicated object 6, and the comment still says "auditd/bootctl's shared-object-0 receive
loops"), and the ten services whose `EP_SLOT` is `0` (a `grep` of the constant gives exactly attestd,
measuredd, proxy-braille, proxy-text, recoveryd, semantic-stream, storaged, svc-presentation-fault,
svc-presentation-liar, verifyd). **Not** re-counted: the per-category send tallies (14 / 3 / …). The entry's
correction note says which is which.

## G6 — gates

* `cargo fmt --all --check`: exit 0.
* **Prebuilts:** rebuilt (`cargo xtask build`, exit 0, 0 warnings; the log shows `fjell-attestation-format` and
  `fjell-attestd` compiled, so the edit was not skipped by the cache) — **every prebuilt byte-identical**: `git`
  reports none modified and `repro-check --skip-build` exits 0 against the unchanged baseline. So
  `tests/repro/baseline-digests.txt` was left alone, and the `measurement`/`audit` profile re-runs the
  handoff asks for *if a digest moves* were not required.
* `consistency-check --all`: PASS, 20 subchecks — `errata-limitations` **PASS (14 ACCEPTED errata, all
  referenced)**, `errata-tracking` **PASS (103 entries)**, `doc-links` PASS (852 links, 0 known-broken).
* `schema check`: 21 files match. `cargo test -p fjell-attestation-format`: 30 pass.
* `release-rehearsal` once at the tip: result in the review request.

## Where the handoff is wrong

Nothing. One precision: G1 says "keep the line count if it is easy"; it was (a single-line clause), and the
prebuilts did not move in any case. G2 added lines to `v2.rs` and the digests still did not move, so the
"panic `Location`" concern does not bite in this tree today.
