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
- **One exception: `assets/patch/en.dlpatch`, the English patch as
  deltas.** The owner chose this on 2026-11-01, with the permission of the
  translators. The file holds the deltas, not the patched files. It changes
  only when the translators change the patch. The original patch files
  stay out of the repository.
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
  of the patch authors, given on 2026-11-01. The app credits them:
  otomvevn for Haunted Dark Bridal and dialovers-translations for
  MORE,BLOOD. The
  app ships only the work of the translators: the English text and the
  changes to the user interface images. It does not ship the patched
  `script.cpk` or the patched `.bea` files, because they also hold the
  Japanese script and the art of the game.

## The NSP reader and the patch file

- **The `nsp` crate does its own CTR and XTS on top of the `aes` crate,
  version 0.8.** Reason: `aes` 0.9 needs Rust 1.89, and the project
  supports Rust 1.88. CTR and XTS are a few lines on a block cipher. A
  test checks the XTS mode against vector 1 of IEEE 1619.
- **The NSP is read in place.** The files decrypt when they are read.
  Reason: a phone has no space for a second copy of 6.4 GB.
- **The delta is our own code, not bsdiff.** Reason: the `bsdiff` crate
  did not finish the 706 scripts in 20 minutes. The delta of the
  importer indexes 16-byte windows of the old file and makes all deltas
  of the patch in about 1 second.
- **The patch file holds deltas, not the patched files.** Reason: a delta
  holds the work of the translators and copies the rest from the files of
  the user.

## The VM

- **The VM queues the commands of a run, as the game does.** The flag
  writes take effect when the session plays the commands. The flag reads
  are immediate. Reason: the handlers of the game queue the flag writes,
  and a run reads the flags from before its own writes.
- **After a run with no jump and no end, the session runs the same label
  again.** Reason: each part of a chapter adds 1 to `switch` and returns.
  The game must run the label again to show the next part.
- **The VM gives the opcodes with no handler to the host and goes on.**
  Reason: the main story reaches them in 167 scripts, so the game must
  handle them in a way that we have not found.
- **`0x4bf` ends a chapter, as `0x700` does.** Reason: the endings end with
  `0x4bf` and have no `0x700` after it.
- **The runner sets `character` for a single chapter.** Reason: the
  chapters after the character select read the pick of an earlier chapter.
  A route carries all flags from one chapter to the next.
- **The finished-game run sets only the flags that end in `_end` or
  `_open`.** Reason: the other system flags are settings and debug
  switches. With all flags set, `start` takes a debug jump to a label that
  does not exist.
- **A save is the state between two runs:** the script, the label, the
  flags, the globals, and the registers. Reason: the game saves at the same
  point, and the state of a run in progress is not needed.

## The import on a phone

- **`--mobile` writes WebP images at quality 90 and Vorbis at quality 0.**
  Reason: the folder is 3.3 GB instead of 6.6 GB. The images come from
  BC7, which is lossy already, so WebP at quality 90 loses little.
- **The import keeps a journal and continues after a stop.** Reason: a
  phone can close the app during an import of several minutes. The journal
  gets one line after each item is written, so a half-written file is
  converted again.

## The player

- **The `scene` crate holds the logic of the screen, with no Godot.**
  Reason: tests can play every chapter through it, and a later frontend
  can use it.
- **Each event goes to GDScript as a `Dictionary` through JSON values.**
  Reason: one conversion serves all events, and an event gets a new field
  with no change to the extension.
- **A save holds the state at the start of the run and the index of the
  current command.** A load runs the label again to that index. Reason:
  a run gives the same commands from the same state, so the save needs no
  copy of the commands.
- **A chapter from the menu starts with the last background of the chapter
  before it.** Reason: most chapters show no background before their first
  message. In the game, the background of the chapter before stays on the
  screen.
- **The player records the finished chapters itself.** Reason: the system
  flags for the end of a chapter have other names than the scripts, for
  example `aya_Dark00_end` and `ayato_dark_01_end`.
- **The chapter menu opens the chapters in order.** The first chapter of
  each `Dark` route is open. The other routes open after the end of the
  `Dark` route. Reason: the menu of the game is Lua bytecode, and the
  player does not run it. The setting "Open all chapters" opens all.
- **The text uses a font of the system.** Reason: the game font is a
  bitmap font of the console (`font/main.bffnt`), and the system font has
  each character of the shown text in both languages.
- **A message with a line that is too wide gets a smaller font.** Reason:
  the English patch breaks the lines for the font of the game. 713 long
  lines are wider than the message box at size 48. A smaller font keeps
  the breaks of the translators. A new wrap breaks a line in a different
  place.
- **The player decodes the MP4 movies itself: `mp4` reads the samples,
  `openh264` decodes H.264, and `symphonia` decodes AAC.** Reason: Godot
  plays only Ogg Theora, and the importer must not need FFmpeg. 7 of the 9
  movies use the Main profile with B-frames, so the decoder must not flush
  after each sample.
- **The `openh264` crate builds OpenH264 from source.** The patent licence
  of Cisco covers only the binaries of Cisco. A user who needs that licence
  can build the player with a binary of Cisco.
- **The importer runs inside the app with half of the CPU cores.** Reason:
  the first start needs only the NSP and the keys, and an import on all
  cores makes the computer slow.
- **The English patch ships inside the app as `patch/en.dlpatch`.**
  Reason: the owner has the permission of the translators.
- **The player gives colour to the transparent pixels at the edges of a
  sprite sheet.** Reason: a transparent pixel is black, and the filter of
  a scaled sprite mixes the black into the edge. Godot has
  `Image.fix_alpha_edges`, but it takes 272 ms for a sheet. The Rust
  function takes 46 ms.
- **These meanings are guesses from the traces, not from the code of the
  game:** the slots of `0x480`, the colours of `0x488`, the order of the
  characters of `0x6fe`, and the time unit of `0x3fb`. Reason: the game
  queues these commands, and the code that plays them is behind tables of
  functions that the analysis did not follow.
- **The library file is removed before the build script copies a new
  one.** Reason: on macOS, a copy over a library that a program has open
  changes the file in place, and each new program that loads it stops.
