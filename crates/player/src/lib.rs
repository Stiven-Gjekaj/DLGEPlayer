//! The Godot extension of the player.
//!
//! `DlPlayer` gives the `scene` crate to GDScript. Each event and each
//! list comes to GDScript as a `Dictionary` or an `Array`, through JSON
//! values. The screen code is in the Godot project in `godot/`.

use std::path::Path;

use godot::prelude::*;
use scene::{Assets, Director, Event, Profile, SaveFile, menu};

mod bleed;
mod import;
pub mod movie;
mod value;

struct Extension;

#[gdextension]
unsafe impl ExtensionLibrary for Extension {}

/// The data of one import folder. The player keeps it until the program
/// stops, so a `Director` can borrow it.
struct Library {
    program: vm::Program,
    assets: Assets,
}

/// The player for GDScript.
#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct DlPlayer {
    library: Option<&'static Library>,
    director: Option<Director<'static>>,
    /// The profile when no chapter plays. A director holds it while a
    /// chapter plays.
    profile: Profile,
    error: String,
}

fn error(message: impl ToString) -> Dictionary<Variant, Variant> {
    let mut d = Dictionary::new();
    d.set("kind", "error");
    d.set("message", message.to_string());
    d
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl DlPlayer {
    fn profile(&self) -> &Profile {
        match &self.director {
            Some(d) => &d.profile,
            None => &self.profile,
        }
    }

    fn profile_mut(&mut self) -> &mut Profile {
        match &mut self.director {
            Some(d) => &mut d.profile,
            None => &mut self.profile,
        }
    }

    fn events(events: Vec<Event>) -> Array<Variant> {
        let mut out = Array::new();
        for e in events {
            out.push(&value::to_variant(&e));
        }
        out
    }
}

#[godot_api]
impl DlPlayer {
    /// Opens an import folder. Gives an empty text, or the error.
    #[func]
    fn open(&mut self, folder: GString) -> GString {
        let folder = folder.to_string();
        let result = (|| -> Result<Library, String> {
            let path = Path::new(&folder);
            let program = vm::Program::load(path).map_err(|e| e.to_string())?;
            let assets = Assets::load(path).map_err(|e| e.to_string())?;
            Ok(Library { program, assets })
        })();
        match result {
            Ok(library) => {
                self.director = None;
                // The library lives until the program stops.
                self.library = Some(Box::leak(Box::new(library)));
                GString::new()
            }
            Err(e) => GString::from(&e),
        }
    }

    /// Gives the full path of the import folder, with a `/` at the end.
    #[func]
    fn root(&self) -> GString {
        match self.library {
            Some(l) => GString::from(&format!("{}/", l.assets.root.display())),
            None => GString::new(),
        }
    }

    /// Reads a profile from JSON text. Bad text gives a new profile.
    #[func]
    fn set_profile(&mut self, text: GString) {
        *self.profile_mut() = Profile::parse(&text.to_string()).unwrap_or_default();
    }

    /// Gives the profile as JSON text.
    #[func]
    fn get_profile(&self) -> GString {
        GString::from(&self.profile().to_json())
    }

    #[func]
    fn set_names(&mut self, family: GString, given: GString) {
        let profile = self.profile_mut();
        profile.family_name = family.to_string();
        profile.given_name = given.to_string();
    }

    #[func]
    fn get_names(&self) -> PackedStringArray {
        let p = self.profile();
        PackedStringArray::from(&[GString::from(&p.family_name), GString::from(&p.given_name)])
    }

    /// Gives the chapters of the menu.
    #[func]
    fn chapters(&self, open_all: bool) -> Array<Variant> {
        let Some(library) = self.library else {
            return Array::new();
        };
        let list = menu::chapters(&library.program, self.profile(), open_all);
        match value::to_variant(&list).try_to::<Array<Variant>>() {
            Ok(a) => a,
            Err(_) => Array::new(),
        }
    }

    /// Gives the event images of the gallery. Each has the key `seen`.
    #[func]
    fn gallery(&self) -> Array<Variant> {
        let Some(library) = self.library else {
            return Array::new();
        };
        let mut out = Array::new();
        for image in library.assets.gallery() {
            let mut d = Dictionary::<Variant, Variant>::new();
            d.set("name", image.name.as_str());
            d.set("file", image.file.as_str());
            d.set("number", image.number);
            d.set("game", image.game);
            d.set("seen", self.profile().seen.contains(&image.number));
            out.push(&d.to_variant());
        }
        out
    }

    /// Gives the music tracks of the sound room.
    #[func]
    fn music(&self) -> Array<Variant> {
        let Some(library) = self.library else {
            return Array::new();
        };
        let mut out = Array::new();
        for (number, track) in library.assets.music_list() {
            let mut d = Dictionary::<Variant, Variant>::new();
            d.set("number", number);
            d.set("file", track.file.as_str());
            d.set("sample_rate", track.sample_rate as i64);
            d.set("loop_start", track.loop_start.unwrap_or(0) as i64);
            out.push(&d.to_variant());
        }
        out
    }

    /// Starts a chapter. Gives an empty text, or the error.
    #[func]
    fn start(&mut self, chapter: GString) -> GString {
        let Some(library) = self.library else {
            return GString::from("no import folder is open");
        };
        let profile = self.profile().clone();
        match Director::new(
            &library.program,
            &library.assets,
            profile,
            &chapter.to_string(),
        ) {
            Ok(d) => {
                self.director = Some(d);
                GString::new()
            }
            Err(e) => GString::from(&e.to_string()),
        }
    }

    /// Stops the chapter. The profile keeps its changes.
    #[func]
    fn stop(&mut self) {
        if let Some(d) = self.director.take() {
            self.profile = d.profile;
        }
    }

    /// Gives the next event as a `Dictionary` with the key `kind`.
    #[func]
    fn next_event(&mut self) -> Dictionary<Variant, Variant> {
        let Some(director) = &mut self.director else {
            return error("no chapter plays");
        };
        match director.next_event() {
            Ok(e) => value::to_variant(&e).to(),
            Err(e) => {
                self.error = e.to_string();
                error(e)
            }
        }
    }

    /// Gives the pick of a choice, 0 for the first option.
    #[func]
    fn choose(&mut self, pick: i32) -> GString {
        match &mut self.director {
            Some(d) => match d.choose(pick) {
                Ok(()) => GString::new(),
                Err(e) => GString::from(&e.to_string()),
            },
            None => GString::from("no chapter plays"),
        }
    }

    /// Gives the pick of the character select, from 1 to 6.
    #[func]
    fn choose_character(&mut self, pick: i32) -> GString {
        match &mut self.director {
            Some(d) => match d.choose_character(pick) {
                Ok(()) => GString::new(),
                Err(e) => GString::from(&e.to_string()),
            },
            None => GString::from("no chapter plays"),
        }
    }

    /// Gives the messages of the backlog, the oldest first.
    #[func]
    fn backlog(&self) -> Array<Variant> {
        match &self.director {
            Some(d) => value::to_variant(&d.backlog()).to(),
            None => Array::new(),
        }
    }

    /// Writes a save of the current event to a file. Gives an empty text,
    /// or the error.
    #[func]
    fn save_to(&self, path: GString) -> GString {
        let Some(d) = &self.director else {
            return GString::from("no chapter plays");
        };
        let path = path.to_string();
        let save = d.save(now());
        let result = (|| -> Result<(), String> {
            if let Some(dir) = Path::new(&path).parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let text = serde_json::to_string(&save).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| e.to_string())
        })();
        match result {
            Ok(()) => GString::new(),
            Err(e) => GString::from(&e),
        }
    }

    /// Reads a save file. Gives the events that build the screen, or an
    /// array with one error event.
    #[func]
    fn load_from(&mut self, path: GString) -> Array<Variant> {
        let Some(library) = self.library else {
            return Self::events(vec![]);
        };
        let save = match read_save(&path.to_string()) {
            Ok(s) => s,
            Err(e) => {
                let mut a = Array::new();
                a.push(&error(e).to_variant());
                return a;
            }
        };
        let profile = self.profile().clone();
        match Director::load(&library.program, &library.assets, profile, save) {
            Ok((d, events)) => {
                self.director = Some(d);
                Self::events(events)
            }
            Err(e) => {
                let mut a = Array::new();
                a.push(&error(e).to_variant());
                a
            }
        }
    }

    /// Gives the data of a save file for the save menu, or an empty
    /// dictionary.
    #[func]
    fn save_info(&self, path: GString) -> Dictionary<Variant, Variant> {
        let Ok(save) = read_save(&path.to_string()) else {
            return Dictionary::new();
        };
        let mut d = Dictionary::new();
        d.set("chapter", save.chapter.as_str());
        d.set("speaker", save.speaker.as_str());
        d.set("text", save.text.join(" ").as_str());
        d.set("picture", save.picture.as_deref().unwrap_or(""));
        d.set("time", save.time as i64);
        d
    }

    /// Gives the chapter that plays, or an empty text.
    #[func]
    fn chapter(&self) -> GString {
        match &self.director {
            Some(d) => GString::from(d.chapter()),
            None => GString::new(),
        }
    }

    /// Gives the events that build the current scene on an empty screen.
    #[func]
    fn rebuild(&self) -> Array<Variant> {
        match &self.director {
            Some(d) => Self::events(d.rebuild()),
            None => Array::new(),
        }
    }

    /// Gives colour to the transparent pixels at the edges of an RGBA
    /// image, so that a scaled sprite shows no dark line at its edges.
    #[func]
    fn bleed_alpha(mut image: Gd<godot::classes::Image>) {
        if image.get_format() != godot::classes::image::Format::RGBA8 {
            image.convert(godot::classes::image::Format::RGBA8);
        }
        let (w, h) = (image.get_width() as usize, image.get_height() as usize);
        let mut data = image.get_data().to_vec();
        bleed::bleed(&mut data, w, h);
        image.set_data(
            w as i32,
            h as i32,
            false,
            godot::classes::image::Format::RGBA8,
            &PackedByteArray::from(data.as_slice()),
        );
    }

    /// Gives the names that gave no file.
    #[func]
    fn missing(&self) -> PackedStringArray {
        let mut out = PackedStringArray::new();
        if let Some(d) = &self.director {
            for name in d.missing() {
                out.push(&GString::from(&name));
            }
        }
        out
    }
}

fn read_save(path: &str) -> Result<SaveFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// A movie for GDScript. `advance` decodes the pictures up to a time, and
/// `image` gives the last picture.
#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct DlMovie {
    movie: Option<movie::Movie>,
}

#[godot_api]
impl DlMovie {
    /// Opens a movie file. Gives an empty text, or the error.
    #[func]
    fn open(&mut self, path: GString) -> GString {
        match movie::Movie::open(Path::new(&path.to_string())) {
            Ok(m) => {
                self.movie = Some(m);
                GString::new()
            }
            Err(e) => GString::from(&e),
        }
    }

    /// Decodes the pictures up to the time in seconds. Gives true when the
    /// picture changed.
    #[func]
    fn advance(&mut self, seconds: f64) -> bool {
        match &mut self.movie {
            Some(m) => m.advance(seconds).unwrap_or_else(|e| {
                godot_warn!("movie: {e}");
                false
            }),
            None => false,
        }
    }

    #[func]
    fn finished(&self) -> bool {
        self.movie.as_ref().is_none_or(|m| m.finished())
    }

    #[func]
    fn duration(&self) -> f64 {
        self.movie.as_ref().map(|m| m.duration()).unwrap_or(0.0)
    }

    /// Gives the last picture.
    #[func]
    fn image(&self) -> Option<Gd<godot::classes::Image>> {
        let m = self.movie.as_ref()?;
        godot::classes::Image::create_from_data(
            m.width as i32,
            m.height as i32,
            false,
            godot::classes::image::Format::RGB8,
            &PackedByteArray::from(m.rgb.as_slice()),
        )
    }

    /// Gives the sound of a movie file, or null when the movie has no sound.
    #[func]
    fn sound(path: GString) -> Option<Gd<godot::classes::AudioStreamWav>> {
        let sound = match movie::decode_sound(Path::new(&path.to_string())) {
            Ok(Some(s)) => s,
            Ok(None) => return None,
            Err(e) => {
                godot_warn!("movie sound: {e}");
                return None;
            }
        };
        let mut bytes = Vec::with_capacity(sound.samples.len() * 2);
        for s in &sound.samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        let mut stream = godot::classes::AudioStreamWav::new_gd();
        stream.set_format(godot::classes::audio_stream_wav::Format::FORMAT_16_BITS);
        stream.set_stereo(sound.channels > 1);
        stream.set_mix_rate(sound.rate as i32);
        stream.set_data(&PackedByteArray::from(bytes.as_slice()));
        Some(stream)
    }
}

/// The import of the first start, for GDScript. A thread runs the
/// importer, and the screen asks for the progress.
#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct DlImporter {
    progress: std::sync::Arc<std::sync::Mutex<import::Progress>>,
}

#[godot_api]
impl DlImporter {
    /// Starts an import. `keys` can be empty for a RomFS folder.
    /// `translation` holds the bytes of a patch file, or nothing. With
    /// `mobile`, the import writes smaller files. `jobs` is the number of
    /// threads, or 0 for half of the CPU cores.
    #[func]
    fn start(
        &mut self,
        input: GString,
        keys: GString,
        out: GString,
        translation: PackedByteArray,
        mobile: bool,
        jobs: i32,
    ) {
        let jobs = if jobs > 0 {
            jobs as usize
        } else {
            std::thread::available_parallelism()
                .map(|n| (n.get() / 2).max(1))
                .unwrap_or(2)
        };
        let mut options = if mobile {
            importer::import::Options::mobile()
        } else {
            importer::import::Options::default()
        };
        options.jobs = Some(jobs);
        let keys = keys.to_string();
        let job = import::Job {
            input: input.to_string().into(),
            keys: (!keys.is_empty()).then(|| keys.into()),
            out: out.to_string().into(),
            translation: (!translation.is_empty()).then(|| translation.to_vec()),
            options,
        };
        self.progress = Default::default();
        let progress = self.progress.clone();
        std::thread::spawn(move || import::run(job, progress));
    }

    /// Gives the lines of progress after the first `from` lines.
    #[func]
    fn lines(&self, from: i32) -> PackedStringArray {
        let mut out = PackedStringArray::new();
        if let Ok(p) = self.progress.lock() {
            for line in p.lines.iter().skip(from.max(0) as usize) {
                out.push(&GString::from(line));
            }
        }
        out
    }

    #[func]
    fn done(&self) -> bool {
        self.progress.lock().map(|p| p.done).unwrap_or(true)
    }

    /// Gives the error of the import, or an empty text.
    #[func]
    fn error(&self) -> GString {
        match self.progress.lock() {
            Ok(p) => GString::from(p.error.as_deref().unwrap_or("")),
            Err(_) => GString::from("the import stopped"),
        }
    }
}
