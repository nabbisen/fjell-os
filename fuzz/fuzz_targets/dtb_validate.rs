#![no_main]
use fjell_dtb_validate::validate_dtb;
use fjell_platform_format::{platform_digest, BoardProfile, PlatformProfile};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let pp = PlatformProfile::qemu_virt_default();
    let bp = BoardProfile::qemu_virt_default(platform_digest(&pp));
    let _ = validate_dtb(data, &bp);
});
