//! Reads and writes the sections of an STCM2L file.
//!
//! All numbers are little-endian. All offsets are absolute file offsets.
//!
//! The layout:
//!
//! - 0x00: the tag. It starts with `STCM2L` and holds a build date. 0x20 bytes.
//! - 0x20: the offset of the export entries.
//! - 0x24: the number of export entries.
//! - 0x28: an unknown value. It is 1 in all known files.
//! - 0x2c: the offset of the collection link body.
//! - 0x30: 0x20 reserved bytes.
//! - 0x50: the marker `GLOBAL_DATA\0`, then the global data.
//! - the marker `CODE_START_\0`, then the actions.
//! - the marker `EXPORT_DATA\0`, then the export entries.
//! - the marker `COLLECTION_LINK\0`, then a 64-byte body.
//! - an appendix. The original files have no appendix. A translation
//!   patch puts new data blocks here and points the parameters to them.
//!
//! An action has a 16-byte head: a call flag, an opcode, the number of
//! parameters, and the length of the full action in bytes.
//! The parameters follow. Each parameter is three u32 values.
//! The data of the action follows the parameters.
//! If the call flag is not zero, the opcode is the offset of the
//! action that the call goes to.

use crate::error::{Error, Result};

pub const MAGIC: &[u8; 6] = b"STCM2L";
pub const GLOBAL_DATA: &[u8; 12] = b"GLOBAL_DATA\0";
pub const CODE_START: &[u8; 12] = b"CODE_START_\0";
pub const EXPORT_DATA: &[u8; 12] = b"EXPORT_DATA\0";
pub const COLLECTION_LINK: &[u8; 16] = b"COLLECTION_LINK\0";

const TAG_LEN: usize = 0x20;
const RESERVED_LEN: usize = 0x20;
const GLOBAL_DATA_AT: usize = 0x50;
const ACTION_HEAD_LEN: usize = 16;
const PARAM_LEN: usize = 12;
const EXPORT_NAME_LEN: usize = 32;
const EXPORT_LEN: usize = 4 + EXPORT_NAME_LEN + 4;
const COLLECTION_LINK_LEN: usize = 64;

/// One parameter of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    pub p0: u32,
    pub p1: u32,
    pub p2: u32,
}

/// One instruction of the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// The file offset of the action. The writer does not use this value.
    /// The writer puts each action after the previous one.
    pub offset: u32,
    /// The call flag. A value that is not zero makes the action a call.
    pub call: u32,
    /// The opcode, or the target offset if the action is a call.
    pub opcode: u32,
    pub params: Vec<Param>,
    /// The bytes after the parameters.
    pub data: Vec<u8>,
}

impl Action {
    pub fn is_call(&self) -> bool {
        self.call != 0
    }

    /// Gives the length of the action in bytes.
    pub fn len(&self) -> usize {
        ACTION_HEAD_LEN + self.params.len() * PARAM_LEN + self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Gives the file offset of the first data byte.
    pub fn data_offset(&self) -> u32 {
        self.offset + (ACTION_HEAD_LEN + self.params.len() * PARAM_LEN) as u32
    }

    /// Gives the data bytes from a file offset to the end of the action,
    /// if the offset is in the data of this action.
    pub fn data_at(&self, offset: u32) -> Option<&[u8]> {
        let start = offset.checked_sub(self.data_offset())? as usize;
        self.data.get(start..)
    }
}

/// An entry point that other scripts can find by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    /// An unknown value. It is 0 in all known files.
    pub kind: u32,
    /// The name, padded with zero bytes.
    pub name: [u8; EXPORT_NAME_LEN],
    /// The offset of the action.
    pub address: u32,
}

impl Export {
    pub fn new(name: &str, address: u32) -> Result<Export> {
        let bytes = name.as_bytes();
        if bytes.len() >= EXPORT_NAME_LEN {
            return Err(Error::Invalid(format!("export name is too long: {name}")));
        }
        let mut padded = [0u8; EXPORT_NAME_LEN];
        padded[..bytes.len()].copy_from_slice(bytes);
        Ok(Export {
            kind: 0,
            name: padded,
            address,
        })
    }

    /// Gives the name up to the first zero byte.
    pub fn name(&self) -> String {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        String::from_utf8_lossy(&self.name[..end]).into_owned()
    }
}

/// A parsed STCM2L file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// The 0x20-byte tag at the start of the file.
    pub tag: [u8; TAG_LEN],
    /// The value at 0x28.
    pub unknown: u32,
    /// The bytes from 0x30 to 0x50.
    pub reserved: [u8; RESERVED_LEN],
    /// The bytes between the `GLOBAL_DATA` marker and the `CODE_START_` marker.
    pub global_data: Vec<u8>,
    pub actions: Vec<Action>,
    pub exports: Vec<Export>,
    /// The first u32 of the collection link body.
    pub link_unknown: u32,
    /// The bytes after the end offset in the collection link body.
    pub link_rest: [u8; COLLECTION_LINK_LEN - 8],
    /// The bytes after the collection link body.
    pub appendix: Vec<u8>,
}

impl Script {
    /// Gives the file offset of the first byte of global data.
    pub fn global_data_offset(&self) -> u32 {
        (GLOBAL_DATA_AT + GLOBAL_DATA.len()) as u32
    }

    /// Gives the file offset of the first action.
    pub fn code_offset(&self) -> u32 {
        self.global_data_offset() + (self.global_data.len() + CODE_START.len()) as u32
    }

    /// Gives the file offset of the first byte of the appendix.
    pub fn appendix_offset(&self) -> u32 {
        let code: usize = self.actions.iter().map(Action::len).sum();
        self.code_offset()
            + (code
                + EXPORT_DATA.len()
                + self.exports.len() * EXPORT_LEN
                + COLLECTION_LINK.len()
                + COLLECTION_LINK_LEN) as u32
    }

    /// Gives the appendix bytes from a file offset to the end of the file,
    /// if the offset is in the appendix.
    pub fn appendix_at(&self, offset: u32) -> Option<&[u8]> {
        let start = offset.checked_sub(self.appendix_offset())? as usize;
        self.appendix.get(start..).filter(|rest| !rest.is_empty())
    }

    /// Gives the text of the tag up to the first zero byte.
    pub fn tag_text(&self) -> String {
        let end = self.tag.iter().position(|&b| b == 0).unwrap_or(TAG_LEN);
        String::from_utf8_lossy(&self.tag[..end]).into_owned()
    }

    /// Finds the action at a file offset.
    pub fn action_at(&self, offset: u32) -> Option<&Action> {
        let index = self
            .actions
            .binary_search_by_key(&offset, |a| a.offset)
            .ok()?;
        Some(&self.actions[index])
    }

    /// Finds the action that holds a file offset.
    pub fn action_containing(&self, offset: u32) -> Option<&Action> {
        let index = match self.actions.binary_search_by_key(&offset, |a| a.offset) {
            Ok(i) => i,
            Err(0) => return None,
            Err(i) => i - 1,
        };
        let action = &self.actions[index];
        (offset < action.offset + action.len() as u32).then_some(action)
    }

    /// Reads an STCM2L file.
    pub fn parse(data: &[u8]) -> Result<Script> {
        let r = Reader { data };
        if !data.starts_with(MAGIC) {
            return Err(Error::BadMagic("STCM2L"));
        }
        let export_offset = r.u32(0x20)? as usize;
        let export_count = r.u32(0x24)? as usize;
        let unknown = r.u32(0x28)?;
        let link_offset = r.u32(0x2c)? as usize;

        r.expect(GLOBAL_DATA_AT, GLOBAL_DATA)?;
        let global_start = GLOBAL_DATA_AT + GLOBAL_DATA.len();
        let code_marker = find_marker(data, global_start, CODE_START)
            .ok_or(Error::Missing("CODE_START_ marker"))?;
        let code_start = code_marker + CODE_START.len();

        let export_marker = export_offset
            .checked_sub(EXPORT_DATA.len())
            .ok_or(Error::OutOfBounds("export offset"))?;
        r.expect(export_marker, EXPORT_DATA)?;
        if export_marker < code_start {
            return Err(Error::OutOfBounds("export offset"));
        }

        let mut actions = Vec::new();
        let mut at = code_start;
        while at < export_marker {
            let call = r.u32(at)?;
            let opcode = r.u32(at + 4)?;
            let count = r.u32(at + 8)? as usize;
            let len = r.u32(at + 12)? as usize;
            let params_end = count
                .checked_mul(PARAM_LEN)
                .and_then(|n| n.checked_add(ACTION_HEAD_LEN))
                .ok_or(Error::Invalid(format!(
                    "action at {at:#x} has too many parameters"
                )))?;
            if len < params_end || at + len > export_marker {
                return Err(Error::Invalid(format!(
                    "action at {at:#x} has a bad length {len:#x}"
                )));
            }
            let params = (0..count)
                .map(|i| {
                    let p = at + ACTION_HEAD_LEN + i * PARAM_LEN;
                    Ok(Param {
                        p0: r.u32(p)?,
                        p1: r.u32(p + 4)?,
                        p2: r.u32(p + 8)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            actions.push(Action {
                offset: at as u32,
                call,
                opcode,
                params,
                data: r.bytes(at + params_end, len - params_end)?.to_vec(),
            });
            at += len;
        }

        let mut exports = Vec::with_capacity(export_count.min(4096));
        for i in 0..export_count {
            let e = export_offset + i * EXPORT_LEN;
            exports.push(Export {
                kind: r.u32(e)?,
                name: r.bytes(e + 4, EXPORT_NAME_LEN)?.try_into().unwrap(),
                address: r.u32(e + 4 + EXPORT_NAME_LEN)?,
            });
        }

        let link_marker = export_offset + export_count * EXPORT_LEN;
        if link_marker + COLLECTION_LINK.len() != link_offset {
            return Err(Error::Invalid(format!(
                "collection link offset {link_offset:#x} does not follow the exports"
            )));
        }
        r.expect(link_marker, COLLECTION_LINK)?;
        let appendix_start = link_offset + COLLECTION_LINK_LEN;
        if appendix_start > data.len() {
            return Err(Error::OutOfBounds("collection link body"));
        }
        let end = r.u32(link_offset + 4)? as usize;
        if end != data.len() {
            return Err(Error::Invalid(format!(
                "collection link gives end {end:#x}, the file ends at {:#x}",
                data.len()
            )));
        }

        Ok(Script {
            tag: data[..TAG_LEN].try_into().unwrap(),
            unknown,
            reserved: data[0x30..0x50].try_into().unwrap(),
            global_data: data[global_start..code_marker].to_vec(),
            actions,
            exports,
            link_unknown: r.u32(link_offset)?,
            link_rest: r
                .bytes(link_offset + 8, COLLECTION_LINK_LEN - 8)?
                .try_into()
                .unwrap(),
            appendix: data[appendix_start..].to_vec(),
        })
    }

    /// Writes the script. The writer calculates the header offsets, the
    /// action lengths, and the end offset. It keeps all other values.
    ///
    /// The writer does not move pointers. If an action changes its length,
    /// the offsets in the parameters and in the exports become wrong.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.tag);
        out.extend_from_slice(&[0u8; 16]);
        out.extend_from_slice(&self.reserved);
        out.extend_from_slice(GLOBAL_DATA);
        out.extend_from_slice(&self.global_data);
        out.extend_from_slice(CODE_START);
        for action in &self.actions {
            put(&mut out, action.call);
            put(&mut out, action.opcode);
            put(&mut out, action.params.len() as u32);
            put(&mut out, action.len() as u32);
            for p in &action.params {
                put(&mut out, p.p0);
                put(&mut out, p.p1);
                put(&mut out, p.p2);
            }
            out.extend_from_slice(&action.data);
        }
        out.extend_from_slice(EXPORT_DATA);
        let export_offset = out.len() as u32;
        for e in &self.exports {
            put(&mut out, e.kind);
            out.extend_from_slice(&e.name);
            put(&mut out, e.address);
        }
        out.extend_from_slice(COLLECTION_LINK);
        let link_offset = out.len() as u32;
        let end = link_offset + (COLLECTION_LINK_LEN + self.appendix.len()) as u32;
        put(&mut out, self.link_unknown);
        put(&mut out, end);
        out.extend_from_slice(&self.link_rest);
        out.extend_from_slice(&self.appendix);

        out[0x20..0x24].copy_from_slice(&export_offset.to_le_bytes());
        out[0x24..0x28].copy_from_slice(&(self.exports.len() as u32).to_le_bytes());
        out[0x28..0x2c].copy_from_slice(&self.unknown.to_le_bytes());
        out[0x2c..0x30].copy_from_slice(&link_offset.to_le_bytes());
        out
    }
}

fn put(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Finds a marker at a 4-byte aligned offset.
fn find_marker(data: &[u8], from: usize, marker: &[u8]) -> Option<usize> {
    let mut at = from.next_multiple_of(4);
    while at + marker.len() <= data.len() {
        if &data[at..at + marker.len()] == marker {
            return Some(at);
        }
        at += 4;
    }
    None
}

struct Reader<'a> {
    data: &'a [u8],
}

impl Reader<'_> {
    fn bytes(&self, at: usize, len: usize) -> Result<&[u8]> {
        at.checked_add(len)
            .and_then(|end| self.data.get(at..end))
            .ok_or(Error::OutOfBounds("read"))
    }

    fn u32(&self, at: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap()))
    }

    fn expect(&self, at: usize, marker: &'static [u8]) -> Result<()> {
        if self.bytes(at, marker.len())? == marker {
            Ok(())
        } else {
            Err(Error::Missing(match marker.len() {
                16 => "COLLECTION_LINK marker",
                _ if marker == GLOBAL_DATA => "GLOBAL_DATA marker",
                _ => "EXPORT_DATA marker",
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag() -> [u8; TAG_LEN] {
        let mut tag = [0u8; TAG_LEN];
        tag[..27].copy_from_slice(b"STCM2L Jan 01 2000 00:00:00");
        tag
    }

    /// Builds a small script. The action offsets follow from the layout.
    fn sample() -> Script {
        let global_data = vec![0u8; 20];
        let code = (GLOBAL_DATA_AT + 12 + global_data.len() + 12) as u32;
        let first = Action {
            offset: code,
            call: 0,
            opcode: 0x4ba,
            params: vec![Param {
                p0: code + 28,
                p1: 0x4000_0000,
                p2: 0x4000_0000,
            }],
            data: [0u32, 1, 1, 4]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .chain(*b"abc\0")
                .collect(),
        };
        let second = Action {
            offset: code + first.len() as u32,
            call: 1,
            opcode: code,
            params: vec![],
            data: vec![],
        };
        Script {
            tag: tag(),
            unknown: 1,
            reserved: [0; RESERVED_LEN],
            global_data,
            actions: vec![first, second],
            exports: vec![Export::new("main", code).unwrap()],
            link_unknown: 0,
            link_rest: [0; COLLECTION_LINK_LEN - 8],
            appendix: vec![],
        }
    }

    #[test]
    fn round_trip() {
        let script = sample();
        let bytes = script.to_bytes();
        let parsed = Script::parse(&bytes).unwrap();
        assert_eq!(parsed, script);
        assert_eq!(parsed.to_bytes(), bytes);
    }

    #[test]
    fn writer_puts_sections_at_the_header_offsets() {
        let bytes = sample().to_bytes();
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());

        let export_offset = u32_at(0x20) as usize;
        assert_eq!(&bytes[export_offset - 12..export_offset], EXPORT_DATA);
        assert_eq!(u32_at(0x24), 1);
        assert_eq!(&bytes[export_offset + 4..export_offset + 9], b"main\0");

        let link = u32_at(0x2c) as usize;
        assert_eq!(&bytes[link - 16..link], COLLECTION_LINK);
        assert_eq!(bytes.len() - link, 64);
        assert_eq!(u32_at(link + 4) as usize, bytes.len());
        // The trailer is the marker and the body: 80 bytes.
        assert_eq!(bytes.len() - (link - 16), 80);
    }

    #[test]
    fn parses_hand_made_bytes() {
        // Build the bytes by hand, so the test does not depend on the writer.
        let mut b = tag().to_vec();
        b.extend([0u8; 0x30]);
        b.extend(GLOBAL_DATA);
        b.extend(7u32.to_le_bytes());
        b.extend(CODE_START);
        let code = b.len() as u32;
        for v in [0u32, 0x6, 1, 28, 0xffff_ff41, code, 0x4000_0000] {
            b.extend(v.to_le_bytes());
        }
        b.extend(EXPORT_DATA);
        let export_offset = b.len() as u32;
        b.extend(0u32.to_le_bytes());
        let mut name = [0u8; 32];
        name[..4].copy_from_slice(b"loop");
        b.extend(name);
        b.extend(code.to_le_bytes());
        b.extend(COLLECTION_LINK);
        let link = b.len() as u32;
        b.extend(0u32.to_le_bytes());
        b.extend((link + 64).to_le_bytes());
        b.extend([0u8; 56]);
        b[0x20..0x24].copy_from_slice(&export_offset.to_le_bytes());
        b[0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
        b[0x28..0x2c].copy_from_slice(&1u32.to_le_bytes());
        b[0x2c..0x30].copy_from_slice(&link.to_le_bytes());

        let script = Script::parse(&b).unwrap();
        assert_eq!(script.tag_text(), "STCM2L Jan 01 2000 00:00:00");
        assert_eq!(script.global_data, 7u32.to_le_bytes());
        assert_eq!(script.code_offset(), code);
        assert_eq!(script.actions.len(), 1);
        let jump = &script.actions[0];
        assert_eq!((jump.offset, jump.opcode, jump.is_call()), (code, 6, false));
        assert_eq!(jump.params[0].p1, code);
        assert_eq!(script.exports[0].name(), "loop");
        assert_eq!(script.exports[0].address, code);
        assert_eq!(script.to_bytes(), b);
    }

    #[test]
    fn finds_actions_by_offset() {
        let script = sample();
        let second = script.actions[1].offset;
        assert_eq!(
            script.action_at(second).unwrap().opcode,
            script.code_offset()
        );
        assert!(script.action_at(second + 4).is_none());
        let first = &script.actions[0];
        assert_eq!(script.action_containing(first.offset + 20), Some(first));
        let block = first.data_at(first.params[0].p0).unwrap();
        assert_eq!(block.len(), 20);
        assert_eq!(&block[16..], b"abc\0");
        assert!(first.data_at(first.offset).is_none());
        assert!(script.action_containing(0).is_none());
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = sample().to_bytes();
        bytes[0] = b'X';
        assert!(matches!(Script::parse(&bytes), Err(Error::BadMagic(_))));
    }

    #[test]
    fn rejects_a_short_action_length() {
        let script = sample();
        let mut bytes = script.to_bytes();
        let at = script.code_offset() as usize + 12;
        bytes[at..at + 4].copy_from_slice(&8u32.to_le_bytes());
        assert!(matches!(Script::parse(&bytes), Err(Error::Invalid(_))));
    }

    #[test]
    fn rejects_bytes_that_the_end_offset_does_not_cover() {
        let mut bytes = sample().to_bytes();
        bytes.extend([0u8; 4]);
        assert!(matches!(Script::parse(&bytes), Err(Error::Invalid(_))));
    }

    #[test]
    fn keeps_the_appendix() {
        let mut script = sample();
        script.appendix = b"appended block\0\0".to_vec();
        let bytes = script.to_bytes();
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());

        let link = u32_at(0x2c) as usize;
        assert_eq!(u32_at(link + 4) as usize, bytes.len());
        assert_eq!(script.appendix_offset() as usize, link + 64);
        assert_eq!(&bytes[link + 64..], b"appended block\0\0");

        let parsed = Script::parse(&bytes).unwrap();
        assert_eq!(parsed, script);
        assert_eq!(parsed.to_bytes(), bytes);
    }

    #[test]
    fn finds_bytes_in_the_appendix() {
        let mut script = sample();
        script.appendix = vec![1, 2, 3, 4];
        let start = script.appendix_offset();
        assert_eq!(script.appendix_at(start + 1), Some(&[2u8, 3, 4][..]));
        assert_eq!(script.appendix_at(start + 4), None);
        assert_eq!(script.appendix_at(start - 1), None);
    }

    #[test]
    fn rejects_truncated_files() {
        let bytes = sample().to_bytes();
        for len in [0, 0x20, 0x60, bytes.len() - 70, bytes.len() - 1] {
            assert!(Script::parse(&bytes[..len]).is_err(), "length {len}");
        }
    }

    #[test]
    fn rejects_a_long_export_name() {
        assert!(Export::new(&"x".repeat(32), 0).is_err());
        assert!(Export::new(&"x".repeat(31), 0).is_ok());
    }
}
