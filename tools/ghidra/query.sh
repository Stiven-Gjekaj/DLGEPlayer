#!/bin/sh
# Runs DlQuery.java on a Ghidra project of the game executable.
#
# GHIDRA_INSTALL_DIR is the Ghidra 12.1 folder.
# DLGE_GHIDRA_PROJECT is the folder of a project named "dlge" that holds
# main.nso, imported with the Switch Loader and analyzed.
#
# Each argument is one command, for example "dec:0x746c0" or "refs:0xb5d600".
set -eu
here=$(cd "$(dirname "$0")" && pwd)
"$GHIDRA_INSTALL_DIR/support/analyzeHeadless" "$DLGE_GHIDRA_PROJECT" dlge \
    -process main.nso -noanalysis -readOnly \
    -scriptPath "$here" -postScript DlQuery.java "$@" 2>&1 |
    sed -n '/DlQuery.java> BASE/,$p' |
    sed -e 's/^INFO  DlQuery.java> //' -e 's/ (GhidraScript) *$//' |
    grep -v -E '^(INFO|WARN|ERROR) '
