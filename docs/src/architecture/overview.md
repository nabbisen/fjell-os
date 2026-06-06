# Architecture Overview

*TODO: Detailed treatment. References RFC 011 (service separation), RFC 038.*

Fjell is a capability-based microkernel. The kernel handles scheduling, IPC, and capability enforcement. All device drivers and system services run as user-mode tasks.

```text
kernel (fjell-kernel)
  ├─ init (spawn services from image table)
  ├─ cap-broker (capability grant policy)
  ├─ service-manager (service lifecycle)
  └─ IPC switchboard (kernel-attested sender identity)

services (user mode)
  ├─ bootctl, storaged, upgraded, diagnosticsd
  ├─ netd, secure-transportd, driver-virtio-net
  ├─ syncd, identityd, summaryd
  └─ fleetd, devmgr
```
