//! Runs the importer on a thread for the first start of the player.
//!
//! The player gives the NSP file and the `prod.keys` file of the user, and
//! the patch file of the English translation. The importer writes the
//! import folder. After that, the player needs neither file.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use importer::import::{Options, Root, Source};

/// What the thread of the import shows to the screen.
#[derive(Debug, Default)]
pub struct Progress {
    pub lines: Vec<String>,
    pub done: bool,
    pub error: Option<String>,
}

/// The settings of one import.
pub struct Job {
    /// The NSP file, or a RomFS folder.
    pub input: PathBuf,
    /// The `prod.keys` file. A RomFS folder does not need it.
    pub keys: Option<PathBuf>,
    pub out: PathBuf,
    /// The bytes of a patch file, for example the English patch.
    pub translation: Option<Vec<u8>>,
    pub options: Options,
}

fn open(input: &Path, keys: Option<&Path>) -> Result<Root, String> {
    if input.is_dir() {
        return Ok(Root::Dir(input.to_path_buf()));
    }
    let keys = keys.ok_or("an NSP file needs the prod.keys file of your console")?;
    let text = std::fs::read_to_string(keys).map_err(|e| format!("{}: {e}", keys.display()))?;
    let keys = nsp::Keys::parse(&text);
    let file = std::fs::File::open(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let game =
        nsp::Game::open(Arc::new(file), &keys).map_err(|e| format!("{}: {e}", input.display()))?;
    Ok(Root::RomFs(Arc::new(game.romfs)))
}

/// Runs an import. Each line of progress goes to `progress`.
pub fn run(job: Job, progress: Arc<Mutex<Progress>>) {
    let say = |line: String| {
        if let Ok(mut p) = progress.lock() {
            p.lines.push(line);
        }
    };
    let result = (|| -> Result<usize, String> {
        say("reading the game".into());
        let root = open(&job.input, job.keys.as_deref())?;
        let translation = match &job.translation {
            Some(bytes) => {
                Some(importer::patch::PatchSet::parse(bytes).map_err(|e| e.to_string())?)
            }
            None => None,
        };
        let source = Source {
            roots: vec![root],
            translation,
        };
        let manifest = importer::import::run(&source, &job.out, &job.options, &|line| {
            say(line.to_string())
        })
        .map_err(|e| e.to_string())?;
        Ok(manifest.failures.len())
    })();
    if let Ok(mut p) = progress.lock() {
        match result {
            Ok(0) => {}
            Ok(failures) => p.error = Some(format!("{failures} files did not convert")),
            Err(e) => p.error = Some(e),
        }
        p.done = true;
    }
}
