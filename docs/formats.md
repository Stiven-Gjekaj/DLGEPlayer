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

## Bezel Engine archives

A `.bea` file has the magic `SCNE`. The `bea` crate reads it.

- The header gives the number of files and an array of offsets to `ASST`
  blocks.
- An `ASST` block gives the compression, the sizes, the data offset, and the
  name of one file, for example `texture/bg/bg01b.dds`.
- All 2196 files of the dump use zstd compression (type 2).

The files in the archives:

| Type | Count | Contents |
|---|---|---|
| `.dds` | 2031 | BNTX textures |
| `.lyt` | 84 | SARC archives with UI layouts |
| `.lua` | 41 | Lua scripts of the menus, magic `BZLA` |
| `.bfsha` | 14 | shaders |
| `.bin` | 10 | `BIN_` tables, for example `static/chara.bin` |
| `.mp4` | 9 | XML movie descriptions |
| `.csv` | 3 | text tables |
| other | 4 | 2 `.ftxb` textures, 1 font, 1 entity |

## BNTX textures

A `.dds` file in an archive is a BNTX texture. The `bntx` crate decodes it.

- 2029 of the 2031 textures use format `0x2006`, BC7 sRGB. The data uses the
  block-linear layout of the Tegra X1.
- All 2029 textures in `Archive/` decode. The test reads every one.
- Backgrounds and event pictures are 1920x1080.

## Character sprites

Each sprite archive, for example `01_aya_01_A`, holds one texture
`chara/<name>/base.dds`. The texture is a sheet. It holds the full body and
a grid of patches for the eyes and the mouth. The game makes an expression
when it draws a patch on the face.

`static/chara.bin` holds the positions of the patches. It has 878 rows,
one for each sprite archive. A row has 13 groups of a name and 12
integers. The first group describes the sheet:

| Index | Value |
|---|---|
| 0, 1 | the width and the height of the sheet |
| 2, 3 | the width and the height of the body, at the top-left corner |
| 4, 5 | the width and the height of an eye patch |
| 6, 7 | the position of the eye patch on the body |
| 8, 9 | the width and the height of a mouth patch |
| 10, 11 | the position of the mouth patch on the body |

Each other group is an expression, for example `emi`, `tujo`, or
`metoji`. Its 12 integers are 6 positions in the sheet: 3 eye patches for
a blink, and then 3 mouth patches for speech. A group with an empty name
and the values -1 is not used.

All 878 rows give the size of their texture. All 9969 frames of the
expressions are inside their sheet and their body. `dlplayer sprite`
builds a sprite with an expression.

## BIN_ tables

The `static/*.bin` files use the format `BIN_`. The header:

| Offset | Value |
|---|---|
| 0x06 | the number of rows, as a u16 |
| 0x08 | a count, as a u32 |
| 0x0c | the number of strings, as a u32 |
| 0x10 | the offset of the values |
| 0x14 | the offset of the string offsets, u32 each |
| 0x18 | the offset of the strings |

The first string gives the type of each column: `s` for a string, `i` for
an integer. The table keeps its values column by column. The strings after
the first one are all rows of the first string column, then all rows of
the next one. The integers are i16 values, kept in the same way. The
number of integer slots is a multiple of 2. The strings are Shift-JIS.

| Table | Columns | Rows | Contents |
|---|---|---|---|
| `bg.bin` | `ssiis` | 795 | background name, file, number, a value, place name |
| `bgm.bin` | `is` | 32 | music number, title |
| `chara.bin` | 13 x `siiiiiiiiiiii` | 878 | sprite layouts |
| `ev.bin` | `ssiiii` | 310 | event picture name, file, number, flags |
| `flag.bin` | `si` | 12 | flag name, start value |
| `flagsys.bin` | `si` | 964 | system flag name, start value |
| `script.bin` | `iis` | 706 | game, number, script name |
| `select.bin` | `ssiii` | 1002 | choice name, choice text, 3 values |
| `speaker.bin` | `iss` | 181 | speaker number, display name, voice prefix |

The English patch replaces `bg.bin`, `select.bin`, and `speaker.bin`. The
patched `speaker.bin` has the English names.

## Audio

The `.hca` files are not encrypted. All 418 files of `bgm.cpk` and `se.cpk`
have the cipher type 0. A sample of 763 files of `voice.cpk`, every 50th
file, also has the cipher type 0. ffmpeg decodes them.

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

- The textures inside the `.lyt` layouts, and the formats `0x1505`, `0x201`,
  and `0x1905`.

- The text has some rare characters where the context needs punctuation.
  For example, `ゞ` appears after `なに`. The font of the game possibly
  shows a different glyph for these characters. This is not verified.
- `op_ff14` has 12 bytes of data that no parameter points to.
- The meaning of most opcodes is not known yet.
- The name tokens of the English patch, such as `#Name[2]`, are not
  decoded yet.
