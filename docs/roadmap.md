# Roadmap

The phases come in this sequence.
Phases 2 and 3 do not depend on each other. They can run at the same time.

| Phase | Goal | Done when |
|---|---|---|
| 1 | Read CPK, CRILAYLA, and STCM2L. Write the scripts as text. | All 706 scripts write to text. Done. |
| 2 | Find the meaning of the 76 opcodes: text, speaker, voice, choice, jump, flag, background, sprite, music, and transition. | A typed decoder knows each action in the corpus. `docs/opcodes.md` describes the opcodes. Done. |
| 3 | Read the Bezel Engine `.bea` archives: textures, sprite and motion data, and UI layouts. | Each background, event image, and character sprite exports to PNG. Done. |
| 4 | Make the importer. It converts the RomFS one time into a normalized folder: scripts as IR, PNG images, Ogg Vorbis audio, MP4 movies, and `manifest.json`. | One command makes the folder from a dump. Done. |
| 4b | Read the NSP with the `prod.keys` of the user: the PFS0 package, the NCA encryption, and the RomFS. Ship the English patch as the text and the image changes of the translators. | `dlplayer import` takes an NSP and `prod.keys` and needs no other tool. The English text shows in the IR. Done. |
| 5 | Make the VM. It runs the IR with no screen. It keeps flags, globals, saves, and choices. | Each route runs from start to end with no screen. A report shows the coverage. Done. |
| 6 | Make the desktop player with Godot 4 and gdext: text box, backlog, choices, images, audio, transitions, launcher, saves for each game, gallery, and settings. | Both games play on macOS, Windows, and Linux. Done on macOS. The CI exports Windows and Linux. |
| 7 | Make the mobile player: touch controls, import on the device, and a small storage size. | Both games play on Android and iOS. Done on an Android 16 emulator. The iOS app builds, but nobody has run it yet. |
| 8 | Apply the English patch: put the patch files over the dump, and map the text encoding and the font. | The English text shows in the player. Done on macOS. The import applies the patch, the player removes the name and keyword marks, and each line fits the message box. |

## Platforms

| Platform | Rust target | Build | Install |
|---|---|---|---|
| macOS | `aarch64-apple-darwin`, `x86_64-apple-darwin` | native | app bundle |
| Windows | `x86_64-pc-windows-msvc` | CI or `cargo-xwin` | zip |
| Linux | `x86_64-unknown-linux-gnu` | CI or `cargo-zigbuild` | zip or AppImage |
| Android | `aarch64-linux-android` | Android NDK | APK |
| iOS | `aarch64-apple-ios` | Xcode on macOS | IPA for sideloading |

- A GitHub Actions matrix makes the desktop builds. Set it up at the start of phase 6.
- Godot gives each platform a `user://` folder. The saves and the imported data go there.
- On iOS, gdext loads the Rust library as an `.xcframework`.
- On iOS, the user signs the IPA and installs it with a sideloading tool, for example SideStore or AltStore.
- On Android and iOS, the user selects the dump folder with the file picker of the system. The importer runs on the device, or on a computer that copies the result to the device.

## Risks

1. The menus of the game are 41 Lua 5.2 bytecode scripts and 521
   NintendoWare layouts. The importer writes the layouts and their
   animations as JSON. The Lua scripts are not converted, so the player
   must make the menu logic again.
2. Godot plays only Ogg Theora video. The player decodes the 9 MP4
   movies with `openh264` and `symphonia`. This risk is closed.
3. The import folder is 6.6 GB, or 3.3 GB with `--mobile`. The movies are
   1.4 GB of it.
4. The slots of the sprites, the tint colours, and the order of the
   character select are guesses from the traces. 9 opcodes run with no
   effect on the screen ([opcodes.md](opcodes.md#meanings-from-the-traces)).
5. The menu that opens the chapters is Lua bytecode. The player opens the
   chapters in order from its own record of finished chapters. The rules
   for the `Heaven` chapters come from the executable. The other rules of
   the game can differ.
6. The game shows the keyword pick of a monologue in the text of the
   monologue, and the player touches the keywords there. The player shows
   the line of each keyword as a button. One English line ends with the
   mark `#n`, which is a fault of the patch.
7. The Windows and Linux builds come from CI. Nobody has played them on
   those systems yet.
8. The iOS template of Godot 4.7.2 has only x86_64 code for the
   simulator. On an Apple silicon Mac, the simulator runs the app with
   Rosetta.
9. On Android, the importer needs the permission for all files, because
   it reads and writes with paths. A store can refuse an app with this
   permission. The player is not for a store, so this is not a problem
   now.
10. The Android import was measured on an emulator only: 680 s for the
    NSP, with 43,339 files and 3.4 GB. A phone can be slower.
