# Roadmap

The phases come in this sequence.
Phases 2 and 3 do not depend on each other. They can run at the same time.

| Phase | Goal | Done when |
|---|---|---|
| 1 | Read CPK, CRILAYLA, and STCM2L. Write the scripts as text. | All 706 scripts write to text. Done. |
| 2 | Find the meaning of the 76 opcodes: text, speaker, voice, choice, jump, flag, background, sprite, music, and transition. | A typed decoder knows each action in the corpus. `docs/opcodes.md` describes the opcodes. Done. |
| 3 | Read the Bezel Engine `.bea` archives: textures, sprite and motion data, and UI layouts. | Each background, event image, and character sprite exports to PNG. Done. |
| 4 | Make the importer. It converts the RomFS one time into a normalized folder: scripts as IR, PNG images, Ogg Vorbis audio, MP4 movies, and `manifest.json`. | One command makes the folder from a dump. Done. |
| 4b | Read the NSP with the `prod.keys` of the user: the PFS0 package, the NCA encryption, and the RomFS. Ship the English patch as the text and the image changes of the translators. | `dlplayer import` takes an NSP and `prod.keys` and needs no other tool. The English text shows in the IR. |
| 5 | Make the VM. It runs the IR with no screen. It keeps flags, globals, saves, and choices. | Each route runs from start to end with no screen. A report shows the coverage. |
| 6 | Make the desktop player with Godot 4 and gdext: text box, backlog, choices, images, audio, transitions, launcher, saves for each game, gallery, and settings. | Both games play on macOS, Windows, and Linux. |
| 7 | Make the mobile player: touch controls, import on the device, and a small storage size. | Both games play on Android and iOS. |
| 8 | Apply the English patch: put the patch files over the dump, and map the text encoding and the font. | The English text shows in the player. |

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
2. Godot plays only Ogg Theora video. The 9 movies are MP4. Phase 6 must
   find a way to play them, for example a video extension or a conversion
   on the computer of the user.
3. The import folder is 6.6 GB. Mobile devices need smaller files, for
   example a lower Vorbis quality and WebP images.
4. 3 opcodes in the scripts have no handler in the game (`0x64f`, `0x69b`,
   `0x69c`). About 25 engine opcodes have no verified meaning. The VM of
   phase 5 must find what they do.
5. The English patch uses name tokens such as `#Name[2]` and some
   characters that need the font of the game. Phase 8 must map them.
