//! The virtio-mmio discovery scan's **decision**, as a pure state machine (0.45 follow-up to
//! RFC-0.45-001, closing the N2b gap).
//!
//! The image maps each of the eight transports, reads two ID registers, and reports. *What it may
//! report* is this module's business and has no MMIO in it: a device is **found** only on the
//! `virtio` magic *and* `DeviceID == 1`; absence is reportable **only after every transport was
//! read**; anything else is **cannot scan**. The RFC-0.45-001 review found that this distinction
//! was demonstrated once and gated by no tier -- with the grant removed and the completeness check
//! dropped, `net-absent` stayed green while the driver claimed an absence it never verified (E-118
//! from the other side), and no committed profile can withhold a grant to show it. So the rule
//! lives here, on the host, where each clause is mutated.
//!
//! This is the one predicate the image takes from this crate. The ring/virtq/feature core (RFC
//! v0.4-001) is still **not wired into the image**; dead-code elimination keeps it out.

use crate::mmio::{VIRTIO_MMIO_MAGIC_VALUE, VIRTIO_NET_DEVICE_ID};

/// What one transport yielded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    /// The transport was mapped and both registers read.
    Read { magic: u32, device_id: u32 },
    /// It could not be mapped or read; the refusal's error code (`0` if the kernel returned a
    /// null mapping with no error).
    Refused(isize),
}

/// What the image may print -- exactly one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    /// A transport with the `virtio` magic and `DeviceID == 1`.
    Found { slot: usize, magic: u32 },
    /// **Every** transport was read and none is a network device.
    NoDevice { scanned: usize },
    /// The scan could not run, or could not finish: `read` of `of` transports were read.
    CannotScan {
        read: usize,
        of: usize,
        last_err: isize,
    },
}

/// A scan over `transports` virtio-mmio transports.
#[derive(Clone, Copy, Debug)]
pub struct Scan {
    transports: usize,
    read: usize,
    last_err: isize,
}

impl Scan {
    pub const fn new(transports: usize) -> Self {
        Scan {
            transports,
            read: 0,
            last_err: 0,
        }
    }

    /// Record what transport `slot` yielded. `Some(Found)` the moment one is a network device.
    pub fn observe(&mut self, slot: usize, o: Observation) -> Option<Report> {
        match o {
            Observation::Refused(e) => {
                self.last_err = e;
                None
            }
            Observation::Read { magic, device_id } => {
                self.read += 1;
                if magic == VIRTIO_MMIO_MAGIC_VALUE && device_id == VIRTIO_NET_DEVICE_ID {
                    Some(Report::Found { slot, magic })
                } else {
                    None
                }
            }
        }
    }

    /// The report once every transport has been tried and none was a network device.
    /// **Absence only if all of them were read.**
    pub fn finish(&self) -> Report {
        if self.read == self.transports {
            Report::NoDevice {
                scanned: self.transports,
            }
        } else {
            Report::CannotScan {
                read: self.read,
                of: self.transports,
                last_err: self.last_err,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: u32 = VIRTIO_NET_DEVICE_ID;
    const M: u32 = VIRTIO_MMIO_MAGIC_VALUE;
    const BLK: u32 = 2;

    fn read(magic: u32, device_id: u32) -> Observation {
        Observation::Read { magic, device_id }
    }

    /// Run a scan over `obs`, as the image does: stop at the first `Found`.
    fn run(obs: &[Observation]) -> Report {
        let mut s = Scan::new(obs.len());
        for (slot, o) in obs.iter().enumerate() {
            if let Some(r) = s.observe(slot, *o) {
                return r;
            }
        }
        s.finish()
    }

    #[test]
    fn a_device_is_found_at_the_slot_it_was_read() {
        let mut obs = [read(M, 0); 8];
        obs[6] = read(M, N);
        assert_eq!(run(&obs), Report::Found { slot: 6, magic: M });
    }

    #[test]
    fn found_needs_both_the_magic_and_the_device_id() {
        // The right id behind the wrong magic, and the right magic with another device: neither.
        let mut obs = [read(M, 0); 8];
        obs[3] = read(0xdead_beef, N);
        obs[4] = read(M, BLK);
        assert_eq!(run(&obs), Report::NoDevice { scanned: 8 });
    }

    #[test]
    fn absence_is_reported_only_after_every_transport_was_read() {
        assert_eq!(run(&[read(M, 0); 8]), Report::NoDevice { scanned: 8 });
    }

    #[test]
    fn a_scan_that_read_nothing_is_cannot_scan_never_absence() {
        // The grant is missing: every map is refused. This is the line that must not say
        // "no device found" (E-118 from the other side).
        let r = run(&[Observation::Refused(-12); 8]);
        assert_eq!(
            r,
            Report::CannotScan {
                read: 0,
                of: 8,
                last_err: -12
            }
        );
    }

    #[test]
    fn a_partial_scan_is_not_a_verified_absence_either() {
        let mut obs = [read(M, 0); 8];
        obs[5] = Observation::Refused(-3);
        assert_eq!(
            run(&obs),
            Report::CannotScan {
                read: 7,
                of: 8,
                last_err: -3
            }
        );
    }

    #[test]
    fn a_found_device_wins_even_if_other_transports_were_refused() {
        let mut obs = [Observation::Refused(-3); 8];
        obs[1] = read(M, N);
        assert_eq!(run(&obs), Report::Found { slot: 1, magic: M });
    }

    #[test]
    fn a_scan_of_zero_transports_has_read_everything_vacuously_and_that_is_the_callers_to_avoid() {
        // Pinned so nobody mistakes it for a property: `Scan::new(0)` reports "no device (0)".
        // The image passes the real transport count.
        assert_eq!(Scan::new(0).finish(), Report::NoDevice { scanned: 0 });
    }
}
