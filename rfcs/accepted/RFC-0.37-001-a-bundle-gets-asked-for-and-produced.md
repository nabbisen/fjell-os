# RFC-0.37-001: A bundle gets asked for and produced

**Status:** Accepted (0.37) — accepted 2026-10-18
**Milestone:** 0.37
**Tracks.** **E-078** — `fjell-diagnosticsd` cannot be asked for anything and every
path inside it is dead — **E-081**, `fjell-fleetd` having no event loop at all, and
**E-083**, `diagnosticsd` sending tag numbers `measuredd` does not answer and the
shared API had already assigned elsewhere. One line because they are one defect in
three places: **a conversation the design specifies and the code cannot hold.**
E-078 cannot close without E-081 (its named asker has exited) and must not close
with E-083 open (the install would look like a fix and change nothing).
**Touches** *(indicative)*: `crates/services/fjell-fleetd/`,
`crates/services/fjell-diagnosticsd/`, `crates/fjell-kernel/src/task/spawn.rs`,
`crates/fjell-service-api/src/lib.rs`, `tests/qemu/profiles/`,
`crates/services/fjell-neg-test/` or `fjell-init` for the trigger,
`tools/fjell-consistency-check/src/capability_topology.rs`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **RFC-v0.8-005**, which specifies the conversation this line makes
real — *"the fleet manager sends a `RemoteDiagRequest`; the node responds with a
`RemoteDiagResponse` containing a signed `DiagBundle` snapshot. No general remote
execution. The node always controls what it exports."*; **RFC-0.35-001**, whose
capability-topology work and `upgrade`-tier pattern this line follows;
**RFC-0.36-001**, which made the bundle's fields honest and whose
`audit_available`/availability-flag mechanism this line is the first to exercise
against a live caller; **RFC-0.36-001's** moving of `verifyd`'s tags into
`fjell_service_api::tags`, which is E-083's precedent.

## Why this line is larger than its erratum said

**E-078 has been corrected twice, and both errors were the architect's.** It first
claimed the work needed the deferred input path — it does not; RFC-v0.8-005 names
the initiator as a fleet manager over the network, not a person at a console. It
then described `fleetd` and `diagnosticsd` as *"two stubs facing each other across
a designed protocol"*. **One of them has exited.**

`fjell-fleetd` runs format self-checks, prints `fleetd: ready`, and calls
`sys_exit(0)` — directly beneath the section header
`// ── IPC event loop (v0.8.1 — storaged + cap-broker wiring) ───`. `loop {`
appears **zero** times in the file; it declares **no** capability slots. So the
asker must become a service before it can ask.

**The owner ruled, 2026-10-18, that it becomes one** rather than being retracted:
RFC-v0.8-005 specifies the conversation and `fleetd` is its named node-side
endpoint.

## What this line does not do, and the architect's own error in saying otherwise

**No bundle leaves the node.** `push_bundle` addresses slot 2, which stays
uninstalled, so it keeps failing closed and replying `PUSH_FAULT`.

**Therefore this line needs no threat-model entry** — and E-078 and 0.36's scope
both said the wiring and the threat-model entry go together. **That was wrong.**
The export boundary becomes real when a bundle can be pushed off-box; until then
`threat-model-v1.md` has nothing new to carry, and adding an entry for a boundary
nothing crosses would be the kind of pre-emptive claim this project files errata
about.

## Requirements

**R1 — Re-derive all three findings at your tip**, each with a control: `fleetd`'s
missing loop and absent slots; `diagnosticsd`'s uninstalled receive slot and the
two-site reachability of `BUILD_BUNDLE`/`PUSH`; and E-083's tag numbers on **both**
sides, including that `0x320`/`0x321` are `recoveryd::READY`/`LIST_SNAPSHOTS` in
the shared API. **Sweep for a fourth**: any other locally-declared tag constant in
either service that duplicates or collides with a shared one. If you find one,
**stop and escalate** — that is how E-083 itself was found, and the sweep is the
part of this line most likely to pay.

**R2 — E-083 closes first, before any capability is installed.** `diagnosticsd`
reads the **shared** `fjell_service_api::measuredd::{GET_HEAD, HEAD_REPLY}`; its
local `0x320`/`0x321` go; the five hand-copied `SXT_*` constants come from one
declaration shared with `secure-transportd`. **Demonstrate the mismatch before
fixing it**: with the capability installed and the old tags, show `measuredd`
matching no arm. Installing a slot and assuming the head is now real is the one
failure mode E-083 was filed to prevent.

**R3 — `fleetd` becomes a service.** A real event loop, its own capability slots,
and no `sys_exit(0)` on the success path. The dead section header becomes true or
goes. **Something must assert it is still alive after `ready`** — printing `ready`
and exiting is indistinguishable, to every instrument this project has, from
printing `ready` and serving, which is exactly why E-081 went unnoticed.

**R4 — `diagnosticsd` becomes reachable**: its receive slot installed, so
`BUILD_BUNDLE` arrives from `fleetd`. Install **only** what the conversation needs
and say in writing which slots and why each was necessary — slot 2 (`sxt`) stays
out, by §Non-goals, and the least-privilege argument is the same one RFC-0.35-001's
R5 made.

**R5 — Slot 4 installed, and the measurement head real.** After R2, with the
shared tags, `query_measurement_head` reaches `measuredd`'s `GET_HEAD` arm and gets
a live digest. `measurement_head_available` then reads **`true`**, set by the code
that obtained the value and by nothing else — RFC-0.36-001's own risk, now
exercised for the first time against a live caller.

**R6 — A tier, triggered by a console byte**, as `semantic-liar`, `health-fail`
and `upgrade` all are. Inject; `fleetd` asks; `diagnosticsd` answers. Assert **at
the deciding service's own output**, not only at the instigator's: the bundle
exists, `measurement_head_available` is `true`, and the fields that genuinely have
no source read `false`. **The flags are the assertion** — a tier that only checks
a bundle was built would pass on a bundle full of honest zeros.

**R7 — `capability-topology` stays honest.** Its discovered-slot count moves;
waivers for the slots this line installs are retired, and any slot still waived
names the entry that actually holds it. The bookkeeping test's own name must not
become a stale claim — RFC-0.36-001 renamed it `..._known_five` for this reason.

**R8 — E-078, E-081 and E-083 CLOSED**, `v1-limitations.md` in the same commit,
and **E-084 and E-082 untouched** — they are RFC-0.37-002's. `fjell-netd` is not
this line's business even though it is the same class as E-081.

**R9 — The gates**, each by its own exit status, reported as the **required set**
with failures included. Guest services change, so prebuilts will move: rebuild
**then** re-record the baseline, in the same commit.

### Non-goals

- **Pushing a bundle off-box.** Slot 2 stays uninstalled; `push_bundle` keeps
  failing closed. The export boundary and its threat-model entry are a later line.
- **A `RemoteDiagRequest` arriving over the network.** That needs `netd`, which is
  a disclosed exit-by-design stub carrying E-084. The trigger here is a console
  byte.
- **`fjell-netd`'s unreachable loop** (E-084) and **the lint suppression** (E-082).
- Gathering real audit events or semantic intents. `audit_available: false` and an
  empty intent list stay correct and stay honest.

## Risks

- **Installing slot 4 before R2 lands would look like success.** The query would
  reach a service that does not match the tag, and a careless line would set
  `measurement_head_available: true` on a digest it never received. R2 is ordered
  first for this reason and the demonstration is required.
- **`fleetd` is the first program this project gives an event loop to.** Every
  previous line fixed a loop that misbehaved. Expect the task-table, stack and
  endpoint limits that `project_adding_a_service…` records, and expect
  `init: spawn error` in `health-fail` as the symptom if a limit is hit.
- **A bundle of honest zeros passes a careless tier.** R6's assertion is the
  flags, not the bundle's existence.
- **This line makes a dead plane live.** The first thing that runs on a path
  nothing has ever exercised tends to find the next defect; if it does, **file and
  escalate** rather than absorbing it, as the 0.36 line did correctly with its own
  fifth finding.
