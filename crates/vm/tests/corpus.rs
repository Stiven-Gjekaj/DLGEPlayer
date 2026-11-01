//! Runs every chapter of a real import folder.
//!
//! The repository holds no game data. Set `DLGE_IMPORT` to a folder that
//! `dlplayer import` wrote. Without the variable, the test does nothing.

use std::path::PathBuf;

#[test]
fn every_chapter_and_route_ends() {
    let Some(dir) = std::env::var_os("DLGE_IMPORT").map(PathBuf::from) else {
        eprintln!("DLGE_IMPORT is not set. The corpus test does nothing.");
        return;
    };
    let program = vm::Program::load(&dir).unwrap();
    let report = vm::runner::run_all(&program, &|_| {});
    let failed: Vec<String> = report
        .runs
        .iter()
        .filter_map(|r| {
            r.result
                .as_ref()
                .err()
                .map(|e| format!("{} ({}): {e}", r.script, r.mode))
        })
        .collect();
    assert!(report.chapters > 0);
    assert!(
        failed.is_empty(),
        "{} runs failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
    let routes: Vec<&String> = report
        .routes
        .iter()
        .filter_map(|r| r.error.as_ref())
        .collect();
    assert!(routes.is_empty(), "routes failed: {routes:?}");
    // Each choice of select.bin that a script shows is reached.
    assert_eq!(report.choices_seen, program.choices.len());
    eprintln!(
        "{} chapters, {} routes, {} of {} reachable text lines",
        report.chapters,
        report.routes.len(),
        report.total.text_visited,
        report.total.text_reachable
    );
}
