# Decisions

This file records each choice of the owner and each technical decision.
Each entry gives the reason.

## What the project is

- **The name is DLGE Player.** The owner chose it on 2026-10-30. The
  repository is `DLGEPlayer`. The command line tool is `dlplayer`. The
  repository folder keeps the name `DLGE Mobile Port`.
- **DLGE Player is a player for Diabolik Lovers Grand Edition** (Switch, JP,
  2019). It plays Haunted Dark Bridal and MORE,BLOOD. Reason: the owner wants
  to play the Switch release on other devices.
- **The repository is public.** The owner chose this on 2026-10-30. Reason:
  the repository holds no game data, so it is safe to publish.
- **The licence is GPL-3.0-or-later.** The owner chose this on 2026-10-30.

## What the repository holds

- **The repository holds no game data, no scripts, and no patch files.**
  Reason: the game and the patch belong to their owners. `.gitignore` refuses
  the formats of the game.
- **The player reads only the data that the user supplies at import time.**
- **The player does not parse a console format at runtime.** The importer
  converts each file one time into a normalized folder. Reason: the player
  stays small, and each platform reads the same folder.
- **No test reads game data.** The tests build the data that they need. The
  corpus test reads a real dump only when `DLGE_SCRIPTS` is set.

## The stack

- **Rust makes the importer, the disassembler, and the VM.**
- **Godot 4 with gdext makes the player.** The owner chose this on
  2026-10-30. Reason: a visual novel is mostly user interface. Godot gives
  text, menus, audio, video, and an Android and iOS export. A headless Rust
  VM behind a small interface keeps the frontend replaceable.
- **The platforms are Windows, macOS, Linux, Android, and iOS.** The owner
  chose this on 2026-10-30. Reason: Godot exports to all of them, and the
  owner accepts sideloading on iOS.

## What the dump showed

- **The Switch release runs on the Bezel Engine, not on the old Otomate
  engine.** `boot.lua` starts `BezelEngineInitializer`. The images are in
  `.bea` archives with the magic `SCNE`. Reason for the record: the first
  plan expected only Otomate formats.
- **The base resolution is 1920x1080, not 1280x720.** `boot.lua` sets it.
- **The scripts are STCM2L.** They are in `script.cpk` and use CRILAYLA
  compression. The first plan was correct for the scripts.
- **The format notes of hkki and ideaf_script_format have no licence.** The
  parser uses no code from them. It comes from the notes and from the real
  files.
