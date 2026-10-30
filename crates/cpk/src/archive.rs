//! Reads the file list and the files of a CPK archive.
//!
//! A CPK file has chunks. Each chunk has a 4-byte magic number, 4 unknown
//! bytes, a little-endian u64 size, and then a `@UTF` table.
//! The `CPK ` chunk at offset 0 holds the header table.
//! The header table gives the offsets of the other chunks.
//!
//! - `TOC `: one row for each file, with the directory and the name.
//! - `ITOC`: the files by number only. Some archives have no `TOC `.

use std::io::{Read, Seek, SeekFrom};

use crate::crilayla;
use crate::error::{Error, Result};
use crate::utf::Table;

/// One file in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: Option<u32>,
    pub dir: String,
    pub name: String,
    /// The absolute offset of the stored data in the archive.
    pub offset: u64,
    /// The size of the stored data.
    pub size: u64,
    /// The size of the data after decompression.
    pub extract_size: u64,
}

impl Entry {
    /// Gives the path of the file with `/` between the directory and the name.
    pub fn path(&self) -> String {
        if self.dir.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.dir, self.name)
        }
    }
}

/// An open CPK archive.
pub struct Archive<R> {
    reader: R,
    pub header: Table,
    pub entries: Vec<Entry>,
}

impl<R: Read + Seek> Archive<R> {
    /// Reads the header and the file list.
    pub fn open(mut reader: R) -> Result<Self> {
        let header = read_chunk(&mut reader, 0, b"CPK ")?;
        let number = |column: &str| header.get_u64(0, column).filter(|&v| v != 0);

        let content_offset = number("ContentOffset");
        let toc_offset = number("TocOffset");
        let itoc_offset = number("ItocOffset");

        let entries = if let Some(toc_offset) = toc_offset {
            let toc = read_chunk(&mut reader, toc_offset, b"TOC ")?;
            let base = match content_offset {
                Some(content) => content.min(toc_offset),
                None => toc_offset,
            };
            toc_entries(&toc, base)?
        } else if let Some(itoc_offset) = itoc_offset {
            let itoc = read_chunk(&mut reader, itoc_offset, b"ITOC")?;
            let content = content_offset.ok_or(Error::OutOfBounds("ContentOffset"))?;
            let align = number("Align").unwrap_or(1);
            itoc_entries(&itoc, content, align)?
        } else {
            return Err(Error::Unsupported("archive has no TOC and no ITOC".into()));
        };

        Ok(Archive {
            reader,
            header,
            entries,
        })
    }

    /// Reads the stored data of a file and decompresses it if necessary.
    pub fn read(&mut self, entry: &Entry) -> Result<Vec<u8>> {
        let mut data = vec![0u8; entry.size as usize];
        self.reader.seek(SeekFrom::Start(entry.offset))?;
        self.reader.read_exact(&mut data)?;
        if crilayla::is_compressed(&data) {
            data = crilayla::decompress(&data)?;
        }
        Ok(data)
    }

    /// Finds a file by its path.
    pub fn find(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path() == path)
    }
}

fn read_chunk<R: Read + Seek>(reader: &mut R, offset: u64, magic: &[u8; 4]) -> Result<Table> {
    let mut head = [0u8; 16];
    reader.seek(SeekFrom::Start(offset))?;
    reader.read_exact(&mut head)?;
    if &head[..4] != magic {
        return Err(Error::BadMagic(match magic {
            b"CPK " => "CPK header",
            b"TOC " => "TOC chunk",
            _ => "ITOC chunk",
        }));
    }
    let size = u64::from_le_bytes(head[8..16].try_into().unwrap());
    let mut table = vec![0u8; size as usize];
    reader.read_exact(&mut table)?;
    Table::parse(&table)
}

fn toc_entries(toc: &Table, base: u64) -> Result<Vec<Entry>> {
    let mut entries = Vec::with_capacity(toc.rows.len());
    for row in 0..toc.rows.len() {
        let size = toc
            .get_u64(row, "FileSize")
            .ok_or(Error::OutOfBounds("TOC FileSize"))?;
        let offset = toc
            .get_u64(row, "FileOffset")
            .ok_or(Error::OutOfBounds("TOC FileOffset"))?;
        entries.push(Entry {
            id: toc.get_u64(row, "ID").map(|v| v as u32),
            dir: toc.get_str(row, "DirName").unwrap_or("").to_string(),
            name: toc.get_str(row, "FileName").unwrap_or("").to_string(),
            offset: base + offset,
            size,
            extract_size: toc.get_u64(row, "ExtractSize").unwrap_or(size),
        });
    }
    Ok(entries)
}

fn itoc_entries(itoc: &Table, content: u64, align: u64) -> Result<Vec<Entry>> {
    // DataL holds the small files with 16-bit sizes.
    // DataH holds the large files with 32-bit sizes.
    let mut found = Vec::new();
    for column in ["DataL", "DataH"] {
        let Some(bytes) = itoc.get(0, column).and_then(|v| v.as_data()) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let table = Table::parse(bytes)?;
        for row in 0..table.rows.len() {
            let id = table
                .get_u64(row, "ID")
                .ok_or(Error::OutOfBounds("ITOC ID"))?;
            let size = table
                .get_u64(row, "FileSize")
                .ok_or(Error::OutOfBounds("ITOC FileSize"))?;
            let extract = table.get_u64(row, "ExtractSize").unwrap_or(size);
            found.push((id as u32, size, extract));
        }
    }
    found.sort_by_key(|f| f.0);

    let mut offset = content;
    let mut entries = Vec::with_capacity(found.len());
    for (id, size, extract_size) in found {
        entries.push(Entry {
            id: Some(id),
            dir: String::new(),
            name: format!("{id:05}"),
            offset,
            size,
            extract_size,
        });
        offset += size;
        if align > 1 {
            offset = offset.div_ceil(align) * align;
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utf::Value;
    use std::io::Cursor;

    const ALIGN: u64 = 0x800;

    fn chunk(magic: &[u8; 4], table: &Table) -> Vec<u8> {
        let body = table.to_bytes().unwrap();
        let mut out = magic.to_vec();
        out.extend_from_slice(&0xffu32.to_le_bytes());
        out.extend_from_slice(&(body.len() as u64).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    fn pad_to(out: &mut Vec<u8>, at: u64) {
        assert!(out.len() as u64 <= at);
        out.resize(at as usize, 0);
    }

    fn header(content: u64, toc: u64, itoc: u64) -> Table {
        Table {
            name: "CpkHeader".into(),
            columns: vec![
                "ContentOffset".into(),
                "TocOffset".into(),
                "ItocOffset".into(),
                "Align".into(),
            ],
            rows: vec![vec![
                Value::U64(content),
                Value::U64(toc),
                Value::U64(itoc),
                Value::U16(ALIGN as u16),
            ]],
        }
    }

    /// Builds an archive with a TOC. Each file is (dir, name, stored bytes).
    fn build_toc_archive(files: &[(&str, &str, Vec<u8>, u64)]) -> Vec<u8> {
        let toc_offset = ALIGN;
        let content_offset = ALIGN * 4;
        let mut rows = Vec::new();
        let mut content = Vec::new();
        for (i, (dir, name, data, extract)) in files.iter().enumerate() {
            let at = content_offset + content.len() as u64;
            rows.push(vec![
                Value::String(dir.to_string()),
                Value::String(name.to_string()),
                Value::U32(data.len() as u32),
                Value::U32(*extract as u32),
                Value::U64(at - toc_offset),
                Value::U32(i as u32),
            ]);
            content.extend_from_slice(data);
            let next = (content.len() as u64).div_ceil(ALIGN) * ALIGN;
            content.resize(next as usize, 0);
        }
        let toc = Table {
            name: "CpkTocInfo".into(),
            columns: [
                "DirName",
                "FileName",
                "FileSize",
                "ExtractSize",
                "FileOffset",
                "ID",
            ]
            .map(String::from)
            .to_vec(),
            rows,
        };

        let mut out = chunk(b"CPK ", &header(content_offset, toc_offset, 0));
        pad_to(&mut out, toc_offset);
        out.extend(chunk(b"TOC ", &toc));
        pad_to(&mut out, content_offset);
        out.extend(content);
        out
    }

    #[test]
    fn reads_files_by_toc() {
        let plain = b"plain file".to_vec();
        let mut big = vec![0u8; 0x100];
        big.extend(std::iter::repeat_n(b"STCM2L ".as_slice(), 100).flatten());
        let packed = crilayla::compress(&big).unwrap();

        let bytes = build_toc_archive(&[
            ("", "a.txt", plain.clone(), plain.len() as u64),
            ("scr", "b.dat", packed.clone(), big.len() as u64),
        ]);
        let mut archive = Archive::open(Cursor::new(bytes)).unwrap();

        let paths: Vec<_> = archive.entries.iter().map(Entry::path).collect();
        assert_eq!(paths, ["a.txt", "scr/b.dat"]);
        assert_eq!(archive.entries[1].id, Some(1));
        assert_eq!(archive.entries[1].size, packed.len() as u64);
        assert_eq!(archive.entries[1].extract_size, big.len() as u64);

        let a = archive.entries[0].clone();
        let b = archive.find("scr/b.dat").unwrap().clone();
        assert_eq!(archive.read(&a).unwrap(), plain);
        assert_eq!(archive.read(&b).unwrap(), big);
    }

    #[test]
    fn reads_files_by_itoc() {
        let small = vec![1u8; 10];
        let large = vec![2u8; 70000];
        let content_offset = ALIGN * 4;

        let data_l = Table {
            name: "CpkItocL".into(),
            columns: ["ID", "FileSize", "ExtractSize"].map(String::from).to_vec(),
            rows: vec![vec![Value::U16(0), Value::U16(10), Value::U16(10)]],
        };
        let data_h = Table {
            name: "CpkItocH".into(),
            columns: ["ID", "FileSize", "ExtractSize"].map(String::from).to_vec(),
            rows: vec![vec![Value::U16(1), Value::U32(70000), Value::U32(70000)]],
        };
        let itoc = Table {
            name: "CpkItocInfo".into(),
            columns: ["FilesL", "FilesH", "DataL", "DataH"]
                .map(String::from)
                .to_vec(),
            rows: vec![vec![
                Value::U32(1),
                Value::U32(1),
                Value::Data(data_l.to_bytes().unwrap()),
                Value::Data(data_h.to_bytes().unwrap()),
            ]],
        };

        let mut out = chunk(b"CPK ", &header(content_offset, 0, ALIGN));
        pad_to(&mut out, ALIGN);
        out.extend(chunk(b"ITOC", &itoc));
        pad_to(&mut out, content_offset);
        out.extend(&small);
        pad_to(&mut out, content_offset + ALIGN);
        out.extend(&large);

        let mut archive = Archive::open(Cursor::new(out)).unwrap();
        let entries = archive.entries.clone();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "00000");
        assert_eq!(entries[1].offset, content_offset + ALIGN);
        assert_eq!(archive.read(&entries[0]).unwrap(), small);
        assert_eq!(archive.read(&entries[1]).unwrap(), large);
    }

    #[test]
    fn rejects_a_file_that_is_not_a_cpk() {
        let result = Archive::open(Cursor::new(vec![0u8; 64]));
        assert!(matches!(result, Err(Error::BadMagic("CPK header"))));
    }
}
