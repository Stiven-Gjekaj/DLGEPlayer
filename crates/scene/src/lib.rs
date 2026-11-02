//! The part of the player that has no screen.
//!
//! `Assets` finds the files of the import folder. `Director` turns the
//! commands of the VM into events for a screen, and makes saves.
//! `Profile` keeps the data between chapters. `menu` lists the chapters.

use std::fmt;

pub mod assets;
pub mod director;
pub mod menu;
pub mod profile;

pub use assets::Assets;
pub use director::{Director, Event, SaveFile, Scene};
pub use profile::Profile;

/// An error of the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Load(String),
    Vm(vm::Error),
    /// The director waits for a pick, or got a pick with no question.
    Waiting(String),
    /// Many runs in a row gave no event.
    Stuck(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Load(e) => write!(f, "load: {e}"),
            Error::Vm(e) => write!(f, "{e}"),
            Error::Waiting(what) => write!(f, "the director waits: {what}"),
            Error::Stuck(chapter) => write!(f, "{chapter}: no event in many runs"),
        }
    }
}

impl std::error::Error for Error {}

impl From<vm::Error> for Error {
    fn from(e: vm::Error) -> Error {
        Error::Vm(e)
    }
}

#[cfg(test)]
mod tests;
