//! Runs the script IR of the import folder with no screen.
//!
//! `Program` loads the scripts and the tables. `Machine` runs a label and
//! queues the commands of the run.

use std::fmt;

pub mod machine;
pub mod program;

pub use machine::{Command, Val};
pub use program::{ChoiceOption, Program, ScriptIr};

/// An error from the VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Load(String),
    UnknownScript(String),
    UnknownLabel(String, String),
    /// A jump or a call to an index outside the code.
    BadAddress(String, u32),
    /// An instruction that the machine cannot run: the script, the index,
    /// and the reason.
    Bad(String, u32, String),
    /// A run did not return in the step limit.
    Loop(String, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Load(e) => write!(f, "load: {e}"),
            Error::UnknownScript(s) => write!(f, "no script {s}"),
            Error::UnknownLabel(s, l) => write!(f, "{s} has no label {l}"),
            Error::BadAddress(s, i) => write!(f, "{s}: no instruction {i}"),
            Error::Bad(s, i, why) => write!(f, "{s} instruction {i}: {why}"),
            Error::Loop(s, l) => write!(f, "{s} {l}: the run does not return"),
        }
    }
}

impl std::error::Error for Error {}

