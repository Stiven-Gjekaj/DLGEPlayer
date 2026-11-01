//! Runs each chapter with no screen and reports the coverage.
//!
//! A chapter is one script from `sure1` to the end command. The runner
//! runs each chapter several times:
//!
//! - once for each option index: the host picks the same index at each
//!   choice, or the last option if the choice has fewer;
//! - once with every system flag set to 1, as in a finished game.
//!
//! A route is the chapters with the same name and a number at the end, for
//! example `mb_aya_Dark_00` to `mb_aya_Dark_12`. The runner also runs each
//! route in order, with the system flags of one chapter given to the next.
//!
//! The coverage counts the instructions that ran, out of the instructions
//! that the `sure` labels can reach.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use stcm2l::decode::Instruction;

use crate::machine::Command;
use crate::program::{ChoiceOption, Program, ScriptIr};
use crate::session::{Flags, Host, Session};

/// The largest number of runs in one chapter.
const RUN_LIMIT: usize = 100_000;

/// A host that picks the same option index each time and counts commands.
struct Policy {
    pick: usize,
    messages: usize,
    choices: usize,
    unregistered: BTreeSet<u32>,
    engine: BTreeMap<u32, usize>,
}

impl Policy {
    fn new(pick: usize) -> Policy {
        Policy {
            pick,
            messages: 0,
            choices: 0,
            unregistered: BTreeSet::new(),
            engine: BTreeMap::new(),
        }
    }
}

impl Host for Policy {
    fn command(&mut self, command: &Command) {
        match command {
            Command::Message(_) => self.messages += 1,
            Command::Unregistered { opcode, .. } => {
                self.unregistered.insert(*opcode);
            }
            Command::Engine { opcode, .. } => *self.engine.entry(*opcode).or_insert(0) += 1,
            _ => {}
        }
    }
    fn choose(&mut self, _name: &str, options: &[ChoiceOption]) -> usize {
        self.choices += 1;
        self.pick.min(options.len().saturating_sub(1))
    }
}

/// The result of one run of a chapter.
#[derive(Debug, Clone, Serialize)]
pub struct ChapterRun {
    pub script: String,
    /// `pick N`, `finished game`, or `route`.
    pub mode: String,
    /// The number of label runs to the end, or the error.
    pub result: Result<usize, String>,
    pub messages: usize,
    pub choices: usize,
}

/// The coverage of one script.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Coverage {
    pub instructions: usize,
    pub reachable: usize,
    pub visited: usize,
}

/// The result of one route.
#[derive(Debug, Clone, Serialize)]
pub struct RouteRun {
    pub route: String,
    pub chapters: Vec<String>,
    /// The chapters that reached the end, in order.
    pub ended: usize,
    pub error: Option<String>,
}

/// The report of all runs.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub chapters: usize,
    /// The chapters that reached the end in every run.
    pub chapters_ended: usize,
    pub routes: Vec<RouteRun>,
    pub runs: Vec<ChapterRun>,
    pub coverage: BTreeMap<String, Coverage>,
    pub total: Coverage,
    pub messages: usize,
    pub choices_seen: usize,
    /// The engine opcodes that ran, with their counts.
    pub engine_opcodes: BTreeMap<String, usize>,
    /// The scripts that reach an opcode with no handler in the game.
    pub unregistered: BTreeMap<String, Vec<String>>,
}

/// Gives the instructions that the `sure` labels can reach.
pub fn reachable(script: &ScriptIr) -> Vec<bool> {
    let mut seen = vec![false; script.code.len()];
    let mut stack: Vec<u32> = script
        .exports
        .iter()
        .filter(|(name, _)| name.starts_with("sure"))
        .map(|(_, &i)| i)
        .collect();
    while let Some(i) = stack.pop() {
        let Some(slot) = seen.get_mut(i as usize) else {
            continue;
        };
        if *slot {
            continue;
        }
        *slot = true;
        match &script.code[i as usize] {
            Instruction::Return => {}
            Instruction::Goto { target } => stack.push(*target),
            Instruction::IfFalseGoto { target, .. } => stack.extend([*target, i + 1]),
            Instruction::Call { target, .. } => stack.extend([*target, i + 1]),
            _ => stack.push(i + 1),
        }
    }
    seen
}

/// Splits a chapter name such as `mb_aya_Dark_03` into the route and the
/// number.
pub fn route_of(name: &str) -> Option<(&str, u32)> {
    let (route, number) = name.rsplit_once('_')?;
    Some((route, number.parse().ok()?))
}

fn run_one(
    program: &Program,
    script: &str,
    flags: Flags,
    host: &mut Policy,
    visited: &mut BTreeMap<String, Vec<bool>>,
) -> (Result<usize, String>, Flags) {
    let mut session = match Session::with_flags(program, script, "sure1", flags.clone()) {
        Ok(s) => s,
        Err(e) => return (Err(e.to_string()), flags),
    };
    let result = session
        .run_chapter(host, RUN_LIMIT)
        .map_err(|e| e.to_string());
    let seen = visited
        .entry(session.script().to_string())
        .or_insert_with(|| vec![false; session.visited().len()]);
    for (a, b) in seen.iter_mut().zip(session.visited()) {
        *a |= b;
    }
    (result, session.flags)
}

/// Runs every chapter and every route.
pub fn run_all(program: &Program, progress: &dyn Fn(&str)) -> Report {
    let mut report = Report::default();
    let names: Vec<String> = program
        .numbers
        .values()
        .filter(|n| program.names().any(|m| m == n.as_str()))
        .cloned()
        .collect();
    let picks = program
        .choices
        .values()
        .map(Vec::len)
        .max()
        .unwrap_or(1)
        .max(1);
    let fresh = Flags {
        script: program.flags.clone(),
        system: program.system_flags.clone(),
    };
    let mut finished = fresh.clone();
    finished.system.values_mut().for_each(|v| *v = 1);

    let mut visited: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    let mut choices = BTreeSet::new();
    let mut engine = BTreeMap::new();
    for (n, name) in names.iter().enumerate() {
        let mut all_ended = true;
        let modes = (0..picks)
            .map(|p| (format!("pick {p}"), p, fresh.clone()))
            .chain([("finished game".to_string(), 0, finished.clone())]);
        for (mode, pick, flags) in modes {
            let mut host = Policy::new(pick);
            let (result, _) = run_one(program, name, flags, &mut host, &mut visited);
            all_ended &= result.is_ok();
            report.messages += host.messages;
            for op in &host.unregistered {
                report
                    .unregistered
                    .entry(name.clone())
                    .or_default()
                    .push(format!("{op:#x}"));
            }
            for (op, count) in host.engine {
                *engine.entry(op).or_insert(0) += count;
            }
            report.runs.push(ChapterRun {
                script: name.clone(),
                mode,
                result,
                messages: host.messages,
                choices: host.choices,
            });
        }
        if let Ok(script) = program.script(name) {
            for i in &script.code {
                if let Instruction::Engine {
                    opcode: 0x599,
                    args,
                } = i
                    && let Some(stcm2l::decode::Value::Text(c)) = args.get(1)
                {
                    choices.insert(c.clone());
                }
            }
        }
        report.chapters += 1;
        report.chapters_ended += all_ended as usize;
        if n % 50 == 49 {
            progress(&format!("{} of {} chapters", n + 1, names.len()));
        }
    }
    report.choices_seen = choices.len();
    report.engine_opcodes = engine
        .into_iter()
        .map(|(op, c)| (format!("{op:#x}"), c))
        .collect();
    for list in report.unregistered.values_mut() {
        list.sort();
        list.dedup();
    }

    let mut routes: BTreeMap<&str, Vec<(u32, &str)>> = BTreeMap::new();
    for name in &names {
        if let Some((route, number)) = route_of(name) {
            routes.entry(route).or_default().push((number, name));
        }
    }
    for (route, mut chapters) in routes {
        chapters.sort();
        let mut flags = fresh.clone();
        let mut run = RouteRun {
            route: route.to_string(),
            chapters: chapters.iter().map(|(_, n)| n.to_string()).collect(),
            ended: 0,
            error: None,
        };
        for (_, name) in &chapters {
            let mut host = Policy::new(0);
            let (result, after) = run_one(program, name, flags, &mut host, &mut visited);
            match result {
                Ok(_) => {
                    run.ended += 1;
                    flags = Flags {
                        script: program.flags.clone(),
                        system: after.system,
                    };
                }
                Err(e) => {
                    run.error = Some(format!("{name}: {e}"));
                    break;
                }
            }
        }
        report.routes.push(run);
    }

    for name in &names {
        let Ok(script) = program.script(name) else {
            continue;
        };
        let reach = reachable(&script);
        let seen = visited.get(name.as_str());
        let c = Coverage {
            instructions: script.code.len(),
            reachable: reach.iter().filter(|&&r| r).count(),
            visited: reach
                .iter()
                .enumerate()
                .filter(|(i, r)| **r && seen.is_some_and(|s| s[*i]))
                .count(),
        };
        report.total.instructions += c.instructions;
        report.total.reachable += c.reachable;
        report.total.visited += c.visited;
        report.coverage.insert(name.clone(), c);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_route_names() {
        assert_eq!(route_of("mb_aya_Dark_03"), Some(("mb_aya_Dark", 3)));
        assert_eq!(route_of("ve_shu_Dark_00"), Some(("ve_shu_Dark", 0)));
        assert_eq!(route_of("mb_aya_After_01"), Some(("mb_aya_After", 1)));
        assert_eq!(route_of("title"), None);
    }

    #[test]
    fn finds_reachable_code() {
        use stcm2l::decode::{Compare, Value};
        let script = ScriptIr {
            name: "t".into(),
            exports: [("sure1".to_string(), 0), ("memory_init".to_string(), 5)].into(),
            globals: Default::default(),
            code: vec![
                Instruction::IfFalseGoto {
                    left: Value::Int(0),
                    compare: Compare::Eq,
                    right: Value::Int(0),
                    target: 3,
                },
                Instruction::Call {
                    target: 6,
                    args: vec![],
                },
                Instruction::Return,
                Instruction::Return,
                Instruction::ShowMessage,
                Instruction::ShowMessage,
                Instruction::Return,
            ],
        };
        assert_eq!(
            reachable(&script),
            [true, true, true, true, false, false, true]
        );
    }
}
