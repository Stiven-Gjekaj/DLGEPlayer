# DLGE player

An open-source player for Diabolik Lovers Grand Edition (Switch, JP, 2019).
The goal is to run Haunted Dark Bridal and MORE,BLOOD on PC and Android.

The player uses only the data that you dump from your own copy of the game.
This repository holds no game data, no scripts, and no patch files.

## Status

Phase 1 is done. The tools read the scripts of the game.
The player does not run the game yet.

## Crates

| Crate | Purpose |
|---|---|
| `cpk` | Reads CRI CPK archives and CRILAYLA data. |
| `stcm2l` | Reads and writes STCM2L scripts. Writes text listings. |
| `importer` | Converts a dump into a normalized asset folder. Empty. |
| `vm` | Runs the script bytecode. Empty. |
| `player` | The Godot 4 extension. Empty. |
| `dlplayer` | The command line tool. |

## Make a dump

You need your own copy of the game and the keys of your own console.

1. Unpack the NSP with `hactool -t pfs0`.
2. Extract the RomFS of the largest NCA with `hactool --romfsdir=romfs`.

## Commands

Build the tool:

    cargo build --release

Print the files in an archive:

    dlplayer cpk list romfs/script.cpk

Extract an archive:

    dlplayer cpk extract romfs/script.cpk scripts

Print the listing of one script:

    dlplayer disasm scripts/mb_aya_Dark_01.dat

Print only the text of one script:

    dlplayer disasm --strings scripts/mb_aya_Dark_01.dat

Write a listing (`.asm`) and a text file (`.txt`) for each script:

    dlplayer dump-scripts romfs/script.cpk scripts_text

## Tests

    cargo test

The corpus test reads real scripts. It runs only if you set
`DLGE_SCRIPTS` to a directory of extracted `.dat` files:

    DLGE_SCRIPTS=scripts cargo test -p stcm2l --test corpus

## Formats

See [docs/formats.md](docs/formats.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
