# RFC-0.35-002: Protocols and authority nobody serves

**Status:** Accepted (0.35) — accepted 2026-10-14
**Milestone:** 0.35
**Tracks.** **E-068** — `fjell-diagnosticsd` documents and sends an `AUDIT_QUERY`
protocol `fjell-auditd` has never implemented — and **E-069**,
`fjell-secure-transportd` declaring four client endpoints, receiving on one, under
a comment saying it polls three in round-robin. One line because they are one
defect: **a component claiming a conversation that does not happen**, once in a
shared API's protocol constants and once in a service's endpoint set.
**Touches** *(indicative)*: `crates/fjell-service-api/src/lib.rs`,
`crates/services/fjell-diagnosticsd/`, `crates/services/fjell-secure-transportd/`,
`crates/services/fjell-auditd/`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Also tracks (added 2026-10-14, at RFC-0.35-001's first review): E-074** —
`fjell-attestd`'s `push_attestation` and `fjell-netd`'s `CAP_DRV_EP` address a peer
through a capability nobody installs and have **no caller at all**. Split from E-072
at its closure because the remedy is this line's question, not that one's: **serve it
or retract it**, and the ruling below applies to both by the same four arguments.
Neither busy-spins; both are promises in the shape of code.
**Relates to:** **RFC-0.35-001**, which establishes that the slots these protocols
would travel through are not installed at all — read it first; **T23** of
`threat-model-v1.md`, the operator shown false state, which E-068's hang is an
instance of; **E-011**, rights validation on install.

## The owner's ruling, and mine

The owner was asked whether `auditd` should **grow** an `AUDIT_QUERY` arm and
`transportd` **serve** its other endpoints, or whether both callers should
**retract** the claims. They returned the question with their standing bar:
*"finally clean, safe and secure, robust and sophisticated design"*, and *"APIs and
UI/UX for users not to be confused or misunderstand are also very important."*

**Ruled: retract.** Four reasons, in the order the bar implies them.

1. **Serving would build the architecture around a claim.** Who may read the audit
   log is a capability question — which right, under what badge, revocable how,
   observed by which tier. None of that has been decided. Bolting a query arm onto
   `auditd` so that an existing comment becomes true is retrofitting a design to a
   sentence somebody already wrote. That is a workaround kept in the architecture,
   which is precisely what *finally clean* refuses.
2. **Serving four endpoints is not available to us.** A real round-robin needs the
   multiplexed `recv` that `transportd`'s own comment dates to "v0.4.0" and which
   does not exist. The alternatives today are a poll across four slots — a
   busy-wait, i.e. a deliberate version of the defect RFC-0.35-001 just found — or
   kernel work far outside this milestone.
3. **Holding authority you do not exercise is a least-privilege violation**, in a
   system whose whole thesis is capabilities. `transportd`'s unserved endpoints
   should not be *installed* the day they are not used; the right time to install
   them is the day something receives on them, with a tier that shows it.
4. **`fjell-service-api` is the surface a service author reads to learn what the
   system speaks.** A three-constant audit-query sub-protocol with per-record and
   stream-end tags, implemented by nothing, teaches a reader something false. That
   is the *users not to be confused* clause, applied to the only users this API
   has.

And retraction is the **reversible** choice. Adding the protocol later, with its
capability design and its tier, is ordinary work. Un-teaching an API that shipped a
dead protocol is not.

**What is not a decision, and happens either way:** the comments stop describing a
loop and a protocol that do not exist, and **the loops stop spinning on absence.**

## Summary

Derived 2026-10-14 at `9dc6454`.

**E-068.** `crates/fjell-service-api/src/lib.rs:417–421` declares `AUDIT_QUERY`
(`0x410`), `AUDIT_RECORD` (`0x411`) and `AUDIT_STREAM_END` (`0x412`) under a
heading citing RFC v0.4-005. `fjell-auditd` matches `AUDIT_QUERY` **zero** times
(word-boundary probe; control: `fjell-diagnosticsd` matches four). `diagnosticsd`
sends it at `:98` and then loops on `recv_msg`, breaking only on
`AUDIT_STREAM_END`, `continue`-ing on anything else — and `recv_msg` maps `Err(_)`
to `(0, 0, 0)`, a tag that is neither. **`collect_audit_events` never returns**, so
`build_bundle` never returns, so the two arms that call it (`:167` on
`BUILD_BUNDLE`, `:172` on `PUSH`) never reply.

**The only reason this has never been seen is that nothing in the tree can send
the trigger**: `BUILD_BUNDLE` has exactly two sites — its declaration and
`diagnosticsd`'s own comparison — so no in-tree caller reaches it. The erratum
recorded *"returns nothing, silently"*; the truth is **the first operator-triggered
diagnostic bundle hangs the service**, and the register is corrected to say so.

**E-069.** `transportd` declares `CAP_SMGR_EP`(1), `CAP_UPGRAD_EP`(2),
`CAP_DIAG_EP`(3), `CAP_ATTEST_EP`(4) at `:51`–`:54`, and `recv_msg` has exactly
**one** call site, `:136`, on `CAP_UPGRAD_EP` — the other three occurrences are the
helper's own definition. `:131` says *"we poll the three client endpoints in
round-robin."* There is no round-robin, there are four endpoints, and one is read.

## Requirements

**R1 — Re-derive** both findings at your tip, each probe with a control, and
**re-derive the hang**: that `collect_audit_events` cannot terminate, and that no
in-tree sender reaches it. If you find a sender, **stop and escalate** — the
severity changes and so does the milestone.

**R2 — The audit-query sub-protocol is retracted.** The three constants leave
`fjell-service-api`; `diagnosticsd` stops sending and stops looping for a reply it
cannot get; what replaces the call is a bundle that **states** the audit section is
unavailable, not one that silently contains nothing. A caller must be able to tell
"no records" from "not implemented" — that distinction is the whole finding.

**R3 — The retraction leaves a record, not a hole.** An entry that says the
audit-query path is **not designed yet** and what designing it requires (the
capability question named in the ruling above), so the next person designs it rather than rediscovers
a stub. A deleted protocol that leaves no trace invites its own reinvention.

**R4 — `transportd` declares only what it uses.** The three unserved endpoint
constants go; `:131`'s comment describes the single-endpoint loop that exists.
Nothing here installs a capability — RFC-0.35-001 decides installs, and its R5
deliberately excludes these.

**R5 — No loop spins on absence.** *(Partly landed early: RFC-0.35-001 added
`sys_yield()` to both recv-error arms, because E-072's closure required the spin to
be gone. What remains here is the rest of the requirement — and note that
`diagnosticsd`'s loop still errors on **every** iteration, politely now, because its
slot is this line's decision.)* In both services, an unexpected tag and a
**recv error** must be distinguishable from each other and from a message, and
neither may re-enter the loop silently. Fail closed and say so on the console; a
service that cannot receive is not a service that should keep trying at full speed.
State which behaviour you chose — exit, back off, or block — and why it is right
for a service nothing is talking to.

**R6 — A demonstration, not an absence.** Show the hang before R2 (a scratch build
that sends `BUILD_BUNDLE` and does not come back), and show it gone after. An
absence probe is not evidence that a loop terminates.

**R7 — E-068, E-069 and E-074 CLOSED**, with E-068's **consequence corrected** in
the register — *"returns nothing, silently"* understates a hang — and
`v1-limitations.md` in the same commit.

**R8 — The gates**, each by its own exit status; `test-all`; a CI run id.

### Non-goals

- Implementing an audit query (that is the thing being deferred, deliberately).
- Giving `attestd`'s `push_attestation` or `netd`'s `CAP_DRV_EP` a caller **and** a
  capability. If either is kept, both are needed in one line; this line's expected
  answer for them is deletion.
- Serving `diagnosticsd`, `attestd` or `service-manager` over the secure transport.
- Installing any capability slot (RFC-0.35-001).
- A kernel multiplexed `recv`.

## Risks

- **Retraction can over-reach.** `AUDIT_RECORD` is a substring of unrelated
  identifiers in the kernel and `fjell-audit-format` (19 loose matches, 4 exact).
  Use word-boundary probes and name the control, or you will delete something that
  is in use.
- **"Unavailable" can become its own lie.** A bundle that says the audit section is
  unavailable must say it for the real reason, and must not be a placeholder that
  outlives the retraction unexamined.
- **The deferred design can be lost.** R3 exists because a clean deletion is the
  easiest way to forget why the deletion happened.
