//! Gives a meaning to the parameters of an action.
//!
//! The meanings come from the shapes in the real scripts:
//!
//! - `p0 == 0xffffff41`: an address in the code. `p1` holds the address.
//! - `p0 >> 24 == 0xff`: a small immediate value in the low byte.
//! - `p0` in the data of the same action: a data block of the action.
//! - `p0` in the global data: a global variable.
//! - `p0` in the appendix: a data block that a patch added.
//!
//! In the first three shapes, `p1` and `p2` are `0x40000000` when not used.

use crate::script::{Action, Param, Script};

pub const CODE_ADDRESS: u32 = 0xffff_ff41;
pub const UNUSED: u32 = 0x4000_0000;

/// A data block. The block has a 16-byte head: the kind, the number of
/// 4-byte words, an unknown value (1 in all known blocks), and the length
/// in bytes. The words follow the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataBlock<'a> {
    pub kind: u32,
    pub unknown: u32,
    pub len: u32,
    pub bytes: &'a [u8],
}

pub const KIND_INT_OR_TEXT: u32 = 0;
pub const KIND_FLOAT: u32 = 1;

impl<'a> DataBlock<'a> {
    /// Reads a block from the start of `data`.
    pub fn parse(data: &'a [u8]) -> Option<DataBlock<'a>> {
        let word = |i: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                data.get(i * 4..i * 4 + 4)?.try_into().ok()?,
            ))
        };
        let kind = word(0)?;
        let words = word(1)? as usize;
        let unknown = word(2)?;
        let len = word(3)?;
        let bytes = data.get(16..16 + words.checked_mul(4)?)?;
        Some(DataBlock {
            kind,
            unknown,
            len,
            bytes,
        })
    }

    /// Gives the value as an integer if the block has one word.
    pub fn as_i32(&self) -> Option<i32> {
        (self.bytes.len() == 4).then(|| i32::from_le_bytes(self.bytes.try_into().unwrap()))
    }

    /// Gives the value as a float if the block has the float kind.
    pub fn as_f32(&self) -> Option<f32> {
        if self.kind != KIND_FLOAT {
            return None;
        }
        self.as_i32().map(|v| f32::from_bits(v as u32))
    }

    /// Gives the bytes before the first zero byte.
    pub fn text_bytes(&self) -> &'a [u8] {
        let end = self
            .bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.bytes.len());
        &self.bytes[..end]
    }

    /// Decodes the block as Shift-JIS text. Gives `None` if the bytes are
    /// not valid text.
    pub fn as_text(&self) -> Option<String> {
        if self.kind != KIND_INT_OR_TEXT {
            return None;
        }
        let raw = self.text_bytes();
        if raw.is_empty() || raw.iter().any(|&b| b < 0x20 && b != b'\t' && b != b'\n') {
            return None;
        }
        // The rest of the block after the text must be zero padding.
        if self.bytes[raw.len()..].iter().any(|&b| b != 0) {
            return None;
        }
        let (text, _, had_errors) = encoding_rs::SHIFT_JIS.decode(raw);
        let bad_char = |c: char| {
            (c.is_control() && c != '\t' && c != '\n') || c == '\u{fffe}' || c == '\u{ffff}'
        };
        (!had_errors && !text.chars().any(bad_char)).then(|| text.into_owned())
    }

    /// Tells if the block is more likely text than an integer.
    ///
    /// A one-word block is text only if it starts with a Shift-JIS lead
    /// byte or holds three characters. An integer such as 30063 also reads
    /// as the two letters `ou`, and the integer 0x22 reads as `"`.
    pub fn looks_like_text(&self) -> bool {
        let Some(_) = self.as_text() else {
            return false;
        };
        let raw = self.text_bytes();
        self.bytes.len() > 4 || raw[0] >= 0x80 || raw.len() >= 3
    }
}

/// The meaning of one parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operand<'a> {
    /// An address in the code.
    Code(u32),
    /// A small immediate value.
    Immediate(u8),
    /// A data block of the action. The offset is the file offset.
    Local(u32, DataBlock<'a>),
    /// A global variable. The offset is the file offset.
    Global(u32, Option<DataBlock<'a>>),
    /// A data block in the appendix. The offset is the file offset.
    Appended(u32, DataBlock<'a>),
    /// A shape that the reader does not know.
    Unknown(Param),
}

impl<'a> Operand<'a> {
    pub fn of(script: &'a Script, action: &'a Action, param: &Param) -> Operand<'a> {
        let p0 = param.p0;
        if p0 == CODE_ADDRESS {
            return Operand::Code(param.p1);
        }
        if p0 >> 24 == 0xff && p0 >> 8 == 0x00ff_ffff {
            return Operand::Immediate(p0 as u8);
        }
        if let Some(data) = action.data_at(p0)
            && let Some(block) = DataBlock::parse(data)
        {
            return Operand::Local(p0, block);
        }
        let global_start = script.global_data_offset();
        if p0 >= global_start && p0 < global_start + script.global_data.len() as u32 {
            let block = DataBlock::parse(&script.global_data[(p0 - global_start) as usize..]);
            return Operand::Global(p0, block);
        }
        if let Some(data) = script.appendix_at(p0)
            && let Some(block) = DataBlock::parse(data)
        {
            return Operand::Appended(p0, block);
        }
        Operand::Unknown(*param)
    }

    /// Gives the data block of a parameter that points to one in the
    /// action or in the appendix.
    pub fn block(&self) -> Option<DataBlock<'a>> {
        match self {
            Operand::Local(_, block) | Operand::Appended(_, block) => Some(*block),
            _ => None,
        }
    }

    /// Tells if `p1` and `p2` hold values that the operand does not explain.
    pub fn has_extra(&self, param: &Param) -> bool {
        match self {
            Operand::Code(_) => param.p2 != UNUSED,
            Operand::Unknown(_) => false,
            _ => param.p1 != UNUSED || param.p2 != UNUSED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(kind: u32, payload: &[u8]) -> Vec<u8> {
        let words = payload.len().div_ceil(4) as u32;
        let mut out = Vec::new();
        for v in [kind, words, 1, words * 4] {
            out.extend(v.to_le_bytes());
        }
        out.extend(payload);
        out.resize(16 + words as usize * 4, 0);
        out
    }

    #[test]
    fn reads_an_integer_block() {
        let bytes = block(0, &(-200i32).to_le_bytes());
        let b = DataBlock::parse(&bytes).unwrap();
        assert_eq!(b.as_i32(), Some(-200));
        assert!(!b.looks_like_text());
    }

    #[test]
    fn reads_a_float_block() {
        let bytes = block(1, &1.5f32.to_bits().to_le_bytes());
        let b = DataBlock::parse(&bytes).unwrap();
        assert_eq!(b.as_f32(), Some(1.5));
        assert_eq!(b.as_text(), None);
    }

    #[test]
    fn decodes_shift_jis_text() {
        // "うう……" in Shift-JIS.
        let sjis = [0x82, 0xa4, 0x82, 0xa4, 0x81, 0x63, 0x81, 0x63, 0];
        let bytes = block(0, &sjis);
        let b = DataBlock::parse(&bytes).unwrap();
        assert_eq!(b.as_text().as_deref(), Some("うう……"));
        assert!(b.looks_like_text());
    }

    #[test]
    fn a_small_integer_is_not_text() {
        let bytes = block(0, &0x22u32.to_le_bytes());
        let b = DataBlock::parse(&bytes).unwrap();
        assert_eq!(b.as_text().as_deref(), Some("\""));
        assert!(!b.looks_like_text());
    }

    #[test]
    fn high_integers_are_not_text() {
        for value in [0x80u32, 0xffff_ffff, 0x9f] {
            let bytes = block(0, &value.to_le_bytes());
            let b = DataBlock::parse(&bytes).unwrap();
            assert!(!b.looks_like_text(), "{value:#x}");
        }
    }

    #[test]
    fn one_double_byte_character_is_text() {
        // "あ" in Shift-JIS.
        let bytes = block(0, &[0x82, 0xa0, 0, 0]);
        assert!(DataBlock::parse(&bytes).unwrap().looks_like_text());
    }

    #[test]
    fn two_letters_in_one_word_are_an_integer() {
        let bytes = block(0, &30063u32.to_le_bytes());
        let b = DataBlock::parse(&bytes).unwrap();
        assert_eq!(b.as_text().as_deref(), Some("ou"));
        assert!(!b.looks_like_text());
        let bytes = block(0, b"abc\0");
        assert!(DataBlock::parse(&bytes).unwrap().looks_like_text());
    }

    #[test]
    fn ascii_text_is_text() {
        let bytes = block(0, b"Hello.\0");
        assert!(DataBlock::parse(&bytes).unwrap().looks_like_text());
    }

    #[test]
    fn text_with_bytes_after_the_end_is_not_text() {
        let bytes = block(0, b"ab\0x");
        assert_eq!(DataBlock::parse(&bytes).unwrap().as_text(), None);
    }

    #[test]
    fn rejects_a_short_block() {
        let bytes = block(0, b"abcdefgh");
        assert!(DataBlock::parse(&bytes[..20]).is_none());
    }

    #[test]
    fn finds_a_block_in_the_appendix() {
        let mut script = Script {
            tag: [0; 32],
            unknown: 1,
            reserved: [0; 32],
            global_data: vec![],
            actions: vec![],
            exports: vec![],
            link_unknown: 0,
            link_rest: [0; 56],
            appendix: vec![],
        };
        let at = script.code_offset();
        script.actions.push(Action {
            offset: at,
            call: 0,
            opcode: 0x4ba,
            params: vec![Param {
                p0: 0,
                p1: UNUSED,
                p2: UNUSED,
            }],
            data: vec![],
        });
        script.appendix = block(0, b"Hello.\0");
        script.actions[0].params[0].p0 = script.appendix_offset();

        let action = &script.actions[0];
        let op = Operand::of(&script, action, &action.params[0]);
        assert!(matches!(op, Operand::Appended(o, _) if o == script.appendix_offset()));
        assert_eq!(
            op.block().and_then(|b| b.as_text()).as_deref(),
            Some("Hello.")
        );
    }

    #[test]
    fn classifies_parameters() {
        let mut global = Vec::new();
        global.extend(block(0, &5i32.to_le_bytes()));
        let mut script = Script {
            tag: [0; 32],
            unknown: 1,
            reserved: [0; 32],
            global_data: global,
            actions: vec![],
            exports: vec![],
            link_unknown: 0,
            link_rest: [0; 56],
            appendix: vec![],
        };
        let g = script.global_data_offset();
        let at = script.code_offset();
        let p = |p0, p1| Param { p0, p1, p2: UNUSED };
        let action = Action {
            offset: at,
            call: 0,
            opcode: 3,
            params: vec![
                p(CODE_ADDRESS, 0x1234),
                p(0xffff_ff20, UNUSED),
                p(at + 16 + 5 * 12, UNUSED),
                p(g, UNUSED),
                p(7, 8),
            ],
            data: block(0, &9i32.to_le_bytes()),
        };
        script.actions.push(action);
        let action = &script.actions[0];
        let ops: Vec<_> = action
            .params
            .iter()
            .map(|param| Operand::of(&script, action, param))
            .collect();

        assert_eq!(ops[0], Operand::Code(0x1234));
        assert_eq!(ops[1], Operand::Immediate(0x20));
        assert!(matches!(ops[2], Operand::Local(_, b) if b.as_i32() == Some(9)));
        assert!(matches!(ops[3], Operand::Global(o, Some(b)) if o == g && b.as_i32() == Some(5)));
        assert!(matches!(ops[4], Operand::Unknown(_)));
        assert_eq!(ops[2].block().and_then(|b| b.as_i32()), Some(9));
        assert_eq!(ops[1].block(), None);
        assert!(!ops[1].has_extra(&action.params[1]));
        assert!(ops[2].has_extra(&p(at + 16 + 5 * 12, 1)));
    }
}
