# RFC-0.34-005 R1, §A–§E, and the defence-to-evidence table

**Governing RFC:** [../done/RFC-0.34-005-the-presentation-in-the-threat-model.md](../done/RFC-0.34-005-the-presentation-in-the-threat-model.md)
**Handoff:** [../handoffs/RFC-0.34-005-the-presentation-in-the-threat-model/implementation-handoff.md](../handoffs/RFC-0.34-005-the-presentation-in-the-threat-model/implementation-handoff.md)

Written in the handoff's order: §A–§E before prose, so the amendment is not
sentences written to fill a heading. A threat model cannot fail a gate for
being wrong (§0 of the handoff); the discipline that replaces the gate is
this document and the review that follows it.

---

## R1 — Finding 1, re-derived at `cc658e1`, with a control

```
grep -c proxy        → 0
grep -c presentation  → 0
grep -c braille       → 0
grep -c T17           → 1   (control: the methodology reads this file)
```

Confirmed: the file contains no proxy and no presentation, in a file that
does contain `T17` — the search finds what it should when the pattern is
present, so its silence on the other three is not a tooling blind spot.

### Finding 3, read from the tree and from `releases/0.33.0.md`, not memory

What exists now that did not when the model was last touched: two
presentations (`proxy-text`, `proxy-braille`), both output-only, both on a
serial console, driven by one stream; an absence and a mid-run crash that
are both committed tiers (`semantic-absent`, `semantic-crash`); a stream that
reports a presentation that has stopped asking, on its own output; and — as
of this cut's other line — a `DISPATCH_ACTION` authority check that no
longer trusts a payload word. `releases/0.33.0.md`'s own *What shipped*
section for RFC-0.34-001: *"A second presentation service now renders the
same semantic-stream intents as braille … fed by a new fan-out engine with a
per-presentation queue and credit so one stalled presentation cannot block a
publisher or another presentation … Closes E-058."* `rfcs/done/RFC-0.34-001`'s
own review record, read for what the fix **guarantees** rather than what the
RFC proposed: the stream never initiates a blocking IPC to a presentation
(ask-and-reply, each presentation woken through its own endpoint), and four
survivors are named — (a) alive-but-silent undetected, (b) the wake-window
fault, (c) the 8 KiB drop-newest bound reported only on the console, (d)
nothing restarts a stuck presentation. A fifth, found while re-deriving this
line rather than assumed closed by E-062: the kernel's 160-byte console-line
bound still splits a long rendered line (visibly now, since E-062's close,
not silently) — unchanged, and not yet hit by any committed line (longest
measured: 153 bytes text, 135 braille), but nothing bounds a title or
description to stay under it.

---

## §A — New threats, T21 onward. Three.

Not an extension of T17: T17 is forgery, and its own residual already
carries the adjacent, non-forgery class as prose. Folding the presentation
into that prose would bury a boundary inside a residual, which is the exact
failure the handoff's lean warns against. **Three threats**, one per
direction D1 requires:

- **T21** — the sender direction: a presentation's return leg claiming an
  authority value it does not have. This **is** T17's adjacent class, with a
  name and a closed defence (RFC-0.34-004) rather than a residual's prose.
- **T22** — the absence/crash direction: E-058's class, now with a mechanism
  and tiers rather than only a paragraph in `v1-limitations.md`.
- **T23** — the receiver direction, reframed: not "what can a presentation be
  given" in the sense of a payload (T21 already covers what it can send), but
  what a **person** is given when the presentation itself is the thing lying.
  This is genuinely new — every prior entry in this model defends the node
  or the fleet; nothing in it has ever had a person as the party being
  deceived.

## §B — Same-node task, in the TCB as every service already is; a separate
## device is named and excluded, not analysed

Checked rather than assumed: `proxy-text` and `proxy-braille` are spawned
from the same init image as every other service
(`crates/fjell-kernel/src/task/spawn.rs`), hold a kernel-issued capability
like every other service, and are identified to the stream by the same
kernel-attested mechanism (`ipc_sender_image_id`) as every other sender. A
compromised presentation is not a new capability for an adversary — it is
**C-NODE-EXEC**, unmodified, applied to one more task. "In the TCB" here
means exactly what it means for `cap-broker` or `semantic-stream`: the
kernel vouches for identity, nothing vouches for payload (T17's own point).

**The separate-device case is named, and explicitly not analysed.**
RFC-0.33-002 §A left *"a desktop-class personal OS"* undecided and named the
personal-device question as a v2+ direction, not a settled non-goal. If a
presentation ever runs off-node, the channel it would need —
authentication, replay protection, confidentiality, device identity — is a
different boundary in kind, not degree, from a same-node task's capability
grant, and §3 of the handoff is explicit that modelling it here would make
this amendment the larger analysis it was scoped not to be. Named, excluded,
pointed forward: this is this erratum's own closing condition
(`v1-limitations.md` item 9, pre-amendment: *"needs its own amendment before
an assistive presentation on a separate or personal device is described as
safe"*), restated as still true rather than quietly resolved by naming three
same-node threats.

## §C — One threat (T22), with E-058's defence and its survivors

As the handoff leans: `v1-limitations.md` already named four survivors with
no adversary attached, which is exactly the gap a threat model entry closes
— a residual with a name needs a threat to be the residual **of**. One
entry, not a residual left where it was. The fifth survivor (the console
bound) is added here because R1 found it live, not because the handoff asked
for a fifth — D5 names it explicitly among the defences to check.

## §D — C-NODE-EXEC, reused for all three. No new adversary class.

A compromised presentation can act only because it is **code running on the
node** — the same capability every other T-entry in this model keys to
C-NODE-EXEC. What changes across T21/T22/T23 is not what the adversary can
do, but what the *consequence* is: a false authority claim (T21), an
unavailable narration (T22), a deceived person (T23). The adversary table is
capability-keyed, not consequence-keyed, and T23's consequence — a human,
not a kernel-attested task, is the one deceived — does not require a new
row: `C-NODE-EXEC` already means "this task can be made to do anything a
compromised binary can do," and showing false output to a person is well
within that.

## §E — Every defence names what demonstrates it; every absence is a named residual

| Threat | Defence | Demonstrated by |
|---|---|---|
| T21 | RFC-0.34-004: `DISPATCH_ACTION`'s authority is a lookup on the kernel-attested sender's compiled policy row, never the payload | Tier `semantic-liar` (forged `0xFFFFFFFF`, refused every run); the scratch "accepted" transcript taken before the fix (RFC-0.34-004's own answer document) |
| T22 | RFC-0.34-001: the stream never initiates a blocking IPC to a presentation (ask-and-reply) | Tiers `semantic-absent`, `semantic-crash` |
| T23 | **None** | — (named residual, not asserted away) |

Residuals, each named:

- T21: dispatch executes nothing yet — the defence is untested against a
  real stake until an input path exists (RFC-0.33-002 §C, unbuilt).
- T22: (a) alive-but-silent undetected; (b) the wake-window fault; (c) the
  8 KiB drop-newest bound, gap reported only on the console; (d) nothing
  restarts a stuck presentation; (e) the 160-byte console-line split, bound
  unchanged, not yet hit.
- T23: no independent channel lets the attending operator cross-check a
  presentation's output against the node's own record (the audit ring, T16,
  surfaces nothing to an operator through a channel independent of the
  presentation itself).

---

## Order of work

R1 (above) → §A–§E (above) → the amendment itself
(`docs/src/security/threat-model-v1.md`: T17's residual gains its pointer;
T21–T23 added with a short framing note for §B) → R6
(`v1-limitations.md` items 8/9, `standards-mapping.md`'s count and two
citation rows) → R7 (E-060 CLOSED, register and limitations together) →
evidence.

## Decisions I am making that nothing above the line specified

1. The framing note before T21 (*"The presentation boundary (T21–T23,
   RFC-0.34-005)"*) as the place §B's TCB answer lives, rather than repeating
   it inside each of the three entries.
2. T23's exact title and its "victim, not adversary, is what's new" framing
   — the handoff's §D asks whether a new class is needed and answers via
   reasoning, not via a specified sentence.
3. Adding the console-line-bound survivor (e) to T22 and to
   `v1-limitations.md` item 4, found during R1, not specified by the RFC.
4. `standards-mapping.md`: besides the required count fix (20 → 23), adding
   T21 to `CRA-I-2f`'s existing citation list (command/data integrity) — a
   precise fit for an existing row, not a new row.

I will not touch T1–T20, `ADR-v0.5-005`, or build any defence this amendment
names as missing (T23's residual is not a task list).
