use std::fmt;

/// An error from the CPK reader.
#[derive(Debug)]
pub enum Error {
    /// The file system gives an error.
    Io(std::io::Error),
    /// The data has a bad magic number. The value tells which structure.
    BadMagic(&'static str),
    /// An offset or a size points outside the data.
    OutOfBounds(&'static str),
    /// The data has a value that the reader does not know.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::BadMagic(what) => write!(f, "bad magic number in {what}"),
            Error::OutOfBounds(what) => write!(f, "{what} points outside the data"),
            Error::Unsupported(what) => write!(f, "unsupported data: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
