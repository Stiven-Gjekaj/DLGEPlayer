//! Reads a PFS0 package, for example an NSP file.
//!
//! All numbers are little-endian. The header has `PFS0`, the file count,
//! the size of the string table, and 4 bytes of padding. Then one 0x18-byte
//! entry for each file: the offset and the size (u64) from the start of the
//! data, the name offset in the string table (u32), and 4 bytes of padding.
//! The string table and then the data come after the entries.

use crate::{Error, ReadAt, Result};

pub const MAGIC: &[u8; 4] = b"PFS0";

/// One file of a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// The offset from the start of the package.
    pub offset: u64,
    pub size: u64,
}

/// Reads the file list of a package that starts at `base` in `file`.
pub fn read(file: &dyn ReadAt, base: u64) -> Result<Vec<Entry>> {
    let mut header = [0u8; 16];
    file.read_at(&mut header, base)?;
    if &header[..4] != MAGIC {
        return Err(Error::BadMagic("PFS0 header"));
    }
    let count = u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64;
    let strings_len = u32::from_le_bytes(header[8..12].try_into().unwrap()) as u64;
    if count > 0x10000 || strings_len > 0x100000 {
        return Err(Error::OutOfBounds("PFS0 header"));
    }
    let mut table = vec![0u8; (count * 0x18 + strings_len) as usize];
    file.read_at(&mut table, base + 16)?;
    let (entries, strings) = table.split_at(count as usize * 0x18);
    let data = base + 16 + table.len() as u64;
    entries
        .as_chunks::<0x18>()
        .0
        .iter()
        .map(|e| {
            let at = u32::from_le_bytes(e[16..20].try_into().unwrap()) as usize;
            let rest = strings.get(at..).ok_or(Error::OutOfBounds("PFS0 name"))?;
            let len = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
            Ok(Entry {
                name: String::from_utf8_lossy(&rest[..len]).into_owned(),
                offset: data + u64::from_le_bytes(e[..8].try_into().unwrap()),
                size: u64::from_le_bytes(e[8..16].try_into().unwrap()),
            })
        })
        .collect()
}

/// Builds a package. The tests of this crate use it.
#[cfg(test)]
pub fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut strings = Vec::new();
    let mut entries = Vec::new();
    let mut offset = 0u64;
    for (name, data) in files {
        entries.extend(offset.to_le_bytes());
        entries.extend((data.len() as u64).to_le_bytes());
        entries.extend((strings.len() as u32).to_le_bytes());
        entries.extend([0; 4]);
        strings.extend(name.as_bytes());
        strings.push(0);
        offset += data.len() as u64;
    }
    strings.resize(strings.len().next_multiple_of(16), 0);
    let mut out = MAGIC.to_vec();
    out.extend((files.len() as u32).to_le_bytes());
    out.extend((strings.len() as u32).to_le_bytes());
    out.extend([0; 4]);
    out.extend(entries);
    out.extend(strings);
    for (_, data) in files {
        out.extend(*data);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_entries() {
        let bytes = build(&[("a.nca", b"first"), ("b.tik", b"second")]);
        let entries = read(&bytes, 0).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].name, "b.tik");
        let e = &entries[1];
        assert_eq!(
            &bytes[e.offset as usize..(e.offset + e.size) as usize],
            b"second"
        );
    }

    #[test]
    fn rejects_bad_packages() {
        assert!(matches!(
            read(&b"HFS0".repeat(8), 0),
            Err(Error::BadMagic(_))
        ));
        let bytes = build(&[("a.nca", b"first")]);
        assert!(read(&bytes[..20].to_vec(), 0).is_err());
    }
}
