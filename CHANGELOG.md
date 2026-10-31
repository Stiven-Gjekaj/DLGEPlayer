# Changelog

## Unreleased

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
