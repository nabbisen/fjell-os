fn main() {
    let target = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target == "riscv64" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo:rustc-link-arg=-T{dir}/link.ld");
    }
    println!("cargo:rerun-if-changed=link.ld");
}
