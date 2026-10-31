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
    /// Print the actions and the text of an STCM2L script.
    Disasm {
        file: PathBuf,
        /// Print only the text lines.
        #[arg(long)]
        strings: bool,
    },
    /// Write a listing and a text file for each script in a CPK archive.
    DumpScripts { archive: PathBuf, out: PathBuf },
    /// Work with Bezel Engine archives.
    Bea {
        #[command(subcommand)]
        command: BeaCommand,
    },
    /// Write the first texture of a .bea or a .bntx file as a PNG image.
    Texture { file: PathBuf, out: PathBuf },
}

#[derive(Subcommand)]
enum BeaCommand {
    /// Print the files in an archive.
    List { archive: PathBuf },
    /// Write all files of an archive to a directory.
    Extract { archive: PathBuf, out: PathBuf },
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
        Command::Disasm { file, strings } => disasm(&file, strings),
        Command::DumpScripts { archive, out } => dump_scripts(&archive, &out),
        Command::Bea { command } => match command {
            BeaCommand::List { archive } => bea_list(&archive),
            BeaCommand::Extract { archive, out } => bea_extract(&archive, &out),
        },
        Command::Texture { file, out } => texture(&file, &out),
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

fn disasm(path: &Path, strings_only: bool) -> Result<()> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bytes = if cpk::crilayla::is_compressed(&bytes) {
        cpk::crilayla::decompress(&bytes)?
    } else {
        bytes
    };
    let script = stcm2l::Script::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = if strings_only {
        stcm2l::disasm::strings(&script)
    } else {
        stcm2l::disasm::disassemble(&script)
    };
    print!("{text}");
    Ok(())
}

fn dump_scripts(path: &Path, out: &Path) -> Result<()> {
    let mut archive = open_cpk(path)?;
    let entries = archive.entries.clone();
    let mut failed = 0;
    let mut lines = 0;
    for entry in &entries {
        let data = archive.read(entry)?;
        let script = match stcm2l::Script::parse(&data) {
            Ok(script) => script,
            Err(e) => {
                eprintln!("{}: {e}", entry.path());
                failed += 1;
                continue;
            }
        };
        let base = safe_join(out, &entry.path())?;
        if let Some(parent) = base.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = stcm2l::disasm::strings(&script);
        lines += text.lines().count();
        std::fs::write(
            base.with_extension("asm"),
            stcm2l::disasm::disassemble(&script),
        )?;
        std::fs::write(base.with_extension("txt"), text)?;
    }
    eprintln!(
        "dumped {} of {} scripts, {lines} text lines, to {}",
        entries.len() - failed,
        entries.len(),
        out.display()
    );
    if failed > 0 {
        return Err(format!("{failed} scripts did not parse").into());
    }
    Ok(())
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    Ok(std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)
}

fn bea_list(path: &Path) -> Result<()> {
    let bytes = read_file(path)?;
    let archive = bea::Archive::parse(&bytes)?;
    for e in &archive.entries {
        println!("{:>10} {:>10} {}", e.stored_size, e.size, e.name);
    }
    eprintln!("{} files in {}", archive.entries.len(), archive.name);
    Ok(())
}

fn bea_extract(path: &Path, out: &Path) -> Result<()> {
    let bytes = read_file(path)?;
    let archive = bea::Archive::parse(&bytes)?;
    for e in &archive.entries {
        let target = safe_join(out, &e.name)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, archive.read(e)?)?;
    }
    eprintln!("wrote {} files to {}", archive.entries.len(), out.display());
    Ok(())
}

fn texture(path: &Path, out: &Path) -> Result<()> {
    let bytes = read_file(path)?;
    let bntx_bytes = if bytes.starts_with(bea::MAGIC) {
        let archive = bea::Archive::parse(&bytes)?;
        let entry = archive
            .entries
            .iter()
            .find(|e| e.name.ends_with(".dds") || e.name.ends_with(".ftxb"))
            .ok_or("the archive holds no texture")?;
        archive.read(entry)?
    } else {
        bytes
    };
    let textures = bntx::parse(&bntx_bytes)?;
    let t = textures.first().ok_or("the file holds no texture")?;
    let rgba = t.to_rgba8()?;
    let file = std::fs::File::create(out)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), t.width, t.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    eprintln!(
        "{}: {}x{}, format {:#x}",
        t.name, t.width, t.height, t.format_code
    );
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
