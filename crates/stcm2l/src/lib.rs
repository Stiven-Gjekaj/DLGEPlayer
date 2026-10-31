//! Reads and writes STCM2L script files.

pub mod decode;
pub mod disasm;
mod error;
pub mod operand;
mod script;

pub use error::{Error, Result};
pub use script::{Action, Export, MAGIC, Param, Script};
