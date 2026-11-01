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
    /// Convert a dump into the folder that the player reads.
    Import {
        /// The NSP file of the game, or a RomFS folder.
        input: PathBuf,
        /// The output folder.
        out: PathBuf,
        /// The prod.keys file of your console. An NSP file needs it.
        #[arg(long)]
        keys: Option<PathBuf>,
        /// A patch file from `make-patch`, for example the English patch.
        #[arg(long)]
        translation: Option<PathBuf>,
        /// A RomFS folder whose files replace the files of the dump, for
        /// example the English patch. Give it more than one time for more
        /// patches. The first patch has priority.
        #[arg(long)]
        patch: Vec<PathBuf>,
        /// The Vorbis quality, from -0.2 to 1.0.
        #[arg(long, default_value_t = 0.4)]
        quality: f32,
        /// Convert only these audio groups: bgm, se, loopse, voice.
        #[arg(long, value_delimiter = ',')]
        audio: Vec<String>,
        /// The number of threads. All CPU cores without this option.
        #[arg(long)]
        jobs: Option<usize>,
    },
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
    /// Print a BIN_ table from a .bea file as tab-separated text.
    Table { file: PathBuf },
    /// Write the files of each layout in a .bea file, and its textures as PNG.
    Layout { file: PathBuf, out: PathBuf },
    /// Make a patch file with the deltas from a dump to a patched RomFS.
    MakePatch {
        /// The NSP file of the game, or a RomFS folder.
        input: PathBuf,
        /// The RomFS folder of the patch, with only the changed files.
        patched: PathBuf,
        /// The patch file to write.
        out: PathBuf,
        /// The prod.keys file of your console. An NSP file needs it.
        #[arg(long)]
        keys: Option<PathBuf>,
    },
    /// Convert an HCA audio file to Ogg Vorbis.
    Audio {
        file: PathBuf,
        out: PathBuf,
        /// The Vorbis quality, from -0.2 to 1.0.
        #[arg(long, default_value_t = 0.4)]
        quality: f32,
    },
    /// Write a character sprite with an expression as a PNG image.
    Sprite {
        /// The Archive directory of the RomFS dump.
        archives: PathBuf,
        /// The sprite name, for example 01_aya_01_A.
        name: String,
        /// The expression name, for example emi. Without it, print the names.
        expression: Option<String>,
        out: Option<PathBuf>,
    },
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
        Command::Import {
            input,
            out,
            keys,
            translation,
            patch,
            quality,
            audio,
            jobs,
        } => import(
            &input,
            &out,
            keys.as_deref(),
            translation.as_deref(),
            patch,
            importer::import::Options {
                quality,
                audio_groups: audio,
                jobs,
            },
        ),
        Command::MakePatch {
            input,
            patched,
            out,
            keys,
        } => make_patch(&input, &patched, &out, keys.as_deref()),
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
        Command::Table { file } => table(&file),
        Command::Layout { file, out } => layout(&file, &out),
        Command::Audio { file, out, quality } => audio(&file, &out, quality),
        Command::Sprite {
            archives,
            name,
            expression,
            out,
        } => sprite(&archives, &name, expression.as_deref(), out.as_deref()),
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

/// Reads the first file of a .bea archive.
fn read_bea_first(path: &Path) -> Result<Vec<u8>> {
    let bytes = read_file(path)?;
    let archive = bea::Archive::parse(&bytes)?;
    let entry = archive.entries.first().ok_or("the archive is empty")?;
    Ok(archive.read(entry)?)
}

fn table(path: &Path) -> Result<()> {
    let data = read_file(path)?;
    let data = if data.starts_with(bea::MAGIC) {
        read_bea_first(path)?
    } else {
        data
    };
    let table = bea::table::Table::parse(&data)?;
    println!("{}", table.columns);
    for row in &table.rows {
        let cells: Vec<String> = row
            .iter()
            .map(|c| match c {
                bea::table::Cell::Int(v) => v.to_string(),
                bea::table::Cell::Text(s) => s.clone(),
            })
            .collect();
        println!("{}", cells.join("\t"));
    }
    Ok(())
}

fn sprite(archives: &Path, name: &str, expression: Option<&str>, out: Option<&Path>) -> Result<()> {
    let table = bea::table::Table::parse(&read_bea_first(
        &archives.join("static~chara.bin.nxonnx32.bea"),
    )?)?;
    let row = table
        .rows
        .iter()
        .find(|r| r[0].as_text() == Some(name))
        .ok_or_else(|| format!("no sprite {name} in chara.bin"))?;
    let sprite = importer::sprite::Sprite::from_row(row)?;
    let Some(expression) = expression else {
        for e in &sprite.expressions {
            println!("{}", e.name);
        }
        return Ok(());
    };
    let index = sprite
        .expressions
        .iter()
        .position(|e| e.name == expression)
        .ok_or_else(|| format!("{name} has no expression {expression}"))?;
    let textures = bntx::parse(&read_bea_first(
        &archives.join(format!("{name}.nxonnx32.bea")),
    )?)?;
    let sheet = textures
        .first()
        .ok_or("the sprite has no texture")?
        .to_rgba8()?;
    let rgba = sprite.compose(&sheet, index, 0, 0)?;
    let out = out.ok_or("give an output file")?;
    write_png(out, sprite.body.x, sprite.body.y, &rgba)?;
    eprintln!("{name} {expression}: {}x{}", sprite.body.x, sprite.body.y);
    Ok(())
}

/// Gives the RomFS of an NSP file, or the folder as it is.
fn open_input(input: &Path, keys: Option<&Path>) -> Result<importer::import::Root> {
    use importer::import::Root;
    if input.is_dir() {
        return Ok(Root::Dir(input.to_path_buf()));
    }
    let keys = keys.ok_or("an NSP file needs --keys with the prod.keys file of your console")?;
    let keys = nsp::Keys::parse(
        &std::fs::read_to_string(keys).map_err(|e| format!("{}: {e}", keys.display()))?,
    );
    let file = std::fs::File::open(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let game = nsp::Game::open(std::sync::Arc::new(file), &keys)
        .map_err(|e| format!("{}: {e}", input.display()))?;
    eprintln!(
        "program {:016x}, {} files",
        game.program_id,
        game.romfs.files().count()
    );
    Ok(Root::RomFs(std::sync::Arc::new(game.romfs)))
}

fn import(
    input: &Path,
    out: &Path,
    keys: Option<&Path>,
    translation: Option<&Path>,
    patches: Vec<PathBuf>,
    options: importer::import::Options,
) -> Result<()> {
    use importer::import::Root;
    let mut roots: Vec<Root> = patches.into_iter().map(Root::Dir).collect();
    roots.push(open_input(input, keys)?);
    let translation = match translation {
        Some(path) => Some(importer::patch::PatchSet::parse(&read_file(path)?)?),
        None => None,
    };
    let source = importer::import::Source { roots, translation };
    let start = std::time::Instant::now();
    let manifest = importer::import::run(&source, out, &options, &|line| {
        eprintln!("[{:>6.1}s] {line}", start.elapsed().as_secs_f32())
    })
    .map_err(|e| e.to_string())?;
    for failure in &manifest.failures {
        eprintln!("failed: {}: {}", failure.path, failure.error);
    }
    if !manifest.failures.is_empty() {
        return Err(format!("{} files did not convert", manifest.failures.len()).into());
    }
    eprintln!("wrote {}", out.join("manifest.json").display());
    Ok(())
}

fn make_patch(input: &Path, patched: &Path, out: &Path, keys: Option<&Path>) -> Result<()> {
    let source = importer::import::Source {
        roots: vec![open_input(input, keys)?],
        translation: None,
    };
    let set = importer::patch::make(&source, patched).map_err(|e| e.to_string())?;
    let bytes = set.to_bytes();
    std::fs::write(out, &bytes)?;
    eprintln!(
        "wrote {} deltas, {} bytes, to {}",
        set.deltas.len(),
        bytes.len(),
        out.display()
    );
    Ok(())
}

fn audio(path: &Path, out: &Path, quality: f32) -> Result<()> {
    let track = importer::audio::hca_to_ogg(&read_file(path)?, quality)?;
    std::fs::write(out, &track.ogg)?;
    let looping = match track.looping {
        Some(l) => format!(", loop {}..{}", l.start, l.end),
        None => String::new(),
    };
    eprintln!(
        "{} Hz, {} channels, {} frames{looping}, {} bytes",
        track.sample_rate,
        track.channels,
        track.frames,
        track.ogg.len()
    );
    Ok(())
}

fn layout(path: &Path, out: &Path) -> Result<()> {
    let bytes = read_file(path)?;
    let archive = bea::Archive::parse(&bytes)?;
    let (mut files, mut images) = (0, 0);
    for e in &archive.entries {
        let data = archive.read(e)?;
        if !data.starts_with(b"SARC") {
            continue;
        }
        let dir = safe_join(out, &e.name)?;
        for f in sarc::parse(&data)? {
            let target = safe_join(&dir, &f.name)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, f.data)?;
            files += 1;
            if f.data.starts_with(b"BNTX") {
                let images_dir = target.with_extension("png.d");
                std::fs::create_dir_all(&images_dir)?;
                for t in bntx::parse(f.data)? {
                    let png = safe_join(&images_dir, &format!("{}.png", t.name))?;
                    write_png(&png, t.width, t.height, &t.to_rgba8()?)?;
                    images += 1;
                }
            }
        }
    }
    eprintln!(
        "wrote {files} files and {images} images to {}",
        out.display()
    );
    Ok(())
}

fn write_png(out: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    std::fs::write(out, importer::image::encode_png(width, height, rgba)?)?;
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
    write_png(out, t.width, t.height, &rgba)?;
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
