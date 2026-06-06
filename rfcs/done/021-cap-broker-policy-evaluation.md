# RFC 021: cap-broker real policy evaluation

**RFC ID:** 021  
**Status:** Implemented (v0.1.0)
**Affects:** `crates/fjell-cap-broker/src/main.rs`

## Problem (H-05)

`cap-broker::evaluate()` checks only whether the requester tag's upper byte is 0xFF.
`POLICY` array is defined but not evaluated.

## Proposed fix

Implement a real policy evaluator:

```rust
fn evaluate(requester: ServiceTag, resource: ResourceClass, requested_rights: CapRights) -> PolicyResult {
    // 1. explicit deny takes priority
    for rule in POLICY.iter() {
        if rule.requester == requester && rule.resource == resource
           && rule.kind == PolicyKind::Deny {
            return PolicyResult::Denied;
        }
    }
    // 2. explicit allow
    for rule in POLICY.iter() {
        if rule.requester == requester && rule.resource == resource
           && rule.kind == PolicyKind::Allow {
            let granted = rule.rights & requested_rights;
            if granted.0 != 0 { return PolicyResult::Granted(granted); }
        }
    }
    // 3. default deny
    PolicyResult::Denied
}
```

Granted capabilities must be lease-bound (create lease, attach to cap).

## Defer condition

Requires RFC 019 (service separation via try_recv) so cap-broker can receive requests
from real service processes.
