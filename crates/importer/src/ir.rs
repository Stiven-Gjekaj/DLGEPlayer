//! Turns an STCM2L script into the intermediate representation (IR) that
//! the VM runs.
//!
//! The IR is a list of typed instructions. A code address becomes the index
//! of an instruction in the list. A global variable becomes its offset in
//! the global data. The importer writes the IR as JSON.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;
use stcm2l::Script;
use stcm2l::decode::{self, DecodeError, Instruction, Value};
use stcm2l::operand::DataBlock;

/// An error from the IR builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// An action does not decode.
    Decode { index: usize, error: DecodeError },
    /// A code address is not the start of an action.
    BadAddress { index: usize, address: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Decode { index, error } => write!(f, "instruction {index}: {error}"),
            Error::BadAddress { index, address } => {
                write!(
                    f,
                    "instruction {index}: address {address:#x} is not an action"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// One script in the IR.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScriptIr {
    /// The name of the script, for example `mb_aya_Dark_01`.
    pub name: String,
    /// The entry points by name. Each value is an instruction index.
    pub exports: BTreeMap<String, u32>,
    /// The first value of each global variable that the code uses, by its
    /// offset in the global data.
    pub globals: BTreeMap<u32, Value>,
    pub code: Vec<Instruction>,
}

/// Builds the IR of a script.
pub fn build(name: &str, script: &Script) -> Result<ScriptIr, Error> {
    let index_of: BTreeMap<u32, u32> = script
        .actions
        .iter()
        .enumerate()
        .map(|(i, a)| (a.offset, i as u32))
        .collect();
    let global_start = script.global_data_offset();
    let mut globals = BTreeMap::new();
    let mut code = Vec::with_capacity(script.actions.len());
    for (index, action) in script.actions.iter().enumerate() {
        let mut instruction =
            decode::decode(script, action).map_err(|error| Error::Decode { index, error })?;
        let target = |address: u32| {
            index_of
                .get(&address)
                .copied()
                .ok_or(Error::BadAddress { index, address })
        };
        let mut fix = |value: &mut Value| -> Result<(), Error> {
            match value {
                Value::Code(address) => *address = target(*address)?,
                Value::Global(offset) => {
                    let relative = *offset - global_start;
                    globals
                        .entry(relative)
                        .or_insert_with(|| initial_value(&script.global_data[relative as usize..]));
                    *offset = relative;
                }
                _ => {}
            }
            Ok(())
        };
        match &mut instruction {
            Instruction::Call { target: t, args } => {
                *t = target(*t)?;
                args.iter_mut().try_for_each(&mut fix)?;
            }
            Instruction::Goto { target: t } => *t = target(*t)?,
            Instruction::IfFalseGoto {
                left,
                right,
                target: t,
                ..
            } => {
                *t = target(*t)?;
                fix(left)?;
                fix(right)?;
            }
            Instruction::Compute {
                dest, left, right, ..
            } => {
                fix(dest)?;
                fix(left)?;
                fix(right)?;
            }
            Instruction::Update { dest, value, .. } => {
                fix(dest)?;
                fix(value)?;
            }
            Instruction::Voice { id, rest } | Instruction::SoundEffect { id, rest } => {
                fix(id)?;
                rest.iter_mut().try_for_each(&mut fix)?;
            }
            Instruction::Unregistered { args, .. } | Instruction::Engine { args, .. } => {
                args.iter_mut().try_for_each(&mut fix)?;
            }
            Instruction::Return
            | Instruction::TextLine { .. }
            | Instruction::ShowMessage
            | Instruction::NoOp { .. } => {}
        }
        code.push(instruction);
    }
    let mut exports = BTreeMap::new();
    for export in &script.exports {
        let index = index_of
            .get(&export.address)
            .copied()
            .ok_or(Error::BadAddress {
                index: code.len(),
                address: export.address,
            })?;
        exports.insert(export.name(), index);
    }
    Ok(ScriptIr {
        name: name.to_string(),
        exports,
        globals,
        code,
    })
}

/// Reads the first value of a global variable.
fn initial_value(data: &[u8]) -> Value {
    match DataBlock::parse(data) {
        Some(block) => match (block.as_f32(), block.as_i32()) {
            (Some(f), _) => Value::Float(f),
            (None, Some(i)) => Value::Int(i),
            _ => Value::Bytes(block.bytes.to_vec()),
        },
        None => Value::Int(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stcm2l::operand::{CODE_ADDRESS, UNUSED};
    use stcm2l::{Action, Export, Param};

    fn block(value: i32) -> Vec<u8> {
        let mut out = Vec::new();
        for v in [0u32, 1, 1, 4] {
            out.extend(v.to_le_bytes());
        }
        out.extend(value.to_le_bytes());
        out
    }

    fn action(call: u32, opcode: u32, params: Vec<Param>) -> Action {
        Action {
            offset: 0,
            call,
            opcode,
            params,
            data: vec![],
        }
    }

    fn code(address: u32) -> Param {
        Param {
            p0: CODE_ADDRESS,
            p1: address,
            p2: UNUSED,
        }
    }

    /// Builds a script, writes it, and reads it again, so that each action
    /// has its real offset.
    fn script(actions: Vec<Action>, exports: &[(&str, usize)]) -> Script {
        let mut tag = [0u8; 32];
        tag[..6].copy_from_slice(stcm2l::MAGIC);
        let mut script = Script {
            tag,
            unknown: 1,
            reserved: [0; 32],
            global_data: block(5),
            actions: vec![],
            exports: vec![],
            link_unknown: 0,
            link_rest: [0; 56],
            appendix: vec![],
        };
        let mut offsets = Vec::new();
        let mut at = script.code_offset();
        for a in &actions {
            offsets.push(at);
            at += a.len() as u32;
        }
        // An address of -1 - n in a test action means the action n.
        script.actions = actions
            .into_iter()
            .map(|mut a| {
                let resolve = |v: u32| match v {
                    v if v >= u32::MAX - 16 => offsets[(u32::MAX - v) as usize],
                    v => v,
                };
                if a.call != 0 {
                    a.opcode = resolve(a.opcode);
                }
                for p in &mut a.params {
                    if p.p0 == CODE_ADDRESS {
                        p.p1 = resolve(p.p1);
                    }
                }
                a
            })
            .collect();
        script.exports = exports
            .iter()
            .map(|(name, i)| Export::new(name, offsets[*i]).unwrap())
            .collect();
        Script::parse(&script.to_bytes()).unwrap()
    }

    const fn action_ref(n: u32) -> u32 {
        u32::MAX - n
    }

    #[test]
    fn turns_addresses_into_indices() {
        let global = Param {
            p0: 0,
            p1: UNUSED,
            p2: UNUSED,
        };
        let mut s = script(
            vec![
                action(0, 0x006, vec![code(action_ref(2))]),
                action(0, 0x4bb, vec![]),
                action(0, 0x000, vec![]),
                action(1, action_ref(1), vec![]),
                action(0, 0xffff_ff14, vec![global, global]),
            ],
            &[("sure1", 0), ("second", 3)],
        );
        // The assignment uses the global variable for both params.
        let at = s.global_data_offset();
        for p in &mut s.actions[4].params {
            p.p0 = at;
        }
        let ir = build("test", &s).unwrap();
        assert_eq!(ir.name, "test");
        assert_eq!(ir.code.len(), 5);
        assert_eq!(ir.code[0], Instruction::Goto { target: 2 });
        assert_eq!(ir.code[1], Instruction::ShowMessage);
        assert_eq!(ir.code[2], Instruction::Return);
        assert_eq!(
            ir.code[3],
            Instruction::Call {
                target: 1,
                args: vec![]
            }
        );
        assert_eq!(
            ir.code[4],
            Instruction::Update {
                op: None,
                dest: Value::Global(0),
                value: Value::Global(0)
            }
        );
        assert_eq!(ir.exports["sure1"], 0);
        assert_eq!(ir.exports["second"], 3);
        assert_eq!(ir.globals[&0], Value::Int(5));
    }

    #[test]
    fn writes_json() {
        let s = script(vec![action(0, 0x006, vec![code(action_ref(0))])], &[]);
        let json = serde_json::to_string(&build("loop", &s).unwrap()).unwrap();
        assert_eq!(
            json,
            r#"{"name":"loop","exports":{},"globals":{},"code":[{"type":"goto","target":0}]}"#
        );
    }

    #[test]
    fn refuses_an_address_inside_an_action() {
        let mut s = script(
            vec![
                action(0, 0x006, vec![code(action_ref(1))]),
                action(0, 0x000, vec![]),
            ],
            &[],
        );
        s.actions[0].params[0].p1 += 4;
        let address = s.actions[0].params[0].p1;
        assert_eq!(
            build("bad", &s),
            Err(Error::BadAddress { index: 0, address })
        );
    }
}
