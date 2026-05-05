# Audit Event Format

Defined in `crates/fjell-audit-format/src/lib.rs`.

Each `AuditEvent` carries: `seq` (monotonic), `tick`, optional `task` index, `kind`, `arg0`, `arg1`, `result`.

The kernel ring is append-only.  When full, `dropped_count` is incremented rather than overwriting existing records.