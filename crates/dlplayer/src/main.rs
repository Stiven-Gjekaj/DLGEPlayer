//! The command line tool for the importer and the debug tools.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dlplayer", about = "Tools for the Diabolik Lovers player")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Work with CRI CPK archives.
    Cpk {
        #[command(subcommand)]
        command: CpkCommand,
    },
}

#[derive(Subcommand)]
enum CpkCommand {
    /// Print the files in an archive.
    List { archive: PathBuf },
    /// Write all files of an archive to a directory.
    Extract { archive: PathBuf, out: PathBuf },
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Cpk { command } => match command {
            CpkCommand::List { archive } => cpk_list(&archive),
            CpkCommand::Extract { archive, out } => cpk_extract(&archive, &out),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn open_cpk(path: &Path) -> Result<cpk::Archive<BufReader<File>>> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(cpk::Archive::open(BufReader::new(file))?)
}

fn cpk_list(path: &Path) -> Result<()> {
    let archive = open_cpk(path)?;
    for entry in &archive.entries {
        let id = entry.id.map(|i| i.to_string()).unwrap_or_default();
        println!(
            "{id:>6} {:>12} {:>12} {:>12x} {}",
            entry.size,
            entry.extract_size,
            entry.offset,
            entry.path()
        );
    }
    eprintln!("{} files", archive.entries.len());
    Ok(())
}

fn cpk_extract(path: &Path, out: &Path) -> Result<()> {
    let mut archive = open_cpk(path)?;
    let entries = archive.entries.clone();
    for entry in &entries {
        let target = safe_join(out, &entry.path())?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = archive.read(entry)?;
        if data.len() as u64 != entry.extract_size {
            return Err(format!(
                "{}: size is {}, the TOC says {}",
                entry.path(),
                data.len(),
                entry.extract_size
            )
            .into());
        }
        std::fs::write(&target, data)?;
    }
    eprintln!("wrote {} files to {}", entries.len(), out.display());
    Ok(())
}

/// Joins an archive path to a directory. Refuses a path that goes up.
fn safe_join(base: &Path, path: &str) -> Result<PathBuf> {
    let mut out = base.to_path_buf();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(format!("unsafe path in archive: {path}").into()),
            _ => out.push(part),
        }
    }
    Ok(out)
}
