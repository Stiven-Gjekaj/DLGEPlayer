#!/bin/sh
# Builds the Rust library of the player and copies it into godot/bin.
#
# Usage: scripts/build-player.sh [debug|release]
set -eu
profile=${1:-debug}
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if [ "$profile" = release ]; then
    cargo build -p player --release
else
    cargo build -p player
fi
case "$(uname -s)" in
Darwin) dir=macos; file=libplayer.dylib ;;
Linux) dir=linux; file=libplayer.so ;;
*) dir=windows; file=player.dll ;;
esac
mkdir -p "godot/bin/$dir"
cp "target/$profile/$file" "godot/bin/$dir/$file"
echo "copied target/$profile/$file to godot/bin/$dir"
