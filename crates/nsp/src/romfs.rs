//! Reads the file table of a RomFS.
//!
//! All numbers are little-endian. The header has 10 u64 values: the header
//! size (0x50), the offset and the size of the directory hash table, of the
//! directory table, of the file hash table, and of the file table, and the
//! offset of the file data.
//!
//! A directory entry has the parent, the next sibling, the first child
//! directory, the first child file, the next entry in the hash chain, and
//! the name length (u32 each), then the name. A file entry has the parent
//! and the next sibling (u32), the data offset and the size (u64), the
//! next entry in the hash chain and the name length (u32), then the name.
//! Each value is an offset in its table. `ffffffff` is no entry. Names are
//! padded to 4 bytes. The root directory is at offset 0.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Error, ReadAt, Region, Result};

const NONE: u32 = 0xffff_ffff;

/// The files of a RomFS by path, for example `Archive/title.nxonnx32.bea`.
pub struct RomFs {
    data: Region,
    files: BTreeMap<String, (u64, u64)>,
    dirs: BTreeSet<String>,
}

fn u32_at(table: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        table
            .get(at..at + 4)
            .ok_or(Error::OutOfBounds("RomFS entry"))?
            .try_into()
            .unwrap(),
    ))
}

fn u64_at(table: &[u8], at: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        table
            .get(at..at + 8)
            .ok_or(Error::OutOfBounds("RomFS entry"))?
            .try_into()
            .unwrap(),
    ))
}

fn name(table: &[u8], at: usize, len: u32) -> Result<String> {
    let bytes = table
        .get(at..at + len as usize)
        .ok_or(Error::OutOfBounds("RomFS name"))?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

impl RomFs {
    /// Reads the file table of a RomFS.
    pub fn read(data: Region) -> Result<RomFs> {
        let mut header = [0u8; 0x50];
        data.read_at(&mut header, 0)?;
        if u64_at(&header, 0)? != 0x50 {
            return Err(Error::BadMagic("RomFS header"));
        }
        let table = |i: usize| -> Result<Vec<u8>> {
            let (offset, size) = (u64_at(&header, 8 + 16 * i)?, u64_at(&header, 16 + 16 * i)?);
            if size > 64 << 20 {
                return Err(Error::OutOfBounds("RomFS table"));
            }
            let mut out = vec![0u8; size as usize];
            data.read_at(&mut out, offset)?;
            Ok(out)
        };
        let dir_table = table(1)?;
        let file_table = table(3)?;
        let data_offset = u64_at(&header, 0x48)?;

        let mut files = BTreeMap::new();
        let mut dirs = BTreeSet::new();
        // A stack of directory offsets and their paths.
        let mut stack = vec![(0u32, String::new())];
        let mut seen = 0usize;
        while let Some((dir, path)) = stack.pop() {
            seen += 1;
            if seen > dir_table.len() {
                return Err(Error::OutOfBounds("RomFS directory loop"));
            }
            let at = dir as usize;
            let mut child = u32_at(&dir_table, at + 8)?;
            while child != NONE {
                let c = child as usize;
                let n = name(&dir_table, c + 0x18, u32_at(&dir_table, c + 0x14)?)?;
                let child_path = format!("{path}{n}/");
                dirs.insert(child_path.trim_end_matches('/').to_string());
                stack.push((child, child_path));
                child = u32_at(&dir_table, c + 4)?;
            }
            let mut file = u32_at(&dir_table, at + 12)?;
            let mut count = 0usize;
            while file != NONE {
                count += 1;
                if count > file_table.len() {
                    return Err(Error::OutOfBounds("RomFS file loop"));
                }
                let f = file as usize;
                let offset = u64_at(&file_table, f + 8)?;
                let size = u64_at(&file_table, f + 16)?;
                let n = name(&file_table, f + 0x20, u32_at(&file_table, f + 0x1c)?)?;
                if data_offset + offset + size > data.len() {
                    return Err(Error::OutOfBounds("RomFS file"));
                }
                files.insert(format!("{path}{n}"), (data_offset + offset, size));
                file = u32_at(&file_table, f + 4)?;
            }
        }
        Ok(RomFs { data, files, dirs })
    }

    /// Gives the paths and the sizes of all files.
    pub fn files(&self) -> impl Iterator<Item = (&str, u64)> {
        self.files.iter().map(|(p, (_, size))| (p.as_str(), *size))
    }

    /// Gives a file. It decrypts when it is read.
    pub fn file(&self, path: &str) -> Option<Region> {
        let (offset, size) = *self.files.get(path)?;
        Some(Region::new(
            std::sync::Arc::new(self.data.clone()),
            offset,
            size,
        ))
    }

    /// Tells if a directory exists. The root is `""`.
    pub fn is_dir(&self, path: &str) -> bool {
        path.is_empty() || self.dirs.contains(path)
    }

    /// Gives the names of the files in a directory. The root is `""`.
    pub fn list(&self, dir: &str) -> Vec<&str> {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        self.files
            .keys()
            .filter_map(|p| p.strip_prefix(&prefix))
            .filter(|rest| !rest.contains('/'))
            .collect()
    }
}

/// Builds a RomFS with the given files. Each path uses `/`. The tests use it.
#[cfg(test)]
pub fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
    // The directories, by path. The root is "".
    let mut dir_paths: Vec<String> = vec![String::new()];
    for (path, _) in files {
        let mut parts: Vec<&str> = path.split('/').collect();
        parts.pop();
        for i in 1..=parts.len() {
            let p = parts[..i].join("/");
            if !dir_paths.contains(&p) {
                dir_paths.push(p);
            }
        }
    }
    let parent = |p: &str| {
        p.rsplit_once('/')
            .map_or(String::new(), |(a, _)| a.to_string())
    };
    let leaf = |p: &str| p.rsplit('/').next().unwrap().to_string();
    let entry_len = |n: &str, head: usize| head + n.len().next_multiple_of(4);

    let mut dir_offsets = Vec::new();
    let mut at = 0;
    for p in &dir_paths {
        dir_offsets.push(at as u32);
        at += entry_len(&leaf(p), 0x18);
    }
    let mut file_offsets = Vec::new();
    let mut at = 0;
    for (p, _) in files {
        file_offsets.push(at as u32);
        at += entry_len(&leaf(p), 0x20);
    }
    let dir_index = |p: &str| dir_paths.iter().position(|d| d == p).unwrap();
    let first = |items: Vec<usize>, offsets: &[u32]| items.first().map_or(NONE, |&i| offsets[i]);

    let mut dir_table = Vec::new();
    for (i, p) in dir_paths.iter().enumerate() {
        let children: Vec<usize> = (1..dir_paths.len())
            .filter(|&j| parent(&dir_paths[j]) == *p)
            .collect();
        let child_files: Vec<usize> = (0..files.len())
            .filter(|&j| parent(files[j].0) == *p)
            .collect();
        let siblings: Vec<usize> = (1..dir_paths.len())
            .filter(|&j| i != 0 && parent(&dir_paths[j]) == parent(p))
            .collect();
        let next = siblings
            .iter()
            .position(|&j| j == i)
            .and_then(|k| siblings.get(k + 1))
            .map_or(NONE, |&j| dir_offsets[j]);
        let n = if i == 0 { String::new() } else { leaf(p) };
        let par = if i == 0 {
            0
        } else {
            dir_offsets[dir_index(&parent(p))]
        };
        for v in [
            par,
            next,
            first(children, &dir_offsets),
            first(child_files, &file_offsets),
            NONE,
            n.len() as u32,
        ] {
            dir_table.extend(v.to_le_bytes());
        }
        dir_table.extend(n.as_bytes());
        dir_table.resize(dir_table.len().next_multiple_of(4), 0);
    }
    let mut file_table = Vec::new();
    let mut blobs = Vec::new();
    for (i, (p, data)) in files.iter().enumerate() {
        let siblings: Vec<usize> = (0..files.len())
            .filter(|&j| parent(files[j].0) == parent(p))
            .collect();
        let next = siblings
            .iter()
            .position(|&j| j == i)
            .and_then(|k| siblings.get(k + 1))
            .map_or(NONE, |&j| file_offsets[j]);
        let n = leaf(p);
        file_table.extend(dir_offsets[dir_index(&parent(p))].to_le_bytes());
        file_table.extend(next.to_le_bytes());
        file_table.extend((blobs.len() as u64).to_le_bytes());
        file_table.extend((data.len() as u64).to_le_bytes());
        file_table.extend(NONE.to_le_bytes());
        file_table.extend((n.len() as u32).to_le_bytes());
        file_table.extend(n.as_bytes());
        file_table.resize(file_table.len().next_multiple_of(4), 0);
        blobs.extend(*data);
        blobs.resize(blobs.len().next_multiple_of(16), 0);
    }
    let dir_at = 0x50u64;
    let file_at = dir_at + dir_table.len() as u64;
    let data_at = (file_at + file_table.len() as u64).next_multiple_of(16);
    let mut out = Vec::new();
    for v in [
        0x50,
        dir_at,
        0,
        dir_at,
        dir_table.len() as u64,
        file_at,
        0,
        file_at,
        file_table.len() as u64,
        data_at,
    ] {
        out.extend(v.to_le_bytes());
    }
    out.extend(dir_table);
    out.extend(file_table);
    out.resize(data_at as usize, 0);
    out.extend(blobs);
    out
}
