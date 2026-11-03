#!/usr/bin/env python3
"""Builds the iOS app for the simulator of an Apple silicon Mac and starts it.

Usage: scripts/ios-simulator.py [--device <name or UDID>] [-- <player options>]

Run scripts/build-player.sh debug ios first. The script exports the iOS
preset, builds the app for the arm64 simulator, installs it, and starts it.
The options after -- go to the player, for example
-- --folder <dir> --chapter mb_aya_Dark_09.

The simulator part of the iOS template of Godot 4.7.2 holds only x86_64
code. A simulator of iOS 27 does not start an x86_64 app. The arm64 device
part holds the same code. The script copies it and marks each object file
as simulator code. It changes the platform field of LC_BUILD_VERSION from
IOS to IOSSIMULATOR. The size of each file stays the same, so the symbol
table of the archive stays correct.
"""
import os
import struct
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXPORT = os.path.join(ROOT, "godot", "export", "ios")
GODOT = os.environ.get(
    "GODOT", os.path.join(ROOT, "tools", "godot", "Godot.app", "Contents", "MacOS", "Godot")
)
BUNDLE = "io.github.stiven-gjekaj.dlgeplayer"

MH_MAGIC_64 = 0xFEEDFACF
LC_BUILD_VERSION = 0x32
PLATFORM_IOS = 2
PLATFORM_IOSSIMULATOR = 7

# The simulator SDK has no definition of these Metal symbols. The Godot
# library refers to them, so the link fails without these empty values.
STUBS = "const void *MTLIOErrorDomain = 0;\nconst void *MTLTensorDomain = 0;\n"


def to_simulator(archive: bytes) -> tuple[bytes, int]:
    """Marks each 64-bit Mach-O member of an ar archive as simulator code.

    Returns the new archive and the number of changed objects.
    """
    data = bytearray(archive)
    if data[:8] != b"!<arch>\n":
        raise ValueError("the file is not an ar archive")
    pos, changed = 8, 0
    while pos < len(data):
        header = data[pos : pos + 60]
        name = header[:16].decode().strip()
        size = int(header[48:58].decode().strip())
        body = pos + 60
        # A BSD archive puts a long name at the start of the member body.
        start = body + (int(name[3:]) if name.startswith("#1/") else 0)
        if size >= 32 and struct.unpack_from("<I", data, start)[0] == MH_MAGIC_64:
            count = struct.unpack_from("<I", data, start + 16)[0]
            command = start + 32
            for _ in range(count):
                kind, length = struct.unpack_from("<II", data, command)
                if kind == LC_BUILD_VERSION:
                    platform = struct.unpack_from("<I", data, command + 8)[0]
                    if platform != PLATFORM_IOS:
                        raise ValueError(f"{name}: platform {platform} is not IOS")
                    struct.pack_into("<I", data, command + 8, PLATFORM_IOSSIMULATOR)
                    changed += 1
                command += length
        pos = body + size + ((body + size) & 1)
    return bytes(data), changed


def run(*command: str) -> None:
    subprocess.run(command, check=True)


def simulator_slice(work: str) -> None:
    """Adds the arm64 simulator code to the simulator part of libgodot.a."""
    frameworks = os.path.join(EXPORT, "DLGE Player.xcframework")
    with open(os.path.join(frameworks, "ios-arm64", "libgodot.a"), "rb") as f:
        archive, changed = to_simulator(f.read())
    arm64 = os.path.join(work, "arm64.a")
    with open(arm64, "wb") as f:
        f.write(archive)
    print(f"marked {changed} objects as simulator code")
    target = os.path.join(frameworks, "ios-arm64_x86_64-simulator", "libgodot.a")
    fat = os.path.join(work, "fat.a")
    run("lipo", "-create", arm64, target, "-output", fat)
    os.replace(fat, target)


def main() -> None:
    args = sys.argv[1:]
    player = []
    if "--" in args:
        index = args.index("--")
        args, player = args[:index], args[index + 1 :]
    device = "booted"
    if args[:1] == ["--device"] and len(args) == 2:
        device = args[1]
    elif args:
        sys.exit(__doc__)

    os.makedirs(EXPORT, exist_ok=True)
    run(GODOT, "--headless", "--path", os.path.join(ROOT, "godot"),
        "--export-debug", "iOS", "export/ios/DLGE Player.ipa")
    with tempfile.TemporaryDirectory() as work:
        simulator_slice(work)
        stub = os.path.join(work, "stub.c")
        with open(stub, "w") as f:
            f.write(STUBS)
        stub_object = os.path.join(work, "stub.o")
        run("xcrun", "-sdk", "iphonesimulator", "clang",
            "-target", "arm64-apple-ios15.0-simulator", "-c", stub, "-o", stub_object)
        run("xcodebuild", "-project", os.path.join(EXPORT, "DLGE Player.xcodeproj"),
            "-scheme", "DLGE Player", "-configuration", "Debug", "-sdk", "iphonesimulator",
            "-derivedDataPath", os.path.join(EXPORT, "simbuild"),
            "ARCHS=arm64", "ONLY_ACTIVE_ARCH=NO", "CODE_SIGNING_ALLOWED=NO",
            f"OTHER_LDFLAGS=$(inherited) {stub_object}", "-quiet", "build")
    app = os.path.join(EXPORT, "simbuild", "Build", "Products",
                       "Debug-iphonesimulator", "DLGE Player.app")
    run("xcrun", "simctl", "install", device, app)
    subprocess.run(["xcrun", "simctl", "terminate", device, BUNDLE], capture_output=True)
    run("xcrun", "simctl", "launch", device, BUNDLE, *(["--", *player] if player else []))


if __name__ == "__main__":
    main()
