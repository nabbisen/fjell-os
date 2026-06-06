# Writing a Service

*TODO: Full walkthrough. References RFC-v0.14-002 (first external service).*

Skeleton:

```rust
#![no_std]
#![no_main]
use fjell_sdk::prelude::*;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop { let _ = fjell_sdk::syscall::sys_yield(); }
}
```

Add a `cap-manifest.toml` at the crate root — see [Capability Manifests](./cap-manifest.md).
