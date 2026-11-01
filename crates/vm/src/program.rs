//! Loads the scripts and the tables of an import folder.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use stcm2l::decode::{Instruction, Value};

use crate::Error;

/// One script in the IR, as the importer writes it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScriptIr {
    pub name: String,
    pub exports: BTreeMap<String, u32>,
    pub globals: BTreeMap<u32, Value>,
    pub code: Vec<Instruction>,
}

/// One option of a choice, from `select.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOption {
    pub text: String,
    /// The three numbers of the row. The scripts do not read them. The
    /// menu of the game possibly uses them as points: a kind, an amount, and
    /// a character.
    pub values: [i32; 3],
}

/// The parts of `manifest.json` that the VM reads.
#[derive(Deserialize)]
struct Manifest {
    scripts: BTreeMap<String, String>,
    tables: BTreeMap<String, String>,
}

/// A `BIN_` table as the importer writes it.
#[derive(Deserialize)]
struct Table {
    rows: Vec<Vec<serde_json::Value>>,
}

/// The scripts and the tables of the game.
pub struct Program {
    dir: Option<PathBuf>,
    files: BTreeMap<String, String>,
    cache: Mutex<HashMap<String, Arc<ScriptIr>>>,
    /// The script names by the game and the number in `script.bin`. The two
    /// games use the same numbers.
    pub numbers: BTreeMap<(i32, i32), String>,
    /// The game of each script in `script.bin`.
    pub games: BTreeMap<String, i32>,
    /// The first values of the script flags, from `flag.bin`.
    pub flags: BTreeMap<String, i32>,
    /// The first values of the system flags, from `flagsys.bin`.
    pub system_flags: BTreeMap<String, i32>,
    /// The options of each choice, from `select.bin`.
    pub choices: BTreeMap<String, Vec<ChoiceOption>>,
}

fn text(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn int(v: &serde_json::Value) -> i32 {
    v.as_i64().unwrap_or_default() as i32
}

impl Program {
    /// Loads the tables of an import folder. The scripts load when the VM
    /// needs them.
    pub fn load(dir: &Path) -> Result<Program, Error> {
        let read = |file: &str| -> Result<String, Error> {
            std::fs::read_to_string(dir.join(file)).map_err(|e| Error::Load(format!("{file}: {e}")))
        };
        let manifest: Manifest = serde_json::from_str(&read("manifest.json")?)
            .map_err(|e| Error::Load(format!("manifest.json: {e}")))?;
        let table = |name: &str| -> Result<Vec<Vec<serde_json::Value>>, Error> {
            let file = manifest
                .tables
                .get(name)
                .ok_or_else(|| Error::Load(format!("the import has no table {name}")))?;
            let t: Table = serde_json::from_str(&read(file)?)
                .map_err(|e| Error::Load(format!("{file}: {e}")))?;
            Ok(t.rows)
        };
        let mut program = Program::empty();
        program.dir = Some(dir.to_path_buf());
        program.files = manifest.scripts;
        for row in table("script")? {
            if let (Some(g), Some(n), Some(name)) = (row.first(), row.get(1), row.get(2)) {
                program.numbers.insert((int(g), int(n)), text(name));
                program.games.insert(text(name), int(g));
            }
        }
        for (target, name) in [
            (&mut program.flags, "flag"),
            (&mut program.system_flags, "flagsys"),
        ] {
            for row in table(name)? {
                if let (Some(n), Some(v)) = (row.first(), row.get(1)) {
                    target.insert(text(n), int(v));
                }
            }
        }
        for row in table("select")? {
            if row.len() < 5 {
                continue;
            }
            program
                .choices
                .entry(text(&row[0]))
                .or_default()
                .push(ChoiceOption {
                    text: text(&row[1]),
                    values: [int(&row[2]), int(&row[3]), int(&row[4])],
                });
        }
        Ok(program)
    }

    /// Makes a program with no scripts and no tables.
    pub fn empty() -> Program {
        Program {
            dir: None,
            files: BTreeMap::new(),
            cache: Mutex::new(HashMap::new()),
            numbers: BTreeMap::new(),
            games: BTreeMap::new(),
            flags: BTreeMap::new(),
            system_flags: BTreeMap::new(),
            choices: BTreeMap::new(),
        }
    }

    /// Adds a script that is already in memory.
    pub fn insert(&mut self, script: ScriptIr) {
        self.files.insert(script.name.clone(), String::new());
        self.cache
            .lock()
            .unwrap()
            .insert(script.name.clone(), Arc::new(script));
    }

    /// Gives the names of all scripts.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// Gives a script by name.
    pub fn script(&self, name: &str) -> Result<Arc<ScriptIr>, Error> {
        if let Some(s) = self.cache.lock().unwrap().get(name) {
            return Ok(s.clone());
        }
        let file = self
            .files
            .get(name)
            .ok_or_else(|| Error::UnknownScript(name.to_string()))?;
        let dir = self
            .dir
            .as_ref()
            .ok_or_else(|| Error::UnknownScript(name.to_string()))?;
        let text = std::fs::read_to_string(dir.join(file))
            .map_err(|e| Error::Load(format!("{file}: {e}")))?;
        let script: ScriptIr =
            serde_json::from_str(&text).map_err(|e| Error::Load(format!("{file}: {e}")))?;
        let script = Arc::new(script);
        self.cache
            .lock()
            .unwrap()
            .insert(name.to_string(), script.clone());
        Ok(script)
    }
}
