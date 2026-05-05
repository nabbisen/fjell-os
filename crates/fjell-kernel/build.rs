/// Build script for the fjell-kernel crate.
///
/// Passes the linker script to `rust-lld` using an absolute path derived from
/// `CARGO_MANIFEST_DIR`.  This is the only reliable way to supply `-T link.ld`
/// regardless of where `cargo build` is invoked from (workspace root, crate
/// directory, or CI).
///
/// Using `-C link-arg=` in `.cargo/config.toml` rustflags is *not* sufficient
/// when cargo is invoked from the workspace root, because subdirectory
/// `.cargo/config.toml` files are not loaded in that case.
fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");

    let linker_script = format!("{manifest_dir}/link.ld");

    // Re-run this build script if the linker script changes.
    println!("cargo:rerun-if-changed={linker_script}");

    // Pass the linker script to the linker.
    // `rustc-link-arg` is equivalent to `-C link-arg=` but uses an absolute
    // path so it works from any working directory.
    println!("cargo:rustc-link-arg=-T{linker_script}");
}
