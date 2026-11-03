#!/bin/sh
# Builds the Rust library of the player and copies it into godot/bin.
#
# Usage: scripts/build-player.sh [debug|release] [desktop|android|ios]
#
# android needs cargo-ndk and the Android NDK. ios needs Xcode. It makes
# one framework for the devices and one for the simulator, in one
# xcframework.
set -eu
profile=${1:-debug}
platform=${2:-desktop}
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
flag=
[ "$profile" = release ] && flag=--release

# The English patch ships with the player. The export reads it from here.
mkdir -p godot/patch
cp assets/patch/en.dlpatch godot/patch/en.dlpatch

case "$platform" in
android)
    if [ -z "${ANDROID_NDK_HOME:-}" ]; then
        sdk=${ANDROID_HOME:-$HOME/Library/Android/sdk}
        ANDROID_NDK_HOME=$(ls -d "$sdk"/ndk/* | sort -V | tail -1)
        export ANDROID_NDK_HOME
    fi
    # API level 24 is Android 7.0, the oldest version that Godot 4 supports.
    cargo ndk -t arm64-v8a --platform 24 build -p player $flag
    mkdir -p godot/bin/android
    rm -f godot/bin/android/libplayer.so
    cp "target/aarch64-linux-android/$profile/libplayer.so" godot/bin/android/
    echo "copied libplayer.so to godot/bin/android"
    ;;
ios)
    out=godot/bin/ios
    work=target/ios-frameworks
    rm -rf "$out/libplayer.xcframework" "$work"
    # .cargo/config.toml gives the oldest iOS version to cargo.
    min=$(sed -n 's/^IPHONEOS_DEPLOYMENT_TARGET = "\(.*\)"/\1/p' .cargo/config.toml)
    # The simulator part of the iOS template of Godot 4.7 holds only
    # x86_64 code, so the simulator library holds x86_64 too. For an
    # Apple silicon Mac, scripts/ios-simulator.py adds arm64 code to it.
    for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
        cargo build -p player $flag --target "$target"
    done
    set --
    for slice in device simulator; do
        framework="$work/$slice/libplayer.framework"
        mkdir -p "$framework"
        if [ $slice = device ]; then
            cp "target/aarch64-apple-ios/$profile/libplayer.dylib" "$framework/libplayer"
        else
            lipo -create -output "$framework/libplayer" \
                "target/aarch64-apple-ios-sim/$profile/libplayer.dylib" \
                "target/x86_64-apple-ios/$profile/libplayer.dylib"
        fi
        install_name_tool -id @rpath/libplayer.framework/libplayer "$framework/libplayer"
        cat >"$framework/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>libplayer</string>
	<key>CFBundleIdentifier</key>
	<string>io.github.stiven-gjekaj.dlgeplayer.libplayer</string>
	<key>CFBundleName</key>
	<string>libplayer</string>
	<key>CFBundlePackageType</key>
	<string>FMWK</string>
	<key>CFBundleShortVersionString</key>
	<string>1.0</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>MinimumOSVersion</key>
	<string>$min</string>
</dict>
</plist>
EOF
        set -- "$@" -framework "$framework"
    done
    mkdir -p "$out"
    xcodebuild -create-xcframework "$@" -output "$out/libplayer.xcframework"
    ;;
*)
    cargo build -p player $flag
    case "$(uname -s)" in
    Darwin) dir=macos; file=libplayer.dylib ;;
    Linux) dir=linux; file=libplayer.so ;;
    *) dir=windows; file=player.dll ;;
    esac
    mkdir -p "godot/bin/$dir"
    # Remove the old file first. On macOS, a copy over a library that a
    # program has open changes the file in place, and the system then stops
    # each new program that loads it.
    rm -f "godot/bin/$dir/$file"
    cp "target/$profile/$file" "godot/bin/$dir/$file"
    echo "copied target/$profile/$file to godot/bin/$dir"
    ;;
esac
