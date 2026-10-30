# File formats of Diabolik Lovers Grand Edition (Switch)

These notes come from the Japanese release, title ID `0100BD700E648000`.
They describe the files that the importer reads.

## The RomFS

The game runs on the Bezel Engine, not on the old Otomate engine.
`boot.lua` sets a base resolution of 1920x1080.

| Path | Contents |
|---|---|
| `script.cpk` | 706 STCM2L scripts, compressed with CRILAYLA |
| `voice.cpk` | 38103 voice clips, `.hca` |
| `bgm.cpk` | 57 music tracks, `.hca` |
| `se.cpk`, `loopse.cpk` | sound effects, `.hca` |
| `lip.cpk` | 24123 lip sync files, `.lip` |
| `SystemSE.acb`, `SystemSE.acf` | CRI ADX2 system sounds |
| `movie/*.mp4` | 9 movies |
| `Archive/*.nxonnx32.bea` | Bezel Engine archives, magic `SCNE` |
| `*.nxonwin32.bea` | the same archives for a Windows build |

Names in `Archive/` use `~` for `/`. For example,
`texture~bg~bg01b.dds.nxonnx32.bea` holds the texture `texture/bg/bg01b.dds`.
The script prefix `mb_` is MORE,BLOOD. The prefix `ve_` is Haunted Dark Bridal.

## CPK

A CPK file has chunks. A chunk has a 4-byte magic number, 4 unknown bytes,
a little-endian u64 size, and a `@UTF` table.

- `CPK ` at offset 0 holds the header table.
- `TocOffset` gives the `TOC ` chunk. It has one row for each file.
- The data offset of a file is `FileOffset` plus the lower value of
  `TocOffset` and `ContentOffset`.
- All archives of this game have a `TOC `.

A `@UTF` table is big-endian. Some tools use an XOR mask on the table.
This game does not use the mask.

## CRILAYLA

| Offset | Size | Value |
|---|---|---|
| 0x00 | 8 | `CRILAYLA` |
| 0x08 | 4 | the size of the body after decompression |
| 0x0c | 4 | the size of the compressed stream |
| 0x10 | stream | the compressed stream |
| after the stream | 0x100 | a prefix that is not compressed |

The decoder reads the stream from the end to the start.
It writes the body from the end to the start.
The output is the prefix and then the body.

## STCM2L

All numbers are little-endian u32. All offsets are absolute.

| Offset | Value |
|---|---|
| 0x00 | the tag, 0x20 bytes, for example `STCM2L Aug 24 2012 16:30:16` |
| 0x20 | the offset of the export entries |
| 0x24 | the number of export entries |
| 0x28 | 1 in all files |
| 0x2c | the offset of the collection link body |
| 0x30 | 0x20 zero bytes |
| 0x50 | `GLOBAL_DATA\0`, then the global data |

The sections follow in this sequence:

1. `CODE_START_\0`, then the actions.
2. `EXPORT_DATA\0`, then the export entries.
3. `COLLECTION_LINK\0`, then a 64-byte body.

The marker and the body of the collection link make an 80-byte trailer.
The body holds 0, the file size, and 56 zero bytes.

In the original files, the trailer is the end of the file.
A translation patch can add an appendix after the trailer.
The file size in the body then includes the appendix.

### Actions

| Field | Value |
|---|---|
| call | 0 for an instruction, 1 for a call |
| opcode | the opcode, or the target offset for a call |
| count | the number of parameters |
| length | the length of the action in bytes |
| parameters | `count` entries of three u32 values: `p0`, `p1`, `p2` |
| data | the bytes up to `length` |

The scripts have 76 different opcodes and 83115 calls.

### Parameters

| Shape | Meaning |
|---|---|
| `p0 = 0xffffff41` | a code address in `p1` |
| `p0 = 0xffffffXX` | the immediate value `XX` |
| `p0` in the data of the action | a data block |
| `p0` in the global data | a global variable |
| `p0` in the appendix | a data block that a patch added |

`p1` and `p2` are `0x40000000` when the shape does not use them.

### Data blocks

A data block has a 16-byte head: the kind, the number of words, 1, and the
length in bytes. The words follow the head.

- Kind 0 holds an integer or Shift-JIS text that ends with a zero byte.
- Kind 1 holds a 32-bit float.

Opcode `0x4ba` shows a line of dialogue.
Opcodes `0x3` and `0x6` have a code address. They are jumps.

### Export entries

Each entry has 40 bytes: 0, a 32-byte name padded with zero bytes, and
the offset of an action.

## The English patch

The English patch is a LayeredFS mod for the title `0100bd700e648000`.
It replaces these files of the RomFS:

- `script.cpk`
- `Archive/advUI.nxonnx32.bea`
- `Archive/lua.nxonnx32.bea`
- `Archive/static~bg.bin.nxonnx32.bea`
- `Archive/static~select.bin.nxonnx32.bea`
- `Archive/static~speaker.bin.nxonnx32.bea`

The patched `script.cpk` holds the same 706 script names.
Each patched script puts the English text in data blocks in the appendix.
The parameters point to these blocks.
The old Japanese blocks stay in the actions, and no parameter points to them.

In the patch that we examined, 137520 actions have the opcode `0x4ba`.
118839 of them point to English text.
18681 of them point to an empty text block.
The text is Shift-JIS. It is mostly ASCII, with some Shift-JIS punctuation
such as `’` and `ー`. It has name tokens such as `#Name[2]`.

## Known open items

- The text has some rare characters where the context needs punctuation.
  For example, `ゞ` appears after `なに`. The font of the game possibly
  shows a different glyph for these characters. This is not verified.
- `op_ff14` has 12 bytes of data that no parameter points to.
- The meaning of most opcodes is not known yet.
- The name tokens of the English patch, such as `#Name[2]`, are not
  decoded yet.
