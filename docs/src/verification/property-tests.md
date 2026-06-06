# Property Tests

10 proptest properties × 1000 cases each.

Run: `cargo test -p fjell-proptest --release`

Covers: capability CSpace round-trips, lease epoch invariants, boot-control state machine, store recovery, semantic schema compatibility.

*References RFC v0.6-001, RFC v0.6-002.*
