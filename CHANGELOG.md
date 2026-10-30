# Changelog

## Unreleased

- The `cpk` crate reads CRI CPK archives by TOC or by ITOC.
- The `cpk` crate decompresses CRILAYLA data.
- The `stcm2l` crate reads and writes STCM2L scripts. All 706 scripts of the
  Switch release write back byte for byte.
- The `stcm2l` crate decodes the parameters and the data blocks of each
  action, with the Shift-JIS text.
- `dlplayer cpk list` and `dlplayer cpk extract` read archives.
- `dlplayer disasm` prints the listing or the text of one script.
- `dlplayer dump-scripts` writes a listing and a text file for each script
  of an archive.
