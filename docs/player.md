# The player

The player shows the game from an import folder. It has three parts:

| Part | Place | Work |
|---|---|---|
| `scene` crate | `crates/scene` | Turns the commands of the VM into events for a screen. Keeps the scene, the backlog, the saves, the profile, and the chapter list. It has no screen. |
| `player` crate | `crates/player` | The Godot extension. It gives the `scene` crate, the movie decoder, and the importer to GDScript. |
| Godot project | `godot/` | The screens: the first start, the title, the chapters, the game, the menus, and the movies. |

## Build and run

You need Rust 1.94 or later for the `player` crate, and Godot 4.7.

    scripts/build-player.sh

The script builds the library, copies it to `godot/bin/<platform>/`, and
copies the English patch to `godot/patch/`. Then open `godot/` in the
Godot editor, or run it:

    godot --path godot

`scripts/build-player.sh release` builds the release library. Copy the
release library before an export.

## The first start

The first start screen asks for three paths:

- the NSP file of the game;
- the `prod.keys` file of the console;
- the import folder. The default is `Documents/DLGE Player`.

A thread of the extension runs the importer with half of the CPU cores.
The English patch comes from `godot/patch/en.dlpatch` inside the app.
After the import, the screen tells the user that the NSP file and the keys
can go. The player needs only the import folder after that.

A user who has an import folder already selects it with "Open an import
folder".

## Events

`Director::next_event` gives one event at a time. A frame is 1/60 of a
second. The screen handles each event:

| Event | From | What the screen does |
|---|---|---|
| `background` | `0x3fa`, `0x3fb` | Shows a background or an event image with a fade. `bg_black` and a missing file give a black screen. |
| `foreground` | `0x402`, `0x403` | Shows or clears an image over the sprites. |
| `figure` | `0x47e` or `0x61c`, then `0x480` | Shows a sprite at a slot. |
| `look` | `0x47f`, `0x61d` | Changes the expression of a sprite. |
| `move` | `0x483` | Moves a sprite to another slot. |
| `hide` | `0x482` | Fades a sprite out. |
| `music`, `music_stop` | `0x456`, `0x690`, `0x457`, `0x691` | Plays or stops a track of `bgm` with a fade. |
| `ambience`, `ambience_stop` | `0x695`, `0x696` | Plays or stops a loop of `loopse`. |
| `sound`, `sound_wait` | sound effect, `0x465` | Plays a sound effect, or waits until it ends. |
| `extra_voice` | `0x466` | Plays a second voice with the message. |
| `text` | message, `0x4bc`, voice | Shows the message with the speaker and the voice. |
| `choice` | `0x599` | Shows the options and gives the pick to `choose`. |
| `select_character` | `0x6fe` | Shows six characters and gives the pick to `choose_character`. |
| `wait` | `0x4be` | Waits. A click ends the wait. |
| `window` | `0x4dd` | Shows or hides the message window. |
| `transition` | `0x491` | Keeps a picture of the screen, and fades it out at the next wait. |
| `tint`, `tint_end` | `0x488`, `0x48a`, `0x489` | Fills the screen with a colour, or removes it. |
| `shake` | `0x513`, `0x510` | Shakes the screen. |
| `camera` | `0x4f4`, `0x3f4` | Zooms or moves the screen. |
| `movie` | `0x44b` | Plays a movie. |
| `hotspots`, `hotspots_end` | `0x69a`, `0x69b`, `0x69c` | Shows touch points until `0x69c`. The story goes on. Each point plays a voice. |
| `monologue` | `0x6b2` | Shows the line of each keyword of the monologue. The player picks 2, and `choose_keyword` gives each pick. |
| `end` | `0x700`, `0x4bf` | The chapter ends. `next` names the next chapter of the route. |
| `other` | other opcodes | The screen does nothing. |

The meaning of each opcode comes from the traces of
`dlplayer trace <folder> <chapter>`. [opcodes.md](opcodes.md) gives the
grade of each meaning.

## The scene and the saves

The director keeps the scene: the background, the foreground, the
sprites, the music, the loop sound, the window, the camera, and the
speaker and the voice of the next message.

A save holds:

- the state of the VM at the start of the current run;
- the scene at the start of the run;
- the picks of the run;
- the index of the current command in the run;
- the speaker, the text, and the background, for the save menu.

A load runs the label again and plays the commands before the index with
no events. The VM and the scene come to the same state, because a run gives
the same commands from the same state. Then `rebuild` gives the events that
build the screen.

A chapter that starts from the menu shows the last background of the
chapter before it in its route. The director plays that chapter with no
screen to find it.

The files are in the `user://` folder of Godot:

| File | Contents |
|---|---|
| `settings.cfg` | The settings and the path of the import folder |
| `profile.json` | The system flags, the finished chapters, the seen event images, the read messages, the heroine names, the last character pick, and the correct keyword picks of each chapter |
| `saves/ve/<n>.json`, `saves/mb/<n>.json` | The 60 save slots of each game |

## The screen

The screen is 1920 by 1080. Godot scales it to the window.

- The message window, the speaker plate, and the ornament come from
  `UI_06_1` of the `advUI` archive of each game. The places come from the
  layouts `UI_Parts_MessageWnd`, `UI_Parts_SpeakerWnd`, and
  `UI_Parts_MainText`.
- The heroine sprites `her_*` are faces of 512 by 512. The screen shows
  them in the left part of the message window.
- A sprite is the body and an eye patch and a mouth patch of one sheet. The
  eyes blink. The mouth follows the lip sync keys while the voice plays.
- The text uses a font of the system with Japanese characters, for example
  Hiragino Mincho on macOS and Yu Mincho on Windows. The size is 48. A
  message with a line that is wider than the box gets a smaller size.
- The player removes the keyword marks, such as `#KW_T1[5]`, and keeps the
  words between them. The keyword pick at the end of the monologue shows
  each keyword in `【】`. The English patch has no keyword words, so `◆`
  shows the place of the keyword.

## Controls

| Input | Action |
|---|---|
| Left click, Enter, or Space | Show the full text, then the next message |
| Right click | Hide or show the message window |
| Mouse wheel up | Open the backlog |
| Ctrl (hold) | Skip |
| Escape | Open the settings |

The bar at the top has Auto, Skip, Log, Save, Load, Config, and Title.

## Phones

The player runs on Android and iOS. The screen is the same as on a
computer.

| Touch | Action |
|---|---|
| Tap | Show the full text, then the next message |
| Long press | Hide or show the message window |
| Swipe down | Open the backlog |
| Back button of Android | Close a menu, or open the settings in a chapter |

The bar at the top also has a Hide button. A phone app has no Quit button
and no full screen setting.

The player saves the profile when the app goes to the background, because
the system can stop the app there with no warning.

The import runs on the phone. The smaller files are the default there.

| | Android | iOS |
|---|---|---|
| Where to put the NSP file and `prod.keys` | The `Download` folder | The folder of the app in the Files app |
| The import folder | `Documents/DLGE Player` | `import` in the folder of the app |
| Permission | All files access. The player asks for it at the first start. | None |

The first start finds the two files in that folder. For a file in another
folder, use the Select button.

### Build for a phone

    scripts/build-player.sh release android
    scripts/build-player.sh release ios

`android` needs `cargo-ndk`, the Android NDK, and the Rust target
`aarch64-linux-android`. `ios` needs Xcode and the Rust targets
`aarch64-apple-ios`, `aarch64-apple-ios-sim`, and `x86_64-apple-ios`. It
makes `godot/bin/ios/libplayer.xcframework`. The simulator part of the
iOS template of Godot 4.7.2 has only x86_64 code, so the simulator
library has x86_64 code too.

`.cargo/config.toml` sets the oldest iOS version, 15.0. The iOS preset uses
the same version.

The iOS preset writes an Xcode project. The team ID `0000000000` is a
placeholder. Build the app with no signature, then put it in an IPA:

    xcodebuild -project "DLGE Player.xcodeproj" -scheme "DLGE Player" \
        -configuration Release -sdk iphoneos -derivedDataPath build \
        CODE_SIGNING_ALLOWED=NO build

The user signs the IPA with a sideloading tool, for example SideStore or
AltStore. The workflow `build.yml` makes the APK and the IPA.

### Run on the iOS simulator

The simulator part of the iOS template of Godot 4.7.2 holds only x86_64
code. A simulator of iOS 27 does not start an x86_64 app. On an Apple
silicon Mac, use this script:

    scripts/build-player.sh debug ios
    scripts/ios-simulator.py -- --folder <import folder> --chapter mb_aya_Dark_09

The script exports the iOS preset to `godot/export/ios`. It copies the
arm64 device code of Godot into the simulator part and marks each object
file as simulator code. It also links empty values for two Metal symbols
that the simulator SDK does not have. Then it builds the app, installs it
on the booted simulator, and starts it. `--device <name or UDID>` selects
a different simulator. The options after `--` go to the player.

The simulator app can read a folder of the Mac, so `--folder` can give an
import folder that the desktop player made.

`python3 -m unittest discover -s scripts -p "test_*.py"` tests the change
of the object files.

## Command line options for tests

The options come after `--`:

| Option | Result |
|---|---|
| `--folder <dir>` | Opens this import folder |
| `--chapter <name>` | Starts a chapter |
| `--movie <name>` | Plays a movie, for example `ve_op` |
| `--screen <name>` with `--game <ve or mb>` | Opens a screen, for example `chapters` |
| `--click <seconds>` | Clicks the game at a fixed interval |

With the movie writer of Godot, they save pictures of the screen:

    godot --path godot --write-movie shots/f.png --fixed-fps 1 --quit-after 20 -- \
        --folder imported --chapter mb_aya_Dark_01 --click 1.5

`godot/tests/smoke.gd` plays a chapter with no screen.
`godot/tests/save.gd` saves in a chapter, loads the save, and compares the
messages after the load with the messages after the save.
`godot/tests/import.gd` runs the import of the first start with no screen.
Use the release library for an import. The debug library converts the
images many times more slowly.

## Export

`godot/export_presets.cfg` has a preset for macOS, Windows, and Linux. The
workflow `.github/workflows/build.yml` builds the library on each platform,
exports the project, and keeps the result as an artifact. The macOS app
holds one library for Apple silicon and Intel, and has an ad hoc
signature.
