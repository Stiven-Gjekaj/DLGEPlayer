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

## NSP files

The `nsp` crate reads the RomFS straight from the NSP file with the keys
of the user. It needs `header_key` and `key_area_key_application_XX` from
`prod.keys`. A dump with a ticket also needs `titlekek_XX`.

- The NSP is a PFS0 package: `PFS0`, the file count, the size of the
  string table, then 0x18-byte entries (offset, size, name offset).
- The game NSP holds 4 NCA files. The program NCA (content type 0) is
  6.4 GB. This dump has no ticket, so the section key is in the key area.
- The NCA header (0xc00 bytes) is encrypted with AES-XTS and the
  `header_key`. The Switch puts the sector number in the tweak as a
  big-endian value. The magic is `NCA3`. The key generation of this game
  is the larger of the bytes at 0x206 and 0x220, less one.
- The key area holds 4 keys, encrypted with AES-ECB and the key area key.
  Key 2 decrypts the sections in AES-CTR mode.
- The RomFS section has IVFC hash levels. The last level is the RomFS. The
  counter of a block is the 8 bytes at 0x140 of the section header in
  reverse order, then the offset in the NCA divided by 16, big-endian.
- The RomFS has a header of 10 u64 values, a directory table, a file
  table, and the file data.

The 2125 files that the crate reads from the NSP have the same bytes as
the files that hactool extracts: 6,401,268,984 bytes. An import from the
NSP gives the same 43339 files as an import from the extracted RomFS.

## The English patch file

The English patch ships as a patch file of deltas. `dlplayer make-patch`
makes it from the dump and the RomFS of the patch, and `dlplayer import
--translation` applies it.

A delta has the magic `DLD1`, the size of the new file, the FNV-1a hash
of the old file, and operations: copy from the old file, or insert new
bytes. The import refuses a delta if the hash of the file of the dump is
different. A patch file has the magic `DLPS`, a version, and the deltas
by key. A key is a RomFS path, or a file in an archive, for example
`script.cpk/mb_aya_Dark_01.dat` or
`Archive/advUI.nxonnx32.bea/layout/mb_advUI.lyt`.

The patch file of the English patch has 717 deltas and 12,465,713 bytes:

| Source | Changed files | New size | Delta |
|---|---|---|---|
| `script.cpk` | 706 scripts | 69,243,048 bytes | 12,400,765 bytes |
| `advUI.bea` | 2 layouts | 22,572,116 bytes | 2,050 bytes |
| `lua.bea` | 7 scripts | 97,500 bytes | 4,637 bytes |
| `static~select.bin.bea` | 1 table | 43,328 bytes | 21,132 bytes |
| `static~bg.bin.bea` | 1 table | 28,353 bytes | 7,597 bytes |
| `static~speaker.bin.bea` | 1 table | 3,954 bytes | 3,256 bytes |

The deltas of the scripts hold mostly the English text. An import with
the patch file gives the same scripts, tables, and layouts as an import
with the patched files over the dump.

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

The scripts name a sprite with two strings: a pose such as `ayaU_03`, and
the pose with an expression such as `ayaU_03_cho`. A row name is the pose
with a number before it and a size letter after it, for example
`01_ayaU_03_A`. In the JP scripts, 45588 calls have two string arguments.
For 39686 of them, the names match a row and an expression of that row.
These rules resolve the names:

- The second name decides the sprite. It is a pose and an expression. In
  `"kan_03", "kan_03b_tujo"`, the pose is `kan_03b`, a costume variant of
  `kan_03`, and the expression is `tujo`.
- The scripts call the Shu of Haunted Dark Bridal `shuu`. His rows are
  named `veshuu`.
- Some rows have no size letter, for example `21_car_01` and the small
  sprites `11_miniaya_01`.
- The scripts spell the expression ちょ as `tyo` (Kunrei romanization).
  The rows spell it `cho` (Hepburn romanization).

`importer::sprite::Catalog` uses these rules. In the JP and the EN scripts,
all 45588 calls resolve to a pose and an expression.

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

## Layouts

A `.lyt` file is a SARC archive. The `sarc` crate reads it. The 84 layouts
hold 521 `.bflyt` layouts, 348 `.bflan` animations, and 83 BNTX files with
the textures of the user interface. These are the NintendoWare layout
formats, version 8.6. The `lyt` crate reads the layouts and the animations.
All 521 layouts and all 348 animations read.

A `.bflyt` file has sections. Each section has a 4-byte magic number and a
u32 size that includes the 8-byte header. The 521 layouts have these
sections:

| Section | Count | Contents |
|---|---|---|
| `pic1` | 1871 | a picture pane: 4 corner colors, a material, and the texture coordinates |
| `prt1` | 1353 | a part pane: it shows another layout of the same archive |
| `pan1` | 866 | a pane that only holds other panes |
| `grp1`, `grs1`, `gre1` | 730, 190, 190 | the groups of panes |
| `pas1`, `pae1` | 716, 716 | the start and the end of the children of a pane |
| `mat1` | 454 | the materials |
| `cnt1` | 428 | the control data |
| `txl1` | 422 | the texture names |
| `usd1` | 257 | user data |
| `txt1` | 156 | a text pane, with UTF-16 text |
| `bnd1` | 56 | a bounding pane |
| `fnl1` | 53 | the font names. All layouts use `main.bffnt`. |
| `ali1` | 6 | an alignment pane |
| `wnd1` | 1 | a window pane |

A pane section starts with 0x4c bytes after the header: the flags (bit 0
is visible), the origin, the alpha, an extra flag byte, the name
(24 bytes), the user data (8 bytes), the translation (3 f32), the rotation
(3 f32), the scale (2 f32), and the size (2 f32). The name of the layout of
a part pane can be longer than 24 bytes. It ends with a zero byte.

A material has a 28-byte name, the flags, 4 unknown bytes, the black color
and the white color (RGBA), and then the texture maps. The low 2 bits of
the flags give the number of texture maps. A texture map has the texture
index (u16) and the wrap modes (2 bytes).

A `.bflan` file has a `pat1` section with the name and the groups, and a
`pai1` section with the frame count, the loop flag, and the targets. A
target is a pane or a material with tags, for example `FLPA` for the pane
position and `FLVC` for the corner colors. A tag has curves. The 348
animations use two curve types:

| Type | Key | Use |
|---|---|---|
| 1, step | frame (f32), value (u16), 2 bytes of padding | `FLVI`, the visibility |
| 2, Hermite | frame, value, slope (f32) | `FLPA`, `FLPS`, `FLVC`, `FLMC`, `FLTS`, `FLDS` |

No animation changes a texture.

The 83 BNTX files hold 1163 textures. All of them decode:

| Format | Count |
|---|---|
| `0x1c06` BC3 sRGB | 1110 |
| `0x1d01` BC4 | 20 |
| `0x1e01` BC5 | 17 |
| `0x2006` BC7 sRGB | 10 |
| `0x1a06` BC1 sRGB | 6 |

The BC4 and BC5 textures use the channel sources of the `BRTI` block. The
suffix of the texture name shows them: `^r` is `RRR1`, `^s` is `111R`, and
`^t` is `RRRG`. For example, `^t` gives a grey image with the alpha from
the second channel.

## Audio

The `.hca` files are not encrypted. All 418 files of `bgm.cpk` and `se.cpk`
have the cipher type 0. All 38103 files of `voice.cpk` convert with no key.

A voice action gives a number, for example `voice 30063`. The file is
`<game>_<number>.hca` with 6 digits, for example `mb_030063.hca` in a
MORE,BLOOD script. Some files have a letter after the number, for example
`ve_011038b.hca`. No script plays these files with this letter.

Two BGM tracks have a loop start that is not 0: `BGM119` and `BGM121`.
The other tracks loop from the start.

`SystemSE.acb` holds 5 menu sounds as ADX, not as HCA. They are standard
ADX (encoding 3, version 4) with no encryption. The `importer::adx`
decoder gives the same samples as the ADX decoder of FFmpeg.

## Lip sync

A `.lip` file of `lip.cpk` gives the mouth state of a sprite while a voice
plays. All numbers are little-endian u32. The file has the sample rate
(44100 in all files), then pairs of a sample position and a state: 1 for
open, 0 for closed. A pair with the position `ffffffff` ends the list. The
file name is the voice number, for example `010001.lip`. All 24123 files
have this shape, and the positions are in order.

## The import folder

`dlplayer import` writes a folder that the player reads. It holds no
console format.

| Path | Contents |
|---|---|
| `manifest.json` | the list of all files, with the sprite layouts and the audio data |
| `scripts/<name>.json` | the IR of each script |
| `tables/<name>.json` | each `BIN_` table, with the column types and the rows |
| `images/<group>/<name>.png` | each standalone texture, for example `images/bg/bg01b.png` |
| `sprites/<name>.png` | each sprite sheet. `manifest.json` gives the body and the patch positions. |
| `ui/<archive>/<name>.png` | each texture of a layout archive |
| `ui/<archive>/layouts/<name>.json` | each `.bflyt` layout |
| `ui/<archive>/animations/<name>.json` | each `.bflan` animation |
| `audio/<group>/<name>.ogg` | `bgm`, `se`, `loopse`, `voice`, and `system`, as Ogg Vorbis |
| `lip.json` | the lip sync keys, by voice number |
| `movies/<name>.mp4` | each movie, as it is |

The IR of a script is a JSON object with the name, the exports, the first
values of the global variables, and the code. The code is a list of
instructions. Each instruction has a `type`, for example
`{"type":"goto","target":7}`. A code address is the index of an instruction
in the list. A global variable is its offset in the global data. A value is
an object with one key, for example `{"int":5}` or `{"text":"..."}`.

A looped audio track ends at its loop end. `manifest.json` gives the loop
start in sample frames. The loop end is the end of the file.

The full import of the JP dump takes 4 to 5 minutes on 10 cores. It
writes 6.6 GB: 2.2 GB of audio, 1.7 GB of images, 1.4 GB of movies, and
1.1 GB of sprites. `--jobs` sets the number of threads.

## RAW_ images

`texture~copyright~copyright.bin` is not a BNTX file. It has the magic
`RAW_`, the width and the height as u16, and then RGBA pixels. It is the
copyright line, 420x48 pixels.

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

- The formats `0x1505`, `0x201`, and `0x1905` in `System.bea`.
- The 41 menu scripts in `lua.bea` are Lua 5.2 bytecode (`BZLA`, then
  `LuaR`). The importer does not convert them.
- The font `font/main.bffnt` is not decoded.
- The meaning of the user data and of the control data of the layouts.

- The text has some rare characters where the context needs punctuation.
  For example, `ゞ` appears after `なに`. The font of the game possibly
  shows a different glyph for these characters. This is not verified.
- `op_ff14` has 12 bytes of data that no parameter points to.
- The meaning of most opcodes is not known yet.
- The name tokens of the English patch, such as `#Name[2]`, are not
  decoded yet.
