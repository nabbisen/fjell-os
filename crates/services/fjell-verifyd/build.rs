fn main() {
    let target = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target == "riscv64" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo:rustc-link-arg=-T{dir}/link.ld");
    }
    println!("cargo:rerun-if-changed=link.ld");

    // RFC-v0.17-001 (Accepted): dev-profile trust-anchor provisioning.
    // If `provision/dev-trust-anchor.key` exists at the workspace root
    // (written only by `cargo xtask provision-dev --allow-tofu-provision`),
    // embed it as the dev anchor. Otherwise embed the legacy all-zero dev
    // key and mark the build UNPROVISIONED — verifyd logs this loudly at
    // startup. Silent default TOFU is prohibited; the unprovisioned state
    // is explicit and visible.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let key_path = std::path::Path::new(&manifest).join("../../provision/dev-trust-anchor.key");
    println!("cargo:rerun-if-changed={}", key_path.display());

    let (key_bytes, provisioned): ([u8; 32], bool) = match std::fs::read_to_string(&key_path) {
        Ok(hex) => {
            let hex = hex.trim();
            let mut k = [0u8; 32];
            let ok = hex.len() == 64
                && (0..32).all(|i| {
                    u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                        .map(|b| {
                            k[i] = b;
                            true
                        })
                        .unwrap_or(false)
                });
            if !ok {
                panic!(
                    "provision/dev-trust-anchor.key exists but is not 64 hex chars — \
                        refusing to build with a malformed anchor"
                );
            }
            (k, true)
        }
        Err(_) => ([0u8; 32], false),
    };

    let out = std::env::var("OUT_DIR").unwrap();
    let dest = std::path::Path::new(&out).join("dev_anchor.rs");
    let body = format!(
        "pub const DEV_ANCHOR_KEY: [u8; 32] = {key_bytes:?};\n\
         pub const DEV_ANCHOR_PROVISIONED: bool = {provisioned};\n"
    );
    std::fs::write(dest, body).unwrap();
}
