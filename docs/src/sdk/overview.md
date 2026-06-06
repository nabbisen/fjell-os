# SDK Overview

`fjell-sdk` is the curated stable surface for service authors. It re-exports capability, IPC, syscall, and semantic intent APIs with explicit stability tiers.

```toml
[dependencies]
fjell-sdk = { path = "../crates/fjell-sdk" }
```

Start with the [prelude](../sdk/writing-a-service.md): `use fjell_sdk::prelude::*;`

*References RFC v0.9-001.*
