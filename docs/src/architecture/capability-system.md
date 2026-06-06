# Capability System

*References RFC 031 (require_cap), RFC 048 (handle-based), RFC 049 (rights).*

Authority is held as opaque handles in a per-task `CSpace`. Every syscall that accesses a resource requires a handle with matching kind and rights.

No ambient authority. No file descriptors. No `setuid`.
