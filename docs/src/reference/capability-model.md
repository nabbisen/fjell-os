# Capability Model

> Implemented in **M3**.

A capability is an unforgeable kernel-managed token that grants specific
rights over a kernel object.  User space holds opaque `CapHandle` values
(slot index + generation counter); the kernel resolves them to internal
`Capability` structs.

## Object types (M3)

`Task` · `AddressSpace` · `Endpoint` · `Reply`

`Frame` is defined but user-visible mapping is deferred to a later milestone.

## Rights

`SEND` · `RECV` · `CALL` · `GRANT` · `MAP_R` · `MAP_W` · `MAP_X` · `INSPECT`

Child capabilities may only have rights that are a subset of their parent's.

## Operations

| Syscall | Effect |
|---|---|
| `cap_copy` | Duplicate a slot |
| `cap_mint` | Copy with attenuated rights or added badge |
| `cap_delete` | Remove this slot only |
| `cap_revoke` | Remove all descendants; keep target |

See [ADR-0003](../adr/0003-capability-security.md) for design rationale.
