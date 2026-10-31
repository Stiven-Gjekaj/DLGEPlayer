//! Reads the `.lip` files of `lip.cpk`. They open and close the mouth of a
//! sprite while a voice plays.
//!
//! All numbers are little-endian u32. A file has the sample rate, then
//! pairs of a sample position and a mouth state: 1 for open, 0 for closed.
//! A pair with the position `ffffffff` ends the list.

use std::fmt;

use serde::Serialize;

const END: u32 = 0xffff_ffff;

/// An error from the `.lip` reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file does not have the shape of a `.lip` file.
    BadShape(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadShape(why) => write!(f, "bad .lip file: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// The mouth states of one voice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lip {
    pub sample_rate: u32,
    /// The pairs of a sample position and a mouth state, by position.
    pub keys: Vec<[u32; 2]>,
}

/// Reads a `.lip` file.
pub fn parse(data: &[u8]) -> Result<Lip, Error> {
    if data.len() < 12 || !(data.len() - 4).is_multiple_of(8) {
        return Err(Error::BadShape("size"));
    }
    let u32_at = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    let mut keys = Vec::new();
    for at in (4..data.len()).step_by(8) {
        let position = u32_at(at);
        if position == END {
            return Ok(Lip {
                sample_rate: u32_at(0),
                keys,
            });
        }
        if keys.last().is_some_and(|[last, _]| *last > position) {
            return Err(Error::BadShape("positions out of order"));
        }
        keys.push([position, u32_at(at + 4)]);
    }
    Err(Error::BadShape("no end marker"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(pairs: &[(u32, u32)]) -> Vec<u8> {
        let mut out = 44_100u32.to_le_bytes().to_vec();
        for (p, v) in pairs {
            out.extend(p.to_le_bytes());
            out.extend(v.to_le_bytes());
        }
        out
    }

    #[test]
    fn reads_keys() {
        let lip = parse(&file(&[(0, 0), (512, 1), (4096, 0), (END, 0)])).unwrap();
        assert_eq!(lip.sample_rate, 44_100);
        assert_eq!(lip.keys, [[0, 0], [512, 1], [4096, 0]]);
    }

    #[test]
    fn rejects_bad_files() {
        assert_eq!(
            parse(&file(&[(0, 0), (512, 1)])),
            Err(Error::BadShape("no end marker"))
        );
        assert_eq!(
            parse(&file(&[(512, 1), (0, 0), (END, 0)])),
            Err(Error::BadShape("positions out of order"))
        );
        assert_eq!(parse(&[0; 9]), Err(Error::BadShape("size")));
    }
}
