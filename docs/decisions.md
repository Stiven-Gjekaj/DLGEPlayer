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

## The import folder

- **The audio is Ogg Vorbis, not Opus.** Reason: Godot 4 loads Ogg Vorbis
  files at runtime with `AudioStreamOggVorbis.load_from_file`. It does not
  load Opus. The first plan said Opus. The voices are 2.2 GB at quality
  0.4. `--quality` makes them smaller.
- **A looped track ends at its loop end.** Reason: an Ogg Vorbis stream in
  Godot loops from a loop offset to the end of the stream. `manifest.json`
  gives the loop start.
- **The `cridecoder` crate decodes HCA.** It has the MIT licence, which
  agrees with the GPL. Reason: it decodes all 38546 HCA files of the dump
  with no error, so a second decoder gives no gain.
- **The `vorbis_rs` crate encodes Ogg Vorbis.** It builds libvorbis from
  source with the C compiler of the target, also for Android and iOS. The
  encoder uses a fixed stream serial, so the same dump gives the same
  files.
- **The ADX decoder is our own code.** Reason: `cridecoder` does not decode
  ADX, and the 5 menu sounds need only the standard ADPCM. The output
  agrees with FFmpeg.
- **The images are PNG.** Reason: Godot loads PNG at runtime, and PNG is
  lossless. The images are 2.8 GB with the sprites. A later phase can add
  WebP for mobile devices.
- **A sprite is one sheet and its layout.** The importer does not compose
  each expression. Reason: 878 sheets are smaller than 9969 composed
  frames. The player draws the body and two patches from the sheet.
- **The movies stay MP4.** Reason: the importer must not need FFmpeg. Godot
  plays only Ogg Theora video, so phase 6 must decide how to play them.
- **The importer uses all CPU cores by default.** `--jobs` limits the
  threads.

## The first start

- **The user supplies the NSP file and `prod.keys` of their own console.**
  The owner chose this on 2026-11-01. The importer decrypts the NSP and
  reads the RomFS itself. The user needs no other tool. Reason: the
  extraction of the RomFS with `hactool` is the hardest step for most
  users.
- **After the import, the player tells the user that they can delete the
  NSP and `prod.keys`.** The owner chose this on 2026-11-01. Reason: the
  player reads only the import folder, and a phone has little space.
- **The app never holds keys.** The user brings them.
- **The English patch comes with the app.** The owner has the permission
  of the patch authors, given on 2026-11-01. The app credits them. The
  app ships only the work of the translators: the English text and the
  changes to the user interface images. It does not ship the patched
  `script.cpk` or the patched `.bea` files, because they also hold the
  Japanese script and the art of the game.
