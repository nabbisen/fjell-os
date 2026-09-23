# Developer Handoff — RFC-0.34-001

**Governing RFC:** [RFC-0.34-001](../../accepted/RFC-0.34-001-a-second-presentation.md)
**Milestone:** 0.34
**Status:** inherited from the governing RFC (Accepted, 2026-09-24)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure is one envelope, two presentations, and a publisher that does not care whether either exists

Both halves are required. A second renderer that works while the node still
stalls when a presentation dies would prove the opposite of this line's claim:
that the core depends on how it is presented.

**When this line is done:**

- one published envelope appears in two renderings, in one QEMU run, asserted by
  their **content**, not by a marker saying "rendered";
- a run with one presentation deliberately absent shows the publisher
  **unaffected** (today it stops: E-058, measured at 313 → 125 output lines).

## 0.1 Re-derive first (R1), each absence with a control

```
wc -l crates/services/fjell-proxy-text/src/*.rs                 # 211 in main.rs, 1,069 in the crate
/usr/bin/grep -rn 'ImageId(' crates/fjell-abi/src/service.rs    # next free id; last assigned 0x1D
ls crates/fjell-kernel/prebuilt/*.bin | wc -l                   # 29 today
/usr/bin/grep -n -A6 'PUBLISH_COMMIT' crates/services/fjell-semantic-stream/src/main.rs
```

**Use `/usr/bin/grep -a`**, never the shell's `grep`: it silently skips files
containing a NUL byte, which put a wrong figure into three records this month
(E-055). Control every absence **on the file the claim is about**.

Reproduce E-058 before you change it — the RFC's figures came from a scratch
build with `proxy-text` not started, and you should see the publisher stop
yourself.

## 0.2 Settled — do not re-open

D1 one decoder, two renderers; D2 the stream does not know how many proxies
exist; D3 one envelope, two renderings, one run; D4 the renderer is a pure
function with committed vectors; D5 `proxy-text` unchanged beyond fan-out; D6
**output only** — ADR-v0.5-005 stands and no input path is opened here; D7 named
for what it is, never "Fjell speaks" or "supports braille"; **D8 no presentation
may stall a publisher**.

---

## 1. Order — decouple before you add

**R1 → §A–§E answered in writing → D8 (decoupling, with its tier) → D4 (the
renderer crate and vectors) → §B (image, endpoint, capability) → D1/D2/D3 (the
service and fan-out, with its tier) → R6/R7 (documents, the readiness row) →
R9/E-058 → evidence.**

**D8 comes first and this is not negotiable.** Adding a presentation before
removing the coupling doubles the number of things a publisher can be stuck
behind, and then you are debugging two faults at once. Land the decoupling, show
the publisher surviving an absent `proxy-text`, and only then add a second one.

## 2. D8 — what "must not stall" means

`semantic-stream` forwards with a blocking call **before** it replies to the
publisher. The publisher's reply may not wait on any presentation.

**State your policy explicitly, because both halves are choices:**

- what the publisher is told (it published; rendering is not its business);
- what happens to an envelope a presentation cannot take right now — **dropped,
  or queued with a bound**. Dropping a rendering is acceptable; stalling the
  node is not. A queue with no bound is a third failure mode, not a fix.

**And the absence must stay observable**: a presentation that is gone should be
visible in the node's own output, not silently tolerated. Say how.

## 3. §B — the endpoint trap, named in advance

A new service needs an `ImageId`, a spawn-table entry, a capability, **and its
endpoint object allocated in the kernel's `EndpointTable`**. RFC-0.33-001 lost a
day to exactly that omission, with the warning comment two lines above the
allocation it skipped — *"skip this step and every IPC to the object fails with
`InvalidCap`"*. **Assert the allocation in a test**, so the next line cannot
repeat it a third time.

Shared endpoints are refused in advance (RFC-0.28-001): the second proxy gets its
own object.

## 4. §A — the modality, and what may be claimed

My lean is **grade-1 (uncontracted) braille**: a table, a width rule, and a rule
for severity and actions — deterministic, no device needed, checkable byte for
byte. An announcement stream for a synthesiser is a defensible alternative and
**easier to get subtly wrong**; if you choose it, say what makes its output
checkable.

**Whatever you choose, the deliverable is the stream a display driver or a
synthesiser would consume.** No page, comment, marker or commit message may say
Fjell speaks, supports braille, or is accessible. QEMU `virt` has neither device
(E-004).

## 5. D4 and §C — vectors, and what the tier asserts

- The renderer is a **pure function**: envelope in, bytes out, in a crate the
  service wraps — so its vectors run in Gate 1, not only under QEMU.
- The committed vectors carry the awkward cases: an intent with no actions; one
  with `MAX_ACTIONS`; a `severity` of each kind; text at `MAX_TEXT_BYTES`; and
  whatever your modality truncates or omits.
- **The QEMU tier asserts rendered content**, from a committed vector — a marker
  that says only "rendered" proves nothing, which is the shape this project keeps
  filing errata about.

## 6. Prohibited shortcuts

- **No second decoder.** Copying `proxy-text`'s decode path would disprove this
  line's claim while appearing to confirm it (D1).
- **No marker-only assertion** of a rendering (§5).
- **No input path**, and no edit to ADR-v0.5-005 (D6).
- Do not modify `proxy-text` beyond what fan-out requires — its markers are
  load-bearing in three tiers.
- Do not add a `BoardProfile` field, an audio device, or any hardware claim.
- **Re-record the repro baseline in the same commit as any rebuild**, and run the
  two-build check first — tier 5 went red unnoticed twice in 0.33.
- Re-record the ABI snapshot for new public items, and say what it reported
  **before** you regenerate.
- Do not run `cargo fmt --all --check` in your head.

## 7. Required evidence

1. R1 re-derived with `/usr/bin/grep -a` and a control per absence, including
   E-058 reproduced before the fix.
2. §A–§E answered in writing.
3. **D8 first**: the decoupling, the policy for an undeliverable envelope, and a
   tier that runs with a presentation absent and shows the publisher continuing.
4. The renderer crate, its committed vectors, and their Gate 1 run.
5. The service: image, endpoint **allocated and asserted**, capability, fan-out.
6. The tier asserting **both renderings of one envelope**, by content.
7. Documents: `abdd-semantic.md` and `v1-limitations.md` updated to what now
   exists and what still does not; **RFC-0.33-002's readiness row** for a second
   modality moved to done, with the run id.
8. **E-058 CLOSED** or its survivor named; register and `v1-limitations.md` in
   the same commit.
9. Prebuilts and the repro baseline; the ABI snapshot.
10. `test-all` all tiers; `consistency-check --all` **by exit status**;
    `cargo fmt --all --check`; a CI run id.

## 8. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The absent-presentation tier** — what the publisher did, first.
- **Your undeliverable-envelope policy** (§2), and why the bound is where it is.
- **Your §A answer**, and one rendering of a real envelope, quoted.
- Anything in `semantic-stream` or `proxy-text` that turned out to be wrong
  beyond E-058 and E-059.
- Any figure of mine you re-derived and found different.
