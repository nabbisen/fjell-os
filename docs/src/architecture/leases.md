# Leases

*References RFC 033 (epoch revocation), RFC 034 (blocked IPC).*

Every capability grant is bounded by a `LeaseEpoch`. When a lease is revoked, all capabilities tied to it are immediately invalidated — including any task blocked on IPC with a revoked endpoint.
