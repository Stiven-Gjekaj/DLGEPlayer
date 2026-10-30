use std::fmt;

/// An error from the STCM2L reader.
#[derive(Debug)]
pub enum Error {
    /// The data has a bad magic number. The value tells which structure.
    BadMagic(&'static str),
    /// A section marker is not at its position.
    Missing(&'static str),
    /// An offset or a size points outside the data.
    OutOfBounds(&'static str),
    /// A value does not agree with the layout of the file.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadMagic(what) => write!(f, "bad magic number in {what}"),
            Error::Missing(what) => write!(f, "{what} is missing"),
            Error::OutOfBounds(what) => write!(f, "{what} points outside the data"),
            Error::Invalid(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
