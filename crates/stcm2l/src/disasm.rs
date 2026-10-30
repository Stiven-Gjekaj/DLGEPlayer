//! Writes a script as a text listing.
//!
//! Each action is one line: the file offset, the opcode, and the operands.
//! A line with a label comes before each action that an export, a jump,
//! or a call points to.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::operand::{DataBlock, Operand};
use crate::script::{Action, Script};

/// Writes the full listing of a script.
pub fn disassemble(script: &Script) -> String {
    let labels = labels(script);
    let mut out = String::new();
    let _ = writeln!(out, "; tag: {}", script.tag_text());
    let _ = writeln!(
        out,
        "; {} actions, {} exports, {} bytes of global data",
        script.actions.len(),
        script.exports.len(),
        script.global_data.len()
    );
    for e in &script.exports {
        let _ = writeln!(out, ".export {} {:#010x}", e.name(), e.address);
    }
    for action in &script.actions {
        if let Some(name) = labels.get(&action.offset) {
            let _ = writeln!(out, "\n{name}:");
        }
        let _ = writeln!(out, "{}", line(script, action, &labels));
    }
    out
}

/// Writes only the text of a script: one line for each data block that
/// holds text, with the offset of its action.
pub fn strings(script: &Script) -> String {
    let mut out = String::new();
    for action in &script.actions {
        for param in &action.params {
            if let Operand::Local(_, block) = Operand::of(script, action, param)
                && block.looks_like_text()
                && let Some(text) = block.as_text()
            {
                let _ = writeln!(out, "{:08x}\t{}", action.offset, escape(&text));
            }
        }
    }
    out
}

/// Gives a name to each offset that the code points to.
pub fn labels(script: &Script) -> BTreeMap<u32, String> {
    let mut labels = BTreeMap::new();
    for action in &script.actions {
        if action.is_call() {
            labels.insert(action.opcode, format!("L_{:08x}", action.opcode));
        }
        for param in &action.params {
            if let Operand::Code(target) = Operand::of(script, action, param) {
                labels.insert(target, format!("L_{target:08x}"));
            }
        }
    }
    // An export name replaces the plain label.
    for e in &script.exports {
        labels.insert(e.address, e.name());
    }
    labels
}

fn line(script: &Script, action: &Action, labels: &BTreeMap<u32, String>) -> String {
    let mut out = format!("  {:08x}  ", action.offset);
    if action.is_call() {
        let target = labels
            .get(&action.opcode)
            .cloned()
            .unwrap_or_else(|| format!("{:#x}", action.opcode));
        let _ = write!(out, "call {target}");
        if action.call != 1 {
            let _ = write!(out, " [flag {:#x}]", action.call);
        }
    } else {
        let _ = write!(out, "{}", mnemonic(action.opcode));
    }

    let operands: Vec<String> = action
        .params
        .iter()
        .map(|param| {
            let operand = Operand::of(script, action, param);
            let mut text = format_operand(&operand, labels);
            if operand.has_extra(param) {
                let _ = write!(text, "{{{:#x},{:#x}}}", param.p1, param.p2);
            }
            text
        })
        .collect();
    if !operands.is_empty() {
        let _ = write!(out, " {}", operands.join(", "));
    }

    let unused = unused_data(script, action);
    if unused > 0 {
        let _ = write!(out, "  ; {unused} more data bytes");
    }
    out
}

/// Gives a name for an opcode. The names of the opcodes are not known yet.
pub fn mnemonic(opcode: u32) -> String {
    if opcode >> 24 == 0xff {
        format!("op_ff{:02x}", opcode as u8)
    } else {
        format!("op_{opcode:03x}")
    }
}

fn format_operand(operand: &Operand<'_>, labels: &BTreeMap<u32, String>) -> String {
    match operand {
        Operand::Code(target) => format!(
            "->{}",
            labels
                .get(target)
                .cloned()
                .unwrap_or_else(|| format!("{target:#x}"))
        ),
        Operand::Immediate(v) => format!("#{v:#x}"),
        Operand::Local(_, block) => format_block(block),
        Operand::Global(offset, block) => {
            let value = block.as_ref().map(format_block).unwrap_or_default();
            format!("g{offset:#x}={value}")
        }
        Operand::Unknown(p) => format!("?({:#x},{:#x},{:#x})", p.p0, p.p1, p.p2),
    }
}

fn format_block(block: &DataBlock<'_>) -> String {
    if let Some(f) = block.as_f32() {
        return format!("{f:?}f");
    }
    if block.looks_like_text()
        && let Some(text) = block.as_text()
    {
        return format!("\"{}\"", escape(&text));
    }
    if let Some(v) = block.as_i32() {
        return v.to_string();
    }
    let hex: Vec<String> = block.bytes.chunks(4).map(hex_word).collect();
    format!("[{}]", hex.join(" "))
}

fn hex_word(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

/// Counts the data bytes that no parameter of the action points to.
fn unused_data(script: &Script, action: &Action) -> usize {
    let mut used = vec![false; action.data.len()];
    let start = action.data_offset();
    for param in &action.params {
        if let Operand::Local(offset, block) = Operand::of(script, action, param) {
            let from = (offset - start) as usize;
            let to = (from + 16 + block.bytes.len()).min(used.len());
            used[from..to].iter_mut().for_each(|u| *u = true);
        }
    }
    used.iter().filter(|u| !**u).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operand::{CODE_ADDRESS, UNUSED};
    use crate::script::{Export, Param};

    fn block(payload: &[u8]) -> Vec<u8> {
        let words = payload.len().div_ceil(4) as u32;
        let mut out = Vec::new();
        for v in [0, words, 1, words * 4] {
            out.extend(v.to_le_bytes());
        }
        out.extend(payload);
        out.resize(16 + words as usize * 4, 0);
        out
    }

    fn sample() -> Script {
        let mut script = Script {
            tag: *b"STCM2L test\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            unknown: 1,
            reserved: [0; 32],
            global_data: vec![],
            actions: vec![],
            exports: vec![],
            link_unknown: 0,
            link_rest: [0; 56],
            appendix: vec![],
        };
        let code = script.code_offset();
        // "んー" in Shift-JIS.
        let text = block(&[0x82, 0xf1, 0x81, 0x5b, 0]);
        let say = Action {
            offset: code,
            call: 0,
            opcode: 0x4ba,
            params: vec![Param {
                p0: code + 28,
                p1: UNUSED,
                p2: UNUSED,
            }],
            data: text,
        };
        let jump = Action {
            offset: code + say.len() as u32,
            call: 0,
            opcode: 6,
            params: vec![Param {
                p0: CODE_ADDRESS,
                p1: code,
                p2: UNUSED,
            }],
            data: vec![0; 12],
        };
        let call = Action {
            offset: jump.offset + jump.len() as u32,
            call: 1,
            opcode: jump.offset,
            params: vec![],
            data: vec![],
        };
        script.exports.push(Export::new("start", code).unwrap());
        script.actions = vec![say, jump, call];
        script
    }

    #[test]
    fn listing_shows_labels_operands_and_text() {
        let listing = disassemble(&sample());
        let code = sample().code_offset();
        let jump = sample().actions[1].offset;
        assert!(listing.contains("; tag: STCM2L test"));
        assert!(listing.contains(&format!(".export start {code:#010x}")));
        assert!(listing.contains("\nstart:\n"));
        assert!(listing.contains(&format!("{code:08x}  op_4ba \"んー\"")));
        assert!(listing.contains(&format!("L_{jump:08x}:")));
        assert!(listing.contains("op_006 ->start  ; 12 more data bytes"));
        assert!(listing.contains(&format!("call L_{jump:08x}")));
    }

    #[test]
    fn strings_lists_only_text() {
        let script = sample();
        let out = strings(&script);
        assert_eq!(out, format!("{:08x}\tんー\n", script.code_offset()));
    }

    #[test]
    fn mnemonics_for_high_opcodes_are_short() {
        assert_eq!(mnemonic(0xffff_ff14), "op_ff14");
        assert_eq!(mnemonic(0x4ba), "op_4ba");
    }

    #[test]
    fn escapes_quotes_and_line_breaks() {
        assert_eq!(escape("a\"b\nc"), "a\\\"b\\nc");
    }
}
