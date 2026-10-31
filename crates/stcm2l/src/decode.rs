//! Turns an action into a typed instruction.
//!
//! `docs/opcodes.md` gives the evidence for each instruction. An opcode with
//! no verified meaning becomes `Instruction::Engine` with its arguments, so
//! the VM can pass it to the engine as it is.

use std::fmt;

use crate::operand::{DataBlock, Operand};
use crate::script::{Action, Param, Script};

/// The value of one parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
    Text(String),
    /// A data block that is not an integer, a float, or text.
    Bytes(Vec<u8>),
    /// A small immediate value.
    Immediate(u8),
    /// A global variable at a file offset.
    Global(u32),
    /// A code address.
    Code(u32),
    /// A parameter with a shape that the reader does not know.
    Raw(Param),
}

/// An operator of the built-in arithmetic opcodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    /// Division. A division by zero gives zero.
    Div,
    /// Remainder. A remainder by zero gives zero.
    Rem,
    Shl,
    /// Arithmetic shift to the right.
    Shr,
    And,
    Xor,
    Or,
}

const BINARY_OPS: [BinaryOp; 10] = [
    BinaryOp::Add,
    BinaryOp::Sub,
    BinaryOp::Mul,
    BinaryOp::Div,
    BinaryOp::Rem,
    BinaryOp::Shl,
    BinaryOp::Shr,
    BinaryOp::And,
    BinaryOp::Xor,
    BinaryOp::Or,
];

/// An operator of the comparison in `IfFalseGoto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compare {
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
}

const COMPARES: [Compare; 6] = [
    Compare::Gt,
    Compare::Lt,
    Compare::Ge,
    Compare::Le,
    Compare::Eq,
    Compare::Ne,
];

/// One typed instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// Push a return frame and jump to the target. The arguments are the
    /// params of the call, for example the two names of a sprite.
    Call {
        target: u32,
        args: Vec<Value>,
    },
    /// Pop a frame. With no frame, end the script.
    Return,
    Goto {
        target: u32,
    },
    /// If `left compare right` is false, jump to the target.
    IfFalseGoto {
        left: Value,
        compare: Compare,
        right: Value,
        target: u32,
    },
    /// `dest = left op right`.
    Compute {
        op: BinaryOp,
        dest: Value,
        left: Value,
        right: Value,
    },
    /// `dest op= value`, or `dest = value` if `op` is `None`.
    Update {
        op: Option<BinaryOp>,
        dest: Value,
        value: Value,
    },
    /// Add a line to the message.
    TextLine {
        text: String,
    },
    /// Show the message.
    ShowMessage,
    /// Play a voice. `id` is the number of the voice file.
    Voice {
        id: Value,
        rest: Vec<Value>,
    },
    /// Play a sound effect. `id` is the number of the sound file.
    SoundEffect {
        id: Value,
        rest: Vec<Value>,
    },
    /// An opcode that does nothing on the Switch.
    NoOp {
        opcode: u32,
    },
    /// An opcode that the game does not register. The VM of the game
    /// would stop on it.
    Unregistered {
        opcode: u32,
        args: Vec<Value>,
    },
    /// An opcode that the engine runs. Its meaning is not verified.
    Engine {
        opcode: u32,
        args: Vec<Value>,
    },
}

/// An action with a shape that does not agree with its opcode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    ParamCount {
        opcode: u32,
        expected: usize,
        found: usize,
    },
    /// A parameter has the wrong kind of value.
    BadParam { opcode: u32, index: usize },
    /// A built-in opcode that the VM does not have.
    UnknownBuiltin { opcode: u32 },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::ParamCount {
                opcode,
                expected,
                found,
            } => write!(f, "opcode {opcode:#x} has {found} params, not {expected}"),
            DecodeError::BadParam { opcode, index } => {
                write!(f, "opcode {opcode:#x} has a bad param {index}")
            }
            DecodeError::UnknownBuiltin { opcode } => {
                write!(f, "unknown built-in opcode {opcode:#x}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// The opcodes that do nothing on the Switch. Their handlers only return.
pub const NO_OPS: [u32; 8] = [0x467, 0x46a, 0x492, 0x493, 0x494, 0x495, 0x61a, 0x668];

/// The opcodes in the scripts that the game registers with no handler.
pub const UNREGISTERED: [u32; 3] = [0x64f, 0x69b, 0x69c];

const BUILTIN_COMPUTE: u32 = 0xffff_ff00;
const BUILTIN_UPDATE: u32 = 0xffff_ff0a;
const BUILTIN_ASSIGN: u32 = 0xffff_ff14;

/// Decodes one action.
pub fn decode(script: &Script, action: &Action) -> Result<Instruction, DecodeError> {
    if action.is_call() {
        return Ok(Instruction::Call {
            target: action.opcode,
            args: action
                .params
                .iter()
                .map(|p| value(Operand::of(script, action, p)))
                .collect(),
        });
    }
    let op = action.opcode;
    let args = || -> Vec<Value> {
        action
            .params
            .iter()
            .map(|p| value(Operand::of(script, action, p)))
            .collect()
    };
    let count = |expected: usize| -> Result<(), DecodeError> {
        if action.params.len() == expected {
            Ok(())
        } else {
            Err(DecodeError::ParamCount {
                opcode: op,
                expected,
                found: action.params.len(),
            })
        }
    };
    let arg = |index: usize| value(Operand::of(script, action, &action.params[index]));
    let code = |index: usize| match Operand::of(script, action, &action.params[index]) {
        Operand::Code(target) => Ok(target),
        _ => Err(DecodeError::BadParam { opcode: op, index }),
    };

    if op >> 8 == 0x00ff_ffff {
        return if (BUILTIN_COMPUTE..BUILTIN_UPDATE).contains(&op) {
            count(3)?;
            Ok(Instruction::Compute {
                op: BINARY_OPS[(op - BUILTIN_COMPUTE) as usize],
                dest: arg(0),
                left: arg(1),
                right: arg(2),
            })
        } else if (BUILTIN_UPDATE..=BUILTIN_ASSIGN).contains(&op) {
            count(2)?;
            let op = (op != BUILTIN_ASSIGN).then(|| BINARY_OPS[(op - BUILTIN_UPDATE) as usize]);
            Ok(Instruction::Update {
                op,
                dest: arg(0),
                value: arg(1),
            })
        } else {
            Err(DecodeError::UnknownBuiltin { opcode: op })
        };
    }

    match op {
        0x000 => {
            count(0)?;
            Ok(Instruction::Return)
        }
        0x003 => {
            count(4)?;
            let compare = match arg(1) {
                Value::Int(n) if (0..6).contains(&n) => COMPARES[n as usize],
                Value::Immediate(n) if n < 6 => COMPARES[n as usize],
                _ => {
                    return Err(DecodeError::BadParam {
                        opcode: op,
                        index: 1,
                    });
                }
            };
            Ok(Instruction::IfFalseGoto {
                left: arg(0),
                compare,
                right: arg(2),
                target: code(3)?,
            })
        }
        0x006 => {
            count(1)?;
            Ok(Instruction::Goto { target: code(0)? })
        }
        0x4ba => {
            count(1)?;
            let block = Operand::of(script, action, &action.params[0])
                .block()
                .ok_or(DecodeError::BadParam {
                    opcode: op,
                    index: 0,
                })?;
            Ok(Instruction::TextLine {
                text: text_line(&block).ok_or(DecodeError::BadParam {
                    opcode: op,
                    index: 0,
                })?,
            })
        }
        0x4bb => {
            count(0)?;
            Ok(Instruction::ShowMessage)
        }
        0x462 | 0x464 => {
            let mut all = args();
            if all.is_empty() {
                return Err(DecodeError::ParamCount {
                    opcode: op,
                    expected: 1,
                    found: 0,
                });
            }
            // The file number is always an integer, even if its bytes
            // look like text.
            let first = Operand::of(script, action, &action.params[0]).block();
            let id = match first.and_then(|b| b.as_i32()) {
                Some(v) => {
                    all.remove(0);
                    Value::Int(v)
                }
                None => all.remove(0),
            };
            Ok(if op == 0x462 {
                Instruction::Voice { id, rest: all }
            } else {
                Instruction::SoundEffect { id, rest: all }
            })
        }
        _ if NO_OPS.contains(&op) => Ok(Instruction::NoOp { opcode: op }),
        _ if UNREGISTERED.contains(&op) => Ok(Instruction::Unregistered {
            opcode: op,
            args: args(),
        }),
        _ => Ok(Instruction::Engine {
            opcode: op,
            args: args(),
        }),
    }
}

/// Gives the text of a text line. An empty block gives an empty line.
fn text_line(block: &DataBlock<'_>) -> Option<String> {
    if block.text_bytes().is_empty() {
        return Some(String::new());
    }
    block.as_text()
}

fn value(operand: Operand<'_>) -> Value {
    match operand {
        Operand::Code(target) => Value::Code(target),
        Operand::Immediate(v) => Value::Immediate(v),
        Operand::Global(offset, _) => Value::Global(offset),
        Operand::Local(_, block) | Operand::Appended(_, block) => {
            if let Some(f) = block.as_f32() {
                Value::Float(f)
            } else if block.looks_like_text()
                && let Some(text) = block.as_text()
            {
                Value::Text(text)
            } else if let Some(v) = block.as_i32() {
                Value::Int(v)
            } else {
                Value::Bytes(block.bytes.to_vec())
            }
        }
        Operand::Unknown(p) => Value::Raw(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operand::{CODE_ADDRESS, UNUSED};

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

    fn empty_script() -> Script {
        Script {
            tag: [0; 32],
            unknown: 1,
            reserved: [0; 32],
            global_data: block(&0i32.to_le_bytes()),
            actions: vec![],
            exports: vec![],
            link_unknown: 0,
            link_rest: [0; 56],
            appendix: vec![],
        }
    }

    /// Builds one action whose params point to the data blocks in order.
    /// A `None` block makes a param that refers to the global variable.
    fn action(script: &Script, opcode: u32, blocks: &[Option<Vec<u8>>]) -> Action {
        action_with(script, opcode, blocks, &[])
    }

    /// Builds one action like `action`, with more params after the blocks.
    fn action_with(
        script: &Script,
        opcode: u32,
        blocks: &[Option<Vec<u8>>],
        extra: &[Param],
    ) -> Action {
        let offset = script.code_offset();
        let data_start = offset + 16 + 12 * (blocks.len() + extra.len()) as u32;
        let mut data = Vec::new();
        let mut params = Vec::new();
        for b in blocks {
            let p0 = match b {
                Some(bytes) => {
                    let at = data_start + data.len() as u32;
                    data.extend(bytes);
                    at
                }
                None => script.global_data_offset(),
            };
            params.push(Param {
                p0,
                p1: UNUSED,
                p2: UNUSED,
            });
        }
        params.extend_from_slice(extra);
        Action {
            offset,
            call: 0,
            opcode,
            params,
            data,
        }
    }

    fn code_param(target: u32) -> Param {
        Param {
            p0: CODE_ADDRESS,
            p1: target,
            p2: UNUSED,
        }
    }

    #[test]
    fn decodes_control_flow() {
        let script = empty_script();
        let mut call = action(&script, 0x1234, &[]);
        call.call = 1;
        assert_eq!(
            decode(&script, &call),
            Ok(Instruction::Call {
                target: 0x1234,
                args: vec![],
            })
        );
        assert_eq!(
            decode(&script, &action(&script, 0, &[])),
            Ok(Instruction::Return)
        );

        let jump = action_with(&script, 6, &[], &[code_param(0x80)]);
        assert_eq!(
            decode(&script, &jump),
            Ok(Instruction::Goto { target: 0x80 })
        );
    }

    #[test]
    fn keeps_the_arguments_of_a_call() {
        let script = empty_script();
        let mut call = action(
            &script,
            0x80,
            &[Some(block(b"ayaU_03\0")), Some(block(b"ayaU_03_cho\0"))],
        );
        call.call = 1;
        assert_eq!(
            decode(&script, &call),
            Ok(Instruction::Call {
                target: 0x80,
                args: vec![
                    Value::Text("ayaU_03".into()),
                    Value::Text("ayaU_03_cho".into())
                ],
            })
        );
    }

    #[test]
    fn decodes_if_false_goto() {
        let script = empty_script();
        let a = action_with(
            &script,
            3,
            &[
                None,
                Some(block(&5i32.to_le_bytes())),
                Some(block(&2i32.to_le_bytes())),
            ],
            &[code_param(0x200)],
        );
        assert_eq!(
            decode(&script, &a),
            Ok(Instruction::IfFalseGoto {
                left: Value::Global(script.global_data_offset()),
                compare: Compare::Ne,
                right: Value::Int(2),
                target: 0x200,
            })
        );
    }

    #[test]
    fn rejects_a_bad_compare_operator() {
        let script = empty_script();
        let a = action_with(
            &script,
            3,
            &[
                None,
                Some(block(&9i32.to_le_bytes())),
                Some(block(&0i32.to_le_bytes())),
            ],
            &[code_param(0)],
        );
        assert_eq!(
            decode(&script, &a),
            Err(DecodeError::BadParam {
                opcode: 3,
                index: 1
            })
        );
    }

    #[test]
    fn decodes_builtin_arithmetic() {
        let script = empty_script();
        let one = || Some(block(&1i32.to_le_bytes()));
        let a = action(&script, 0xffff_ff02, &[None, one(), one()]);
        assert!(matches!(
            decode(&script, &a),
            Ok(Instruction::Compute {
                op: BinaryOp::Mul,
                ..
            })
        ));
        let a = action(&script, 0xffff_ff0b, &[None, one()]);
        assert!(matches!(
            decode(&script, &a),
            Ok(Instruction::Update {
                op: Some(BinaryOp::Sub),
                ..
            })
        ));
        let a = action(&script, 0xffff_ff14, &[None, one()]);
        assert_eq!(
            decode(&script, &a),
            Ok(Instruction::Update {
                op: None,
                dest: Value::Global(script.global_data_offset()),
                value: Value::Int(1),
            })
        );
        let a = action(&script, 0xffff_ff15, &[None, one()]);
        assert_eq!(
            decode(&script, &a),
            Err(DecodeError::UnknownBuiltin {
                opcode: 0xffff_ff15
            })
        );
    }

    #[test]
    fn decodes_text_and_audio() {
        let script = empty_script();
        // "Oh" fits in one word, but a text line is always text.
        let a = action(&script, 0x4ba, &[Some(block(b"Oh\0"))]);
        assert_eq!(
            decode(&script, &a),
            Ok(Instruction::TextLine { text: "Oh".into() })
        );
        let a = action(&script, 0x4ba, &[Some(block(&[0, 0, 0, 0]))]);
        assert_eq!(
            decode(&script, &a),
            Ok(Instruction::TextLine {
                text: String::new()
            })
        );
        let a = action(
            &script,
            0x462,
            &[
                Some(block(&30063i32.to_le_bytes())),
                Some(block(&0i32.to_le_bytes())),
            ],
        );
        assert_eq!(
            decode(&script, &a),
            Ok(Instruction::Voice {
                id: Value::Int(30063),
                rest: vec![Value::Int(0)],
            })
        );
    }

    #[test]
    fn a_voice_number_that_looks_like_text_is_an_integer() {
        let script = empty_script();
        let a = action(&script, 0x462, &[Some(block(&0x9fe0i32.to_le_bytes()))]);
        assert_eq!(
            decode(&script, &a),
            Ok(Instruction::Voice {
                id: Value::Int(0x9fe0),
                rest: vec![],
            })
        );
    }

    #[test]
    fn checks_the_param_count() {
        let script = empty_script();
        let a = action(&script, 0x4bb, &[None]);
        assert_eq!(
            decode(&script, &a),
            Err(DecodeError::ParamCount {
                opcode: 0x4bb,
                expected: 0,
                found: 1,
            })
        );
    }

    #[test]
    fn sorts_the_other_opcodes() {
        let script = empty_script();
        let a = action(&script, 0x492, &[]);
        assert_eq!(decode(&script, &a), Ok(Instruction::NoOp { opcode: 0x492 }));
        let a = action(&script, 0x69b, &[]);
        assert!(matches!(
            decode(&script, &a),
            Ok(Instruction::Unregistered { opcode: 0x69b, .. })
        ));
        let a = action(&script, 0x3fa, &[Some(block(b"bg_black\0"))]);
        assert_eq!(
            decode(&script, &a),
            Ok(Instruction::Engine {
                opcode: 0x3fa,
                args: vec![Value::Text("bg_black".into())],
            })
        );
    }
}
