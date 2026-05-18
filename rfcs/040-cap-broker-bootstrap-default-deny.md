# RFC 040: cap-broker bootstrap handoff and default-deny policy engine

**RFC ID:** 040  
**Also known as:** RFC-v0.2-010  
**Status:** Proposed  
**Target version:** v0.2.0  
**Phase:** Phase 7 — cap-broker Bootstrap and Policy Enforcement  
**Related epics:** F (cap-broker Policy Closure), A (Unified Capability Enforcement)

## Problem

A default-deny policy without an explicit bootstrap path creates a
**bootstrap paradox**: if every grant is denied by default, who
grants the first capability?

At v0.1.0, `fjell-cap-broker` exists but does not enforce
default-deny.  Switching to default-deny without an explicit
bootstrap handoff would brick the boot path.

## Proposed fix

### One-way state machine

```rust
pub enum CapBrokerState {
    Bootstrap,
    Enforcing,
}
```

Transition is `Bootstrap → Enforcing` only.  No path returns to
`Bootstrap`.  This is enforced both at type level (no helper to
move back) and audited at every grant.

### Bootstrap handoff sequence

```
1. init receives bootstrap capabilities.
2. init starts configd and cap-broker.
3. init passes the immutable initial policy payload to cap-broker.
4. cap-broker validates the policy schema.
5. cap-broker receives a scoped LeaseAdmin / GrantAdmin bootstrap
   capability.
6. cap-broker enters Enforcing state.
7. cap-broker rejects further bootstrap policy mutation.
8. init drops or revokes the bootstrap authority.
```

### Initial policy payload

The payload is a **statically verified, immutable** policy:

- digest-bound (SHA-256 over the canonical CBOR encoding),
- signed by the platform key (development-grade Ed25519 stand-in
  per RFC 012),
- delivered as a single IPC message from init to cap-broker.

### PolicyRule shape

```rust
pub struct PolicyRule {
    pub requester:     ServiceTag,
    pub resource:      ResourceClass,
    pub resource_name: ResourceName,
    pub rights:        CapRights,
    pub kind:          PolicyKind,
}

pub enum PolicyKind { Allow, Deny }
```

### Evaluation order (normative)

```
1. explicit deny
2. explicit allow
3. default deny
```

Explicit deny precedes explicit allow so a narrow deny can override
a broad allow without rule reordering.

### Lease-bound grants

Every grant in Enforcing state must include a `LeaseBinding` (RFC
033 §2.6).  Unleased grants are bootstrap-only.

### Recursive policy revoke

Implemented entirely in cap-broker.  Walks the
`DelegationRecord` tree (RFC 033 §2.11), extracts affected
`lease_id`s, calls `sys_lease_revoke` for each.  The kernel still
sees only O(1) per-lease revoke calls.

### Rules

```
- Enforcing → Bootstrap is impossible
- runtime policy replacement requires explicit signed policy flow,
  not the bootstrap path
- default deny is enabled only after Enforcing
- cap-broker cannot grant outside the initial policy while in
  Bootstrap
```

## Rationale

The bootstrap-paradox is solved the same way OSes have solved it
for decades: a tiny privileged init that hands off authority, then
relinquishes its own.  Encoding this as a one-way state machine
makes the policy boundary auditable.

Explicit-deny-precedes-allow matches POSIX ACL semantics and is
the rule operators expect; reversing it would create surprising
holes when adding a new deny rule.

Recursive revoke in user space, not the kernel, preserves the
mechanism/policy split that is the design principle of Fjell OS.

## Impact

- Crates: `fjell-cap-broker` (most changes here), `fjell-init`
  (handoff sequence), `fjell-service-api` (handoff protocol),
  `fjell-service-manager` (waits for Enforcing before allowing
  service grants).
- Backward compatibility: **breaks** any code path that depended
  on cap-broker’s permissive v0.1.x behaviour.
- New audit kinds: `CapBrokerHandoff`, `PolicyGranted`,
  `PolicyDenied`, `PolicyDelegationRevoked`.

## Test plan

### Unit tests
- Bootstrap → Enforcing succeeds with valid payload.
- Enforcing → Bootstrap attempt rejected.
- Explicit deny overrides matching allow.
- Default deny rejects unknown requester.

### QEMU negative tests
- `NEG:POLICY:GRANT_BEFORE_BOOTSTRAP_REJECTED:PASS`
- `NEG:POLICY:RETURN_TO_BOOTSTRAP_REJECTED:PASS`
- `NEG:POLICY:UNKNOWN_REQUEST_DENIED:PASS`
- `NEG:POLICY:EXPLICIT_DENY_OVERRIDES_ALLOW:PASS`
- `NEG:POLICY:OVERBROAD_RIGHTS_DENIED:PASS`
- `NEG:POLICY:UNLEASED_GRANT_DENIED:PASS`
- `NEG:POLICY:REVOKED_LEASE_BOUND_GRANT_FAILS:PASS`

### Acceptance gates
- cap-broker no longer grants by tag shortcut.
- Bootstrap handoff is explicit.
- Default deny is active.
- Grants are lease-bound.

## Implementation notes

- Out of scope: networked policy push, multi-policy layering,
  user-side policy authoring tools.
- The state-machine type can be expressed via the typestate
  pattern: `CapBroker<Bootstrap>` vs `CapBroker<Enforcing>` with
  the `enforce()` consuming the value.  This makes
  Enforcing→Bootstrap impossible at compile time.
- The handoff message is single-shot; cap-broker rejects all
  subsequent payload-update attempts in Enforcing.
