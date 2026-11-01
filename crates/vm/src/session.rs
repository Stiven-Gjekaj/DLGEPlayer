//! Plays the commands of each run and decides what runs next.
//!
//! After a run, the scene engine of the game plays the queued commands. A
//! flag command changes the flags. A choice asks the player and writes the
//! pick to the flag `select`. Then:
//!
//! - after `End`, the chapter is over;
//! - after `JumpLabel`, the label of this script runs;
//! - after `JumpScript`, the label of another script runs;
//! - else the same label runs again. A script uses the flag `switch` to go
//!   on from the part that it showed last.
//!
//! A save is the state between two runs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::machine::{Command, FlagSource, Machine, MachineState};
use crate::program::{ChoiceOption, Program};

/// The number of runs with no command that the session accepts before it
/// stops. A script that makes no command again and again does not go on.
const IDLE_LIMIT: u32 = 3;

/// The flags of a game.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Flags {
    pub script: BTreeMap<String, i32>,
    pub system: BTreeMap<String, i32>,
}

impl FlagSource for Flags {
    fn flag(&self, name: &str) -> i32 {
        self.script.get(name).copied().unwrap_or(0)
    }
    fn system_flag(&self, name: &str) -> i32 {
        self.system.get(name).copied().unwrap_or(0)
    }
}

/// The player, or a program in the place of the player.
pub trait Host {
    /// Receives each command in order.
    fn command(&mut self, _command: &Command) {}
    /// Picks an option of a choice. Gives the index of the option.
    fn choose(&mut self, name: &str, options: &[ChoiceOption]) -> usize;
}

/// The state that a save keeps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Save {
    pub script: String,
    pub label: String,
    pub flags: Flags,
    pub machine: MachineState,
}

/// What a step gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The next run is ready.
    Continue,
    /// The chapter is over.
    End,
}

/// One game in progress.
pub struct Session<'p> {
    program: &'p Program,
    machine: Machine,
    label: String,
    pub flags: Flags,
    idle: u32,
}

impl<'p> Session<'p> {
    /// Starts a script at a label with the first values of the flags.
    pub fn new(program: &'p Program, script: &str, label: &str) -> Result<Session<'p>, Error> {
        let flags = Flags {
            script: program.flags.clone(),
            system: program.system_flags.clone(),
        };
        Session::with_flags(program, script, label, flags)
    }

    /// Starts a script at a label with the given flags.
    pub fn with_flags(
        program: &'p Program,
        script: &str,
        label: &str,
        flags: Flags,
    ) -> Result<Session<'p>, Error> {
        Ok(Session {
            program,
            machine: Machine::new(program.script(script)?),
            label: label.to_string(),
            flags,
            idle: 0,
        })
    }

    /// Continues a save.
    pub fn load(program: &'p Program, save: Save) -> Result<Session<'p>, Error> {
        let mut machine = Machine::new(program.script(&save.script)?);
        machine.state = save.machine;
        Ok(Session {
            program,
            machine,
            label: save.label,
            flags: save.flags,
            idle: 0,
        })
    }

    /// Makes a save of the state before the next run.
    pub fn save(&self) -> Save {
        Save {
            script: self.machine.script.name.clone(),
            label: self.label.clone(),
            flags: self.flags.clone(),
            machine: self.machine.state.clone(),
        }
    }

    pub fn script(&self) -> &str {
        &self.machine.script.name
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Gives the instructions of the current script that ran.
    pub fn visited(&self) -> &[bool] {
        &self.machine.visited
    }

    /// Runs the label and plays its commands.
    pub fn step(&mut self, host: &mut dyn Host) -> Result<Step, Error> {
        let commands = self.machine.run(&self.label, &self.flags)?;
        let mut next: Option<(Option<i32>, String)> = None;
        let mut end = false;
        for command in &commands {
            host.command(command);
            match command {
                Command::SetFlag { name, value } => {
                    self.flags.script.insert(name.clone(), *value);
                }
                Command::AddFlag { name, value } => {
                    *self.flags.script.entry(name.clone()).or_insert(0) += value;
                }
                Command::SetSystemFlag { name, value } => {
                    self.flags.system.insert(name.clone(), *value);
                }
                Command::Choice { name } => {
                    let options = self
                        .program
                        .choices
                        .get(name)
                        .ok_or_else(|| Error::UnknownChoice(name.clone()))?;
                    let pick = host
                        .choose(name, options)
                        .min(options.len().saturating_sub(1));
                    self.flags.script.insert("select".into(), pick as i32);
                }
                Command::JumpLabel(label) => next = Some((None, label.clone())),
                Command::JumpScript { number, label } => {
                    next = Some((Some(*number), label.clone()))
                }
                Command::End => end = true,
                _ => {}
            }
        }
        if end {
            return Ok(Step::End);
        }
        match next {
            Some((None, label)) => self.label = label,
            Some((Some(number), label)) => {
                let name = self
                    .program
                    .numbers
                    .get(&number)
                    .ok_or_else(|| Error::UnknownScript(format!("number {number}")))?;
                self.machine = Machine::new(self.program.script(name)?);
                self.label = label;
            }
            None => {}
        }
        self.idle = if commands.is_empty() {
            self.idle + 1
        } else {
            0
        };
        if self.idle >= IDLE_LIMIT {
            return Err(Error::Stuck(self.script().to_string(), self.label.clone()));
        }
        Ok(Step::Continue)
    }

    /// Runs steps until the chapter is over or `limit` steps ran.
    pub fn run_chapter(&mut self, host: &mut dyn Host, limit: usize) -> Result<usize, Error> {
        for n in 1..=limit {
            if self.step(host)? == Step::End {
                return Ok(n);
            }
        }
        Err(Error::Stuck(self.script().to_string(), self.label.clone()))
    }
}
