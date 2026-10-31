//! Tests the reader on the archives of a real dump.
//!
//! The repository holds no game data. Set `DLGE_ARCHIVES` to the `Archive`
//! directory of a RomFS dump to run this test. Without the variable, the
//! test does nothing.

use std::path::PathBuf;

use bea::Archive;

#[test]
fn every_file_of_every_archive_reads() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let mut archives = 0;
    let mut files = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read DLGE_ARCHIVES") {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "bea") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        archives += 1;
        match Archive::parse(&bytes) {
            Ok(archive) => {
                for e in &archive.entries {
                    files += 1;
                    if let Err(err) = archive.read(e) {
                        failures.push(format!("{} {}: {err}", path.display(), e.name));
                    }
                }
            }
            Err(err) => failures.push(format!("{}: {err}", path.display())),
        }
    }
    assert!(archives > 0, "no .bea files in {}", dir.display());
    assert!(
        failures.is_empty(),
        "{} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("{archives} archives, {files} files read");
}

#[test]
fn every_table_reads() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let mut tables = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read DLGE_ARCHIVES") {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "bea") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        for e in &archive.entries {
            let data = archive.read(e).unwrap();
            if !data.starts_with(bea::table::MAGIC) {
                continue;
            }
            let table = bea::table::Table::parse(&data)
                .unwrap_or_else(|err| panic!("{} {}: {err}", path.display(), e.name));
            tables.push(format!(
                "{} {} rows {}",
                e.name,
                table.rows.len(),
                table.columns.len()
            ));
        }
    }
    assert!(!tables.is_empty(), "no tables in {}", dir.display());
    tables.sort();
    eprintln!("{} tables read:\n{}", tables.len(), tables.join("\n"));
}
