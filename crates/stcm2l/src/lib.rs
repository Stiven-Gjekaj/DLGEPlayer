//! Reads and writes STCM2L script files.

mod error;
mod script;

pub use error::{Error, Result};
pub use script::{Action, Export, Param, Script};
