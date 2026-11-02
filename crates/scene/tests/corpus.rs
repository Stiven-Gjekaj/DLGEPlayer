//! Plays every chapter of a real import folder through the director.
//!
//! The repository holds no game data. Set `DLGE_IMPORT` to a folder that
//! `dlplayer import` wrote. Without the variable, the test does nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use scene::{Assets, Director, Event, Profile};

#[test]
fn every_chapter_plays_to_the_end() {
    let Some(dir) = std::env::var_os("DLGE_IMPORT").map(PathBuf::from) else {
        eprintln!("DLGE_IMPORT is not set. The corpus test does nothing.");
        return;
    };
    let program = vm::Program::load(&dir).unwrap();
    let assets = Assets::load(&dir).unwrap();
    let names: Vec<String> = program.names().map(str::to_string).collect();
    let mut failed = Vec::new();
    let mut missing = BTreeSet::new();
    let mut other: BTreeMap<u32, usize> = BTreeMap::new();
    let mut texts = 0;
    let mut monologues = 0;
    let mut scored = BTreeSet::new();
    for name in &names {
        let mut director = Director::new(&program, &assets, Profile::default(), name).unwrap();
        let mut events = 0;
        let result = loop {
            events += 1;
            if events > 200_000 {
                break Err("too many events".to_string());
            }
            match director.next_event() {
                Ok(Event::End { .. }) => break Ok(()),
                Ok(Event::Choice { .. }) => director.choose(0).unwrap(),
                Ok(Event::SelectCharacter) => director.choose_character(1).unwrap(),
                Ok(Event::Monologue { picks, .. }) => {
                    monologues += 1;
                    for pick in 0..picks {
                        director.choose_keyword(pick).unwrap();
                    }
                }
                Ok(Event::Text { .. }) => texts += 1,
                Ok(Event::Other { opcode, .. }) => *other.entry(opcode).or_default() += 1,
                Ok(_) => {}
                Err(e) => break Err(e.to_string()),
            }
        };
        if let Err(e) = result {
            failed.push(format!("{name}: {e}"));
        }
        missing.extend(director.missing());
        scored.extend(director.profile.monologue.keys().cloned());
    }
    eprintln!(
        "{} chapters, {texts} messages, {monologues} keyword picks in {} chapters",
        names.len(),
        scored.len()
    );
    eprintln!("opcodes with no event: {other:x?}");
    eprintln!("{} names with no file: {missing:?}", missing.len());
    assert!(
        failed.is_empty(),
        "{} chapters failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
    assert!(missing.is_empty());
}
