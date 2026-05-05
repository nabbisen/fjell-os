# Design Philosophy

Fjell OS is guided by six principles that apply at every layer of the system.

## Small core

The kernel does exactly five things: address-space isolation, task management,
IPC, capability enforcement, and interrupt handling.  Nothing else belongs
inside the trust boundary.  Every kilobyte added to the kernel is a kilobyte
that must be formally reasoned about.

## Explicit boundary

There is no ambient authority.  A service cannot read a file, send an IPC
message, or access a device register unless it holds a capability that
explicitly grants that right.  The source and chain of every capability is
auditable.

## No ambient authority

Root does not exist.  Privilege escalation through a SUID binary, a kernel
module, or an LD_PRELOAD is structurally impossible — there is no privilege
level to escalate to.  Authority flows only downward through the capability
derivation tree.

## Readable state

Every significant kernel transition — VM mapping, task switch, syscall,
capability delegation — is recorded in an append-only audit ring.  The ring
can be flushed to JSON Lines and read by any tool without specialist knowledge.

## Semantic interface (ABDD)

Applications emit structured *intent* — a description of what they want the
user to understand or decide — rather than pixel coordinates.  A Presentation
Proxy translates that intent into whatever modality the user needs: terminal
text, synthesized speech, braille, or machine-readable JSON.  The OS never
hard-codes presentation logic.

## Recoverable system

Updates are atomic A/B image swaps.  Configurations are declarative TOML
checked before application.  A failed update or bad configuration never
produces a unbootable system; the previous state is always reachable.

---

## What Fjell OS deliberately is not

| Temptation | Why we refuse it |
|---|---|
| Monolithic kernel with "fast paths" | Every line in the kernel expands the verification surface |
| POSIX compatibility shim | Compatibility constraints would require ambient authority |
| GUI stack in the kernel | Pixels are not semantics; rendering belongs in user space |
| AI-native kernel | AI is a proxy concern, not a scheduling primitive |
| Root-based privilege | Root is ambient authority by another name |
