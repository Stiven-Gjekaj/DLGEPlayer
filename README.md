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
  <a href="#make-a-dump"><b>Dump</b></a> |
  <a href="#build"><b>Build</b></a> |
  <a href="docs/formats.md"><b>Formats</b></a> |
  <a href="docs/decisions.md"><b>Decisions</b></a> |
  <a href="docs/roadmap.md"><b>Roadmap</b></a>
</p>

</div>

---

> [!NOTE]
> **Phase 1 of 8 is done. The player does not run the game yet.**
> The tools read the archives and the scripts of the game. All 706 scripts
> of the Switch release parse, write back byte for byte, and dump to text.
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

- Read CRI CPK archives by TOC or by ITOC
- Decompress CRILAYLA data
- Read and write STCM2L scripts, byte for byte
- Decode the parameters and the data blocks of each action
- Write each script as a listing and as text, with the Shift-JIS text
  decoded

Planned:

- The meaning of each opcode ([phase 2](docs/roadmap.md))
- Read the Bezel Engine `.bea` archives: textures, sprites, and layouts
- The importer, the VM, and the Godot player
- Builds for Windows, macOS, Linux, Android, and iOS
- The English patch

## You need a copy of the game

The art, the voices, the music, and the text of the game belong to their
owners. The tools read them from your copy. You need:

- your own Switch copy of Diabolik Lovers Grand Edition (JP, title ID
  `0100BD700E648000`)
- the keys of your own console

## Make a dump

1. Unpack the NSP:

       hactool -k prod.keys -t pfs0 --outdir=nsp "DIABOLIK LOVERS GRAND EDITION.nsp"

2. Extract the RomFS of the largest NCA:

       hactool -k prod.keys --romfsdir=romfs nsp/<largest>.nca

Keep the dump outside the repository.

## Build

You need Rust 1.88 or later.

    cargo build --release

The tool is `target/release/dlplayer`.

## Commands

| Command | Result |
| ------- | ------ |
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

## Crates

| Crate | Purpose |
| ----- | ------- |
| `cpk` | Reads CRI CPK archives and CRILAYLA data |
| `stcm2l` | Reads and writes STCM2L scripts, and writes listings |
| `bea` | Reads Bezel Engine archives |
| `bntx` | Decodes BNTX textures to RGBA pixels |
| `sarc` | Reads SARC archives, such as the layouts |
| `importer` | Converts a dump into a normalized folder. Builds sprites now. |
| `vm` | Runs the script bytecode. Empty. |
| `player` | The Godot 4 extension. Empty. |
| `dlplayer` | The command line tool |

## Tests

    cargo test --workspace

The tests build the data that they need. They do not read a copy of the
game. The corpus test reads real scripts only if you set `DLGE_SCRIPTS` to a
directory of extracted `.dat` files:

    DLGE_SCRIPTS=scripts cargo test -p stcm2l --test corpus

## Documentation

| File | Contents |
| ---- | -------- |
| [docs/formats.md](docs/formats.md) | The CPK, CRILAYLA, and STCM2L formats, and the layout of the RomFS |
| [docs/roadmap.md](docs/roadmap.md) | The phases, the platforms, and the risks |
| [docs/decisions.md](docs/decisions.md) | Each decision and its reason |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to help |
| [AGENTS.md](AGENTS.md) | The rules for each change, by a human or an agent |

## Support

If you find DLGE Player useful, you can support its development here.

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/stivengjekaj)

## Licence

The code is under the GPL-3.0-or-later licence. See [LICENSE](LICENSE).
Diabolik Lovers and its art, voices, music, and text belong to Idea Factory,
Otomate, and Rejet.
