//! Runs the IR of one script from a label to the return at the top level.
//!
//! The VM of the game runs a whole part of a script in one frame. Most
//! opcodes do not act at once: they queue a command for the scene engine.
//! The machine does the same. It gives the queued commands to the session,
//! which plays them. A flag read is immediate, so it sees the flags from
//! before the commands of the same run.
//!
//! An operand `#n` with `n` below 0x20 is argument `n` of the current call.
//! An argument is a reference: a write to it writes to the place that the
//! caller gave. `#0x20` to `#0x2f` are registers.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use stcm2l::decode::{BinaryOp, Compare, Instruction, Value};

use crate::Error;
use crate::program::ScriptIr;

/// The number of registers, `#0x20` to `#0x2f`.
const REGISTERS: usize = 16;
/// The largest number of instructions in one run. A larger run is a loop.
const STEP_LIMIT: u64 = 10_000_000;

/// A value at run time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Val {
    Int(i32),
    Float(f32),
    Text(String),
    Bytes(Vec<u8>),
    /// A code address, as an instruction index.
    Code(u32),
}

impl Val {
    pub fn as_int(&self) -> i32 {
        match self {
            Val::Int(v) => *v,
            Val::Float(f) => *f as i32,
            Val::Code(c) => *c as i32,
            Val::Text(_) | Val::Bytes(_) => 0,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Val::Text(t) => Some(t),
            _ => None,
        }
    }

    fn from_ir(v: &Value) -> Option<Val> {
        Some(match v {
            Value::Int(i) => Val::Int(*i),
            Value::Float(f) => Val::Float(*f),
            Value::Text(t) => Val::Text(t.clone()),
            Value::Bytes(b) => Val::Bytes(b.clone()),
            Value::Code(c) => Val::Code(*c),
            Value::Immediate(_) | Value::Global(_) | Value::Raw(_) => return None,
        })
    }
}

/// A place that an operand names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Place {
    Global(u32),
    Register(usize),
    /// A value in the code. A write to it changes nothing.
    Const(Val),
}

/// A command for the scene engine. The session plays them in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Show a message with its lines.
    Message(Vec<String>),
    Voice {
        id: i32,
        args: Vec<Val>,
    },
    SoundEffect {
        id: i32,
        args: Vec<Val>,
    },
    /// Set a script flag (`0x41b`).
    SetFlag {
        name: String,
        value: i32,
    },
    /// Add to a script flag (`0x41c`).
    AddFlag {
        name: String,
        value: i32,
    },
    /// Set a system flag, for example the end of a chapter (`0x425`).
    SetSystemFlag {
        name: String,
        value: i32,
    },
    /// Show a choice from `select.bin` (`0x599`). The pick goes to the flag
    /// `select`.
    Choice {
        name: String,
    },
    /// Run a label of this script after this run (`0x44e`).
    JumpLabel(String),
    /// Run a label of another script after this run (`0x44f`).
    JumpScript {
        number: i32,
        label: String,
    },
    /// End the chapter (`0x700`).
    End,
    /// An opcode with no handler in the game. The game stops on it.
    Unregistered {
        opcode: u32,
        args: Vec<Val>,
    },
    /// Another opcode, with its arguments.
    Engine {
        opcode: u32,
        args: Vec<Val>,
    },
}

/// One call frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Frame {
    return_to: u32,
    args: Vec<Place>,
}

/// The flags that a run reads.
pub trait FlagSource {
    fn flag(&self, name: &str) -> i32;
    fn system_flag(&self, name: &str) -> i32;
}

/// The state of the machine that a save keeps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineState {
    pub globals: BTreeMap<u32, Val>,
    pub registers: Vec<Val>,
}

/// The machine for one loaded script.
pub struct Machine {
    pub script: Arc<ScriptIr>,
    pub state: MachineState,
    pc: u32,
    frames: Vec<Frame>,
    lines: Vec<String>,
    /// The index of each instruction that ran, for the coverage.
    pub visited: Vec<bool>,
}

impl Machine {
    /// Loads a script. The globals get their first values.
    pub fn new(script: Arc<ScriptIr>) -> Machine {
        let globals = script
            .globals
            .iter()
            .map(|(k, v)| (*k, Val::from_ir(v).unwrap_or(Val::Int(0))))
            .collect();
        let len = script.code.len();
        Machine {
            script,
            state: MachineState {
                globals,
                registers: vec![Val::Int(0); REGISTERS],
            },
            pc: 0,
            frames: Vec::new(),
            lines: Vec::new(),
            visited: vec![false; len],
        }
    }

    /// Runs a label to the return at the top level. Gives the queued
    /// commands.
    pub fn run(&mut self, label: &str, flags: &dyn FlagSource) -> Result<Vec<Command>, Error> {
        self.pc = *self
            .script
            .exports
            .get(label)
            .ok_or_else(|| Error::UnknownLabel(self.script.name.clone(), label.to_string()))?;
        self.frames.clear();
        let mut out = Vec::new();
        let script = self.script.clone();
        for _ in 0..STEP_LIMIT {
            let index = self.pc;
            let instruction = script
                .code
                .get(index as usize)
                .ok_or(Error::BadAddress(script.name.clone(), index))?;
            self.visited[index as usize] = true;
            self.pc += 1;
            if self.step(instruction, flags, &mut out)? {
                return Ok(out);
            }
        }
        Err(Error::Loop(script.name.clone(), label.to_string()))
    }

    fn err(&self, what: &str) -> Error {
        Error::Bad(
            self.script.name.clone(),
            self.pc.saturating_sub(1),
            what.to_string(),
        )
    }

    fn place(&self, v: &Value) -> Result<Place, Error> {
        if let Some(c) = Val::from_ir(v) {
            return Ok(Place::Const(c));
        }
        match v {
            Value::Global(o) => Ok(Place::Global(*o)),
            Value::Immediate(n) if (*n as usize) < 0x20 => self
                .frames
                .last()
                .and_then(|f| f.args.get(*n as usize))
                .cloned()
                .ok_or_else(|| self.err(&format!("argument {n} with no call"))),
            Value::Immediate(n) if (*n as usize) < 0x20 + REGISTERS => {
                Ok(Place::Register(*n as usize - 0x20))
            }
            _ => Err(self.err(&format!("operand {v:?}"))),
        }
    }

    fn read(&self, p: &Place) -> Val {
        match p {
            Place::Global(o) => self.state.globals.get(o).cloned().unwrap_or(Val::Int(0)),
            Place::Register(r) => self.state.registers[*r].clone(),
            Place::Const(v) => v.clone(),
        }
    }

    fn get(&self, v: &Value) -> Result<Val, Error> {
        Ok(self.read(&self.place(v)?))
    }

    fn write(&mut self, p: &Place, v: Val) {
        match p {
            Place::Global(o) => {
                self.state.globals.insert(*o, v);
            }
            Place::Register(r) => self.state.registers[*r] = v,
            Place::Const(_) => {}
        }
    }

    fn args(&self, args: &[Value]) -> Result<Vec<Val>, Error> {
        args.iter().map(|a| self.get(a)).collect()
    }

    fn text_arg(&self, args: &[Value], i: usize) -> Result<String, Error> {
        let v = self.get(args.get(i).ok_or_else(|| self.err("missing argument"))?)?;
        v.as_text()
            .map(str::to_string)
            .ok_or_else(|| self.err(&format!("argument {i} is not a name")))
    }

    fn int_arg(&self, args: &[Value], i: usize) -> Result<i32, Error> {
        Ok(self
            .get(args.get(i).ok_or_else(|| self.err("missing argument"))?)?
            .as_int())
    }

    /// Runs one instruction. Gives true at the return of the top level.
    fn step(
        &mut self,
        instruction: &Instruction,
        flags: &dyn FlagSource,
        out: &mut Vec<Command>,
    ) -> Result<bool, Error> {
        match instruction {
            Instruction::Call { target, args } => {
                let args = args
                    .iter()
                    .map(|a| self.place(a))
                    .collect::<Result<_, _>>()?;
                self.frames.push(Frame {
                    return_to: self.pc,
                    args,
                });
                self.pc = *target;
            }
            Instruction::Return => match self.frames.pop() {
                Some(f) => self.pc = f.return_to,
                None => return Ok(true),
            },
            Instruction::Goto { target } => self.pc = *target,
            Instruction::IfFalseGoto {
                left,
                compare,
                right,
                target,
            } => {
                if !compare_values(*compare, &self.get(left)?, &self.get(right)?) {
                    self.pc = *target;
                }
            }
            Instruction::Compute {
                op,
                dest,
                left,
                right,
            } => {
                let v = arithmetic(*op, &self.get(left)?, &self.get(right)?);
                let d = self.place(dest)?;
                self.write(&d, v);
            }
            Instruction::Update { op, dest, value } => {
                let d = self.place(dest)?;
                let v = self.get(value)?;
                let new = match op {
                    None => v,
                    Some(op) => arithmetic(*op, &self.read(&d), &v),
                };
                self.write(&d, new);
            }
            Instruction::TextLine { text } => self.lines.push(text.clone()),
            Instruction::ShowMessage => {
                if !self.lines.is_empty() {
                    out.push(Command::Message(std::mem::take(&mut self.lines)));
                }
            }
            Instruction::Voice { id, rest } => out.push(Command::Voice {
                id: self.get(id)?.as_int(),
                args: self.args(rest)?,
            }),
            Instruction::SoundEffect { id, rest } => out.push(Command::SoundEffect {
                id: self.get(id)?.as_int(),
                args: self.args(rest)?,
            }),
            Instruction::NoOp { .. } => {}
            Instruction::Unregistered { opcode, args } => out.push(Command::Unregistered {
                opcode: *opcode,
                args: self.args(args)?,
            }),
            Instruction::Engine { opcode, args } => self.engine(*opcode, args, flags, out)?,
        }
        Ok(false)
    }

    fn engine(
        &mut self,
        opcode: u32,
        args: &[Value],
        flags: &dyn FlagSource,
        out: &mut Vec<Command>,
    ) -> Result<(), Error> {
        match opcode {
            0x41b => out.push(Command::SetFlag {
                name: self.text_arg(args, 0)?,
                value: self.int_arg(args, 1)?,
            }),
            0x41c => out.push(Command::AddFlag {
                name: self.text_arg(args, 0)?,
                value: self.int_arg(args, 1)?,
            }),
            0x425 => out.push(Command::SetSystemFlag {
                name: self.text_arg(args, 0)?,
                value: self.int_arg(args, 1)?,
            }),
            0x41d | 0x427 => {
                let name = self.text_arg(args, 0)?;
                let dest = self.place(args.get(1).ok_or_else(|| self.err("missing argument"))?)?;
                let value = if opcode == 0x41d {
                    flags.flag(&name)
                } else {
                    flags.system_flag(&name)
                };
                self.write(&dest, Val::Int(value));
            }
            0x44e => out.push(Command::JumpLabel(self.text_arg(args, 0)?)),
            0x44f => out.push(Command::JumpScript {
                number: self.int_arg(args, 0)?,
                label: self.text_arg(args, 1)?,
            }),
            0x599 => out.push(Command::Choice {
                name: self.text_arg(args, 1)?,
            }),
            0x700 => out.push(Command::End),
            _ => out.push(Command::Engine {
                opcode,
                args: self.args(args)?,
            }),
        }
        Ok(())
    }
}

fn compare_values(c: Compare, a: &Val, b: &Val) -> bool {
    let ord = match (a, b) {
        (Val::Float(_), _) | (_, Val::Float(_)) => {
            let (x, y) = (as_float(a), as_float(b));
            x.partial_cmp(&y)
        }
        (Val::Text(x), Val::Text(y)) => Some(x.cmp(y)),
        _ => Some(a.as_int().cmp(&b.as_int())),
    };
    let Some(ord) = ord else {
        return c == Compare::Ne;
    };
    match c {
        Compare::Gt => ord.is_gt(),
        Compare::Lt => ord.is_lt(),
        Compare::Ge => ord.is_ge(),
        Compare::Le => ord.is_le(),
        Compare::Eq => ord.is_eq(),
        Compare::Ne => ord.is_ne(),
    }
}

fn as_float(v: &Val) -> f32 {
    match v {
        Val::Float(f) => *f,
        other => other.as_int() as f32,
    }
}

/// Does an operator. The float form has only add, subtract, multiply, and
/// divide. A division or a remainder by zero gives zero.
fn arithmetic(op: BinaryOp, a: &Val, b: &Val) -> Val {
    let float = matches!(a, Val::Float(_)) || matches!(b, Val::Float(_));
    if float
        && matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
        )
    {
        let (x, y) = (as_float(a), as_float(b));
        return Val::Float(match op {
            BinaryOp::Add => x + y,
            BinaryOp::Sub => x - y,
            BinaryOp::Mul => x * y,
            _ if y == 0.0 => 0.0,
            _ => x / y,
        });
    }
    let (x, y) = (a.as_int(), b.as_int());
    Val::Int(match op {
        BinaryOp::Add => x.wrapping_add(y),
        BinaryOp::Sub => x.wrapping_sub(y),
        BinaryOp::Mul => x.wrapping_mul(y),
        BinaryOp::Div => x.checked_div(y).unwrap_or(0),
        BinaryOp::Rem => x.checked_rem(y).unwrap_or(0),
        BinaryOp::Shl => x.wrapping_shl(y as u32),
        BinaryOp::Shr => x.wrapping_shr(y as u32),
        BinaryOp::And => x & y,
        BinaryOp::Xor => x ^ y,
        BinaryOp::Or => x | y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_arithmetic() {
        let i = Val::Int;
        assert_eq!(arithmetic(BinaryOp::Add, &i(2), &i(3)), i(5));
        assert_eq!(arithmetic(BinaryOp::Div, &i(7), &i(0)), i(0));
        assert_eq!(arithmetic(BinaryOp::Rem, &i(7), &i(0)), i(0));
        assert_eq!(arithmetic(BinaryOp::Shr, &i(-8), &i(1)), i(-4));
        assert_eq!(
            arithmetic(BinaryOp::Mul, &Val::Float(1.5), &i(2)),
            Val::Float(3.0)
        );
        assert_eq!(arithmetic(BinaryOp::Or, &Val::Float(1.5), &i(2)), i(3));
        assert_eq!(arithmetic(BinaryOp::Add, &i(i32::MAX), &i(1)), i(i32::MIN));
    }

    #[test]
    fn compares() {
        let i = Val::Int;
        assert!(compare_values(Compare::Eq, &i(4), &i(4)));
        assert!(compare_values(Compare::Ne, &i(4), &i(5)));
        assert!(compare_values(Compare::Gt, &Val::Float(2.5), &i(2)));
        assert!(compare_values(Compare::Le, &i(2), &i(2)));
        assert!(!compare_values(Compare::Lt, &i(2), &i(2)));
    }
}
