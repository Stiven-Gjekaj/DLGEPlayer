//! Decodes every texture of a real dump.
//!
//! The repository holds no game data. Set `DLGE_ARCHIVES` to the `Archive`
//! directory of a RomFS dump to run this test. Without the variable, the
//! test does nothing.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn every_texture_decodes() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let mut decoded = 0;
    let mut unsupported: BTreeMap<u32, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read DLGE_ARCHIVES") {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "bea") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let archive = bea::Archive::parse(&bytes).unwrap();
        for e in &archive.entries {
            let data = archive.read(e).unwrap();
            if !data.starts_with(b"BNTX") {
                continue;
            }
            for t in bntx::parse(&data).unwrap() {
                if t.format().is_none() {
                    *unsupported.entry(t.format_code).or_default() += 1;
                    continue;
                }
                match t.to_rgba8() {
                    Ok(rgba) if rgba.len() == (t.width * t.height * 4) as usize => decoded += 1,
                    Ok(_) => failures.push(format!("{} {}: wrong size", path.display(), t.name)),
                    Err(err) => failures.push(format!("{} {}: {err}", path.display(), t.name)),
                }
            }
        }
    }
    assert!(decoded > 0, "no textures in {}", dir.display());
    assert!(
        failures.is_empty(),
        "{} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("{decoded} textures decode; unsupported formats: {unsupported:x?}");
}

#[test]
fn every_layout_texture_decodes() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let mut decoded = 0;
    let mut formats: BTreeMap<u32, usize> = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("read DLGE_ARCHIVES") {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "bea") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let archive = bea::Archive::parse(&bytes).unwrap();
        for e in &archive.entries {
            let data = archive.read(e).unwrap();
            if !data.starts_with(b"SARC") {
                continue;
            }
            for f in sarc::parse(&data).unwrap() {
                if !f.data.starts_with(b"BNTX") {
                    continue;
                }
                for t in bntx::parse(f.data).unwrap() {
                    let rgba = t
                        .to_rgba8()
                        .unwrap_or_else(|err| panic!("{} {}: {err}", path.display(), t.name));
                    assert_eq!(rgba.len(), (t.width * t.height * 4) as usize, "{}", t.name);
                    *formats.entry(t.format_code).or_default() += 1;
                    decoded += 1;
                }
            }
        }
    }
    assert!(decoded > 0, "no layout textures in {}", dir.display());
    eprintln!("{decoded} layout textures decode: {formats:x?}");
}
