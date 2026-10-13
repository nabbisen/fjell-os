# RFC-0.34-004: A word that claims an authority it does not carry

**Status:** Accepted — by the owner (nabbisen), 2026-10-13; implementation may begin (RFC 000)
**Milestone:** 0.34
**Tracks.** **E-059** — the presentation's action return leg sends the rights it
holds as an ordinary payload word and `semantic-stream` authorises against that
word, under a comment saying the value is kernel-verified — and **E-034**, four
`send` helpers whose signatures promise a data word the kernel has never carried.
One line because they are one defect: **a value that claims a provenance it does
not have**, once in an authority decision and once in four signatures.
**Touches** *(indicative)*: `crates/services/fjell-semantic-stream/src/main.rs`,
`crates/services/fjell-proxy-text/`, `crates/services/fjell-proxy-braille/`,
`crates/services/fjell-attestd/`, `fjell-diagnosticsd`, `fjell-secure-transportd`,
`fjell-upgraded`, `tests/qemu/profiles/`, `docs/src/releasing/v1-limitations.md`.
**Relates to:** **T17** of `threat-model-v1.md`, whose residual already names this
class — *"the kernel attests who sent the bytes, never that…"*; RFC-0.34-005, which
puts the class in the model while this line fixes the instance; E-011 (capability
rights validation on install), the adjacent mechanism if §A chooses capabilities.

## Summary

Re-derived 2026-10-13 at `33182e0`.

### Finding 1 — the receiver authorises on a number the sender chose

`semantic-stream/src/main.rs:441` takes `let granted_rights = w2 as u32;` from the
incoming message and hands it to `dispatch_action_checked`, which decides at `:78`
with `let allowed = required & !granted_rights == 0;`. The function's own doc
comment, `:60`–`:61`, says *"the caller's kernel-verified rights bitmask (read via
`sys_cap_inspect` on the caller's side, not self-asserted)"*.

**Both halves of that sentence are true and the conclusion is false.** The proxy
does read its own rights through `sys_cap_inspect`; **nothing on the receiving side
can confirm that the word it was handed came from that read.** A presentation with a
bug, or a compromised one, sends whatever word it likes and the stream believes it.

### Finding 2 — the receiver already has what it needs in order not to ask

The stream keeps the **kernel-attested sender** and uses it: `presentation_of(sender)`
at `:269`–`:270` resolves `ipc_sender_image_id(sender)`, and the fan-out engine is
keyed by it. T17's defence is exactly that attestation, and **T17's residual already
says what Finding 1 is**: the kernel attests who sent the bytes, never what they
mean. So the material for an honest decision is in the receiver's hand today; what is
missing is a policy that uses it instead of the payload.

### Finding 3 — the decision it gets wrong has no effect yet

A permitted action **executes nothing**: `dispatch_action_checked` returns `Ok` or
`Denied` as a message. So this is a false claim about authority rather than a way to
exercise one — which is why it is cheap to fix now and why fixing it must not be
reported as having made the authority real.

### Finding 4 — four signatures promise a word that is never sent (E-034)

`fjell-attestd::sxt_send(tag, w0)`, `fjell-diagnosticsd::send_tag(ep, tag, w0)`,
`fjell-secure-transportd::send_tag(ep, tag, w0)` and `fjell-upgraded::send_sxt(tag,
w0)` each take a data word and, by their signature, send it. The kernel carries no
word, because the tag packs no word count. Every caller that passes a value is
writing into nothing.

## The settled part

**D1 — a receiver never authorises on a value the sender chose.** Whatever replaces
the word, the test is: can a presentation with a bug or a compromise obtain a
permission it does not hold by sending different bytes? If yes, it is not a fix.

**D2 — the authority comes from something the kernel attests or validates**, and
which it is, is argued in writing (§A) — the attested sender identity with a policy
the receiver owns, or a capability the kernel validates. Not a convention between
two services.

**D3 — no comment claims a verification the code does not perform.** The comment at
`:60`–`:61` either becomes true or goes. This project has filed this class against
itself three times in two milestones; a correct mechanism under a false comment is
half a fix.

**D4 — E-034's four helpers either carry their word or stop taking it.** A signature
that promises a word is the same defect in a smaller place.

**D5 — demonstrated failing, and the failing case is live today:** a presentation (or
a test-only service) that asserts rights it does not hold is **accepted** before the
change and **refused** after, with both transcripts. A tier asserts the refusal.

**D6 — whether a permitted action does anything is out of scope.** If dispatch stays
inert, the register and `v1-limitations.md` say so in the same words after this line
as before it. **Closing E-059 must not read as "actions now work."**

## The open questions

**§A — attested identity plus a policy, or a capability?** *Lean: identity plus a
policy the receiver owns*, because it needs no new ABI and the attestation is already
in hand (Finding 2) — with the capability route named as the correct end state, since
this is a capability system and a permission that travels as a capability cannot be
misquoted. If you choose the policy, **say where it lives and what makes it not a
second hand-maintained table** (E-014's family): derived from the presentation rows
the stream already compiles in, or from the image id, not a new list.

**§B — can a receiver ask the kernel about a sender's rights at all today?**
`sys_cap_inspect` inspects the caller's own capability. **Verify** whether anything
lets a task ask about another task's rights, with a control, and say so plainly — the
answer decides whether §A's capability route is a new syscall (ABI, Gate 4, and a
much larger line) or already possible.

**§C — E-034: carry the word, or drop it?** *Lean: drop it from the four signatures*
— check first, with a control, whether any caller passes a value it needs delivered.
If one does, carrying it is an ABI change to the tag packing and belongs in its own
line, not bolted onto this one.

**§D — what does the stream do with an action it now refuses?** Today `Denied` is a
reply nobody reads. *Lean: the refusal is reported on the stream's own output and
asserted by a tier*, because an authority decision no one can observe is the
condition this whole line exists to end.

**§E — order.** *Lean: E-059 first* — it is the false claim — then E-034.

**Answer all five in writing before implementing.**

## Settled at the review, 2026-10-13

**D7 — accepted, and the mechanism is the right one for the right reason.** Verified
here rather than read: `granted_rights` is now
`PRESENTATIONS[presentation_of(sender)].dispatch_rights`, with `unwrap_or(0)` for a
sender the stream does not know — **fail-closed on an unrecognised sender**, which the
RFC did not ask for and which is the correct default. The payload word is gone from
the decision, and the comment at `:60` now says it is **not** a payload word.

**§3's binding is exactly what §A demanded and nothing looser.**
`CapRights::PROXY_TEXT_DEMO` is declared once (`fjell-cap/src/rights.rs:100`) and read
twice — by the kernel's CSpace install (`spawn.rs:863`) and by the stream's row
(`main.rs:149`). **One declaration, two read sites**, so the grant and the check cannot
drift; every other row is `0`, matching that no other presentation has ever held a
dispatch capability. That is the difference between a policy and a second table, and
you found the form yourself.

**§B's answer is confirmed.** `sys_cap_inspect(tf, tidx, ct)` takes `tidx` from the
dispatcher's `cur_id.index` — never a caller-supplied index — so a task cannot ask
about another task's rights, and the capability route is a new syscall and a different
line. **Your "no" is correct and it is what makes §A's route a conclusion rather than
a preference.**

**D8 — the live failing transcript is the evidence that could not be re-taken, and you
took it first.** 6 accepted / 0 denied with a forged `0xFFFFFFFF` against the
unmodified receiver, 5/1 as the honest control, then 5/1 either way against the fixed
one. That last figure is the one that matters: **the forgery now has no effect**, and
the `semantic-liar` tier keeps it that way.

**D9 — E-034's four signatures: the right call, and the one exception is now its own
erratum.** Three of the four word-categories were inert and are gone; the fourth —
`upgraded`'s `channel_id` — **is read by a real receiver that always sees `0`**, and
§C said carrying it is an ABI change in its own line. You stopped there, which was
right, and the consequence is **E-070**: the update-index fetch has never completed.
The five prebuilts being byte-identical after dropping the parameters is the proof
that dropping changed no behaviour.

**D10 — your question 1: both go in the register, not in a closing note.** **E-068**
(`diagnosticsd` sends an `AUDIT_QUERY` nothing answers — zero mentions in `auditd`)
and **E-069** (`secure-transportd` declares four client endpoints, polls exactly one,
and says in a comment that it polls three) are filed, ACCEPTED, unscheduled. Prose in
a closing note is where a defect goes to be forgotten; the register is what Gate 7 and
every cut read. **Raising them instead of fixing them silently was the right
instinct** — both are outside this line.

**D11 — your question 2: yes, E-070 is different in kind, and it is scheduled.** It is
**behaviour**, not a promise, and it sits in the one area whose tiers (`store`,
`upgrade`) are not release-gated and have no emitting scenario — so nothing would have
caught it and nothing will catch the next one. Filed ACCEPTED, tracked **0.35**, and
**recommended as 0.35's first candidate together with those emitting scenarios.**

**D12 — the near-miss you disclosed is now E-071, and it is an instrument defect, not
an operator error.** `fjell-repro-check` reads its flags with
`args.iter().any(|a| a == "--skip-build")`, so **any unrecognised argument is silently
ignored** and the tool falls through to its full two-build mode — and it has **no
`--help`**, which makes asking for usage the way to start a build. That is the second
time this has fired. An unknown flag must exit non-zero with the usage. Tracked
**0.35**. *Disclosing it, checking `git status` and every prebuilt size immediately,
and confirming the live `test-all` was unaffected is exactly the handling trap 7 asks
for.*

**Accepted as delivered:** the comment sweep, which found and corrected two comments
false for unrelated reasons (`diagnosticsd`'s `AUDIT_QUERY` protocol and
`secure-transportd`'s `OpenChannel` kind word) in files this line already touched;
the `L` trigger and a dedicated test intent after the first design's dependence on
boot-order timing proved wrong — *and saying so*; the three registrations the new
test-only crate needed, the same three every prior one needed; and `--verify` read
before regenerating (`Changed sig: 1`, `Added: 3`, then 445 = 445).

## Requirements

**R1 — Re-derive** Findings 1–4 at your tip: the two line numbers and the comment;
the attested-sender use; that a permitted action executes nothing; and the four
helpers, with a control on each probe (`/usr/bin/grep -a`).

**R2 — §A–§E answered in writing**, §B with its control.

**R3 — D1/D2/§A:** the mechanism, and the argument for why a sender cannot influence
it.

**R4 — D3:** the comment true or gone, and a sweep for any neighbouring comment
making the same kind of claim.

**R5 — D4/§C:** the four helpers.

**R6 — D5:** the two transcripts and the tier.

**R7 — E-059 and E-034 CLOSED** or survivors named, with `v1-limitations.md` in the
same commit as the register, and **D6's wording checked** — the entries must not
imply dispatch became real.

**R8 — The gates**, each by its own exit status, `test-all`, the repro baseline
re-recorded in the same commit as any rebuild, and a CI run id.

### Non-goals

- Making a dispatched action **do** anything (D6).
- An input path for a person (the readiness row stays `IN PROGRESS → v1.x`; the owner
  ruled *no v1 yet* at 0.34's scoping).
- A new syscall, unless §B establishes one is unavoidable — and then escalate rather
  than write it here.
- E-011's rights validation on `cap_install`.

## Risks

**A policy the receiver owns can become the next hand-maintained table.** §A's
constraint is not decoration: if the policy is a list someone edits, this line has
moved the defect rather than removed it.

**The comment is the part most likely to be left behind.** The mechanism will be
reviewed; a sentence two lines above it will not, unless R4 is done deliberately.
