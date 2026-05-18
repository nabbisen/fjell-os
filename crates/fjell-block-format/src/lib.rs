//! Block device protocol types for Fjell OS M6.
#![no_std]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockError { Unsupported, OutOfRange, ReadOnly, Io, Timeout, QueueFull }

#[derive(Clone, Copy, Debug)]
pub struct BlockDeviceInfo {
    pub sector_size:      u32,
    pub sector_count:     u64,
    pub readonly:         bool,
    pub flush_supported:  bool,
}
