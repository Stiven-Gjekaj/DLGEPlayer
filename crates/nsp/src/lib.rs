//! Reads the RomFS of a game from an NSP file with the keys of the user.
//!
//! An NSP file is a PFS0 package. It holds NCA files and, for some
//! dumps, a ticket. The program NCA holds the RomFS in an encrypted
//! section. `Game::open` finds the program NCA, decrypts its header and its
//! section key, and reads the file table of the RomFS. The files decrypt
//! when they are read, so the NSP is never copied.

use std::fmt;
use std::io;
use std::sync::Arc;

mod crypto;
pub mod keys;
pub mod nca;
pub mod pfs0;
pub mod romfs;

pub use keys::Keys;
pub use romfs::RomFs;

/// An error from the NSP reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Io(String),
    BadMagic(&'static str),
    OutOfBounds(&'static str),
    /// The keys file does not have a key that the file needs.
    MissingKey(String),
    Unsupported(String),
    /// The NSP has no program NCA with a RomFS.
    NoProgram,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "read error: {e}"),
            Error::BadMagic(what) => {
                write!(f, "bad magic number in {what}. Check the keys file.")
            }
            Error::OutOfBounds(what) => write!(f, "{what} points outside the data"),
            Error::MissingKey(name) => write!(f, "the keys file has no {name}"),
            Error::Unsupported(what) => write!(f, "unsupported file: {what}"),
            Error::NoProgram => write!(f, "the NSP has no program with a RomFS"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A source that reads bytes at an offset. Many threads can read at the
/// same time.
pub trait ReadAt: Send + Sync {
    /// Fills `buf` with the bytes at `offset`.
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
}

#[cfg(unix)]
impl ReadAt for std::fs::File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        std::os::unix::fs::FileExt::read_exact_at(self, buf, offset)
    }
}

#[cfg(windows)]
impl ReadAt for std::fs::File {
    fn read_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match std::os::windows::fs::FileExt::seek_read(self, buf, offset) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => {
                    buf = &mut buf[n..];
                    offset += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl ReadAt for Vec<u8> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let start = usize::try_from(offset).map_err(|_| io::ErrorKind::UnexpectedEof)?;
        let src = start
            .checked_add(buf.len())
            .and_then(|end| self.get(start..end))
            .ok_or(io::ErrorKind::UnexpectedEof)?;
        buf.copy_from_slice(src);
        Ok(())
    }
}

/// A part of another source.
#[derive(Clone)]
pub struct Region {
    inner: Arc<dyn ReadAt>,
    offset: u64,
    len: u64,
}

impl Region {
    pub fn new(inner: Arc<dyn ReadAt>, offset: u64, len: u64) -> Region {
        Region { inner, offset, len }
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reads all bytes of the region.
    pub fn read_all(&self) -> io::Result<Vec<u8>> {
        let len = usize::try_from(self.len).map_err(|_| io::ErrorKind::OutOfMemory)?;
        let mut out = vec![0u8; len];
        self.read_at(&mut out, 0)?;
        Ok(out)
    }

    /// Gives a reader with a position, for code that needs `Read` and `Seek`.
    pub fn reader(&self) -> RegionReader {
        RegionReader {
            region: self.clone(),
            position: 0,
        }
    }
}

impl ReadAt for Region {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        if offset
            .checked_add(buf.len() as u64)
            .is_none_or(|end| end > self.len)
        {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.inner.read_at(buf, self.offset + offset)
    }
}

/// A region with a position.
pub struct RegionReader {
    region: Region,
    position: u64,
}

impl io::Read for RegionReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.region.len.saturating_sub(self.position);
        let n = (buf.len() as u64).min(left) as usize;
        self.region.read_at(&mut buf[..n], self.position)?;
        self.position += n as u64;
        Ok(n)
    }
}

impl io::Seek for RegionReader {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let target = match pos {
            io::SeekFrom::Start(n) => Some(n),
            io::SeekFrom::End(d) => self.region.len.checked_add_signed(d),
            io::SeekFrom::Current(d) => self.position.checked_add_signed(d),
        };
        self.position = target.ok_or(io::ErrorKind::InvalidInput)?;
        Ok(self.position)
    }
}

/// The game in an NSP file.
pub struct Game {
    /// The title ID of the program.
    pub program_id: u64,
    pub romfs: RomFs,
}

impl Game {
    /// Opens the program NCA of an NSP file and reads its RomFS.
    pub fn open(file: Arc<dyn ReadAt>, keys: &Keys) -> Result<Game> {
        let entries = pfs0::read(&*file, 0)?;
        let tickets: Vec<&pfs0::Entry> = entries
            .iter()
            .filter(|e| e.name.ends_with(".tik"))
            .collect();
        for entry in entries.iter().filter(|e| e.name.ends_with(".nca")) {
            if entry.name.ends_with(".cnmt.nca") {
                continue;
            }
            let region = Region::new(file.clone(), entry.offset, entry.size);
            let header = nca::Header::read(&region, keys)?;
            if header.content_type != nca::PROGRAM {
                continue;
            }
            let Some(section) = header.romfs_section() else {
                continue;
            };
            let key = if header.rights_id == [0; 16] {
                header.key_area_key(keys)?
            } else {
                let name = format!("{}.tik", hex(&header.rights_id));
                let ticket = tickets
                    .iter()
                    .find(|t| t.name.eq_ignore_ascii_case(&name))
                    .ok_or_else(|| Error::MissingKey(format!("ticket {name}")))?;
                let ticket = Region::new(file.clone(), ticket.offset, ticket.size).read_all()?;
                nca::title_key(&ticket, header.key_revision(), keys)?
            };
            let data = section.data(region, &key)?;
            return Ok(Game {
                program_id: header.program_id,
                romfs: RomFs::read(data)?,
            });
        }
        Err(Error::NoProgram)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
