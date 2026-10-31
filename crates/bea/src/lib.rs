//! Reads Bezel Engine archives (`.bea`, magic `SCNE`).
//!
//! An archive holds one or more files. All numbers are little-endian.
//!
//! The header:
//!
//! - 0x00: the magic `SCNE` and 4 zero bytes.
//! - 0x08: the version.
//! - 0x0c: the byte order mark `ff fe`.
//! - 0x20: the number of files, as a u64.
//! - 0x28: the offset of an array of u64 offsets, one for each `ASST` block.
//! - 0x40: the offset of the archive name.
//!
//! An `ASST` block describes one file:
//!
//! - 0x00: the magic `ASST`.
//! - 0x10: the compression, as a u16. 0 is none and 2 is zstd.
//! - 0x12: the alignment of the data, as a power of 2.
//! - 0x14: the stored size, as a u32.
//! - 0x18: the size after decompression, as a u32.
//! - 0x20: the offset of the data, as a u64.
//! - 0x28: the offset of the name, as a u64.
//!
//! A name is a u16 length and then the bytes.

use std::fmt;
use std::io::Read;

pub const MAGIC: &[u8; 4] = b"SCNE";
const ASSET_MAGIC: &[u8; 4] = b"ASST";
const COMPRESSION_NONE: u16 = 0;
const COMPRESSION_ZSTD: u16 = 2;

/// An error from the archive reader.
#[derive(Debug)]
pub enum Error {
    BadMagic(&'static str),
    OutOfBounds(&'static str),
    Unsupported(String),
    Decompress(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadMagic(what) => write!(f, "bad magic number in {what}"),
            Error::OutOfBounds(what) => write!(f, "{what} points outside the data"),
            Error::Unsupported(what) => write!(f, "unsupported data: {what}"),
            Error::Decompress(what) => write!(f, "decompression failed: {what}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// One file in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub compression: u16,
    /// The alignment of the data, as a power of 2.
    pub alignment: u16,
    /// The offset of the stored data.
    pub offset: u64,
    /// The size of the stored data.
    pub stored_size: u32,
    /// The size after decompression.
    pub size: u32,
}

/// A parsed archive. It borrows the bytes of the file.
#[derive(Debug)]
pub struct Archive<'a> {
    data: &'a [u8],
    pub name: String,
    pub entries: Vec<Entry>,
}

impl<'a> Archive<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Archive<'a>> {
        if data.len() < 0x48 || &data[..4] != MAGIC {
            return Err(Error::BadMagic("SCNE header"));
        }
        if data[0x0c..0x0e] != [0xff, 0xfe] {
            return Err(Error::Unsupported("big-endian archive".into()));
        }
        let count = u64_at(data, 0x20)?;
        let array = u64_at(data, 0x28)?;
        let name = string_at(data, u64_at(data, 0x40)?)?;
        // Each entry needs at least 8 bytes in the offset array.
        if count > data.len() as u64 / 8 {
            return Err(Error::OutOfBounds("file count"));
        }
        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            let at = u64_at(data, array + 8 * i)?;
            let block = bytes(data, at, 0x30, "ASST block")?;
            if &block[..4] != ASSET_MAGIC {
                return Err(Error::BadMagic("ASST block"));
            }
            entries.push(Entry {
                compression: u16::from_le_bytes([block[0x10], block[0x11]]),
                alignment: u16::from_le_bytes([block[0x12], block[0x13]]),
                stored_size: u32::from_le_bytes(block[0x14..0x18].try_into().unwrap()),
                size: u32::from_le_bytes(block[0x18..0x1c].try_into().unwrap()),
                offset: u64::from_le_bytes(block[0x20..0x28].try_into().unwrap()),
                name: string_at(
                    data,
                    u64::from_le_bytes(block[0x28..0x30].try_into().unwrap()),
                )?,
            });
        }
        Ok(Archive {
            data,
            name,
            entries,
        })
    }

    /// Reads a file and decompresses it.
    pub fn read(&self, entry: &Entry) -> Result<Vec<u8>> {
        let stored = bytes(
            self.data,
            entry.offset,
            entry.stored_size as u64,
            "file data",
        )?;
        let out = match entry.compression {
            COMPRESSION_NONE => stored.to_vec(),
            COMPRESSION_ZSTD => {
                let mut decoder = ruzstd::decoding::StreamingDecoder::new(stored)
                    .map_err(|e| Error::Decompress(e.to_string()))?;
                let mut out = Vec::with_capacity(entry.size as usize);
                decoder
                    .read_to_end(&mut out)
                    .map_err(|e| Error::Decompress(e.to_string()))?;
                out
            }
            other => return Err(Error::Unsupported(format!("compression {other}"))),
        };
        if out.len() != entry.size as usize {
            return Err(Error::Decompress(format!(
                "{} has {} bytes, the header says {}",
                entry.name,
                out.len(),
                entry.size
            )));
        }
        Ok(out)
    }

    /// Finds a file by its name.
    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }
}

fn bytes<'a>(data: &'a [u8], at: u64, len: u64, what: &'static str) -> Result<&'a [u8]> {
    let start = usize::try_from(at).map_err(|_| Error::OutOfBounds(what))?;
    let len = usize::try_from(len).map_err(|_| Error::OutOfBounds(what))?;
    data.get(start..start.checked_add(len).ok_or(Error::OutOfBounds(what))?)
        .ok_or(Error::OutOfBounds(what))
}

fn u64_at(data: &[u8], at: u64) -> Result<u64> {
    Ok(u64::from_le_bytes(
        bytes(data, at, 8, "header field")?.try_into().unwrap(),
    ))
}

fn string_at(data: &[u8], at: u64) -> Result<String> {
    let len = u16::from_le_bytes(bytes(data, at, 2, "name")?.try_into().unwrap());
    let raw = bytes(data, at + 2, len as u64, "name")?;
    Ok(String::from_utf8_lossy(raw).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an archive. Each file is (name, compression, contents).
    fn build(files: &[(&str, u16, &[u8])]) -> Vec<u8> {
        let mut out = vec![0u8; 0x48];
        out[..4].copy_from_slice(MAGIC);
        out[0x08..0x0c].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        out[0x0c..0x0e].copy_from_slice(&[0xff, 0xfe]);
        let put_u64 = |out: &mut Vec<u8>, at: usize, v: u64| {
            out[at..at + 8].copy_from_slice(&v.to_le_bytes());
        };
        let add_string = |out: &mut Vec<u8>, s: &str| -> u64 {
            let at = out.len() as u64;
            out.extend((s.len() as u16).to_le_bytes());
            out.extend(s.as_bytes());
            at
        };
        let archive_name = add_string(&mut out, "test.bea");
        put_u64(&mut out, 0x40, archive_name);
        put_u64(&mut out, 0x20, files.len() as u64);

        let array = out.len();
        put_u64(&mut out, 0x28, array as u64);
        out.resize(array + 8 * files.len(), 0);
        for (i, (name, compression, contents)) in files.iter().enumerate() {
            let stored = match *compression {
                COMPRESSION_ZSTD => ruzstd::encoding::compress_to_vec(
                    *contents,
                    ruzstd::encoding::CompressionLevel::Fastest,
                ),
                _ => contents.to_vec(),
            };
            let name_at = add_string(&mut out, name);
            let data_at = out.len() as u64;
            out.extend(&stored);
            let block = out.len();
            put_u64(&mut out, array + 8 * i, block as u64);
            out.resize(block + 0x30, 0);
            out[block..block + 4].copy_from_slice(ASSET_MAGIC);
            out[block + 0x10..block + 0x12].copy_from_slice(&compression.to_le_bytes());
            out[block + 0x12..block + 0x14].copy_from_slice(&12u16.to_le_bytes());
            out[block + 0x14..block + 0x18].copy_from_slice(&(stored.len() as u32).to_le_bytes());
            out[block + 0x18..block + 0x1c].copy_from_slice(&(contents.len() as u32).to_le_bytes());
            put_u64(&mut out, block + 0x20, data_at);
            put_u64(&mut out, block + 0x28, name_at);
        }
        out
    }

    #[test]
    fn reads_zstd_and_plain_files() {
        let big: Vec<u8> = (0..5000u32).flat_map(|i| (i % 7).to_le_bytes()).collect();
        let bytes = build(&[
            ("texture/bg/a.dds", COMPRESSION_ZSTD, &big),
            ("lua/b.lua", COMPRESSION_NONE, b"print 1"),
        ]);
        let archive = Archive::parse(&bytes).unwrap();
        assert_eq!(archive.name, "test.bea");
        assert_eq!(archive.entries.len(), 2);
        let a = archive.find("texture/bg/a.dds").unwrap();
        assert_eq!(a.compression, COMPRESSION_ZSTD);
        assert!((a.stored_size as usize) < big.len());
        assert_eq!(archive.read(a).unwrap(), big);
        let b = archive.find("lua/b.lua").unwrap();
        assert_eq!(archive.read(b).unwrap(), b"print 1");
    }

    #[test]
    fn rejects_a_wrong_size() {
        let mut bytes = build(&[("a", COMPRESSION_NONE, b"abcd")]);
        let archive = Archive::parse(&bytes).unwrap();
        let block = bytes.len() - 0x30;
        drop(archive);
        bytes[block + 0x18..block + 0x1c].copy_from_slice(&5u32.to_le_bytes());
        bytes[block + 0x14..block + 0x18].copy_from_slice(&4u32.to_le_bytes());
        let archive = Archive::parse(&bytes).unwrap();
        assert!(matches!(
            archive.read(&archive.entries[0]),
            Err(Error::Decompress(_))
        ));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            Archive::parse(&[0u8; 0x60]),
            Err(Error::BadMagic(_))
        ));
        let bytes = build(&[("a", COMPRESSION_NONE, b"abcd")]);
        for len in [0x10, 0x50, bytes.len() - 1] {
            assert!(Archive::parse(&bytes[..len]).is_err(), "length {len}");
        }
        let mut bytes = build(&[("a", 7, b"abcd")]);
        let archive = Archive::parse(&bytes).unwrap();
        assert!(matches!(
            archive.read(&archive.entries[0]),
            Err(Error::Unsupported(_))
        ));
        // A huge file count must not allocate memory for each entry.
        bytes[0x20..0x28].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(Archive::parse(&bytes).is_err());
    }
}
