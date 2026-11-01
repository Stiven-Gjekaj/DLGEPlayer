//! Runs small scripts that copy the shape of the game scripts.

use std::collections::BTreeMap;

use stcm2l::decode::{BinaryOp, Compare, Instruction as I, Value as V};

use crate::machine::Command;
use crate::program::{ChoiceOption, Program, ScriptIr};
use crate::session::{Host, Session, Step};

fn engine(opcode: u32, args: Vec<V>) -> I {
    I::Engine { opcode, args }
}

fn text(t: &str) -> V {
    V::Text(t.into())
}

/// A chapter in the shape of the game:
///
/// - `sure1` reads `switch`. With 0 it shows part 1, adds 1 to `switch`,
///   and returns. With 1 it shows a choice and adds 1. With 2 it reads
///   `select` and jumps to `sure2` or `sure3`.
/// - `sure2` and `sure3` show a line, record the end, and end the chapter.
fn chapter() -> Program {
    let g = V::Global(0);
    let code = vec![
        // 0: sure1
        engine(0x41d, vec![text("switch"), g.clone()]),
        I::IfFalseGoto {
            left: g.clone(),
            compare: Compare::Eq,
            right: V::Int(0),
            target: 7,
        },
        // 2: part 1, with a call that writes to its argument
        I::Call {
            target: 30,
            args: vec![V::Global(4)],
        },
        I::TextLine {
            text: "part one".into(),
        },
        I::ShowMessage,
        engine(0x41c, vec![text("switch"), V::Int(1)]),
        I::Return,
        // 7
        I::IfFalseGoto {
            left: g.clone(),
            compare: Compare::Eq,
            right: V::Int(1),
            target: 12,
        },
        I::TextLine {
            text: "which?".into(),
        },
        I::ShowMessage,
        engine(0x599, vec![text("select"), text("q1")]),
        engine(0x41c, vec![text("switch"), V::Int(1)]),
        // 12
        I::IfFalseGoto {
            left: g.clone(),
            compare: Compare::Eq,
            right: V::Int(2),
            target: 19,
        },
        engine(0x41b, vec![text("switch"), V::Int(0)]),
        engine(0x41d, vec![text("select"), g.clone()]),
        I::IfFalseGoto {
            left: g.clone(),
            compare: Compare::Eq,
            right: V::Int(0),
            target: 18,
        },
        engine(0x44e, vec![text("sure2")]),
        I::Return,
        // 18
        engine(0x44e, vec![text("sure3")]),
        // 19
        I::Return,
        // 20: sure2
        I::Voice {
            id: V::Int(30063),
            rest: vec![V::Int(0)],
        },
        I::TextLine {
            text: "answer A".into(),
        },
        I::ShowMessage,
        engine(0x425, vec![text("q_end"), V::Int(1)]),
        engine(0x700, vec![]),
        I::Return,
        // 26: sure3
        I::TextLine {
            text: "answer B".into(),
        },
        I::ShowMessage,
        engine(0x700, vec![]),
        I::Return,
        // 30: a function that adds 5 to its argument and uses register 0
        I::Update {
            op: Some(BinaryOp::Add),
            dest: V::Immediate(0),
            value: V::Int(5),
        },
        I::Update {
            op: None,
            dest: V::Immediate(0x20),
            value: V::Immediate(0),
        },
        I::Return,
    ];
    let mut program = Program::empty();
    program.insert(ScriptIr {
        name: "test_ch01".into(),
        exports: BTreeMap::from([
            ("sure1".into(), 0),
            ("sure2".into(), 20),
            ("sure3".into(), 26),
        ]),
        globals: BTreeMap::from([(0, V::Int(0)), (4, V::Int(10))]),
        code,
    });
    program.flags = BTreeMap::from([("switch".into(), 0), ("select".into(), 0)]);
    program.choices.insert(
        "q1".into(),
        vec![
            ChoiceOption {
                text: "A".into(),
                values: [1, 5, 4],
            },
            ChoiceOption {
                text: "B".into(),
                values: [0, 0, 4],
            },
        ],
    );
    program
}

struct Record {
    pick: usize,
    commands: Vec<Command>,
    asked: Vec<String>,
}

impl Host for Record {
    fn command(&mut self, command: &Command) {
        self.commands.push(command.clone());
    }
    fn choose(&mut self, name: &str, options: &[ChoiceOption]) -> usize {
        self.asked.push(format!("{name}:{}", options.len()));
        self.pick
    }
}

fn messages(commands: &[Command]) -> Vec<String> {
    commands
        .iter()
        .filter_map(|c| match c {
            Command::Message(lines) => Some(lines.join("/")),
            _ => None,
        })
        .collect()
}

#[test]
fn plays_a_chapter_with_each_choice() {
    let program = chapter();
    for (pick, answer) in [(0, "answer A"), (1, "answer B")] {
        let mut session = Session::new(&program, "test_ch01", "sure1").unwrap();
        let mut host = Record {
            pick,
            commands: vec![],
            asked: vec![],
        };
        let steps = session.run_chapter(&mut host, 20).unwrap();
        assert_eq!(steps, 4);
        assert_eq!(messages(&host.commands), ["part one", "which?", answer]);
        assert_eq!(host.asked, ["q1:2"]);
        assert_eq!(
            session.flags.system.get("q_end").copied(),
            (pick == 0).then_some(1)
        );
    }
}

#[test]
fn passes_arguments_by_reference() {
    let program = chapter();
    let mut session = Session::new(&program, "test_ch01", "sure1").unwrap();
    let mut host = Record {
        pick: 0,
        commands: vec![],
        asked: vec![],
    };
    session.step(&mut host).unwrap();
    let save = session.save();
    assert_eq!(save.machine.globals[&4], crate::Val::Int(15));
    assert_eq!(save.machine.registers[0], crate::Val::Int(15));
}

#[test]
fn reads_flags_from_before_the_run() {
    // In the run that shows the choice, the add to `switch` waits until
    // the commands play. So one run shows the choice once.
    let program = chapter();
    let mut session = Session::new(&program, "test_ch01", "sure1").unwrap();
    let mut host = Record {
        pick: 1,
        commands: vec![],
        asked: vec![],
    };
    session.step(&mut host).unwrap();
    assert_eq!(session.flags.script["switch"], 1);
    session.step(&mut host).unwrap();
    assert_eq!(session.flags.script["switch"], 2);
    assert_eq!(session.flags.script["select"], 1);
}

#[test]
fn continues_from_a_save() {
    let program = chapter();
    let mut session = Session::new(&program, "test_ch01", "sure1").unwrap();
    let mut host = Record {
        pick: 0,
        commands: vec![],
        asked: vec![],
    };
    session.step(&mut host).unwrap();
    session.step(&mut host).unwrap();
    let text = serde_json::to_string(&session.save()).unwrap();

    let save = serde_json::from_str(&text).unwrap();
    let mut loaded = Session::load(&program, save).unwrap();
    let mut host = Record {
        pick: 0,
        commands: vec![],
        asked: vec![],
    };
    assert_eq!(loaded.step(&mut host).unwrap(), Step::Continue);
    assert_eq!(loaded.label(), "sure2");
    assert_eq!(loaded.step(&mut host).unwrap(), Step::End);
    assert_eq!(messages(&host.commands), ["answer A"]);
}

#[test]
fn ends_on_finish_and_asks_for_a_character() {
    let mut program = Program::empty();
    program.insert(ScriptIr {
        name: "pro".into(),
        exports: BTreeMap::from([("sure1".into(), 0)]),
        globals: BTreeMap::from([(0, V::Int(0))]),
        code: vec![
            engine(0x41d, vec![text("character"), V::Global(0)]),
            I::IfFalseGoto {
                left: V::Global(0),
                compare: Compare::Eq,
                right: V::Int(3),
                target: 5,
            },
            I::TextLine {
                text: "three".into(),
            },
            I::ShowMessage,
            engine(0x4bf, vec![]),
            // 5
            engine(0x6fe, vec![]),
            I::Return,
        ],
    });
    struct Pick3(Vec<Command>);
    impl Host for Pick3 {
        fn command(&mut self, c: &Command) {
            self.0.push(c.clone());
        }
        fn choose(&mut self, _: &str, _: &[ChoiceOption]) -> usize {
            0
        }
        fn choose_character(&mut self) -> i32 {
            3
        }
    }
    let mut session = Session::new(&program, "pro", "sure1").unwrap();
    let mut host = Pick3(vec![]);
    assert_eq!(session.run_chapter(&mut host, 10).unwrap(), 2);
    assert_eq!(session.flags.script["character"], 3);
    assert_eq!(messages(&host.0), ["three"]);
}

#[test]
fn reports_problems() {
    let program = chapter();
    assert!(Session::new(&program, "none", "sure1").is_err());
    let mut session = Session::new(&program, "test_ch01", "sure9").unwrap();
    let mut host = Record {
        pick: 0,
        commands: vec![],
        asked: vec![],
    };
    assert!(matches!(
        session.step(&mut host),
        Err(crate::Error::UnknownLabel(..))
    ));
}
