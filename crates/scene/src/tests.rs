//! Plays a small chapter from an import folder that the test writes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::json;
use stcm2l::decode::{Compare, Instruction as I, Value as V};
use vm::{ChoiceOption, Program, ScriptIr};

use crate::director::Event;
use crate::{Assets, Director, Profile, menu};

fn engine(opcode: u32, args: Vec<V>) -> I {
    I::Engine { opcode, args }
}

fn text(t: &str) -> V {
    V::Text(t.into())
}

fn line(t: &str) -> [I; 2] {
    [I::TextLine { text: t.into() }, I::ShowMessage]
}

/// Writes an import folder with one background, one event image, one
/// sprite, one music track, and one voice.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scene-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tables")).unwrap();
    let table = |file: &str, rows: serde_json::Value| {
        std::fs::write(dir.join(file), json!({ "rows": rows }).to_string()).unwrap();
    };
    table(
        "tables/speaker.json",
        json!([[0, "#Name[2]", "her"], [1, "Ayato", "aya"], [34, "", ""]]),
    );
    table(
        "tables/bg.json",
        json!([["bg01", "bg01", 101, 0, "Mansion"]]),
    );
    table(
        "tables/ev.json",
        json!([["cg01", "mb0010", 20010, 0, 0, 1]]),
    );
    let point = |x: u32, y: u32| json!({ "x": x, "y": y });
    let sprite = json!({
        "file": "sprites/01_aya_01_A.png",
        "name": "01_aya_01_A",
        "sheet": point(8, 4),
        "body": point(4, 4),
        "eye_size": point(2, 1),
        "eye_at": point(1, 1),
        "mouth_size": point(1, 1),
        "mouth_at": point(2, 3),
        "expressions": [{
            "name": "emi",
            "eyes": [point(4, 0), point(4, 1), point(4, 2)],
            "mouths": [point(6, 0), point(6, 1), point(6, 2)],
        }],
    });
    let track =
        |file: &str| json!({ "file": file, "sample_rate": 44100, "channels": 2, "frames": 10 });
    let manifest = json!({
        "format": 1,
        "scripts": {},
        "tables": {
            "speaker": "tables/speaker.json",
            "bg": "tables/bg.json",
            "ev": "tables/ev.json",
        },
        "images": {
            "bg": { "bg01": "images/bg/bg01.png" },
            "ev": { "mb0010": "images/ev/mb0010.png" },
        },
        "sprites": { "01_aya_01_A": sprite },
        "audio": {
            "bgm": { "BGM03": track("audio/bgm/BGM03.ogg") },
            "voice": { "mb_030063": track("audio/voice/mb_030063.ogg") },
        },
        "movies": {},
        "lip": "lip.json",
    });
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(
        dir.join("lip.json"),
        json!({ "030063": { "sample_rate": 44100, "keys": [[0, 1], [100, 0]] } }).to_string(),
    )
    .unwrap();
    dir
}

/// A chapter in the shape of the game. `sure1` shows a part for each value
/// of `switch`: a scene with a sprite and a voice, then a choice, then an
/// answer and the end.
fn program() -> Program {
    let g = V::Global(0);
    let mut code = vec![
        // 0
        engine(0x41d, vec![text("switch"), g.clone()]),
        I::IfFalseGoto {
            left: g.clone(),
            compare: Compare::Eq,
            right: V::Int(0),
            target: 14,
        },
        engine(
            0x3fa,
            vec![text("bg01"), V::Int(0), V::Int(0), V::Int(30), V::Int(0)],
        ),
        engine(0x456, vec![V::Int(3), V::Int(0)]),
        engine(
            0x47e,
            vec![
                text("aya_01"),
                text("aya_01_emi"),
                text("suma"),
                V::Int(-1),
                V::Int(-1),
                V::Int(-1),
                V::Int(1),
            ],
        ),
        engine(
            0x480,
            vec![
                text("aya_01"),
                V::Int(3),
                V::Int(1),
                V::Int(0),
                V::Int(6),
                V::Int(0),
            ],
        ),
        engine(0x4bc, vec![V::Int(1)]),
        I::Voice {
            id: V::Int(30063),
            rest: vec![V::Int(0)],
        },
    ];
    code.extend(line("Oi, #Name[2]."));
    code.extend([
        engine(0x41c, vec![text("switch"), V::Int(1)]),
        I::Return,
        I::Return,
        I::Return,
    ]);
    // 14
    assert_eq!(code.len(), 14);
    code.push(I::IfFalseGoto {
        left: g.clone(),
        compare: Compare::Eq,
        right: V::Int(1),
        target: 21,
    });
    code.push(engine(0x4bc, vec![V::Int(34)]));
    code.extend(line("Which?"));
    code.push(engine(0x599, vec![text("select"), text("q1")]));
    code.push(engine(0x41c, vec![text("switch"), V::Int(1)]));
    code.push(I::Return);
    // 21
    assert_eq!(code.len(), 21);
    code.push(engine(0x41d, vec![text("select"), g.clone()]));
    code.push(I::IfFalseGoto {
        left: g.clone(),
        compare: Compare::Eq,
        right: V::Int(0),
        target: 29,
    });
    code.push(engine(0x3fb, vec![text("cg01"), V::Float(3.0)]));
    code.extend(line("Answer A"));
    code.push(engine(0x425, vec![text("q_end"), V::Int(1)]));
    code.push(engine(0x700, vec![]));
    code.push(I::Return);
    // 29
    assert_eq!(code.len(), 29);
    code.extend(line("Answer B"));
    code.push(engine(0x700, vec![]));
    code.push(I::Return);

    let mut program = Program::empty();
    for name in ["mb_aya_Dark_00", "mb_aya_Dark_01"] {
        program.insert(ScriptIr {
            name: name.into(),
            exports: BTreeMap::from([("sure1".into(), 0)]),
            globals: BTreeMap::from([(0, V::Int(0))]),
            code: code.clone(),
        });
    }
    program.numbers.insert((1, 100), "mb_aya_Dark_00".into());
    program.numbers.insert((1, 101), "mb_aya_Dark_01".into());
    program.flags = BTreeMap::from([("switch".into(), 0), ("select".into(), 0)]);
    program.choices.insert(
        "q1".into(),
        vec![
            ChoiceOption {
                text: "Go with #Name[2]".into(),
                values: [0, 0, 0],
            },
            ChoiceOption {
                text: "Stay".into(),
                values: [0, 0, 0],
            },
        ],
    );
    program
}

fn kind(e: &Event) -> String {
    serde_json::to_value(e).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Gives events until a choice or the end.
fn until_stop(director: &mut Director) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        let e = director.next_event().unwrap();
        let stop = matches!(e, Event::Choice { .. } | Event::End { .. });
        events.push(e);
        if stop {
            return events;
        }
    }
}

fn texts(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Text { lines, .. } => Some(lines.join("/")),
            _ => None,
        })
        .collect()
}

#[test]
fn plays_a_chapter_as_events() {
    let dir = folder("play");
    let assets = Assets::load(&dir).unwrap();
    let program = program();
    let mut director =
        Director::new(&program, &assets, Profile::default(), "mb_aya_Dark_00").unwrap();

    let events = until_stop(&mut director);
    let kinds: Vec<String> = events.iter().map(kind).collect();
    assert_eq!(
        kinds,
        ["background", "music", "figure", "text", "text", "choice"]
    );
    match &events[0] {
        Event::Background {
            file,
            place,
            frames,
            ..
        } => {
            assert_eq!(file.as_deref(), Some("images/bg/bg01.png"));
            assert_eq!(place, "Mansion");
            assert_eq!(*frames, 30);
        }
        e => panic!("{e:?}"),
    }
    match &events[2] {
        Event::Figure {
            key, file, slot, ..
        } => {
            assert_eq!(key, "aya_01");
            assert_eq!(file, "sprites/01_aya_01_A.png");
            assert_eq!(*slot, 3);
        }
        e => panic!("{e:?}"),
    }
    match &events[3] {
        Event::Text {
            speaker,
            lines,
            voice,
            lip,
            read,
            ..
        } => {
            assert_eq!(speaker, "Ayato");
            assert_eq!(lines, &["Oi, Yui."]);
            assert_eq!(voice.as_deref(), Some("audio/voice/mb_030063.ogg"));
            assert_eq!(lip.as_ref().unwrap().keys, [(0, 1), (100, 0)]);
            assert!(!read);
        }
        e => panic!("{e:?}"),
    }
    match &events[5] {
        Event::Choice { options } => assert_eq!(options, &["Go with Yui", "Stay"]),
        e => panic!("{e:?}"),
    }
    // The director waits for the pick.
    assert!(director.next_event().is_err());
    director.choose(0).unwrap();

    let events = until_stop(&mut director);
    assert_eq!(texts(&events), ["Answer A"]);
    match events.last().unwrap() {
        Event::End { next } => assert_eq!(next.as_deref(), Some("mb_aya_Dark_01")),
        e => panic!("{e:?}"),
    }
    assert!(director.profile.finished.contains("mb_aya_Dark_00"));
    assert!(director.profile.seen.contains(&20010));
    assert_eq!(director.profile.system.get("q_end"), Some(&1));
    assert_eq!(director.backlog().len(), 3);
}

#[test]
fn continues_from_a_save() {
    let dir = folder("save");
    let assets = Assets::load(&dir).unwrap();
    let program = program();
    let mut director =
        Director::new(&program, &assets, Profile::default(), "mb_aya_Dark_00").unwrap();
    until_stop(&mut director);
    let text = serde_json::to_string(&director.save(7)).unwrap();
    director.choose(1).unwrap();
    let expected = texts(&until_stop(&mut director));
    assert_eq!(expected, ["Answer B"]);

    let save = serde_json::from_str(&text).unwrap();
    let (mut loaded, rebuild) =
        Director::load(&program, &assets, Profile::default(), save).unwrap();
    let kinds: Vec<String> = rebuild.iter().map(kind).collect();
    assert_eq!(
        kinds,
        [
            "background",
            "foreground",
            "figure",
            "camera",
            "music",
            "ambience_stop",
            "window"
        ]
    );
    // The load shows the choice again. The save keeps the text before it.
    let events = until_stop(&mut loaded);
    assert_eq!(events.iter().map(kind).collect::<Vec<_>>(), ["choice"]);
    let save: crate::SaveFile = serde_json::from_str(&text).unwrap();
    assert_eq!(save.text, ["Which?"]);
    loaded.choose(1).unwrap();
    assert_eq!(texts(&until_stop(&mut loaded)), expected);
}

#[test]
fn starts_with_the_background_of_the_chapter_before() {
    let dir = folder("before");
    let assets = Assets::load(&dir).unwrap();
    let program = program();
    let director = Director::new(&program, &assets, Profile::default(), "mb_aya_Dark_01").unwrap();
    // The chapter before ends on the event image of the first option.
    match &director.rebuild()[0] {
        Event::Background { name, file, .. } => {
            assert_eq!(name, "cg01");
            assert_eq!(file.as_deref(), Some("images/ev/mb0010.png"));
        }
        e => panic!("{e:?}"),
    }
    // The silent run does not mark messages or images.
    assert!(director.profile.read.is_empty());
    assert!(director.profile.seen.is_empty());
}

#[test]
fn marks_read_messages() {
    let dir = folder("read");
    let assets = Assets::load(&dir).unwrap();
    let program = program();
    let mut director =
        Director::new(&program, &assets, Profile::default(), "mb_aya_Dark_00").unwrap();
    until_stop(&mut director);
    let profile = director.profile.clone();
    let mut again = Director::new(&program, &assets, profile, "mb_aya_Dark_00").unwrap();
    let events = until_stop(&mut again);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::Text { read: false, .. }))
    );
}

#[test]
fn opens_chapters_in_order() {
    let program = program();
    let mut profile = Profile::default();
    let open = |profile: &Profile| -> Vec<bool> {
        menu::chapters(&program, profile, false)
            .iter()
            .map(|c| c.open)
            .collect()
    };
    assert_eq!(open(&profile), [true, false]);
    profile.finished.insert("mb_aya_Dark_00".into());
    assert_eq!(open(&profile), [true, true]);
    assert_eq!(
        menu::split("mb_aya_Dark_03"),
        Some(("mb", "aya", "Dark", 3))
    );
}

#[test]
fn removes_the_keyword_marks() {
    use crate::director::keywords;
    assert_eq!(
        keywords("I could#KW_T1[5]｡#KW_ED[]no longer go back."),
        "I could no longer go back."
    );
    assert_eq!(
        keywords("#KW_F[]先生#KW_ED[]が言うように"),
        "先生が言うように"
    );
    assert_eq!(
        keywords("今までのそれとは#KW_T2[5]どこか違う#KW_ED[]ような"),
        "今までのそれとはどこか違うような"
    );
    assert_eq!(keywords("poison.#KW_F[]｡#KW_ED[]"), "poison. ");
    assert_eq!(keywords("A sign ｡ stays."), "A sign ｡ stays.");
    assert_eq!(keywords("#KW_T1[5 with no end"), "#KW_T1[5 with no end");
}

#[test]
fn picks_the_keywords_of_a_monologue() {
    let dir = folder("monologue");
    let assets = Assets::load(&dir).unwrap();
    let mut code = vec![engine(0x5a7, vec![])];
    code.extend(line("I wonder how I can earn his#KW_T1[5]｡#KW_ED[]trust?"));
    code.extend(line("#KW_F[]先生#KW_ED[]が言うように"));
    code.extend(line("それとは#KW_T2[5]どこか違う#KW_ED[]ような"));
    code.extend([engine(0x6b2, vec![]), engine(0x5a8, vec![])]);
    code.extend(line("After"));
    code.extend([engine(0x700, vec![]), I::Return]);
    let mut program = Program::empty();
    program.insert(ScriptIr {
        name: "mb_aya_Dark_07".into(),
        exports: BTreeMap::from([("sure1".into(), 0)]),
        globals: BTreeMap::new(),
        code,
    });
    program.numbers.insert((1, 107), "mb_aya_Dark_07".into());
    let mut director =
        Director::new(&program, &assets, Profile::default(), "mb_aya_Dark_07").unwrap();

    let mut texts = Vec::new();
    let (options, picks) = loop {
        match director.next_event().unwrap() {
            Event::Monologue { options, picks } => break (options, picks),
            Event::Text { lines, .. } => texts.push(lines.join("/")),
            _ => {}
        }
    };
    assert_eq!(
        texts,
        [
            "I wonder how I can earn his trust?",
            "先生が言うように",
            "それとはどこか違うような"
        ]
    );
    assert_eq!(
        options,
        [
            "I wonder how I can earn his【◆】trust?",
            "【先生】が言うように",
            "それとは【どこか違う】ような"
        ]
    );
    assert_eq!(picks, 2);
    assert!(director.next_event().is_err());
    assert!(director.choose_keyword(0).unwrap());
    assert!(
        director.choose_keyword(0).is_err(),
        "a keyword is picked one time"
    );
    assert!(!director.choose_keyword(1).unwrap());
    assert_eq!(director.profile.monologue.get("mb_aya_Dark_07"), Some(&1));
    let rest: Vec<String> = std::iter::from_fn(|| director.next_event().ok())
        .take_while(|e| !matches!(e, Event::End { .. }))
        .filter_map(|e| match e {
            Event::Text { lines, .. } => Some(lines.join("/")),
            _ => None,
        })
        .collect();
    assert_eq!(rest, ["After"]);
}
