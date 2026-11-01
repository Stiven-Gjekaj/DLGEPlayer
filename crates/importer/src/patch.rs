//! A set of deltas that turns the files of a dump into the files of a
//! patch, for example the English patch.
//!
//! A key names one file: a path in the RomFS, for example `boot.lua`, or a
//! file in an archive: `script.cpk/mb_aya_Dark_01.dat` or
//! `Archive/advUI.nxonnx32.bea/layout/mb_advUI.lyt`.
//!
//! The format: the magic `DLPS`, the version (u32), the entry count (u32),
//! then each entry: the key length (u16), the key, the delta length (u32),
//! and the delta. All numbers are little-endian.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::delta;
use crate::import::{BoxError, Source};

const MAGIC: &[u8; 4] = b"DLPS";
const VERSION: u32 = 1;

/// An error from the patch reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadPatch(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadPatch(why) => write!(f, "bad patch file: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// The deltas of a patch, by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatchSet {
    pub deltas: BTreeMap<String, Vec<u8>>,
}

impl PatchSet {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend(VERSION.to_le_bytes());
        out.extend((self.deltas.len() as u32).to_le_bytes());
        for (key, d) in &self.deltas {
            out.extend((key.len() as u16).to_le_bytes());
            out.extend(key.as_bytes());
            out.extend((d.len() as u32).to_le_bytes());
            out.extend(d);
        }
        out
    }

    pub fn parse(data: &[u8]) -> Result<PatchSet, Error> {
        if data.len() < 12 || &data[..4] != MAGIC {
            return Err(Error::BadPatch("magic"));
        }
        if u32::from_le_bytes(data[4..8].try_into().unwrap()) != VERSION {
            return Err(Error::BadPatch("version"));
        }
        let count = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let mut at = 12;
        let mut take = |len: usize| -> Result<&[u8], Error> {
            let part = data.get(at..at + len).ok_or(Error::BadPatch("cut entry"))?;
            at += len;
            Ok(part)
        };
        let mut deltas = BTreeMap::new();
        for _ in 0..count {
            let len = u16::from_le_bytes(take(2)?.try_into().unwrap()) as usize;
            let key = String::from_utf8(take(len)?.to_vec()).map_err(|_| Error::BadPatch("key"))?;
            let len = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
            deltas.insert(key, take(len)?.to_vec());
        }
        Ok(PatchSet { deltas })
    }

    /// Applies the delta of a key to the data of the dump, if the patch has
    /// one. Gives the data as it is if not.
    pub fn apply(&self, key: &str, data: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        match self.deltas.get(key) {
            Some(d) => Ok(delta::apply(&data, d).map_err(|e| format!("{key}: {e}"))?),
            None => Ok(data),
        }
    }
}

/// Makes the deltas from the files of a dump to the files of a patch. The
/// patch is a RomFS folder with only the changed files. A CPK archive and a
/// Bezel Engine archive get one delta for each changed file in them. Other
/// files get one delta.
pub fn make(base: &Source, patched: &Path) -> Result<PatchSet, BoxError> {
    let mut files = Vec::new();
    let mut stack = vec![patched.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let key = path
                    .strip_prefix(patched)?
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((key, path));
            }
        }
    }
    files.sort();
    let mut set = PatchSet::default();
    for (key, path) in files {
        let new = std::fs::read(&path)?;
        let old = base.read(&key)?;
        if key.ends_with(".cpk") {
            let mut new_cpk = cpk::Archive::open(std::io::Cursor::new(&new))?;
            let mut old_cpk = cpk::Archive::open(std::io::Cursor::new(&old))?;
            for entry in new_cpk.entries.clone() {
                let name = entry.path();
                let old_entry = old_cpk
                    .find(&name)
                    .cloned()
                    .ok_or_else(|| format!("{key}/{name} is not in the dump"))?;
                add(
                    &mut set,
                    format!("{key}/{name}"),
                    &old_cpk.read(&old_entry)?,
                    &new_cpk.read(&entry)?,
                );
            }
        } else if new.starts_with(bea::MAGIC) {
            let (new_bea, old_bea) = (bea::Archive::parse(&new)?, bea::Archive::parse(&old)?);
            for entry in &new_bea.entries {
                let old_entry = old_bea
                    .find(&entry.name)
                    .ok_or_else(|| format!("{key}/{} is not in the dump", entry.name))?;
                add(
                    &mut set,
                    format!("{key}/{}", entry.name),
                    &old_bea.read(old_entry)?,
                    &new_bea.read(entry)?,
                );
            }
        } else {
            add(&mut set, key, &old, &new);
        }
    }
    Ok(set)
}

fn add(set: &mut PatchSet, key: String, old: &[u8], new: &[u8]) {
    if old != new {
        set.deltas.insert(key, delta::diff(old, new));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::Root;

    #[test]
    fn round_trips_a_patch_file() {
        let mut set = PatchSet::default();
        set.deltas.insert(
            "script.cpk/a.dat".into(),
            delta::diff(b"old text", b"new text"),
        );
        set.deltas.insert("boot.lua".into(), delta::diff(b"", b"x"));
        let bytes = set.to_bytes();
        assert_eq!(PatchSet::parse(&bytes).unwrap(), set);
        assert!(PatchSet::parse(&bytes[..bytes.len() - 1]).is_err());
        assert_eq!(PatchSet::parse(b"DLPX"), Err(Error::BadPatch("magic")));
    }

    #[test]
    fn applies_only_its_keys() {
        let mut set = PatchSet::default();
        set.deltas
            .insert("a".into(), delta::diff(b"dump file", b"patched file"));
        assert_eq!(
            set.apply("a", b"dump file".to_vec()).unwrap(),
            b"patched file"
        );
        assert_eq!(set.apply("b", b"other".to_vec()).unwrap(), b"other");
        assert!(set.apply("a", b"another dump".to_vec()).is_err());
    }

    #[test]
    fn makes_deltas_for_changed_files() {
        let base_dir = std::env::temp_dir().join(format!("dlge-patch-{}", std::process::id()));
        let (dump, patched) = (base_dir.join("dump"), base_dir.join("patched"));
        for (root, files) in [
            (
                &dump,
                &[
                    ("boot.lua", "boot"),
                    ("same.txt", "same"),
                    ("x/y.txt", "old"),
                ][..],
            ),
            (
                &patched,
                &[("same.txt", "same"), ("x/y.txt", "new and longer")][..],
            ),
        ] {
            for (name, text) in files {
                let path = root.join(name);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text).unwrap();
            }
        }
        let source = Source {
            roots: vec![Root::Dir(dump.clone())],
            translation: None,
        };
        let set = make(&source, &patched).unwrap();
        assert_eq!(set.deltas.keys().collect::<Vec<_>>(), ["x/y.txt"]);
        assert_eq!(
            set.apply("x/y.txt", b"old".to_vec()).unwrap(),
            b"new and longer"
        );
        std::fs::remove_dir_all(&base_dir).unwrap();
    }
}
