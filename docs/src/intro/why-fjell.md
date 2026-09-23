# Why Fjell?

Fjell exists for operators who must answer the question *"prove what this
device did, and prove who authorised it"* — and cannot, on a conventional
OS, because authority there is ambient (root, file permissions, network by
default) and evidence is incidental (logs that may or may not exist).

It exists, equally, for nodes whose interface is *meaning* rather than pixels —
so that how a person perceives or operates a node is a presentation that can be
supplied, not something the OS decides. That is inclusion as a goal and a
mechanism, not a delivery: [What is Fjell?](what-is-fjell.md) says how far it
goes, and A4 below says what it is for.

## The four archetypes Fjell is designed around

**A1 — Industrial gateway.** A long-lived control-network gateway in a
regulated industrial setting: substation telemetry, a factory-floor cell
controller, a water-treatment bridge. Strict change control over a long
operational lifetime; every action must trace to a signed authority.

**A2 — Sensor / edge fleet node.** One node among 10²–10⁵ devices, often
power-constrained and intermittently connected: environmental monitoring,
asset tracking, distributed metering. Offline-first operation; the fleet
operator needs per-node attested state without per-node access.

**A3 — Regulated field device.** A device under certification regimes
(IEC 62304, IEC 61508, ISO 27001, IEC 62443 adjacent): medical-adjacent
controllers, safety-critical edge instruments. A compliance auditor must be
able to reconstruct any state from recorded evidence.

**A4 — Operator-attended node with an assistive presentation.** The same kind of
node as A1 — a control-network gateway, a cell controller — attended by a person
who reads its state, warnings and offered actions through something other than a
screen: speech, braille, or a simplified summary. The core, the capabilities and
the audit trail are exactly A1's; only the *presentation proxy* that consumes the
node's intent stream differs. That is the archetype the founding requirements
name (§3.1, *devices requiring integration with accessible external UIs*) and
this book had not carried. A4 joins A1–A3; it does not displace them.

What A4 is **today**, without rounding up: the presentation is text on a serial
console (`fjell-proxy-text`), and nothing has been tested with assistive
technology; speech and braille presentations do not exist (a second presentation
is proposed for the next milestone); the operator cannot answer the node through
any presentation;
and **if the presentation is unavailable, the service that was publishing waits
— measured, and a defect against the design, whose intent is that the node keeps
operating and the operator loses only the view.** The audit trail is produced by
the core, not the presentation. The list of what a person needing a presentation
cannot do today is in [what does not exist yet](../releasing/v1-limitations.md#accessibility-and-inclusion--what-does-not-exist-yet).

## What that buys you concretely

- A compromise of one service is bounded by its capability space, not by
  whatever "the same user" can touch.
- An update either verifies (signature, anti-rollback, boot-control health)
  or the node falls back to the proven-good mirror — and either way the
  decision is recorded.
- A revoked authority is dead immediately (epoch-based lease revocation,
  formally proved), not whenever a daemon next rereads a config file.
- An audit is a read of typed records, not an archaeology project across
  unstructured logs.

## When not to use Fjell

Fjell is for new services on dedicated nodes, not ported workloads. If you
need POSIX software, containers, a desktop environment or a GUI stack, or hard
real-time guarantees, run those on an adjacent system — see
[v1.0 Non-Goals](../releasing/v1-non-goals.md) for the full list with rationale
and operator alternatives. Not needing a screen is not the same as not being for
people: that is what A4 is for, and what it cannot yet do is stated beside it.
