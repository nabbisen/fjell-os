# Fjell OS — Requirements Analysis (Discovery)

The original is a discovery dialogue;
this is a structured summary of its substance — the problems it identified and
the reasoning that led to Fjell's value proposition. The normative requirements
live in [Requirements Definition](./requirements-definition.md).*

## 1. Problems current OSes fail to solve

The analysis began by surveying, broadly, what industry-facing operating
systems do not solve as of 2026. Four problem clusters emerged.

**Fragile "memory" and "boundaries".** The Achilles heel of mainstream
monolithic-kernel OSes remains implicit trust and memory unsafety. Zero-trust is
hard to implement when in-kernel privilege separation is weak, so one driver or
subsystem vulnerability can cascade system-wide. The vast body of memory-unsafe
C/C++ legacy code remains a supply-chain-attack breeding ground. And formal
verification covers only a very limited area of large OSes, so mission-critical
industrial use cannot be given an "absolute" guarantee.

**The limits of bolting AI on.** Current OSes treat AI as just another process.
Heterogeneous compute (CPU/GPU/NPU/quantum accelerators) is not abstracted or
scheduled transparently at the OS level; time-sharing schedulers cannot control
latency for the bursty resource demands of LLM/agent inference; and there is no
OS-level answer to the conflicting demand of isolating edge data safely while
sharing it fast.

**"Process debt" — a disconnect from the social OS.** A large gap has opened
between the software OS and organizations' operational rules. Many business
systems still assume a human clicks a screen to enter data, so agentic workflows
lack a standard, human-out-of-the-loop secure auth mechanism. Siloed data
structures mean over 20% of working time is spent merely searching for
information.

**The sustainability-vs-legacy dilemma.** Energy efficiency is opaque:
"green scheduling" that controls and visualizes per-process power in real time
is not standardized. And regulatory tightening (e.g. the EU Cyber Resilience
Act) risks making non-updatable legacy industrial devices legally unusable,
while OS-level guarantees for safe long-term support and upgrade lag behind.

The synthesis: current OSes may have reached the limit of bloat-in-the-name-of-
generality. What industry most needs is to recover the "simple and robust
aesthetic" of the Unix philosophy while natively accepting the upheaval of AI
and hardware — a refactoring.

## 2. Candidate value propositions

Five sharply-focused OS directions were considered as candidates for a
value-proposition-narrowed rebuild:

1. **AI-Native / Agentic OS** — treats AI models as first-class citizens at the
   kernel level, with semantic scheduling over heterogeneous accelerators.
2. **Formally Verified / Memory-Safe OS** — makes vulnerabilities
   "non-existent by design" via a memory-safe language and seL4-style
   verification with a strict microkernel.
3. **ABDD (Accessible by Default and by Design) OS** — builds accessibility in
   as a basic requirement, normalizing semantic UI information as standard data.
4. **Local-First / Decentralized OS** — cuts cloud dependence, giving the device
   data sovereignty via CRDT-style sync and decentralized identity.
5. **Sustainable / Minimalist Unix OS** — redefines the Unix philosophy for
   "maximum lifespan at minimum power", with a green scheduler.

## 3. The chosen combination

The decision was to fuse **(2) Formally Verified / Memory-Safe** with **(5)
Sustainable / Minimalist Unix**. This pairing gives existing industrial systems
(factories, infrastructure) both the "mature reliability" and the "modern
safety" they crave, and Rust's maturity makes rewriting from kernel to drivers
under one philosophy feasible.

The resulting core philosophy:

- **Memory-Safe & Verifiable** — Rust throughout; ownership eliminates data
  races and memory bugs at compile time; formal verification applied to the
  core scheduler and IPC.
- **Microkernel & Capability-based** — the kernel does only process separation
  and communication; drivers and file systems run in user space; authority moves
  only via unforgeable capability tickets.
- **Accessible by Default and by Design** — internal state, error logs, and
  configuration are normalized to machine-readable, human-understandable
  formats, giving auditors and operators transparency at the kernel level.
- **Sustainable & Green** — unnecessary abstraction layers removed; hardware
  features (PMP, enclaves) driven directly; idle power driven toward zero.

## 4. Integrating ABDD without scope creep

A key concern was that adding ABDD would cause scope creep and break the
project. The analysis reached the opposite conclusion: the ABDD approach —
separating intent from rendering — **resonates with (2) and (5) and makes the OS
core smaller**, for three reasons.

**Unix philosophy, modernized: "everything is a semantic stream."** Where Unix
said "everything is a file (text stream)", Fjell removes the window manager and
graphics stack from the kernel and base system entirely. What flows between
application and user is not pixels but a typed stream of pure intent. The OS's
one job (done well) is to route that stream safely.

**Separation of concerns makes verification realistic.** GUI stacks (X11,
Wayland, font rendering) are too large to formally verify. Stripping rendering
responsibility from the OS and pushing it to a Personal Proxy dramatically
reduces core code and attack surface, making a fully verifiable core achievable
at realistic cost.

**Contribution to sustainability.** Removing screen rendering from the base
requirements drops base-system power sharply. A display-less industrial robot
and a visually-impaired user's personal device can run the exact same minimal,
robust core OS — only the output target (proxy) differs. This is the ultimate
modularization and minimizes long-term maintenance cost.

### The reframed accessibility approach

Crucially, the analysis rejected "enumerate every marginalized pattern and
branch on it (if-then)" as an approach that must eventually collapse — human
diversity is continuous and new needs are discovered continually. Three paradigm
shifts replace it:

1. **Full separation of "rendering" from "intent" (Intent-Based UI).**
   Applications do not draw a "blue rectangle (button)" that a screen reader
   must reverse-engineer; they emit a tree of pure semantic data — "this node
   asks for a consent/decline decision" — and how it is expressed (visual,
   speech, braille, neural trigger) is entirely outside the OS's responsibility.
2. **Dynamic compilation via a Personal Proxy.** Instead of pattern-matching in
   the OS, each user holds a personal agent that knows their physical/cognitive
   traits and current fatigue. The OS emits structured semantic state; the proxy
   compiles it, in real time, into the interface optimal for that user right now.
   A newly discovered need is handled by the proxy learning a new translation
   rule — no change to OS or app code.
3. **From categories to continuous state measurement.** Fixed labels
   ("visually impaired", "motor impaired") are discarded. The system monitors
   continuous telemetry (input latency, mistap frequency, pointer jitter) and
   adapts seamlessly (enlarging target sizes, lowering information density). This
   also serves a fully-abled user operating one-handed on a crowded train or
   when exhausted — "continuous environmental adaptation to state" rather than
   "branching for a specific someone".

A truly ABDD OS is therefore **not** an OS with countless accessibility settings
screens, but one with **no settings screen for adapting to the user** — where a
proxy standing between system and user continuously translates semantic data
into the optimal form.

## 5. The open architectural question this raised

The analysis closed on the boundary problem this design creates: if the OS core
emits pure intent data and a user-side Personal Proxy translates it into an
interface, then the trust boundary between OS and proxy needs authentication and
tamper-prevention that is **minimal yet robust**. That question is answered in
the requirements definition as **FR-SEM-004** (per-session capability, message
signing, monotonic sequence numbers, replay prevention, read-only channels, and
per-proxy authority scope), and is realized in the implementation by the signed,
capability-scoped semantic-stream design.
