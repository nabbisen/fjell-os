# RFC-0.33-002 R1, §A–§D, and what writing A4 found out

**Governing RFC:** [../accepted/RFC-0.33-002-who-fjell-is-for.md](../accepted/RFC-0.33-002-who-fjell-is-for.md)
**Handoff:** [../handoffs/RFC-0.33-002-who-fjell-is-for/implementation-handoff.md](../handoffs/RFC-0.33-002-who-fjell-is-for/implementation-handoff.md)

Written after R1 and before any page, in the handoff's order. Every absence is
probed with `/usr/bin/grep -a` and a control **on the file or directory the
claim is about** (the E-055 lesson). The limitations section (D6) was written
before the pages that make the claim; this document says why it *could* be.

---

## Two things first, because they change what the pages may say

### A4 cannot say "the node carries on when the presentation is unavailable" — the tree does the opposite

The handoff asks A4 to say *"what it does when the presentation is unavailable"*
and warns that if that cannot be written concretely, that is a finding. **It is
a finding.** Measured, in a scratch worktree (not the working tree; two edits,
both reverted with the worktree):

| Variant | Edit | Result (`semantic` profile, 60 s) |
|---|---|---|
| **V1** — the presentation never starts | `init` does not spawn `proxy-text` | **125 lines against the normal 313.** `init` stops advancing at its first semantic publish after boot: no `[STATE]`/`[EVENT]`/`[INTENT]`, no `TEST:M7:PASS`, and `driver-uart` — `init`'s last phase — never starts. Other services carry on (`NEG:SVC:*` markers still print). |
| **V2** — the presentation crashes mid-run | `proxy-text` faults on its 40th received call | The fault is reported (`[task#9 proxy-text]: fault(LoadPageFault)`), 229 lines against 313; **`init` stops at its next publish**, again before `driver-uart`. Everything rendered before the crash is intact. |

*To reproduce (a `git worktree add --detach <dir> HEAD`, then `cargo xtask build`
and `cargo run -p fjell-tools -- qemu-run --profile semantic` inside it):* **V1**
— replace `fjell-init/src/main.rs`'s `spawn(ImageId::PROXY_TEXT, "M5: proxy-text
started");` with a `sys_debug_writeln`. **V2** — in `fjell-proxy-text/src/main.rs`,
count calls at the top of the `loop { let (tag_packed, …) = recv_call(); … }` and
`core::ptr::read_volatile(0usize as *const u8)` when the count reaches 40.
Counts: `grep -ac` of `[STATE]`, `[EVENT]`, `[INTENT]`, `Verified boot status`,
`TEST:M7:PASS`, `driver-uart: ready` in the run's `serial.log` (V1: 0, 0, 0, 0, 0, 0;
V2: 3, 1, 1, 1, 1, 0; normal: 8, 3, 3, 1, 2, 1).

**The mechanism is in the source, not in the experiment.** `semantic-stream`
forwards each envelope to `proxy-text` with `chunked::send` — a blocking
`ipc_call` — **before** it replies to the publisher
(`fjell-semantic-stream/src/main.rs`, `forward_to_proxy_text`, called from the
`PUBLISH_COMMIT` arm ahead of `reply(PUBLISH_OK …)`). The publisher is itself
blocked in a call to `semantic-stream`. **An absent or dead presentation
therefore stalls the emitting service, not just the view.** For an archetype
whose defining property is *"the same core, only the proxy differs"* that is the
wrong way round: the proxy's availability gates the core.

This is a design defect against the project's own claim, not a documentation
slip, and this line does not fix it (D8: documents, not code). It is what
RFC-0.34-001's fan-out must not multiply — a second proxy doubles the number of
things a publisher can be stuck behind. **Recommend it is stated in 0.34's RFC
as a requirement (an unavailable presentation must not stall a publisher) and
filed as an erratum by the architect;** I have written it into D6's limitations
in the words above and into A4 as the answer to *"what does it do when the
presentation is unavailable"*: *today, the publisher waits.*

### The proxy's "return leg" is not authority-bearing, and its comment overstates it

`fjell-proxy-text` issues `DISPATCH_ACTION` back to `semantic-stream` with a
`granted_rights` word it reads from its own capability (`sys_cap_inspect` on
`DEMO_CAP_SLOT`); `semantic-stream` checks the intent's `required_capability`
against that word and replies `Ok`/`Denied`. Two facts bear on §C and §D:

1. **The word is carried in the IPC payload.** `semantic-stream` cannot inspect
   another task's CSpace, so it trusts the number. The doc comment on
   `dispatch_action_checked` says the mask is *"kernel-verified … not
   self-asserted"*: it is kernel-verified **where the proxy reads it**, and
   self-asserted **where the stream receives it**. A compromised proxy chooses
   its own rights. (Sender identity is attested — T17 — but the payload word is
   not.)
2. **Nothing executes.** A permitted action returns `"action accepted"`; no
   service is asked to do anything. The path demonstrates the *shape* of a
   checked return leg — proven by refusal in the `semantic` tier — and is driven
   by the proxy's own code for each action of a rendered node, **not by a
   person**.

So the sentence "there is no input path through the proxy" (Finding 4) is
correct but incomplete, and I have written the complete version: **the proxy
never reads a person's input (ADR-v0.5-005); a proxy→stream action leg exists,
driven by the proxy's own code under a demonstration capability, executing
nothing.** This is source-comment territory (code), so it is reported here, not
edited.

---

## R1 — Findings 1–5, re-derived

Sources: `.git-exclude/specs/fjell-os-requirements-v1-20260504.md` (the founding
document), the book's `requirements/requirements-definition.md`, and the tree.

| Claim | Re-derived | |
|---|---|---|
| **F1.** The closing note names inclusion as one of four things the design aligns | Present (spec, *Supplement*): *"…a design principle for separating presentation from processing and allowing the OS to transport semantic streams safely. This aligns formal verification, minimalist Unix, sustainability, and inclusion in the same direction."* | ✔ **but the RFC's quotation is not the founding document's wording.** It is the *book's* English rendering (*"separating display from processing so the OS can carry semantic streams safely"*, `requirements-definition.md` last section). Same meaning, two renderings. **The pages quote the spec verbatim and cite the book's rendering separately.** |
| F1. §2.6 ABDD one of six design principles; §3.1 lists accessible external UIs as **primary**; §8 GUI-independent semantic stream **Must** | §2.6 ✔; §3.1 `Devices requiring integration with accessible external UIs` ✔ (primary list); §8 Must ✔ | ✔ |
| **F2.** §4.1 excludes desktop environment, app store, gaming, video/3D, drop-in compat; §8 `Won't (initial phase)` | ✔ all five; §8 `Won't for Initial Phase: General-purpose desktop OS conversion` ✔ | ✔ **plus a drift the RFC did not name: the spec says the desktop is not an *initial* goal — §4.1 opens *"shall not **initially** aim to replace…"* and *"not included in the **initial** goals"*. The book's §4.1 (`requirements-definition.md`) drops the word:** *"Not aim to be a general-purpose desktop OS"*. The book states a permanent non-goal where the founding document states a sequencing one. D3's re-statement restores it. |
| F2. §4.5 no GUI rendering stack in the core | ✔ verbatim in both | ✔ |
| **(new) §4.6** *Do Not Attempt to Enumerate Every Accessibility Pattern* — no OS-side categories *"for people with visual impairments"* | Spec §4.6 ✔, book §4.6 ✔ | **A constraint on how A4 and D6 are written**: modalities (speech, braille, simplified) are *presentations*, never user categories; D6 speaks of what a *person needing* a presentation cannot do because the handoff asks for that, and says nothing on the OS side keyed to a person. NFR-ACC-003 says the same. |
| **F3.** Intro pages do not mention accessibility, ABDD or inclusion | `grep -rna 'accessib\|ABDD\|inclusion' docs/src/intro/` → nothing; **control**: the same directory returns hits for another term (`capabilit`: what-is 3, why 1) | ✔ |
| F3. N3 rationale: *"Fjell targets headless edge/fleet nodes (A1/A2/A3)"* | `v1-non-goals.md:36` ✔; **only occurrence in the tree** (`docs/src`, `README`, `ROADMAP`) | ✔ |
| F3. `identity/v1-direction.md` lists *"Desktop / laptop user environments"* | ✔ (`What Fjell is not`, first list) | ✔ |
| **(new) F3 — the same narrowing is in six more places** | `README.md:39` *"Primary archetypes … (A1) (A2) (A3)"* and `:41` *"Not for: … desktop environments"*; `intro/non-goals.md` *"Desktop GUI or web browser hosting"*; `intro/why-fjell.md` *"When not to use Fjell … a GUI"*; `identity/v1-direction.md:22` *"three archetypes"* and `:132` *"Demonstration against at least one of A1, A2, A3"* (the research-track promotion rule); `v1-non-goals.md:98` (N10, *"no current A1/A2/A3 use case"*); `v1-readiness.md:15` *"Archetypes A1, A2, A3 defined"* (**DONE**, v0.9.4) | **The RFC names three edits; the audience is stated in nine places.** All but the last are corrected (README, both intro pages, identity ×2, N3). **N10 and the DONE row are left**: N10 is about WASM and A4 is not a use case for it, and the row is a dated fact about v0.9.4. |
| **F4.** ADR-v0.5-005 makes `proxy-text` output-only; operator input goes through `fjell-tools` over a separate channel | ADR ✔ (*"Operator input is delivered through the `fjell-tools` CLI over a separate capability-gated IPC path"*) | ✔ **as a statement in the ADR. As a fact about the tree it is not:** no `fjell-tools` subcommand delivers anything to a running node (the dispatch list in `fjell-tools/src/main.rs` has none; the harness's stdin injection is test-only). The only input that reaches a node is UART RX (RFC-0.25-001) → `driver-uart` → **`init`**, the sole consumer of that endpoint (`UART_RX_EP`, one site), which uses it for a test hook. **D6 states the ADR's route as intended and unbuilt.** `v0.23-direction-options.md` M4 (*"no console input path anywhere"*) is older than 0.25 and no longer exactly true; it is a dated record and is not edited. |
| **F5.** One proxy; no conformance claimed anywhere | `ls crates/*` → only `fjell-proxy-text` (control: `fjell-semantic-toolkit` is generated emitters, not a presentation); `grep -rnaiE 'EN 301 549\|section 508\|WCAG'` over `docs/src`, `crates`, `README`, `ROADMAP` → **nothing** (control: the same command finds `screen reader` in three files, so the pattern reaches those directories) | ✔ |
| **(new) What reaches the proxy** | **Only two services publish to `semantic-stream`: `init` and `sample-service`** (`PUBLISH_BEGIN` sites; control: `init` has one). No running service reports its live state through the stream. | **A limitation the RFC did not name and D6 must:** the intent stream a presentation renders is boot-time narration from `init` plus one SDK sample, not the state of the running node. |
| E-054 item 4 quotes `abdd-semantic.md` as *"a screen, a screen reader and an assistive personal device run the same core; only the proxy differs"* | The file says *"A display-less industrial robot and an assistive personal device run the *same* core; only the proxy differs."* (`abdd-semantic.md` §4) | **A paraphrase presented as a quotation** (in E-054 and again in RFC-0.34-001). The claim is the same; the words are not. Not corrected in the register (dated record); the pages quote the file. |
| **Owner's note: the matrix has "80 rows"** | `v1-readiness.md`: **80 table lines**, of which 9 are separators, 8 headers, 5 the summary table → **58 status rows** (`readiness-check`: 55 DONE + 3 DEFERRED; the same file has no mention of ABDD/accessibility/proxy/inclusion/semantic — control: `Ed25519` appears) | ✔ the absence; **the figure is 58, not 80.** I use 58. |
| **(new) The matrix's prose contradicts D9's marking** | Header: *"Every cell must be DONE or DEFERRED … before the v1.0.0 tag. OPEN cells block the release."* Summary: *"v1.0.0 released. Zero OPEN cells. Zero IN PROGRESS items."* — **there is no v1.0 tag** (96 tags, last `0.32.0`; workspace `0.32.0`). | The header says an `IN PROGRESS` cell may not survive to the tag; D9 adds three. **See "Instrument" below and question 3 in the review request.** The summary line is false today and predates this line. |
| **(new) `abdd-semantic.md` contradicts the RFC** | §3 as-built evidence for FR-SEM-005 (*same operation via console, API, tooling, proxy*) reads *"text console + structured API"* — there is **no console input** and no operator API into a node (above). §5 says audio and braille are *"out of v1.0 scope"* — D9/D10 make a second modality a v1.x criterion. | Both are corrected (the RFC lists this page as touched). |

**Disagreements with the RFC, summarised:** the F1 quotation is the book's, not
the spec's; §4.1 lost *"initially"* in the book; §4.6 exists and constrains the
wording; the audience is narrowed in nine places, not three; the ADR's
`fjell-tools` input route is unbuilt; only two services publish to the stream;
the matrix has 58 status rows, not 80; and the matrix header contradicts D9's
marking. **None changes a settled decision.** One (the unavailable presentation)
changes what A4 may say.

---

## §A — Does general-purpose *personal* computing become a long-term goal?

**Not decided here.** An opinion, labelled as one: **leave it undecided, and do
not add it to `ROADMAP.md`.** D3's wording says *at v1* and the founding
document already says *initially*; that is the whole of what can be claimed. Two
reasons for not going further. First, the honest answer depends on a question
this line found unanswerable from the tree: whether personal use *is*
"a node whose interface is meaning" (in which case A4 already covers it) or "a
desktop" (in which case it needs an application model, an input plane, display
drivers and a font stack, the last of which §4.5 keeps out of the core). Second,
a v2+ entry reads as a commitment to the person it would matter most to, and
the RFC's own risk section says why that is dangerous. **If the owner decides
"eventually yes", it belongs in the v2+ table as a direction, worded as a
question the architecture would have to answer, not in §4.1.**

## §B — What does "inclusive" commit to?

**Agree with the lean**, and the tree supports it: speech, braille and
simplified presentation are named as *roadmap* (one is 0.34's); the input path
is named as an *open design question* (§C). Three modalities with one proxy is a
promise; one demonstrated is evidence. Two additions from R1. **Say "presentation",
never "user category"** (§4.6, NFR-ACC-003) — the pages name what the node can
emit, not who it is for. And **the commitment is bounded by the platform:**
QEMU `virt` has no audio device or braille display (RFC-0.34-001 Finding 3), so
until hardware exists a modality can only be *emitted*, never *heard or felt*;
the pages say "the stream a synthesiser would consume", not "speech".

## §C — How does input reach the system without bypassing capability policy?

**Not yet; here is the shape of the question.** ADR-v0.5-005's reason is sound
and stays. What R1 adds:

1. **Today there is no path to relax.** The ADR's `fjell-tools` route is
   unbuilt; UART RX reaches `init` and nothing else. The question is not "should
   the proxy accept input" but "what is the first input path at all".
2. **The stream already has the outline of an answer**: an *input request* is a
   kind of intent (FR-SEM-001), an intent's actions carry the capability they
   require, and a return leg with an accept/refuse decision exists (proven by
   refusal). The natural candidate is to **let a proxy forward a person's
   *selection of an action the node itself offered*** — a choice among the
   node's own options, not free text — with authority held by a **session
   capability** issued to the proxy (FR-SEM-004: *session-scoped capabilities,
   per-proxy permission scopes, read-only channels*). Free-form operator commands
   remain `fjell-tools`'s.
3. **Before that can be trusted, the existing leg needs its own fix**: the
   rights word is self-asserted at the receiver (above). An input path built on
   it would inherit that.
4. **It interacts with the unavailable-presentation defect**: an input path
   through a proxy that can wedge the publisher is a path an assistive user
   cannot rely on.

**Recommend a 0.34-adjacent ADR** (D9's second row asks only for a *decision*):
scope it to "selection of an offered action under a session capability", say
what it rules out, and leave implementation to a later line.

## §D — Does A4 change the threat model?

**Yes, and the threat model has no proxy in it to change.**
`docs/src/security/threat-model-v1.md` contains no mention of a proxy or a
presentation (`grep -nai 'proxy\|presentation'` → nothing; control: `T17` is
found in the same file; nothing under `docs/src/security/` mentions a proxy).
So this is not "A4 adds a boundary", it is *the boundary that carries every
operator-facing byte was never modelled*. T17's adjacent class (a correctly
identified sender with a malformed payload) is exactly a proxy's situation, and
the self-asserted rights word above is a second. **It should be a threat-model
amendment with its own RFC**, as the RFC says — it is `Authoritative for
v1.0.0` and governed by RFC-v0.15-002, so it is not edited in a documents line
that is about audience. D6 lists it as a limitation; the review request lists it
as a candidate.

---

## What this line does, in order

R1 (this document) → **D6** (the limitations section, first) → D1/D4/D5 (intro
pages, identity, N3, README) → D3 (§4.1 restated; §4.5 shown untouched) → D9
(three rows; `readiness-check` before **55 DONE / 0 IN PROGRESS / 3 DEFERRED /
0 OPEN**) → D10 (roadmap) → R6 (E-054) → gates and the built book.

**What it does not:** touch code (including the comment above), decide §A,
edit the specs, claim a standard, or fix the unavailable-presentation defect.
