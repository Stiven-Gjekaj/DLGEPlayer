//! Turns the commands of the VM into events for a screen.
//!
//! The VM gives the commands of one run of a label. The director gives
//! them to the screen one event at a time. A command that changes only the
//! flags gives no event. A choice waits until the screen calls `choose`.
//!
//! The director keeps the state of the screen in a `Scene`: the
//! background, the sprites, the music, and the values that the next
//! message reads, such as the speaker. A save holds the state of the VM and
//! the scene at the start of a run, the picks of the run, and the index of
//! the current command. A load runs the label again and plays the commands
//! before the index with no events. The VM and the scene come to the same
//! state, because a run gives the same commands from the same state.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use chara::Sprite;
use serde::{Deserialize, Serialize};
use vm::{ChoiceOption, Command, Program, Session, Step, Val};

use crate::Error;
use crate::assets::{Assets, Lip};
use crate::profile::Profile;

/// The largest number of lines in the backlog.
pub const BACKLOG_LIMIT: usize = 300;

/// The number of runs in a row with no event before the director stops.
const QUIET_LIMIT: usize = 10_000;

/// A sprite on the screen, or a sprite that a script prepared.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FigureState {
    /// The second name of the script call, for example `aya_03_maji`.
    pub look: String,
    /// The colour of the sprite. 128 is the normal value, and -1 is the
    /// default.
    pub tint: [i32; 3],
    pub slot: i32,
    pub layer: i32,
    pub offset: i32,
    pub visible: bool,
}

/// The background layer: a background of `bg.bin` or an event image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backdrop {
    pub name: String,
    /// The name came from `0x3fb`, which also shows event images.
    pub picture: bool,
}

/// The camera of `0x4f4` and `0x3f4`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            x: 0,
            y: 0,
            zoom: 1.0,
        }
    }
}

/// The state of the screen.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub backdrop: Option<Backdrop>,
    pub foreground: Option<String>,
    /// The sprites by the first name of the script call, for example
    /// `aya_03`.
    pub figures: BTreeMap<String, FigureState>,
    pub music: Option<i32>,
    pub ambience: Option<i32>,
    pub window: bool,
    pub camera: Camera,
    /// The speaker of the next message, from `speaker.bin`.
    pub speaker: i32,
    /// The voice of the next message.
    pub voice: Option<i32>,
    /// The touch points of `0x69a`: an index, a position, and a voice.
    pub spots: Vec<[i32; 4]>,
    /// The shake of `0x513` for each axis: the size and the count.
    pub shake: [[i32; 2]; 2],
}

/// A touch point of a picture.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hotspot {
    /// The position on a screen of 960 by 544.
    pub x: i32,
    pub y: i32,
    pub voice: Option<String>,
}

/// One event for the screen. A frame is 1/60 of a second.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// Show a background or an event image. With no file, the screen is
    /// black.
    Background {
        name: String,
        file: Option<String>,
        place: String,
        frames: i32,
    },
    Foreground {
        file: Option<String>,
        frames: i32,
    },
    /// Show a sprite at a place.
    Figure {
        key: String,
        file: String,
        layout: Sprite,
        expression: usize,
        tint: [i32; 3],
        slot: i32,
        layer: i32,
        offset: i32,
        frames: i32,
    },
    /// Change the expression of a sprite on the screen.
    Look {
        key: String,
        file: String,
        layout: Sprite,
        expression: usize,
        tint: [i32; 3],
    },
    /// Move a sprite to another place.
    Move {
        key: String,
        slot: i32,
        layer: i32,
        frames: i32,
    },
    Hide {
        key: String,
        frames: i32,
    },
    Music {
        file: String,
        sample_rate: u32,
        loop_start: u64,
        fade: i32,
    },
    MusicStop {
        fade: i32,
    },
    Ambience {
        file: String,
        fade: i32,
    },
    AmbienceStop {
        fade: i32,
    },
    Sound {
        file: String,
    },
    /// Wait until the sound effect ends.
    SoundWait,
    /// Play a second voice at the same time as the message.
    ExtraVoice {
        file: String,
    },
    /// Show a message and wait for the player.
    Text {
        speaker: String,
        character: String,
        lines: Vec<String>,
        voice: Option<String>,
        lip: Option<Lip>,
        /// The player saw this message before.
        read: bool,
    },
    /// Wait until the player picks an option. Then call `choose`.
    Choice {
        options: Vec<String>,
    },
    /// Wait until the player picks a character. Then call
    /// `choose_character`.
    SelectCharacter,
    Wait {
        frames: i32,
    },
    Window {
        visible: bool,
    },
    /// Change the screen with a fade. The changes after this event appear
    /// at the end of the fade.
    Transition {
        frames: i32,
        mode: i32,
    },
    /// Fill the screen with a colour.
    Tint {
        layer: i32,
        colour: i32,
        alpha: i32,
        frames: i32,
    },
    TintEnd {
        colour: i32,
        frames: i32,
    },
    Shake {
        axis: i32,
        size: i32,
        count: i32,
        frames: i32,
    },
    Camera {
        x: i32,
        y: i32,
        zoom: f32,
        frames: i32,
    },
    Movie {
        file: String,
    },
    /// Wait until the player touches each point.
    Hotspots {
        spots: Vec<Hotspot>,
    },
    /// The chapter is over. `next` is the next chapter of the route.
    End {
        next: Option<String>,
    },
    /// An opcode that the player does not show.
    Other {
        opcode: u32,
        args: Vec<String>,
    },
}

/// One message of the backlog.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    pub speaker: String,
    pub lines: Vec<String>,
    pub voice: Option<String>,
}

/// The data of a save.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    pub chapter: String,
    /// The state of the VM at the start of the run.
    pub machine: vm::Save,
    /// The scene at the start of the run.
    pub scene: Scene,
    /// The index of the current command in the run.
    pub index: usize,
    /// The picks of the run, in order.
    pub picks: Vec<i32>,
    /// The speaker and the text of the current message, for the menu.
    pub speaker: String,
    pub text: Vec<String>,
    /// The image of the background, for the menu.
    pub picture: Option<String>,
    /// The time of the save, in seconds since 1970.
    pub time: u64,
}

/// A host for the VM that gives a fixed pick.
struct Fixed(i32);

impl vm::Host for Fixed {
    fn choose(&mut self, _name: &str, _options: &[ChoiceOption]) -> usize {
        self.0.max(0) as usize
    }
    fn choose_character(&mut self) -> i32 {
        self.0
    }
}

/// What the director waits for.
#[derive(Debug, Clone, PartialEq)]
enum Pending {
    Choice(Command),
    Character(Command),
}

/// Plays one chapter for a screen.
pub struct Director<'a> {
    program: &'a Program,
    assets: &'a Assets,
    session: Session<'a>,
    pub profile: Profile,
    chapter: String,
    commands: Vec<Command>,
    /// The index of the next command.
    index: usize,
    /// The index of the command of the last event.
    current: usize,
    /// The state of the VM and the scene at the start of the run.
    start: (vm::Save, Scene),
    picks: Vec<i32>,
    scene: Scene,
    pending: Option<Pending>,
    backlog: Vec<Line>,
    last_text: (String, Vec<String>),
    /// A run started, so the next run needs `finish` first.
    started: bool,
    ended: bool,
    /// The names that gave no file, for example `music 3`.
    missing: RefCell<BTreeSet<String>>,
}

/// Gives a text argument.
fn text(args: &[Val], i: usize) -> String {
    match args.get(i) {
        Some(Val::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// Gives an integer argument.
fn int(args: &[Val], i: usize) -> i32 {
    args.get(i).map(Val::as_int).unwrap_or(0)
}

/// Gives a number argument.
fn float(args: &[Val], i: usize) -> f32 {
    match args.get(i) {
        Some(Val::Float(f)) => *f,
        Some(v) => v.as_int() as f32,
        None => 0.0,
    }
}

fn describe(args: &[Val]) -> Vec<String> {
    args.iter()
        .map(|v| match v {
            Val::Int(i) => i.to_string(),
            Val::Float(f) => f.to_string(),
            Val::Text(t) => t.clone(),
            Val::Bytes(b) => format!("{} bytes", b.len()),
            Val::Code(c) => format!("code {c}"),
        })
        .collect()
}

/// A 64-bit FNV-1a hash.
fn hash(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in parts {
        for b in part.iter().chain([&0u8]) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

impl<'a> Director<'a> {
    /// Starts a chapter at `sure1`. The script flags have their first
    /// values. The system flags come from the profile.
    ///
    /// A chapter often shows no background before its first message,
    /// because the background of the chapter before it stays on the screen.
    /// So the director plays the chapter before it with no screen, and
    /// starts with its last background. `rebuild` gives the events that
    /// show it.
    pub fn new(
        program: &'a Program,
        assets: &'a Assets,
        profile: Profile,
        chapter: &str,
    ) -> Result<Director<'a>, Error> {
        let before = crate::menu::previous_chapter(program, chapter)
            .and_then(|name| Director::bare(program, assets, profile.clone(), &name).ok())
            .and_then(|mut d| d.last_scene());
        let mut director = Director::bare(program, assets, profile, chapter)?;
        if let Some(scene) = before {
            director.scene.backdrop = scene.backdrop;
            director.start.1 = director.scene.clone();
        }
        Ok(director)
    }

    /// Plays the chapter to its end with the first option of each choice,
    /// and gives the scene at the end.
    fn last_scene(&mut self) -> Option<Scene> {
        for _ in 0..1_000_000 {
            match self.next_event().ok()? {
                Event::End { .. } => return Some(self.scene.clone()),
                Event::Choice { .. } => self.choose(0).ok()?,
                Event::SelectCharacter => {
                    let pick = self.profile.character;
                    self.choose_character(pick).ok()?
                }
                _ => {}
            }
        }
        None
    }

    fn bare(
        program: &'a Program,
        assets: &'a Assets,
        profile: Profile,
        chapter: &str,
    ) -> Result<Director<'a>, Error> {
        let mut flags = vm::Flags {
            script: program.flags.clone(),
            system: program.system_flags.clone(),
        };
        flags.system.extend(profile.system.clone());
        // A chapter after the character select reads the pick.
        flags.script.insert("character".into(), profile.character);
        let session = Session::with_flags(program, chapter, "sure1", flags)?;
        let start = (session.save(), Scene::default());
        Ok(Director {
            program,
            assets,
            session,
            profile,
            chapter: chapter.to_string(),
            commands: Vec::new(),
            index: 0,
            current: 0,
            start,
            picks: Vec::new(),
            scene: Scene::default(),
            pending: None,
            backlog: Vec::new(),
            last_text: Default::default(),
            started: false,
            ended: false,
            missing: RefCell::default(),
        })
    }

    /// Continues a save. Gives the director and the events that build the
    /// screen again.
    pub fn load(
        program: &'a Program,
        assets: &'a Assets,
        profile: Profile,
        save: SaveFile,
    ) -> Result<(Director<'a>, Vec<Event>), Error> {
        let mut machine = save.machine.clone();
        // The system flags of the profile are newer than the save.
        for (name, value) in &profile.system {
            machine.flags.system.insert(name.clone(), *value);
        }
        let session = Session::load(program, machine.clone())?;
        let mut director = Director {
            program,
            assets,
            session,
            profile,
            chapter: save.chapter.clone(),
            commands: Vec::new(),
            index: 0,
            current: 0,
            start: (machine, save.scene.clone()),
            picks: Vec::new(),
            scene: save.scene.clone(),
            pending: None,
            backlog: Vec::new(),
            last_text: Default::default(),
            started: false,
            ended: false,
            missing: RefCell::default(),
        };
        director.commands = director.session.run()?;
        director.started = true;
        director.current = save.index;
        let mut picks = save.picks.iter().copied();
        while director.index < save.index.min(director.commands.len()) {
            let command = director.commands[director.index].clone();
            director.index += 1;
            match command {
                Command::Choice { .. } | Command::SelectCharacter => {
                    let pick = picks.next().unwrap_or(0);
                    director.session.play(&command, &mut Fixed(pick))?;
                    director.picks.push(pick);
                }
                _ => {
                    director.session.play(&command, &mut Fixed(0))?;
                    director.translate(&command);
                }
            }
        }
        let events = director.rebuild();
        Ok((director, events))
    }

    /// Makes a save of the current event.
    pub fn save(&self, time: u64) -> SaveFile {
        let picture = self.scene.backdrop.as_ref().and_then(|b| {
            self.assets
                .picture(self.game_number(), &b.name)
                .map(|p| p.file)
        });
        SaveFile {
            chapter: self.chapter.clone(),
            machine: self.start.0.clone(),
            scene: self.start.1.clone(),
            index: self.current,
            picks: self.picks.clone(),
            speaker: self.last_text.0.clone(),
            text: self.last_text.1.clone(),
            picture,
            time,
        }
    }

    /// Gives the names that gave no file.
    pub fn missing(&self) -> Vec<String> {
        self.missing.borrow().iter().cloned().collect()
    }

    /// Records a name that gives no file.
    fn found<T>(&self, what: impl FnOnce() -> String, value: Option<T>) -> Option<T> {
        if value.is_none() {
            self.missing.borrow_mut().insert(what());
        }
        value
    }

    pub fn chapter(&self) -> &str {
        &self.chapter
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn backlog(&self) -> &[Line] {
        &self.backlog
    }

    /// Gives the next event. After `Choice` or `SelectCharacter`, the
    /// screen must give the pick first.
    pub fn next_event(&mut self) -> Result<Event, Error> {
        if let Some(pending) = &self.pending {
            return Err(Error::Waiting(format!("{pending:?}")));
        }
        if self.ended {
            return Ok(Event::End {
                next: crate::menu::next_chapter(self.program, &self.chapter),
            });
        }
        let mut quiet = 0;
        loop {
            if self.index >= self.commands.len() {
                if self.started && self.session.finish(self.commands.is_empty())? == Step::End {
                    return Ok(self.end());
                }
                quiet += 1;
                if quiet > QUIET_LIMIT {
                    return Err(Error::Stuck(self.chapter.clone()));
                }
                self.start = (self.session.save(), self.scene.clone());
                self.picks.clear();
                self.commands = self.session.run()?;
                self.index = 0;
                self.started = true;
                continue;
            }
            let command = self.commands[self.index].clone();
            self.current = self.index;
            self.index += 1;
            match &command {
                Command::Choice { name } => {
                    let options = self
                        .program
                        .choices
                        .get(name)
                        .ok_or_else(|| Error::Vm(vm::Error::UnknownChoice(name.clone())))?
                        .iter()
                        .map(|o| self.names(&o.text))
                        .collect();
                    self.pending = Some(Pending::Choice(command.clone()));
                    return Ok(Event::Choice { options });
                }
                Command::SelectCharacter => {
                    self.pending = Some(Pending::Character(command.clone()));
                    return Ok(Event::SelectCharacter);
                }
                _ => {}
            }
            self.session.play(&command, &mut Fixed(0))?;
            if matches!(command, Command::End | Command::Finish) {
                continue;
            }
            if let Some(event) = self.translate(&command) {
                return Ok(event);
            }
        }
    }

    fn end(&mut self) -> Event {
        self.ended = true;
        self.profile.finished.insert(self.chapter.clone());
        self.profile.system = self.session.flags.system.clone();
        Event::End {
            next: crate::menu::next_chapter(self.program, &self.chapter),
        }
    }

    /// Gives the pick of a choice, 0 for the first option.
    pub fn choose(&mut self, pick: i32) -> Result<(), Error> {
        match self.pending.take() {
            Some(Pending::Choice(command)) => {
                self.session.play(&command, &mut Fixed(pick))?;
                self.picks.push(pick);
                Ok(())
            }
            other => {
                self.pending = other;
                Err(Error::Waiting("no choice".into()))
            }
        }
    }

    /// Gives the pick of the character select, from 1 to 6.
    pub fn choose_character(&mut self, pick: i32) -> Result<(), Error> {
        match self.pending.take() {
            Some(Pending::Character(command)) => {
                self.session.play(&command, &mut Fixed(pick))?;
                self.picks.push(pick);
                self.profile.character = pick;
                Ok(())
            }
            other => {
                self.pending = other;
                Err(Error::Waiting("no character select".into()))
            }
        }
    }

    /// Puts the names of the heroine into a text.
    fn names(&self, text: &str) -> String {
        text.replace("#Name[1]", &self.profile.family_name)
            .replace("#Name[2]", &self.profile.given_name)
    }

    /// Gives the game prefix of the current script: `ve` or `mb`.
    fn game(&self) -> &str {
        self.session.script().get(..2).unwrap_or("ve")
    }

    /// Gives the game number of the current script: 0 for `ve`, 1 for
    /// `mb`.
    fn game_number(&self) -> i32 {
        (self.game() == "mb") as i32
    }

    fn figure_event(&self, key: &str, state: &FigureState, frames: i32) -> Option<Event> {
        let figure = self.found(
            || format!("sprite {}", state.look),
            self.assets.figure(&state.look),
        )?;
        Some(Event::Figure {
            key: key.to_string(),
            file: figure.file,
            layout: figure.layout,
            expression: figure.expression,
            tint: state.tint,
            slot: state.slot,
            layer: state.layer,
            offset: state.offset,
            frames,
        })
    }

    fn backdrop_event(&self, backdrop: Option<&Backdrop>, frames: i32) -> Event {
        let Some(backdrop) = backdrop else {
            return Event::Background {
                name: String::new(),
                file: None,
                place: String::new(),
                frames,
            };
        };
        let picture = self.assets.picture(self.game_number(), &backdrop.name);
        if backdrop.name != "bg_black" {
            self.found(|| format!("picture {}", backdrop.name), picture.as_ref());
        }
        Event::Background {
            name: backdrop.name.clone(),
            file: picture.as_ref().map(|p| p.file.clone()),
            place: picture.map(|p| p.place).unwrap_or_default(),
            frames,
        }
    }

    /// Gives the events that build the current scene on an empty screen.
    pub fn rebuild(&self) -> Vec<Event> {
        let mut events = vec![self.backdrop_event(self.scene.backdrop.as_ref(), 0)];
        events.push(Event::Foreground {
            file: self
                .scene
                .foreground
                .as_deref()
                .and_then(|f| self.assets.foreground(f))
                .map(str::to_string),
            frames: 0,
        });
        for (key, state) in &self.scene.figures {
            if state.visible
                && let Some(e) = self.figure_event(key, state, 0)
            {
                events.push(e);
            }
        }
        let c = self.scene.camera;
        events.push(Event::Camera {
            x: c.x,
            y: c.y,
            zoom: c.zoom,
            frames: 0,
        });
        events.push(match self.scene.music.and_then(|n| self.assets.music(n)) {
            Some(t) => Event::Music {
                file: t.file.clone(),
                sample_rate: t.sample_rate,
                loop_start: t.loop_start.unwrap_or(0),
                fade: 0,
            },
            None => Event::MusicStop { fade: 0 },
        });
        events.push(
            match self.scene.ambience.and_then(|n| self.assets.ambience(n)) {
                Some(t) => Event::Ambience {
                    file: t.file.clone(),
                    fade: 0,
                },
                None => Event::AmbienceStop { fade: 0 },
            },
        );
        events.push(Event::Window {
            visible: self.scene.window,
        });
        events
    }

    /// Changes the scene for a command and gives its event.
    fn translate(&mut self, command: &Command) -> Option<Event> {
        let (opcode, args) = match command {
            Command::Message(lines) => return Some(self.message(lines)),
            Command::Voice { id, .. } => {
                self.scene.voice = Some(*id);
                return None;
            }
            Command::SoundEffect { id, .. } => {
                let file = self
                    .found(|| format!("sound {id}"), self.assets.sound(*id))?
                    .file
                    .clone();
                return Some(Event::Sound { file });
            }
            Command::Engine { opcode, args } | Command::Unregistered { opcode, args } => {
                (*opcode, args.as_slice())
            }
            _ => return None,
        };
        let game_number = self.game_number();
        let scene = &mut self.scene;
        Some(match opcode {
            // The speaker of the next message.
            0x4bc => {
                scene.speaker = int(args, 0);
                return None;
            }
            0x4be => Event::Wait {
                frames: int(args, 0),
            },
            0x4dd => {
                scene.window = int(args, 0) != 0;
                Event::Window {
                    visible: scene.window,
                }
            }
            0x491 => Event::Transition {
                frames: float(args, 1) as i32,
                mode: int(args, 2),
            },
            0x3fa => {
                scene.backdrop = Some(Backdrop {
                    name: text(args, 0),
                    picture: false,
                });
                let backdrop = scene.backdrop.clone();
                self.backdrop_event(backdrop.as_ref(), int(args, 3))
            }
            0x3fb => {
                let name = text(args, 0);
                if let Some(number) = self
                    .assets
                    .picture(game_number, &name)
                    .and_then(|p| p.gallery)
                {
                    self.profile.seen.insert(number);
                }
                scene.backdrop = Some(Backdrop {
                    name,
                    picture: true,
                });
                let backdrop = scene.backdrop.clone();
                // The value is a time. 3.0 is about half a second.
                self.backdrop_event(backdrop.as_ref(), (float(args, 1) * 10.0) as i32)
            }
            0x402 => {
                scene.foreground = Some(text(args, 0));
                Event::Foreground {
                    file: self.assets.foreground(&text(args, 0)).map(str::to_string),
                    frames: int(args, 3),
                }
            }
            0x403 => {
                scene.foreground = None;
                Event::Foreground {
                    file: None,
                    frames: int(args, 2),
                }
            }
            // Prepare a sprite: the pose, the look, a motion, and the
            // colour. 0x61c is the same for the heroine.
            0x47e | 0x61c => {
                let state = scene.figures.entry(text(args, 0)).or_default();
                state.look = text(args, 1);
                state.tint = [int(args, 3), int(args, 4), int(args, 5)];
                return None;
            }
            // Change the look of a sprite.
            0x47f | 0x61d => {
                let key = text(args, 0);
                let state = scene.figures.entry(key.clone()).or_default();
                state.look = text(args, 1);
                state.tint = [int(args, 3), int(args, 4), int(args, 5)];
                if !state.visible {
                    return None;
                }
                let state = state.clone();
                let figure = self.found(
                    || format!("sprite {}", state.look),
                    self.assets.figure(&state.look),
                )?;
                Event::Look {
                    key,
                    file: figure.file,
                    layout: figure.layout,
                    expression: figure.expression,
                    tint: state.tint,
                }
            }
            // Show a sprite at a place: the slot, the layer, the offset,
            // and the time.
            0x480 => {
                let key = text(args, 0);
                let state = scene.figures.entry(key.clone()).or_default();
                state.slot = int(args, 1);
                state.layer = int(args, 2);
                state.offset = int(args, 3);
                state.visible = true;
                let state = state.clone();
                self.figure_event(&key, &state, int(args, 4))?
            }
            0x483 => {
                let key = text(args, 0);
                let state = scene.figures.entry(key.clone()).or_default();
                state.slot = int(args, 1);
                state.layer = int(args, 2);
                Event::Move {
                    key,
                    slot: state.slot,
                    layer: state.layer,
                    frames: int(args, 3),
                }
            }
            0x482 => {
                let key = text(args, 0);
                scene.figures.remove(&key);
                Event::Hide {
                    key,
                    frames: int(args, 2),
                }
            }
            0x456 | 0x690 => {
                scene.music = Some(int(args, 0));
                let t = self.found(
                    || format!("music {}", int(args, 0)),
                    self.assets.music(int(args, 0)),
                )?;
                Event::Music {
                    file: t.file.clone(),
                    sample_rate: t.sample_rate,
                    loop_start: t.loop_start.unwrap_or(0),
                    fade: int(args, 1),
                }
            }
            0x457 => {
                scene.music = None;
                Event::MusicStop { fade: 1000 }
            }
            0x691 => {
                scene.music = None;
                Event::MusicStop { fade: int(args, 0) }
            }
            0x695 => {
                scene.ambience = Some(int(args, 0));
                Event::Ambience {
                    file: self
                        .found(
                            || format!("loop {}", int(args, 0)),
                            self.assets.ambience(int(args, 0)),
                        )?
                        .file
                        .clone(),
                    fade: int(args, 1),
                }
            }
            0x696 => {
                scene.ambience = None;
                Event::AmbienceStop { fade: int(args, 0) }
            }
            0x465 => Event::SoundWait,
            0x466 => {
                let game = self.game().to_string();
                Event::ExtraVoice {
                    file: self
                        .found(
                            || format!("voice {}", int(args, 1)),
                            self.assets.voice(&game, int(args, 1)),
                        )?
                        .file
                        .clone(),
                }
            }
            0x488 | 0x48a => Event::Tint {
                layer: int(args, 2),
                colour: int(args, 3),
                alpha: int(args, 4),
                frames: int(args, 5),
            },
            0x489 => Event::TintEnd {
                colour: int(args, 2),
                frames: int(args, 3),
            },
            0x513 => {
                let axis = int(args, 1).clamp(0, 1) as usize;
                scene.shake[axis] = [int(args, 2), int(args, 3)];
                return None;
            }
            0x510 => {
                let axis = int(args, 1).clamp(0, 1);
                let [size, count] = scene.shake[axis as usize];
                Event::Shake {
                    axis,
                    size,
                    count,
                    frames: int(args, 2),
                }
            }
            0x4f4 => {
                scene.camera = Camera {
                    x: int(args, 1),
                    y: int(args, 2),
                    zoom: float(args, 4),
                };
                Event::Camera {
                    x: scene.camera.x,
                    y: scene.camera.y,
                    zoom: scene.camera.zoom,
                    frames: 0,
                }
            }
            0x3f4 => {
                scene.camera.y = int(args, 1);
                Event::Camera {
                    x: scene.camera.x,
                    y: scene.camera.y,
                    zoom: scene.camera.zoom,
                    frames: int(args, 2),
                }
            }
            0x44b => Event::Movie {
                file: self
                    .found(
                        || format!("movie {}", text(args, 0)),
                        self.assets.movie(&text(args, 0)),
                    )?
                    .to_string(),
            },
            0x69a => {
                scene
                    .spots
                    .push([int(args, 0), int(args, 1), int(args, 2), int(args, 3)]);
                return None;
            }
            0x69b => {
                let spots = std::mem::take(&mut scene.spots);
                let game = self.game().to_string();
                Event::Hotspots {
                    spots: spots
                        .iter()
                        .map(|s| Hotspot {
                            x: s[1],
                            y: s[2],
                            voice: self.assets.voice(&game, s[3]).map(|t| t.file.clone()),
                        })
                        .collect(),
                }
            }
            _ => Event::Other {
                opcode,
                args: describe(args),
            },
        })
    }

    fn message(&mut self, lines: &[String]) -> Event {
        let mut lines: Vec<String> = lines.iter().map(|l| self.names(l)).collect();
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        let speaker = self.assets.speaker(self.scene.speaker);
        let name = speaker.map(|s| self.names(&s.name)).unwrap_or_default();
        let character = speaker.map(|s| s.character.clone()).unwrap_or_default();
        let voice_number = self.scene.voice.take();
        let game = self.game().to_string();
        let voice = voice_number
            .and_then(|n| self.found(|| format!("voice {n}"), self.assets.voice(&game, n)))
            .map(|t| t.file.clone());
        let lip = voice_number.and_then(|n| self.assets.lip(n)).cloned();
        let key = hash(&[
            self.session.script().as_bytes(),
            self.session.label().as_bytes(),
            &(self.current as u32).to_le_bytes(),
        ]);
        let read = !self.profile.read.insert(key);
        self.backlog.push(Line {
            speaker: name.clone(),
            lines: lines.clone(),
            voice: voice.clone(),
        });
        if self.backlog.len() > BACKLOG_LIMIT {
            self.backlog.remove(0);
        }
        self.last_text = (name.clone(), lines.clone());
        Event::Text {
            speaker: name,
            character,
            lines,
            voice,
            lip,
            read,
        }
    }
}
