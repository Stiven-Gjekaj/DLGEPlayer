//! Reads the layouts of a real dump.
//!
//! The repository holds no game data. Set `DLGE_ARCHIVES` to the `Archive`
//! directory of a RomFS dump to run this test. Without the variable, the
//! test does nothing.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn every_layout_reads() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let (mut archives, mut files) = (0, 0);
    let mut magics: BTreeMap<String, usize> = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("read DLGE_ARCHIVES") {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "bea") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let bea = bea::Archive::parse(&bytes).unwrap();
        for e in &bea.entries {
            let data = bea.read(e).unwrap();
            if !data.starts_with(b"SARC") {
                continue;
            }
            archives += 1;
            for f in sarc::parse(&data).unwrap() {
                // The stored hash must agree with the name.
                assert_eq!(f.hash, sarc::name_hash(&f.name, 0x65), "{}", f.name);
                let magic = String::from_utf8_lossy(&f.data[..4.min(f.data.len())]).into_owned();
                *magics.entry(magic).or_default() += 1;
                files += 1;
            }
        }
    }
    assert!(archives > 0, "no SARC files in {}", dir.display());
    eprintln!("{archives} layouts, {files} files: {magics:?}");
}
