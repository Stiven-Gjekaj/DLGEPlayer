//! Tests the parser on the scripts of a real dump.
//!
//! The repository holds no game data. Set `DLGE_SCRIPTS` to a directory of
//! decompressed `.dat` files to run this test. Without the variable, the
//! test does nothing.

use std::path::PathBuf;

use stcm2l::Script;

#[test]
fn every_script_round_trips() {
    let Some(dir) = std::env::var_os("DLGE_SCRIPTS").map(PathBuf::from) else {
        eprintln!("DLGE_SCRIPTS is not set. The corpus test does nothing.");
        return;
    };
    let mut count = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read DLGE_SCRIPTS") {
        let path = entry.unwrap().path();
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dat"))
        {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        count += 1;
        match Script::parse(&bytes) {
            Ok(script) if script.to_bytes() == bytes => {}
            Ok(_) => failures.push(format!("{}: output differs", path.display())),
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    assert!(count > 0, "no .dat files in {}", dir.display());
    assert!(
        failures.is_empty(),
        "{} of {count} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("{count} scripts round-trip");
}
