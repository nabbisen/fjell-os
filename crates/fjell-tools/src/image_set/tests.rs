//! The host recomputation, against vectors made with Python's `hashlib` (the
//! same ones `fjell-measure-format`'s tests pin) and against the real tree.

use super::*;
use std::path::PathBuf;

/// A scratch tree with a `service.rs` of `ImageId` constants and some `.bin`s.
struct Tree(PathBuf);

impl Tree {
    fn new(tag: &str, abi: &str, bins: &[(&str, &[u8])]) -> Self {
        let root =
            std::env::temp_dir().join(format!("fjell-image-set-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(PREBUILT_DIR)).unwrap();
        fs::create_dir_all(root.join(SERVICE_ABI).parent().unwrap()).unwrap();
        fs::write(root.join(SERVICE_ABI), abi).unwrap();
        for (name, bytes) in bins {
            fs::write(root.join(PREBUILT_DIR).join(name), bytes).unwrap();
        }
        Tree(root)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const ABI: &str = "
    pub const ALPHA: ImageId = ImageId(0);
    pub const BETA: ImageId = ImageId(1);
    pub const GAMMA_RAY: ImageId = ImageId(0x5);
    // not an image constant:
    pub const OTHER: u16 = 9;
";

#[test]
fn matches_the_independent_vector() {
    let t = Tree::new(
        "kat",
        ABI,
        &[
            // written out of id order on purpose: the digest is by id, not by
            // directory order
            ("fjell-gamma-ray.bin", b"gamma"),
            ("fjell-alpha.bin", b"alpha"),
            ("fjell-beta.bin", b"beta"),
        ],
    );
    assert_eq!(
        image_set_digest(&t.0).unwrap(),
        (
            3,
            "5ca1c2f9b3322797c51f9beebd987d5e1fc9ebef90e9c1ce6243d5c1b116705f".to_string()
        )
    );
}

#[test]
fn one_changed_byte_changes_the_digest() {
    let a = Tree::new("a", ABI, &[("fjell-alpha.bin", b"alpha")]);
    let b = Tree::new("b", ABI, &[("fjell-alpha.bin", b"alphb")]);
    assert_ne!(
        image_set_digest(&a.0).unwrap().1,
        image_set_digest(&b.0).unwrap().1
    );
    assert_eq!(
        image_set_digest(&a.0).unwrap().1,
        "d6038da7f295a9d71b7dcdbe881f8efa9d8a86bf4a30d7ce12f2777cb0c063eb"
    );
}

#[test]
fn a_stray_binary_is_refused_not_skipped() {
    let t = Tree::new(
        "stray",
        ABI,
        &[("fjell-alpha.bin", b"a"), ("fjell-nope.bin", b"b")],
    );
    let e = image_set_digest(&t.0).unwrap_err();
    assert!(e.contains("fjell-nope.bin") && e.contains("NOPE"), "{e}");
}

#[test]
fn a_misnamed_file_and_an_empty_dir_are_refused() {
    let t = Tree::new("misname", ABI, &[("alpha.bin", b"a")]);
    assert!(image_set_digest(&t.0).unwrap_err().contains("alpha.bin"));
    let t = Tree::new("empty", ABI, &[]);
    assert!(
        image_set_digest(&t.0)
            .unwrap_err()
            .contains("no .bin images")
    );
    let t = Tree::new("noabi", "// nothing here", &[("fjell-alpha.bin", b"a")]);
    assert!(
        image_set_digest(&t.0)
            .unwrap_err()
            .contains("no `ImageId` constants")
    );
}

#[test]
fn two_files_for_one_id_are_refused() {
    // `ALPHA` twice under different spellings of the same id cannot happen by
    // file name, so give two constants the same number.
    let abi = "pub const ONE: ImageId = ImageId(3);\npub const TWO: ImageId = ImageId(3);\n";
    let t = Tree::new(
        "dup",
        abi,
        &[("fjell-one.bin", b"a"), ("fjell-two.bin", b"b")],
    );
    assert!(
        image_set_digest(&t.0)
            .unwrap_err()
            .contains("both resolve to image id 3")
    );
}

#[test]
fn const_names_follow_the_file_names() {
    assert_eq!(
        const_name_for("fjell-driver-uart.bin").as_deref(),
        Some("DRIVER_UART")
    );
    assert_eq!(const_name_for("fjell-init.bin").as_deref(), Some("INIT"));
    assert_eq!(const_name_for("init.bin"), None);
    assert_eq!(const_name_for("fjell-init.elf"), None);
}

/// The real tree: this host recomputation and `fjell-measure-format` — the code
/// the kernel runs — agree on the committed prebuilt images. Two independent
/// implementations of one documented layout, over real bytes, in `cargo test`.
#[test]
fn agrees_with_the_in_tree_implementation_on_the_real_prebuilts() {
    use fjell_measure_format::{Digest32, ImageEntry, compute_image_set_digest};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (count, host) = image_set_digest(&root).expect("host recomputation");

    let abi = fs::read_to_string(root.join(SERVICE_ABI)).unwrap();
    let ids = image_id_constants(&abi);
    let mut entries: Vec<ImageEntry> = Vec::new();
    for e in fs::read_dir(root.join(PREBUILT_DIR)).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(c) = const_name_for(&name) {
            entries.push(ImageEntry {
                image_id: ids[&c],
                digest: Digest32::of(&fs::read(e.path()).unwrap()),
            });
        }
    }
    entries.sort_by_key(|e| e.image_id);
    assert_eq!(entries.len(), count);
    let mut buf = [0u8; 64];
    let in_tree = compute_image_set_digest(&entries).unwrap();
    assert_eq!(in_tree.to_hex(&mut buf), host);
}
