//! Reads Nintendo SARC archives, for example the `.lyt` layouts.
//!
//! All numbers are little-endian. The archive has three parts:
//!
//! - The header: `SARC`, the header size, the byte order mark `ff fe`, the
//!   file size, and the offset of the file data.
//! - `SFAT`: the header size, the number of files, and a hash key. Then one
//!   16-byte node for each file: the name hash, the attributes, and the
//!   start and the end of the data from the data offset. If bit 24 of the
//!   attributes is set, the low 16 bits are the name offset divided by 4.
//! - `SFNT`: the names. Each name ends with a zero byte and is padded to 4.

use std::fmt;

const MAGIC: &[u8; 4] = b"SARC";
const SFAT: &[u8; 4] = b"SFAT";
const SFNT: &[u8; 4] = b"SFNT";
const HAS_NAME: u32 = 1 << 24;

/// An error from the SARC reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadMagic(&'static str),
    OutOfBounds(&'static str),
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadMagic(what) => write!(f, "bad magic number in {what}"),
            Error::OutOfBounds(what) => write!(f, "{what} points outside the data"),
            Error::Unsupported(what) => write!(f, "unsupported archive: {what}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// One file in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<'a> {
    /// The name. A file with no name has an empty name.
    pub name: String,
    pub hash: u32,
    pub data: &'a [u8],
}

/// Reads all files of a SARC archive.
pub fn parse(data: &[u8]) -> Result<Vec<Entry<'_>>> {
    if data.len() < 0x14 || &data[..4] != MAGIC {
        return Err(Error::BadMagic("SARC header"));
    }
    if data[6..8] != [0xff, 0xfe] {
        return Err(Error::Unsupported("big-endian archive"));
    }
    let u16_at = |at: usize| -> Result<usize> {
        let b = data.get(at..at + 2).ok_or(Error::OutOfBounds("field"))?;
        Ok(u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let u32_at = |at: usize| -> Result<usize> {
        let b = data.get(at..at + 4).ok_or(Error::OutOfBounds("field"))?;
        Ok(u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };

    let fat = u16_at(4)?;
    let data_at = u32_at(0x0c)?;
    if data.get(fat..fat + 4) != Some(SFAT) {
        return Err(Error::BadMagic("SFAT"));
    }
    let count = u16_at(fat + 6)?;
    let nodes = fat + u16_at(fat + 4)?;
    let fnt = nodes + 16 * count;
    if data.get(fnt..fnt + 4) != Some(SFNT) {
        return Err(Error::BadMagic("SFNT"));
    }
    let names = fnt + u16_at(fnt + 4)?;

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let node = nodes + 16 * i;
        let hash = u32_at(node)? as u32;
        let attributes = u32_at(node + 4)? as u32;
        let start = data_at + u32_at(node + 8)?;
        let end = data_at + u32_at(node + 12)?;
        let name = if attributes & HAS_NAME != 0 {
            let at = names + 4 * (attributes & 0xffff) as usize;
            let rest = data.get(at..).ok_or(Error::OutOfBounds("name"))?;
            let len = rest
                .iter()
                .position(|&b| b == 0)
                .ok_or(Error::OutOfBounds("name"))?;
            String::from_utf8_lossy(&rest[..len]).into_owned()
        } else {
            String::new()
        };
        if start > end {
            return Err(Error::OutOfBounds("file data"));
        }
        entries.push(Entry {
            name,
            hash,
            data: data
                .get(start..end)
                .ok_or(Error::OutOfBounds("file data"))?,
        });
    }
    Ok(entries)
}

/// Gives the hash of a name with a hash key. Nintendo tools use the key
/// 0x65.
pub fn name_hash(name: &str, key: u32) -> u32 {
    name.bytes()
        .fold(0u32, |h, b| h.wrapping_mul(key).wrapping_add(b as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an archive. Each file has a name and its bytes.
    fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut names = Vec::new();
        let mut name_offsets = Vec::new();
        for (name, _) in files {
            name_offsets.push(names.len() / 4);
            names.extend(name.as_bytes());
            names.push(0);
            names.resize(names.len().next_multiple_of(4), 0);
        }
        let mut blobs = Vec::new();
        let mut ranges = Vec::new();
        for (_, bytes) in files {
            let start = blobs.len();
            blobs.extend(*bytes);
            ranges.push((start, blobs.len()));
            blobs.resize(blobs.len().next_multiple_of(8), 0);
        }
        let fat = 0x14;
        let fnt = fat + 0x0c + 16 * files.len();
        let data_at = (fnt + 8 + names.len()).next_multiple_of(16);
        let mut out = MAGIC.to_vec();
        out.extend(0x14u16.to_le_bytes());
        out.extend([0xff, 0xfe]);
        out.extend(((data_at + blobs.len()) as u32).to_le_bytes());
        out.extend((data_at as u32).to_le_bytes());
        out.extend(0x0100u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(SFAT);
        out.extend(0x0cu16.to_le_bytes());
        out.extend((files.len() as u16).to_le_bytes());
        out.extend(0x65u32.to_le_bytes());
        for (i, (name, _)) in files.iter().enumerate() {
            out.extend(name_hash(name, 0x65).to_le_bytes());
            out.extend((HAS_NAME | name_offsets[i] as u32).to_le_bytes());
            out.extend((ranges[i].0 as u32).to_le_bytes());
            out.extend((ranges[i].1 as u32).to_le_bytes());
        }
        out.extend(SFNT);
        out.extend(8u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(&names);
        out.resize(data_at, 0);
        out.extend(blobs);
        out
    }

    #[test]
    fn reads_files() {
        let bytes = build(&[
            ("blyt/title.bflyt", b"FLYT layout"),
            ("timg/__Combined.bntx", b"BNTX"),
        ]);
        let entries = parse(&bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "blyt/title.bflyt");
        assert_eq!(entries[0].data, b"FLYT layout");
        assert_eq!(entries[1].name, "timg/__Combined.bntx");
        assert_eq!(entries[1].data, b"BNTX");
        assert_eq!(entries[1].hash, name_hash("timg/__Combined.bntx", 0x65));
    }

    #[test]
    fn hashes_names() {
        assert_eq!(name_hash("", 0x65), 0);
        assert_eq!(name_hash("a", 0x65), 0x61);
        assert_eq!(name_hash("ab", 0x65), 0x61 * 0x65 + 0x62);
    }

    #[test]
    fn rejects_bad_archives() {
        assert!(matches!(parse(&[0u8; 0x20]), Err(Error::BadMagic(_))));
        let bytes = build(&[("a", b"abc")]);
        // The last 5 bytes are padding, so a cut of 6 bytes cuts the data.
        for len in [0x10, 0x24, bytes.len() - 6] {
            assert!(parse(&bytes[..len]).is_err(), "length {len}");
        }
    }
}
