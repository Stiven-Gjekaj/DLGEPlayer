//! Converts a RomFS dump into the normalized folder that the player reads.
//!
//! The source is a list of RomFS roots. A file in an earlier root replaces
//! the file with the same path in a later root. The English patch is a
//! root that comes before the dump.
//!
//! The output folder:
//!
//! | Path | Contents |
//! |---|---|
//! | `manifest.json` | the list of all files, with their data |
//! | `scripts/<name>.json` | the IR of each script |
//! | `tables/<name>.json` | each `BIN_` table of `Archive/static~*.bin` |
//! | `images/<group>/<name>.png` | each standalone texture, or `.webp` |
//! | `sprites/<name>.png` | each character sprite sheet |
//! | `ui/<archive>/<name>.png` | each texture of a UI layout archive |
//! | `ui/<archive>/layouts/<name>.json` | each `.bflyt` layout |
//! | `ui/<archive>/animations/<name>.json` | each `.bflan` animation |
//! | `audio/<group>/<name>.ogg` | each HCA file of the audio archives |
//! | `audio/system/<cue>.ogg` | each cue of `SystemSE.acb`, the menu sounds |
//! | `lip.json` | the mouth states of each voice in `lip.cpk`, by voice number |
//! | `movies/<name>.mp4` | each movie, as it is |

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use serde::Serialize;

use crate::patch::PatchSet;
use crate::{adx, audio, image, ir, lip, sprite};

/// The version of the folder format. A change that the player cannot
/// read makes a new version.
pub const FORMAT: u32 = 1;

/// The suffix of the Switch archives in `Archive/`.
const ARCHIVE_SUFFIX: &str = ".nxonnx32.bea";

/// The audio archives, with the name of the output group.
const AUDIO: [(&str, &str); 4] = [
    ("bgm.cpk", "bgm"),
    ("se.cpk", "se"),
    ("loopse.cpk", "loopse"),
    ("voice.cpk", "voice"),
];

/// The ACB file of the menu sounds, and its output group.
const SYSTEM_SOUNDS: (&str, &str) = ("SystemSE.acb", "system");

/// The number of audio files that the importer reads before it converts
/// them in parallel.
const AUDIO_BATCH: usize = 256;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, BoxError>;

/// The settings of an import.
#[derive(Debug, Clone)]
pub struct Options {
    /// The Vorbis quality, from -0.2 to 1.0.
    pub quality: f32,
    /// The audio groups to convert. All groups if empty.
    pub audio_groups: Vec<String>,
    /// The number of threads. All CPU cores if `None`.
    pub jobs: Option<usize>,
    /// The format of the images.
    pub images: image::Format,
}

impl Options {
    /// The settings for a phone: lossy WebP images at quality 90 and Vorbis
    /// quality 0. The folder is about half the size.
    pub fn mobile() -> Options {
        Options {
            quality: 0.0,
            images: image::Format::Webp(90.0),
            ..Options::default()
        }
    }
}

impl Default for Options {
    fn default() -> Options {
        Options {
            quality: 0.4,
            audio_groups: Vec::new(),
            jobs: None,
            images: image::Format::Png,
        }
    }
}

/// A RomFS: a directory, or the RomFS of an NSP file.
pub enum Root {
    Dir(PathBuf),
    RomFs(Arc<nsp::RomFs>),
}

/// A file that the importer reads in parts.
pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// A list of RomFS roots. The first root that holds a path gives the file.
/// The translation changes the files of scripts and archives after they
/// are read.
pub struct Source {
    pub roots: Vec<Root>,
    pub translation: Option<PatchSet>,
}

impl Source {
    /// Applies the translation to a file, if it has a delta for the key.
    pub fn translate(&self, key: &str, data: Vec<u8>) -> Result<Vec<u8>> {
        match &self.translation {
            Some(set) => set.apply(key, data),
            None => Ok(data),
        }
    }

    /// Tells if a path is a file in a root.
    pub fn contains(&self, path: &str) -> bool {
        self.roots.iter().any(|root| match root {
            Root::Dir(dir) => dir.join(path).is_file(),
            Root::RomFs(fs) => fs.file(path).is_some(),
        })
    }

    /// Gives the names of the files in a directory of all roots.
    pub fn list(&self, dir: &str) -> Result<Vec<String>> {
        let mut names = std::collections::BTreeSet::new();
        for root in &self.roots {
            match root {
                Root::Dir(base) => {
                    let Ok(entries) = std::fs::read_dir(base.join(dir)) else {
                        continue;
                    };
                    for entry in entries {
                        let entry = entry?;
                        if entry.file_type()?.is_file() {
                            names.insert(entry.file_name().to_string_lossy().into_owned());
                        }
                    }
                }
                Root::RomFs(fs) => names.extend(fs.list(dir).into_iter().map(str::to_string)),
            }
        }
        Ok(names.into_iter().collect())
    }

    /// Opens a file to read it in parts.
    pub fn open(&self, path: &str) -> Result<Option<Box<dyn ReadSeek>>> {
        for root in &self.roots {
            match root {
                Root::Dir(dir) => {
                    let file = dir.join(path);
                    if file.is_file() {
                        let f =
                            File::open(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                        return Ok(Some(Box::new(BufReader::new(f))));
                    }
                }
                Root::RomFs(fs) => {
                    // No buffer: the readers of the importer read large
                    // parts, and a buffer decrypts bytes that a seek drops.
                    if let Some(region) = fs.file(path) {
                        return Ok(Some(Box::new(region.reader())));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Reads a whole file.
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        for root in &self.roots {
            match root {
                Root::Dir(dir) => {
                    let file = dir.join(path);
                    if file.is_file() {
                        return Ok(
                            std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?
                        );
                    }
                }
                Root::RomFs(fs) => {
                    if let Some(region) = fs.file(path) {
                        return Ok(region.read_all().map_err(|e| format!("{path}: {e}"))?);
                    }
                }
            }
        }
        Err(format!("{path} is not in the dump").into())
    }
}

/// The data of one sprite sheet.
#[derive(Debug, Clone, Serialize)]
pub struct SpriteEntry {
    pub file: String,
    #[serde(flatten)]
    pub layout: sprite::Sprite,
}

/// The data of one audio file.
#[derive(Debug, Clone, Serialize)]
pub struct AudioEntry {
    pub file: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub frames: u64,
    /// The loop start in sample frames. The loop ends at the end of the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_start: Option<u64>,
}

/// The files of one UI layout archive. Each map gives the files by name.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UiEntry {
    pub textures: BTreeMap<String, String>,
    pub layouts: BTreeMap<String, String>,
    pub animations: BTreeMap<String, String>,
}

/// A file that the importer could not convert.
#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    pub path: String,
    pub error: String,
}

/// The contents of `manifest.json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Manifest {
    pub format: u32,
    /// The scripts by name. Each value is the file.
    pub scripts: BTreeMap<String, String>,
    pub tables: BTreeMap<String, String>,
    /// The images by group, then by name.
    pub images: BTreeMap<String, BTreeMap<String, String>>,
    pub sprites: BTreeMap<String, SpriteEntry>,
    /// The UI files by layout archive.
    pub ui: BTreeMap<String, UiEntry>,
    /// The audio by group, then by name.
    pub audio: BTreeMap<String, BTreeMap<String, AudioEntry>>,
    pub movies: BTreeMap<String, String>,
    /// The file of the lip sync data, if the dump has `lip.cpk`.
    pub lip: Option<String>,
    pub failures: Vec<Failure>,
}

/// Imports a dump into `out`. `progress` receives a line for each step.
pub fn run(
    source: &Source,
    out: &Path,
    options: &Options,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<Manifest> {
    let mut pool = rayon::ThreadPoolBuilder::new();
    if let Some(jobs) = options.jobs {
        pool = pool.num_threads(jobs.max(1));
    }
    pool.build()?
        .install(|| run_in_pool(source, out, options, progress))
}

fn run_in_pool(
    source: &Source,
    out: &Path,
    options: &Options,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<Manifest> {
    std::fs::create_dir_all(out)?;
    let manifest = Mutex::new(Manifest {
        format: FORMAT,
        ..Manifest::default()
    });
    let fail = |path: &str, error: BoxError| {
        manifest.lock().unwrap().failures.push(Failure {
            path: path.to_string(),
            error: error.to_string(),
        });
    };

    import_scripts(source, out, &manifest, &fail)?;
    progress(&format!(
        "scripts: {}",
        manifest.lock().unwrap().scripts.len()
    ));

    let archives: Vec<String> = source
        .list("Archive")?
        .into_iter()
        .filter(|n| n.ends_with(ARCHIVE_SUFFIX))
        .collect();
    import_tables(source, out, &archives, &manifest, &fail);
    progress(&format!(
        "tables: {}",
        manifest.lock().unwrap().tables.len()
    ));

    import_images(source, out, &archives, options.images, &manifest, &fail);
    let images: usize = manifest
        .lock()
        .unwrap()
        .images
        .values()
        .map(|g| g.len())
        .sum();
    progress(&format!("images: {images}"));

    import_sprites(source, out, options.images, &manifest, &fail);
    progress(&format!(
        "sprites: {}",
        manifest.lock().unwrap().sprites.len()
    ));

    import_ui(source, out, &archives, options.images, &manifest, &fail);
    let (textures, layouts, animations) =
        manifest
            .lock()
            .unwrap()
            .ui
            .values()
            .fold((0, 0, 0), |(t, l, a), e| {
                (
                    t + e.textures.len(),
                    l + e.layouts.len(),
                    a + e.animations.len(),
                )
            });
    progress(&format!(
        "UI: {textures} textures, {layouts} layouts, {animations} animations"
    ));

    for (archive, group) in AUDIO {
        if !options.audio_groups.is_empty() && !options.audio_groups.iter().any(|g| g == group) {
            continue;
        }
        import_audio(
            source,
            out,
            archive,
            group,
            options.quality,
            &manifest,
            &fail,
        )?;
        let count = manifest
            .lock()
            .unwrap()
            .audio
            .get(group)
            .map_or(0, |g| g.len());
        progress(&format!("audio {group}: {count}"));
    }

    let (acb, group) = SYSTEM_SOUNDS;
    if options.audio_groups.is_empty() || options.audio_groups.iter().any(|g| g == group) {
        import_acb(source, out, acb, group, options.quality, &manifest, &fail);
        let count = manifest
            .lock()
            .unwrap()
            .audio
            .get(group)
            .map_or(0, |g| g.len());
        progress(&format!("audio {group}: {count}"));
    }

    let voices = import_lip(source, out, &manifest, &fail)?;
    progress(&format!("lip sync: {voices} voices"));

    import_movies(source, out, &manifest, &fail)?;
    progress(&format!(
        "movies: {}",
        manifest.lock().unwrap().movies.len()
    ));

    let mut manifest = manifest.into_inner().unwrap();
    manifest.failures.sort_by(|a, b| a.path.cmp(&b.path));
    std::fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

/// Writes a file and makes its directory.
fn write(out: &Path, relative: &str, data: &[u8]) -> Result<()> {
    let path = out.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

/// Gives the name of a file in an archive with no directory and no
/// extension.
fn stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.split_once('.').map_or(name, |(s, _)| s)
}

/// Splits the name of a texture archive, for example
/// `texture~ev_thum~mb0610.dds.nxonnx32.bea`, into the group `ev_thum` and
/// the name `mb0610`. A deeper path joins its groups with `/`.
pub fn texture_name(archive: &str) -> Option<(String, String)> {
    let rest = archive
        .strip_prefix("texture~")?
        .strip_suffix(ARCHIVE_SUFFIX)?;
    let (group, name) = rest.rsplit_once('~')?;
    Some((group.replace('~', "/"), stem(name).to_string()))
}

/// Gives the table name of an archive such as `static~chara.bin.nxonnx32.bea`.
pub fn table_name(archive: &str) -> Option<&str> {
    let rest = archive
        .strip_prefix("static~")?
        .strip_suffix(ARCHIVE_SUFFIX)?;
    rest.strip_suffix(".bin")
}

/// Turns a `BIN_` table into JSON: the column types and the rows.
pub fn table_json(table: &bea::table::Table) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = table
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    bea::table::Cell::Int(v) => serde_json::Value::from(*v),
                    bea::table::Cell::Text(s) => serde_json::Value::from(s.as_str()),
                })
                .collect()
        })
        .collect();
    serde_json::json!({ "columns": table.columns, "rows": rows })
}

fn open_cpk(source: &Source, name: &str) -> Result<Option<cpk::Archive<Box<dyn ReadSeek>>>> {
    let Some(file) = source.open(name)? else {
        return Ok(None);
    };
    Ok(Some(cpk::Archive::open(file)?))
}

type Fail<'a> = dyn Fn(&str, BoxError) + Sync + 'a;

fn import_scripts(
    source: &Source,
    out: &Path,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) -> Result<()> {
    let mut archive = open_cpk(source, "script.cpk")?.ok_or("script.cpk is not in the dump")?;
    let entries = archive.entries.clone();
    for entry in &entries {
        let path = format!("script.cpk/{}", entry.path());
        let result = (|| -> Result<(String, String)> {
            let data = source.translate(&path, archive.read(entry)?)?;
            let script = stcm2l::Script::parse(&data)?;
            let name = stem(&entry.name).to_string();
            let program = ir::build(&name, &script)?;
            let file = format!("scripts/{name}.json");
            write(out, &file, &serde_json::to_vec(&program)?)?;
            Ok((name, file))
        })();
        match result {
            Ok((name, file)) => {
                manifest.lock().unwrap().scripts.insert(name, file);
            }
            Err(e) => fail(&path, e),
        }
    }
    Ok(())
}

/// Reads the first file of an archive in `Archive/`.
fn read_archive_first(source: &Source, archive: &str) -> Result<Vec<u8>> {
    let path = format!("Archive/{archive}");
    let bytes = source.read(&path)?;
    let parsed = bea::Archive::parse(&bytes)?;
    let entry = parsed.entries.first().ok_or("the archive is empty")?;
    source.translate(&format!("{path}/{}", entry.name), parsed.read(entry)?)
}

fn import_tables(
    source: &Source,
    out: &Path,
    archives: &[String],
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) {
    for archive in archives {
        let Some(name) = table_name(archive) else {
            continue;
        };
        let result = (|| -> Result<String> {
            let table = bea::table::Table::parse(&read_archive_first(source, archive)?)?;
            let file = format!("tables/{name}.json");
            write(out, &file, &serde_json::to_vec(&table_json(&table))?)?;
            Ok(file)
        })();
        match result {
            Ok(file) => {
                manifest
                    .lock()
                    .unwrap()
                    .tables
                    .insert(name.to_string(), file);
            }
            Err(e) => fail(&format!("Archive/{archive}"), e),
        }
    }
}

fn import_images(
    source: &Source,
    out: &Path,
    archives: &[String],
    format: image::Format,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) {
    archives.par_iter().for_each(|archive| {
        let Some((group, name)) = texture_name(archive) else {
            return;
        };
        let result = (|| -> Result<String> {
            let data = read_archive_first(source, archive)?;
            let png = match image::decode_raw(&data) {
                Some((width, height, rgba)) => format.encode(width, height, rgba)?,
                None => {
                    let textures = bntx::parse(&data)?;
                    let texture = textures.first().ok_or("the file holds no texture")?;
                    format.encode(texture.width, texture.height, &texture.to_rgba8()?)?
                }
            };
            let file = format!("images/{group}/{name}.{}", format.extension());
            write(out, &file, &png)?;
            Ok(file)
        })();
        match result {
            Ok(file) => {
                manifest
                    .lock()
                    .unwrap()
                    .images
                    .entry(group)
                    .or_default()
                    .insert(name, file);
            }
            Err(e) => fail(&format!("Archive/{archive}"), e),
        }
    });
}

fn import_sprites(
    source: &Source,
    out: &Path,
    format: image::Format,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) {
    let rows = match read_archive_first(source, &format!("static~chara.bin{ARCHIVE_SUFFIX}"))
        .and_then(|data| Ok(bea::table::Table::parse(&data)?))
    {
        Ok(table) => table.rows,
        Err(e) => {
            fail("Archive/static~chara.bin", e);
            return;
        }
    };
    rows.par_iter().for_each(|row| {
        let result = (|| -> Result<SpriteEntry> {
            let layout = sprite::Sprite::from_row(row)?;
            let archive = format!("{}{ARCHIVE_SUFFIX}", layout.name);
            let textures = bntx::parse(&read_archive_first(source, &archive)?)?;
            let texture = textures.first().ok_or("the sprite has no texture")?;
            if (texture.width, texture.height) != (layout.sheet.x, layout.sheet.y) {
                return Err(format!(
                    "the sheet is {}x{}, chara.bin says {}x{}",
                    texture.width, texture.height, layout.sheet.x, layout.sheet.y
                )
                .into());
            }
            let file = format!("sprites/{}.{}", layout.name, format.extension());
            let png = format.encode(texture.width, texture.height, &texture.to_rgba8()?)?;
            write(out, &file, &png)?;
            Ok(SpriteEntry { file, layout })
        })();
        let name = row
            .first()
            .and_then(|c| c.as_text())
            .unwrap_or("")
            .to_string();
        match result {
            Ok(entry) => {
                manifest.lock().unwrap().sprites.insert(name, entry);
            }
            Err(e) => fail(&format!("sprite {name}"), e),
        }
    });
}

fn import_ui(
    source: &Source,
    out: &Path,
    archives: &[String],
    format: image::Format,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) {
    archives.par_iter().for_each(|archive| {
        let path = format!("Archive/{archive}");
        let Ok(bytes) = source.read(&path) else {
            return;
        };
        let Ok(parsed) = bea::Archive::parse(&bytes) else {
            return;
        };
        for entry in parsed.entries.iter().filter(|e| e.name.ends_with(".lyt")) {
            let layout = stem(&entry.name).to_string();
            let result = (|| -> Result<UiEntry> {
                let data =
                    source.translate(&format!("{path}/{}", entry.name), parsed.read(entry)?)?;
                let mut files = UiEntry::default();
                for f in sarc::parse(&data)? {
                    let name = stem(&f.name).to_string();
                    if f.data.starts_with(b"BNTX") {
                        for t in bntx::parse(f.data)? {
                            let file = format!("ui/{layout}/{}.{}", t.name, format.extension());
                            write(
                                out,
                                &file,
                                &format.encode(t.width, t.height, &t.to_rgba8()?)?,
                            )?;
                            files.textures.insert(t.name.clone(), file);
                        }
                    } else if f.data.starts_with(lyt::MAGIC) {
                        let file = format!("ui/{layout}/layouts/{name}.json");
                        write(out, &file, &serde_json::to_vec(&lyt::parse(f.data)?)?)?;
                        files.layouts.insert(name, file);
                    } else if f.data.starts_with(lyt::anim::MAGIC) {
                        let file = format!("ui/{layout}/animations/{name}.json");
                        write(out, &file, &serde_json::to_vec(&lyt::anim::parse(f.data)?)?)?;
                        files.animations.insert(name, file);
                    }
                }
                Ok(files)
            })();
            match result {
                Ok(files) => {
                    manifest.lock().unwrap().ui.insert(layout, files);
                }
                Err(e) => fail(&format!("{path}/{}", entry.name), e),
            }
        }
    });
}

fn import_audio(
    source: &Source,
    out: &Path,
    archive_name: &str,
    group: &str,
    quality: f32,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) -> Result<()> {
    let Some(mut archive) = open_cpk(source, archive_name)? else {
        fail(archive_name, "the archive is not in the dump".into());
        return Ok(());
    };
    let entries: Vec<cpk::Entry> = archive
        .entries
        .iter()
        .filter(|e| e.name.ends_with(".hca"))
        .cloned()
        .collect();
    for batch in entries.chunks(AUDIO_BATCH) {
        let mut data = Vec::with_capacity(batch.len());
        for entry in batch {
            data.push(archive.read(entry));
        }
        batch.par_iter().zip(data).for_each(|(entry, bytes)| {
            let name = stem(&entry.name).to_string();
            let result = bytes
                .map_err(BoxError::from)
                .and_then(|hca| convert_hca(out, group, &name, &hca, quality));
            match result {
                Ok(entry) => {
                    manifest
                        .lock()
                        .unwrap()
                        .audio
                        .entry(group.to_string())
                        .or_default()
                        .insert(name, entry);
                }
                Err(e) => fail(&format!("{archive_name}/{}", entry.path()), e),
            }
        });
    }
    Ok(())
}

/// Converts one HCA file to `audio/<group>/<name>.ogg`.
fn convert_hca(
    out: &Path,
    group: &str,
    name: &str,
    hca: &[u8],
    quality: f32,
) -> Result<AudioEntry> {
    let track = audio::hca_to_ogg(hca, quality)?;
    let file = format!("audio/{group}/{name}.ogg");
    write(out, &file, &track.ogg)?;
    Ok(AudioEntry {
        file,
        sample_rate: track.sample_rate,
        channels: track.channels,
        frames: track.frames,
        loop_start: track.looping.map(|l| l.start),
    })
}

/// Converts each HCA or ADX cue of an ACB file with an embedded AWB.
fn import_acb(
    source: &Source,
    out: &Path,
    acb: &str,
    group: &str,
    quality: f32,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) {
    let tracks = match source.read(acb).and_then(|data| {
        Ok(cridecoder::extract_acb_to_memory(
            std::io::Cursor::new(data),
            None,
        )?)
    }) {
        Ok(tracks) => tracks,
        Err(e) => {
            fail(acb, e);
            return;
        }
    };
    tracks.par_iter().for_each(|track| {
        let path = format!("{acb}/{}", track.name);
        let result = (|| -> Result<AudioEntry> {
            let converted = match track.extension.trim_start_matches('.') {
                "hca" => audio::hca_to_ogg(&track.data, quality)?,
                "adx" => audio::pcm_to_ogg(&adx::decode(&track.data)?, quality)?,
                other => return Err(format!("unsupported track type {other}").into()),
            };
            let file = format!("audio/{group}/{}.ogg", track.name);
            write(out, &file, &converted.ogg)?;
            Ok(AudioEntry {
                file,
                sample_rate: converted.sample_rate,
                channels: converted.channels,
                frames: converted.frames,
                loop_start: converted.looping.map(|l| l.start),
            })
        })();
        match result {
            Ok(entry) => {
                manifest
                    .lock()
                    .unwrap()
                    .audio
                    .entry(group.to_string())
                    .or_default()
                    .insert(track.name.clone(), entry);
            }
            Err(e) => fail(&path, e),
        }
    });
}

/// Writes the data of all `.lip` files to one file. Gives the number of
/// voices.
fn import_lip(
    source: &Source,
    out: &Path,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) -> Result<usize> {
    let Some(mut archive) = open_cpk(source, "lip.cpk")? else {
        return Ok(0);
    };
    let entries = archive.entries.clone();
    let mut all = BTreeMap::new();
    for entry in entries.iter().filter(|e| e.name.ends_with(".lip")) {
        let result = archive
            .read(entry)
            .map_err(BoxError::from)
            .and_then(|data| Ok(lip::parse(&data)?));
        match result {
            Ok(keys) => {
                all.insert(stem(&entry.name).to_string(), keys);
            }
            Err(e) => fail(&format!("lip.cpk/{}", entry.path()), e),
        }
    }
    let file = "lip.json".to_string();
    write(out, &file, &serde_json::to_vec(&all)?)?;
    manifest.lock().unwrap().lip = Some(file);
    Ok(all.len())
}

fn import_movies(
    source: &Source,
    out: &Path,
    manifest: &Mutex<Manifest>,
    fail: &Fail,
) -> Result<()> {
    for name in source.list("movie")? {
        if !name.ends_with(".mp4") {
            continue;
        }
        let file = format!("movies/{name}");
        let result = (|| -> Result<()> {
            let mut input = source
                .open(&format!("movie/{name}"))?
                .ok_or("the movie is not in the dump")?;
            let path = out.join(&file);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut output = File::create(&path)?;
            std::io::copy(&mut input, &mut output)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                manifest
                    .lock()
                    .unwrap()
                    .movies
                    .insert(stem(&name).to_string(), file);
            }
            Err(e) => fail(&format!("movie/{name}"), e),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_texture_names() {
        assert_eq!(
            texture_name("texture~ev_thum~mb0610.dds.nxonnx32.bea"),
            Some(("ev_thum".to_string(), "mb0610".to_string()))
        );
        assert_eq!(
            texture_name("texture~a~b~c.dds.nxonnx32.bea"),
            Some(("a/b".to_string(), "c".to_string()))
        );
        assert_eq!(texture_name("01_aya_01_A.nxonnx32.bea"), None);
        assert_eq!(texture_name("texture~bg~bg01.dds.nxonwin32.bea"), None);
    }

    #[test]
    fn finds_table_names() {
        assert_eq!(table_name("static~chara.bin.nxonnx32.bea"), Some("chara"));
        assert_eq!(table_name("static~chara.nxonnx32.bea"), None);
        assert_eq!(table_name("chara.bin.nxonnx32.bea"), None);
    }

    #[test]
    fn writes_tables_as_json() {
        let table = bea::table::Table {
            columns: "si".to_string(),
            rows: vec![vec![
                bea::table::Cell::Text("aya".to_string()),
                bea::table::Cell::Int(-1),
            ]],
        };
        assert_eq!(
            table_json(&table).to_string(),
            r#"{"columns":"si","rows":[["aya",-1]]}"#
        );
    }

    #[test]
    fn takes_a_file_from_the_first_root() {
        let base = std::env::temp_dir().join(format!("dlge-source-{}", std::process::id()));
        let (patch, dump) = (base.join("patch"), base.join("dump"));
        for (root, files) in [(&patch, &["a.txt"][..]), (&dump, &["a.txt", "b.txt"][..])] {
            std::fs::create_dir_all(root.join("dir")).unwrap();
            for f in files {
                std::fs::write(root.join("dir").join(f), root.to_string_lossy().as_bytes())
                    .unwrap();
            }
        }
        let romfs = nsp::romfs::build(&[("dir/c.txt", b"from the nsp"), ("dir/a.txt", b"hidden")]);
        let len = romfs.len() as u64;
        let fs = nsp::RomFs::read(nsp::Region::new(Arc::new(romfs), 0, len)).unwrap();
        let source = Source {
            roots: vec![
                Root::Dir(patch.clone()),
                Root::Dir(dump.clone()),
                Root::RomFs(Arc::new(fs)),
            ],
            translation: None,
        };
        let text = |p: &str| String::from_utf8(source.read(p).unwrap()).unwrap();
        assert_eq!(text("dir/a.txt"), patch.to_string_lossy());
        assert_eq!(text("dir/b.txt"), dump.to_string_lossy());
        assert_eq!(text("dir/c.txt"), "from the nsp");
        let mut opened = String::new();
        source
            .open("dir/c.txt")
            .unwrap()
            .unwrap()
            .read_to_string(&mut opened)
            .unwrap();
        assert_eq!(opened, "from the nsp");
        assert!(source.contains("dir/c.txt"));
        assert!(!source.contains("dir/d.txt"));
        assert!(source.read("dir/d.txt").is_err());
        assert!(source.open("dir/d.txt").unwrap().is_none());
        assert_eq!(source.list("dir").unwrap(), vec!["a.txt", "b.txt", "c.txt"]);
        assert!(source.list("none").unwrap().is_empty());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
