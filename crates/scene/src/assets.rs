//! Finds the files of the import folder by the names that the scripts use.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chara::{Catalog, Sprite};
use serde::{Deserialize, Serialize};

use crate::Error;

/// One sprite sheet of `manifest.json`.
#[derive(Deserialize)]
struct SpriteEntry {
    file: String,
    #[serde(flatten)]
    layout: Sprite,
}

/// One audio file of `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Track {
    pub file: String,
    pub sample_rate: u32,
    /// The loop start in sample frames. The loop ends at the end of the file.
    #[serde(default)]
    pub loop_start: Option<u64>,
}

/// The parts of `manifest.json` that the player reads.
#[derive(Deserialize)]
struct Manifest {
    tables: BTreeMap<String, String>,
    images: BTreeMap<String, BTreeMap<String, String>>,
    sprites: BTreeMap<String, SpriteEntry>,
    audio: BTreeMap<String, BTreeMap<String, Track>>,
    movies: BTreeMap<String, String>,
    #[serde(default)]
    lip: Option<String>,
}

/// A `BIN_` table as the importer writes it.
#[derive(Deserialize)]
struct Table {
    rows: Vec<Vec<serde_json::Value>>,
}

/// The lip sync keys of one voice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lip {
    pub sample_rate: u32,
    /// Pairs of a sample position and a mouth state: 1 for open.
    pub keys: Vec<(u32, u8)>,
}

/// A background or an event image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub file: String,
    /// The place name of a background, from `bg.bin`.
    pub place: String,
    /// The gallery number of an event image, from `ev.bin`.
    pub gallery: Option<i32>,
}

/// A character sprite with one expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figure {
    pub file: String,
    pub layout: Sprite,
    /// The index of the expression in `layout.expressions`.
    pub expression: usize,
}

/// One speaker of `speaker.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Speaker {
    pub name: String,
    /// The short character name, for example `aya`. It is empty for a
    /// speaker with no sprite.
    pub character: String,
}

/// One event image of the gallery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GalleryImage {
    pub name: String,
    pub file: String,
    pub number: i32,
    /// 0 for Haunted Dark Bridal, 1 for MORE,BLOOD.
    pub game: i32,
}

/// The files of an import folder.
pub struct Assets {
    pub root: PathBuf,
    manifest: Manifest,
    catalog: Catalog,
    files: Vec<String>,
    speakers: BTreeMap<i32, Speaker>,
    backgrounds: HashMap<String, (String, String)>,
    /// The event images by the game and the name. Some names are in
    /// both games.
    events: HashMap<(i32, String), (String, i32)>,
    lip: OnceLock<HashMap<String, Lip>>,
}

fn text(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn int(v: &serde_json::Value) -> i32 {
    v.as_i64().unwrap_or_default() as i32
}

impl Assets {
    /// Reads `manifest.json` and the tables of an import folder.
    pub fn load(root: &Path) -> Result<Assets, Error> {
        let read = |file: &str| -> Result<String, Error> {
            std::fs::read_to_string(root.join(file))
                .map_err(|e| Error::Load(format!("{file}: {e}")))
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
        let mut speakers = BTreeMap::new();
        for row in table("speaker")? {
            if row.len() >= 3 {
                speakers.insert(
                    int(&row[0]),
                    Speaker {
                        name: text(&row[1]),
                        character: text(&row[2]),
                    },
                );
            }
        }
        // A row of bg.bin is the name, the file, a number, a value, and
        // the place.
        let mut backgrounds = HashMap::new();
        for row in table("bg")? {
            if row.len() >= 5 {
                backgrounds.insert(text(&row[0]), (text(&row[1]), text(&row[4])));
            }
        }
        // A row of ev.bin is the name, the file, the gallery number, two
        // values, and the game.
        let mut events = HashMap::new();
        for row in table("ev")? {
            if row.len() >= 6 {
                events.insert((int(&row[5]), text(&row[0])), (text(&row[1]), int(&row[2])));
            }
        }
        let mut files = Vec::new();
        let mut layouts = Vec::new();
        for entry in manifest.sprites.values() {
            files.push(entry.file.clone());
            layouts.push(entry.layout.clone());
        }
        Ok(Assets {
            root: root.to_path_buf(),
            catalog: Catalog::new(layouts),
            files,
            manifest,
            speakers,
            backgrounds,
            events,
            lip: OnceLock::new(),
        })
    }

    /// Gives the full path of a file of the import folder.
    pub fn path(&self, file: &str) -> PathBuf {
        self.root.join(file)
    }

    /// Gives an image by its group and its name.
    pub fn image(&self, group: &str, name: &str) -> Option<&str> {
        self.manifest
            .images
            .get(group)?
            .get(name)
            .map(String::as_str)
    }

    /// Gives a background of `bg.bin`.
    pub fn background(&self, name: &str) -> Option<Picture> {
        let (file, place) = self.backgrounds.get(name)?;
        let file = self.image("bg", file)?;
        Some(Picture {
            file: file.to_string(),
            place: place.clone(),
            gallery: None,
        })
    }

    /// Gives an event image of `ev.bin`, or else a background. `game` is
    /// 0 for Haunted Dark Bridal and 1 for MORE,BLOOD. An event image of
    /// the other game is the last choice.
    pub fn picture(&self, game: i32, name: &str) -> Option<Picture> {
        let event = self
            .events
            .get(&(game, name.to_string()))
            .or_else(|| self.events.get(&(1 - game, name.to_string())));
        if let Some((file, number)) = event {
            let file = self.image("ev", file)?;
            return Some(Picture {
                file: file.to_string(),
                place: String::new(),
                gallery: Some(*number),
            });
        }
        self.background(name)
    }

    /// Gives a foreground image, for example `fg_kako`.
    pub fn foreground(&self, name: &str) -> Option<&str> {
        self.image("fg", name)
    }

    /// Gives a sprite by the second name of a script call, for example
    /// `aya_03_maji`.
    pub fn figure(&self, look: &str) -> Option<Figure> {
        let resolved = self.catalog.resolve(look).ok()?;
        resolved.rows.iter().find_map(|&row| {
            let layout = &self.catalog.sprites[row];
            let expression = layout
                .expressions
                .iter()
                .position(|e| e.name == resolved.expression)?;
            Some(Figure {
                file: self.files[row].clone(),
                layout: layout.clone(),
                expression,
            })
        })
    }

    /// Gives a speaker of `speaker.bin`.
    pub fn speaker(&self, number: i32) -> Option<&Speaker> {
        self.speakers.get(&number)
    }

    /// Gives an audio file of a group by name.
    pub fn track(&self, group: &str, name: &str) -> Option<&Track> {
        self.manifest.audio.get(group)?.get(name)
    }

    /// Gives a music track by number. `3` is `BGM03`.
    pub fn music(&self, number: i32) -> Option<&Track> {
        self.track("bgm", &format!("BGM{number:02}"))
    }

    /// Gives a loop sound by number, for example `104`.
    pub fn ambience(&self, number: i32) -> Option<&Track> {
        self.track("loopse", &number.to_string())
    }

    /// Gives a sound effect by number.
    pub fn sound(&self, number: i32) -> Option<&Track> {
        self.track("se", &number.to_string())
    }

    /// Gives a voice of a game. `game` is `ve` or `mb`. Some scripts play
    /// a voice of the other game, so that game is the second choice.
    pub fn voice(&self, game: &str, number: i32) -> Option<&Track> {
        let other = if game == "mb" { "ve" } else { "mb" };
        self.track("voice", &format!("{game}_{number:06}"))
            .or_else(|| self.track("voice", &format!("{other}_{number:06}")))
    }

    /// Gives a movie by name. A name such as `MOVIE1.usm` loses its
    /// extension.
    pub fn movie(&self, name: &str) -> Option<&str> {
        let name = name.split('.').next().unwrap_or(name);
        self.manifest.movies.get(name).map(String::as_str)
    }

    /// Gives the lip sync keys of a voice. The first call reads `lip.json`.
    pub fn lip(&self, number: i32) -> Option<&Lip> {
        let all = self.lip.get_or_init(|| {
            self.manifest
                .lip
                .as_ref()
                .and_then(|file| std::fs::read_to_string(self.root.join(file)).ok())
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default()
        });
        all.get(&format!("{number:06}"))
    }

    /// Gives the event images of the gallery in the order of their number.
    pub fn gallery(&self) -> Vec<GalleryImage> {
        let mut all: Vec<GalleryImage> = self
            .events
            .iter()
            .filter_map(|((game, name), (file, number))| {
                Some(GalleryImage {
                    name: name.clone(),
                    file: self.image("ev", file)?.to_string(),
                    number: *number,
                    game: *game,
                })
            })
            .collect();
        all.sort_by_key(|g| g.number);
        all
    }

    /// Gives the music tracks, by number.
    pub fn music_list(&self) -> Vec<(i32, &Track)> {
        let Some(group) = self.manifest.audio.get("bgm") else {
            return Vec::new();
        };
        let mut all: Vec<(i32, &Track)> = group
            .iter()
            .filter_map(|(name, track)| Some((name.strip_prefix("BGM")?.parse().ok()?, track)))
            .collect();
        all.sort_by_key(|(n, _)| *n);
        all
    }
}
