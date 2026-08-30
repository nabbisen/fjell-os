#![no_std]
// docs.rs and `cargo doc` resolve neither relative paths nor the repository
// root, so the sidebar logo has to be an absolute URL.
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/nabbisen/fjell-os/main/assets/logo-256.png",
    html_favicon_url = "https://raw.githubusercontent.com/nabbisen/fjell-os/main/assets/favicon.svg"
)]
#![doc = include_str!("../README.md")]

/// The stable ABI surface, re-exported from [`fjell_abi`].
///
/// This is the only Fjell crate published so far beyond this one. It carries
/// the syscall numbers, capability kinds and rights, service image identifiers,
/// and boot-control types shared between the kernel and every user-space
/// service.
pub use fjell_abi as abi;
