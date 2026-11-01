//! Checks the English patch file that the repository ships.

use importer::patch::PatchSet;

#[test]
fn shipped_patch_file_reads() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/patch/en.dlpatch");
    let set = PatchSet::parse(&std::fs::read(path).unwrap()).unwrap();
    let scripts = set
        .deltas
        .keys()
        .filter(|k| k.starts_with("script.cpk/"))
        .count();
    assert!(scripts > 0, "the patch has no script");
    for (key, delta) in &set.deltas {
        assert!(
            key.starts_with("script.cpk/") || key.starts_with("Archive/"),
            "unexpected key {key}"
        );
        assert!(delta.starts_with(b"DLD1"), "{key} is not a delta");
    }
}
