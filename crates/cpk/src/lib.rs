//! Reads CRI CPK archives.

mod archive;
pub mod crilayla;
mod error;
pub mod utf;

pub use archive::{Archive, Entry};
pub use error::{Error, Result};
