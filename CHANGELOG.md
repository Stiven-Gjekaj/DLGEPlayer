# Changelog

## Unreleased

- `scripts/ios-simulator.py` builds the iOS app for the arm64 simulator
  and starts it.
- The player shows the keyword pick at the end of a monologue. The
  profile keeps the correct picks, and their sum opens the `Heaven`
  chapters as in the game.
- The touch points of `0x69b` stay on the screen until `0x69c`, and the
  story goes on.
- The player removes the keyword marks, such as `#KW_T1[5]`, from the
  text. In the English text, a keyword mark gives a space.
- A message with a line that is too wide for the message box gets a
  smaller font, so the English text stays in the window.
- The player runs on Android and iOS. A tap shows the next message, a
  long press hides the message window, a swipe down opens the backlog, and
  the back button of Android closes a menu.
- The first start on a phone finds the NSP file and `prod.keys` in the
  `Download` folder of Android or in the folder of the app on iOS. The
  import runs on the phone, with the smaller files by default.
- `scripts/build-player.sh` builds the library for Android and iOS. The
  CI makes an APK and an IPA with no signature.
- The player saves the profile when the app goes to the background.
- The Godot player plays both games. It shows the message window of each
  game, the speaker, the sprites with blinks and lip sync, the heroine face,
  the backgrounds, the event images, the effects, and the choices. It plays
  the music, the loop sounds, the sound effects, and the voices.
- The player has a title screen for each game, a chapter menu, 60 save
  slots for each game, a backlog with voices, auto and skip modes, a
  gallery, a music room, and settings.
- The `scene` crate turns the commands of the VM into events. All 706
  chapters play to their end through it, and each name resolves to a file.
- A chapter from the menu starts with the last background of the chapter
  before it.
- The player decodes the 9 MP4 movies with `openh264` and `symphonia`,
  with no FFmpeg. 7 of them use B-frames.
- The first start screen imports the NSP with the keys, in the app, with
  half of the CPU cores. The English patch ships inside the app.
- `dlplayer trace` prints each command of each run of a chapter.
- `docs/opcodes.md` gives the meanings of the display opcodes from the
  traces, and `docs/player.md` describes the player.
- The CI exports the player for macOS, Windows, and Linux.
- The `chara` crate holds the sprite layouts and the name resolver.
- The `vm` crate runs the script IR with no screen. It queues the commands
  of each run, plays the flag writes in order, asks the host at each
  choice, and follows the label and script jumps. A save holds the state
  between two runs.
- `dlplayer run` runs all chapters. With the JP scripts and with the EN
  scripts, all 706 chapters reach their end with each choice, and all 98
  routes run to the end. The runs reach all 500 choices. They reach
  134,455 of the 134,503 reachable JP text lines and 137,472 of the
  137,520 reachable EN text lines.
- `docs/opcodes.md` gives the operand modes, the flags, the jumps, the
  choice, and the ends of a chapter.
- `dlplayer import --mobile` writes WebP images and Vorbis at quality 0.
  The folder is 3.3 GB instead of 6.6 GB.
- The import keeps a journal and continues after a stop.
- The `nsp` crate reads the RomFS of an NSP with the keys of the user. All
  2125 files agree with hactool, byte for byte.
- `dlplayer import` takes an NSP and `--keys`. The import from the NSP
  gives the same files as the import from an extracted RomFS.
- `dlplayer make-patch` makes a patch file of deltas, and `dlplayer import
  --translation` applies it. The English patch is 717 deltas and 12.5 MB.
- `dlplayer import` converts a dump into the folder that the player reads,
  with `manifest.json`. The full JP dump imports with no failure.
- The import writes each script as JSON IR. A code address becomes an
  instruction index. All 706 JP and 706 EN scripts convert.
- The import converts all 38546 HCA files and the 5 ADX menu sounds of
  `SystemSE.acb` to Ogg Vorbis. A looped track ends at its loop end.
- The import writes all standalone textures, the `RAW_` copyright image,
  the 878 sprite sheets, and the 1163 UI textures as PNG.
- The `lyt` crate reads `.bflyt` layouts and `.bflan` animations. All 521
  layouts and 348 animations read, and the import writes them as JSON.
- The import writes the `BIN_` tables and the lip sync data of `lip.cpk`
  as JSON.
- `--patch` puts the English patch over the dump. `--jobs` limits the
  threads.
- The sprite resolver reads the expression `tyo` as `cho`. All 45588 sprite
  calls resolve.
- `dlplayer audio` converts one HCA file to Ogg Vorbis.
- The `cpk` crate reads CRI CPK archives by TOC or by ITOC.
- The `cpk` crate decompresses CRILAYLA data.
- The `stcm2l` crate reads and writes STCM2L scripts. All 706 scripts of the
  Switch release write back byte for byte.
- The `stcm2l` crate decodes the parameters and the data blocks of each
  action, with the Shift-JIS text.
- The `stcm2l` crate reads the appendix that the English patch adds after
  the collection link. All 706 patched scripts write back byte for byte.
- `docs/opcodes.md` describes the script VM of the game and the meaning of
  the opcodes, with the evidence for each meaning.
- The listing names the opcodes whose meaning is verified, such as
  `text_line`, `voice`, and `if_false_goto`.
- The listing shows a voice number as a number, not as two letters.
- `stcm2l::decode` turns each action into a typed instruction. All actions
  of the 706 JP and the 706 EN scripts decode.
- The `bea` crate reads Bezel Engine archives. All 2177 files of the 2097
  archives in `Archive/` read.
- The `bntx` crate decodes BNTX textures. All 2029 textures in `Archive/`
  decode.
- `dlplayer bea list`, `dlplayer bea extract`, and `dlplayer texture` read
  archives and write a texture as a PNG image.
- The `bea` crate reads `BIN_` tables, such as `static/chara.bin`. All 9
  tables of the dump read.
- The `importer` crate builds a character sprite with an expression from
  its sheet. All 878 sprites and 9969 frames fit.
- `dlplayer table` prints a table, and `dlplayer sprite` writes a sprite as
  a PNG image.
- The typed decoder keeps the arguments of a call.
- `importer::sprite::Catalog` resolves the sprite names of the scripts.
- The `sarc` crate reads SARC archives. The `bntx` crate decodes BC4 and
  BC5 and applies the channel sources. All 1163 layout textures decode.
- `dlplayer layout` writes the files of the layouts and their textures.
- The listing and the decoder read voice and sound effect numbers as
  integers.
- `dlplayer cpk list` and `dlplayer cpk extract` read archives.
- `dlplayer disasm` prints the listing or the text of one script.
- `dlplayer dump-scripts` writes a listing and a text file for each script
  of an archive.
