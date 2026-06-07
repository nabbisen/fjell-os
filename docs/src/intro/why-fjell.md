# Why Fjell?

Fjell exists for operators who must answer the question *"prove what this
device did, and prove who authorised it"* — and cannot, on a conventional
OS, because authority there is ambient (root, file permissions, network by
default) and evidence is incidental (logs that may or may not exist).

## The three archetypes Fjell is designed around

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
need POSIX software, containers, a GUI, or hard real-time guarantees, run
those on an adjacent system — see [v1.0 Non-Goals](../release/v1-non-goals.md)
for the full list with rationale and operator alternatives.
