# Non-Goals

Fjell explicitly does **not** target the following before v1.0:

- POSIX compatibility
- Desktop GUI or web browser hosting
- Package manager with dependency resolution
- General-purpose remote shell
- Container orchestration substrate (Kubernetes)
- Hard real-time scheduling guarantees

None of these excludes a person. They are about the OS core hosting a desktop, a
browser, a package ecosystem or a shell — not about who may operate a node:
presentation is a proxy's job, and a node operated through an assistive
presentation is [an archetype](why-fjell.md), with its limits stated.

See [v1.0 Non-Goals](../releasing/v1-non-goals.md) for the full list with rationale.

*References RFC-v0.15-005 and RFC 061 §3.4.*
