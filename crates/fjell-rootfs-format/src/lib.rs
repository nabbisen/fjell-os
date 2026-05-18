//! Immutable rootfs types for Fjell OS M7.
#![no_std]

/// A reference to a service image within the rootfs.
#[derive(Clone, Copy)]
pub struct ServiceImageRef {
    pub name:   [u8; 32],
    pub digest: [u8; 32],
    pub size:   u32,
}

impl ServiceImageRef {
    pub const fn named(name: &[u8]) -> Self {
        let mut n = [0u8; 32];
        let mut i = 0;
        while i < name.len() && i < 32 { n[i] = name[i]; i += 1; }
        ServiceImageRef { name: n, digest: [0u8; 32], size: 0 }
    }
}

/// Rootfs namespace — a flat list of service images.
pub struct RootfsNamespace {
    images:  [ServiceImageRef; 16],
    count:   usize,
}

impl RootfsNamespace {
    pub const fn empty() -> Self {
        RootfsNamespace { images: [ServiceImageRef::named(b""); 16], count: 0 }
    }
    pub fn add(&mut self, img: ServiceImageRef) {
        if self.count < 16 { self.images[self.count] = img; self.count += 1; }
    }
    pub fn count(&self) -> usize { self.count }
}

/// Status of the immutable rootfs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootfsStatus { NotLoaded, Verified, Degraded }
