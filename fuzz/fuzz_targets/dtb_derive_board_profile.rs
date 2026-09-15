#![no_main]
use fjell_dtb_derive::{derive_board_profile, DeriveContext};
use fjell_platform_format::PlatformProfile;
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let ctx = DeriveContext::qemu_virt_default(PlatformProfile::qemu_virt_default());
    let _ = derive_board_profile(data, &ctx, b"fuzz-board\0\0\0\0\0\0", b"rev1\0\0\0\0");
    // derive_board_profile stops at the first error; walk every token too.
    if let Ok(hdr) = fjell_dtb_derive::parser::parse_header(data) {
        for ev in fjell_dtb_derive::parser::FdtIter::new(data, &hdr) {
            if ev.is_err() {
                break;
            }
        }
    }
});
