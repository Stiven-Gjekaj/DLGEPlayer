<div align="center">

![DLGE Player](assets/wordmark.svg)

### An open-source player for Diabolik Lovers Grand Edition

_Haunted Dark Bridal and MORE,BLOOD, on your computer and your phone, from your own copy_

![Rust](https://img.shields.io/badge/rust-7f1d1d?style=for-the-badge&logo=rust&logoColor=ffffff)
![Godot 4](https://img.shields.io/badge/godot_4-9f1239?style=for-the-badge&logo=godotengine&logoColor=ffffff)
![Windows](https://img.shields.io/badge/windows-be123c?style=for-the-badge&logo=windows&logoColor=ffffff)
![macOS](https://img.shields.io/badge/macos-be123c?style=for-the-badge&logo=apple&logoColor=ffffff)
![Linux](https://img.shields.io/badge/linux-be123c?style=for-the-badge&logo=linux&logoColor=ffffff)
![Android](https://img.shields.io/badge/android-e11d48?style=for-the-badge&logo=android&logoColor=ffffff)
![iOS](https://img.shields.io/badge/ios-e11d48?style=for-the-badge&logo=apple&logoColor=ffffff)
![SideStore](https://img.shields.io/badge/sidestore-4c0519?style=for-the-badge)
[![GPL-3.0 licence](https://img.shields.io/badge/gpl--3.0_licence-1f0a0e?style=for-the-badge)](LICENSE)
[![Ko-fi](https://img.shields.io/badge/ko--fi-support_this_project-f26a1b?style=for-the-badge&logo=ko-fi&logoColor=ffffff)](https://ko-fi.com/stivengjekaj)

<p align="center">
  <a href="#overview"><b>Overview</b></a> |
  <a href="#you-need-a-copy-of-the-game"><b>Your copy</b></a> |
  <a href="#what-you-need-from-your-switch"><b>Dump</b></a> |
  <a href="#build"><b>Build</b></a> |
  <a href="docs/formats.md"><b>Formats</b></a> |
  <a href="docs/decisions.md"><b>Decisions</b></a> |
  <a href="docs/roadmap.md"><b>Roadmap</b></a>
</p>

</div>

---

> [!NOTE]
> **Phases 1 to 5 of 8 are done. The player does not show the game yet.**
> `dlplayer import` converts your NSP into the folder that the player
> reads. `dlplayer run` runs all 706 chapters of both games with no screen:
> each chapter reaches its end with each choice.
> [docs/roadmap.md](docs/roadmap.md) gives the next phases.
> [CHANGELOG.md](CHANGELOG.md) lists the changes.

> [!IMPORTANT]
> This is a fan project. It is not from Idea Factory, Otomate, or Rejet.
> The repository and the releases hold no file of the game and no file of a
> patch. You need your own copy.

---

## Overview

**DLGE Player** plays Diabolik Lovers Grand Edition, the Switch release of
2019, on Windows, macOS, Linux, Android, and iOS. The edition holds two
games: Haunted Dark Bridal and MORE,BLOOD.

The project has three parts:

1. **The importer** reads a dump of your copy one time. It converts each file
   into a normalized folder: the scripts, the images, the audio, and the
   movies, with a `manifest.json`.
2. **The VM** runs the STCM2L bytecode of the scripts: the text, the choices,
   the flags, the jumps, the sprites, the music, the voices, and the
   transitions.
3. **The player** shows the game from the normalized folder only. It is a
   Godot 4 app with a launcher, separate saves for each game, and a shared
   gallery and settings.

The player never reads a console format. A later phase applies the
community English patch over the dump.

## Features

Done:

- Read CRI CPK archives by TOC or by ITOC, and decompress CRILAYLA data
- Read and write STCM2L scripts, byte for byte
- Decode each action into a typed instruction, with the meaning of the
  opcodes in [docs/opcodes.md](docs/opcodes.md)
- Read Bezel Engine archives, BNTX textures, `BIN_` tables, and SARC
  archives
- Read NintendoWare `.bflyt` layouts and `.bflan` animations
- Build the character sprites with their expressions
- Convert HCA and ADX audio to Ogg Vorbis
- Import an NSP with your keys into a normalized folder with one command
- Apply the English patch as a file of deltas

- Import for a phone with `--mobile`, and continue an import that stopped
- Run the scripts with no screen: flags, choices, jumps, saves, and a
  coverage report

Planned:

- The Godot player ([phase 6](docs/roadmap.md))
- Builds for Windows, macOS, Linux, Android, and iOS
- The English text in the player

## You need a copy of the game

The art, the voices, the music, and the text of the game belong to their
owners. The tools read them from your copy. You need:

- your own Switch copy of Diabolik Lovers Grand Edition (JP, title ID
  `0100BD700E648000`)
- the keys of your own console

## What you need from your Switch

- the NSP file of the game, dumped from your copy
- the `prod.keys` file of your console

The importer reads the NSP with your keys. You need no other tool. After
the import, you can delete the NSP and the keys.

## Build

You need Rust 1.88 or later.

    cargo build --release

The tool is `target/release/dlplayer`.

## Import a dump

    dlplayer import game.nsp imported --keys prod.keys

The command writes the folder `imported` with a `manifest.json`. A full
import takes 4 to 5 minutes on 10 cores and writes 6.6 GB.

- `--translation assets/patch/en.dlpatch` applies the English patch. The
  repository ships it. `make-patch` makes a file of this type.
- `--patch <dir>` puts the files of a patch folder over the dump.
- The input can also be a RomFS folder that another tool extracted. A
  folder needs no keys.
- `--jobs <n>` uses `n` threads. Without it, the import uses all cores.
- `--quality <q>` sets the Vorbis quality, from -0.2 to 1.0. The default is
  0.4.
- `--audio bgm,se` converts only these audio groups: `bgm`, `se`,
  `loopse`, `voice`, and `system`.
- `--mobile` writes WebP images and smaller audio. The folder is 3.3 GB
  instead of 6.6 GB. `--webp <q>` sets only the image quality.

If the import stops, run the same command again. It continues where it
stopped.

[docs/formats.md](docs/formats.md#the-import-folder) describes the folder.

## Commands

| Command | Result |
| ------- | ------ |
| `dlplayer import game.nsp imported --keys prod.keys` | Converts a dump into the folder that the player reads |
| `dlplayer make-patch game.nsp patch_romfs en.dlpatch --keys prod.keys` | Makes a patch file of deltas from the dump to a patched RomFS |
| `dlplayer cpk list romfs/script.cpk` | Prints the files in an archive |
| `dlplayer cpk extract romfs/script.cpk scripts` | Writes the files of an archive to a directory |
| `dlplayer disasm scripts/mb_aya_Dark_01.dat` | Prints the listing of one script |
| `dlplayer disasm --strings scripts/mb_aya_Dark_01.dat` | Prints only the text of one script |
| `dlplayer dump-scripts romfs/script.cpk scripts_text` | Writes a `.asm` listing and a `.txt` file for each script |
| `dlplayer bea list romfs/Archive/title.nxonnx32.bea` | Prints the files in a Bezel Engine archive |
| `dlplayer texture "romfs/Archive/texture~bg~bg01b.dds.nxonnx32.bea" bg01b.png` | Writes a texture as a PNG image |
| `dlplayer table "romfs/Archive/static~speaker.bin.nxonnx32.bea"` | Prints a table of the game |
| `dlplayer sprite romfs/Archive 01_aya_01_A emi aya.png` | Writes a sprite with an expression as a PNG image |
| `dlplayer layout romfs/Archive/title.nxonnx32.bea title` | Writes the files of the layouts and their textures |
| `dlplayer audio BGM01.hca BGM01.ogg` | Converts an HCA file to Ogg Vorbis |
| `dlplayer run imported --report report.json` | Runs all chapters with no screen and reports the coverage |

## Crates

| Crate | Purpose |
| ----- | ------- |
| `cpk` | Reads CRI CPK archives and CRILAYLA data |
| `stcm2l` | Reads and writes STCM2L scripts, and writes listings |
| `bea` | Reads Bezel Engine archives |
| `bntx` | Decodes BNTX textures to RGBA pixels |
| `sarc` | Reads SARC archives, such as the layouts |
| `lyt` | Reads NintendoWare `.bflyt` layouts and `.bflan` animations |
| `nsp` | Reads the RomFS of an NSP file with the keys of the user |
| `importer` | Converts a dump into a normalized folder: the script IR, the images, the sprites, the audio, and the layouts |
| `vm` | Runs the script IR with no screen: the flags, the choices, the jumps, and the saves |
| `player` | The Godot 4 extension. Empty. |
| `dlplayer` | The command line tool |

## Tests

    cargo test --workspace

The tests build the data that they need. They do not read a copy of the
game. The corpus tests read a real dump only if you set these variables:

- `DLGE_SCRIPTS`: a directory of extracted `.dat` scripts
- `DLGE_ARCHIVES`: the `Archive` directory of the RomFS
- `DLGE_NSP`, `DLGE_KEYS`, and `DLGE_ROMFS`: the NSP, the keys, and an
  extracted RomFS to compare with
- `DLGE_IMPORT`: a folder that `dlplayer import` wrote

```
DLGE_SCRIPTS=scripts DLGE_ARCHIVES=romfs/Archive cargo test --workspace --release
```

## Documentation

| File | Contents |
| ---- | -------- |
| [docs/formats.md](docs/formats.md) | The formats of the game, the layout of the RomFS, and the import folder |
| [docs/opcodes.md](docs/opcodes.md) | The script VM of the game and the meaning of the opcodes |
| [docs/roadmap.md](docs/roadmap.md) | The phases, the platforms, and the risks |
| [docs/decisions.md](docs/decisions.md) | Each decision and its reason |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to help |
| [AGENTS.md](AGENTS.md) | The rules for each change, by a human or an agent |

## Credits

The English translation comes from the community English patch. Its
authors gave permission to ship it with DLGE Player.

| Game | Translation |
| ---- | ----------- |
| Haunted Dark Bridal | otomvevn |
| MORE,BLOOD | dialovers-translations |

## Support

If you find DLGE Player useful, you can support its development here.

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/stivengjekaj)

## Licence

The code is under the GPL-3.0-or-later licence. See [LICENSE](LICENSE).
Diabolik Lovers and its art, voices, music, and text belong to Idea Factory,
Otomate, and Rejet.
