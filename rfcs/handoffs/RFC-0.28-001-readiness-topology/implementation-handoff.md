# Developer Handoff — RFC-0.28-001

**Governing RFC:** [RFC-0.28-001](../../proposed/RFC-0.28-001-readiness-topology.md)
**Milestone:** 0.28
**Status:** inherited from the governing RFC (Proposed — awaiting owner acceptance)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Reproduce the hang before you fix anything

Nine services are blocked in `send_ready()` right now and are alive only because
`init` reaches each wait in a fixed order. **Change who waits and you change
what unblocks them, and the failure mode is a hang, not an error.**

This is RFC-0.26-001's situation exactly: the two-line fix hung M6 boot, and the
deliverable turned out to be the explanation, not the patch. Expect the same
here. **Deliberately break the current arrangement, capture the hang, identify
which task is waiting on what — then decide the fix.**

If you fix first and the hang never appears, you have not proven it could not
have; you have lost the finding.

## 0.1 What "explained" means

Not "readiness was broken." Specifically:

- **Which** task fails to progress, and what it is waiting on.
- **Why it completes today** — what `init` does, while running ahead, that
  satisfies it.
- Whether anything besides readiness has been arriving on those endpoints and
  being silently eaten by the missing-`else` loop.

That last one is the finding most likely to be real and unlooked-for. The loop
at `fjell-init/src/main.rs:122-139` discards any tag it does not recognise, and
has for many releases. **Instrument it to log what it discards before you remove
it.** Whatever comes out is a second finding, and it belongs in the review
request rather than in a fix.

## 0.2 Design decisions settled — do not re-open

1. **Finish RFC 058; do not design a replacement** (D1).
2. **Do not fix this with another `ep_obj` entry or by repointing slot 0**
   (D2). Readiness must name service-manager explicitly. The bug is that a slot
   number's meaning is decided by an unrelated table; a fix that leaves that
   true has fixed nothing.
3. **The svc profile ends expecting all four markers** (D3). Do not lower
   `n_ready >= 10` to make `READY_ACCEPTED` fire — if 10 is the wrong number,
   say why in writing.
4. **RFC-0.26-004's invariant text gets corrected** (D4).
5. **Evidence is committed** to `tests/evidence/` with provenance (D5).

---

## 1. Order

**§5 answered → reproduce and explain → topology change → `init`'s waits →
markers restored → invariant text → errata.**

§5 first because it decides the shape of everything after it: whether `init`
waits at all determines what you do to its wait functions, and retrofitting that
decision into written code is how it gets made by accident.

## 2. §5 is the deliverable

**Should `init` wait for readiness at all?** Three shapes in the RFC. My
inclination is shape 1 — `init` waits for one signal from service-manager — and
it is an inclination, not a ruling. Three lines this milestone overturned one of
mine and each was right to. Argue it.

Note what shape 1 buys: the one-receiver invariant becomes **true** rather than
restated, and E-021's missing-`else` disappears along with the loop containing
it. Note what it costs: `init`'s boot sequence interleaves waits with work, and
that shape changes.

## 3. The arithmetic to check, not trust

I claim: 14 images, 9 with dedicated endpoints, **at most 5** can reach
service-manager, threshold is 10, therefore `READY_ACCEPTED` is unreachable by
construction. **Re-derive it.** I read `spawn.rs`'s table and
`service-manager`'s `n_ready >= 10`; I did not confirm how many of the 14
actually spawn in the `svc` profile, and that number is what the threshold
should be judged against.

If the real reachable count differs from mine, **report the difference** — the
last four lines each corrected one of my counts, and every correction was worth
more than the thing it corrected.

## 4. Prohibited shortcuts

- Do not change the topology before the hang is reproduced and explained.
- Do not add an `ep_obj` entry or repoint slot 0 as the fix.
- Do not lower the ready threshold to make a marker fire.
- Do not narrow `expected-markers.txt` further. It is already narrowed; that is
  finding 5.
- Do not "fix" the missing-`else` by widening the accepted tag set — that hides
  what is being discarded.
- Do not add a syscall or flag; escalate if §5 concludes one is needed.
- Do not claim host coverage — E-013; cite a committed log.
- Do not run `cargo fmt --all --check` in your head.

## 5. Required evidence

1. **§5 answered in writing, committed**, with the two rejected shapes.
2. **The hang reproduced and explained**, with what the discard loop was eating.
3. Readiness addressed to service-manager by a named constant, not slot 0's
   accidental meaning.
4. **`NEG:SVC:READY_ACCEPTED:PASS` and `NEG:SVC:UNAUTHORIZED_READY_REJECTED:PASS`
   in `expected-markers.txt` and in a real run** — demonstrated by a log promoted
   to `tests/evidence/` with provenance.
5. RFC-0.26-004's invariant text corrected.
6. `docs/release/v1-limitations.md:328`'s "startup-timing" cause corrected — it
   is topology.
7. **E-024 → `CLOSED`**; **E-031** filed or closed as the work decides. Register
   and `v1-limitations.md` edited in the **same commit**.
8. Whether RFC-0.27-002 §4's non-blocking-send need survives the new topology,
   answered either way.
9. `release-rehearsal` green; `test-all` **21/21**; `syscall-surface` 35/29/6.
10. `cargo fmt --all --check` — run it.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The explanation**, and which parts are observation versus judgement. This is
  what I will read hardest and where a plausible-but-unverified mechanism is
  easiest to write.
- **What the discard loop was eating.**
- Your §5 answer, and whether §4's need survived.
- Any count of mine you re-derived and found different.
- Anything that changed behaviour in a tier you did not expect to touch.
