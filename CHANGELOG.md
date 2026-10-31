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
- `dlplayer cpk list` and `dlplayer cpk extract` read archives.
- `dlplayer disasm` prints the listing or the text of one script.
- `dlplayer dump-scripts` writes a listing and a text file for each script
  of an archive.
