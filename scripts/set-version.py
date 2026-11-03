#!/usr/bin/env python3
"""Writes a version into the export presets of the Godot project.

Usage: scripts/set-version.py <version> [presets file]

The build workflow runs it for a tag such as v1.2.3, so the apps get the
version of the tag. A commit does not change the version. The version has
the form MAJOR.MINOR.PATCH. The Android version code is
MAJOR * 10000 + MINOR * 100 + PATCH, so a newer version has a larger code.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PRESETS = os.path.join(ROOT, "godot", "export_presets.cfg")

# The keys of the presets that hold a version, and the value of each key.
KEYS = {
    "application/short_version": lambda v, c: f'"{v}"',
    "application/version": lambda v, c: f'"{v}"',
    "version/name": lambda v, c: f'"{v}"',
    "version/code": lambda v, c: str(c),
}


def version_code(version: str) -> int:
    match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", version)
    if match is None:
        raise ValueError(f"{version} does not have the form MAJOR.MINOR.PATCH")
    major, minor, patch = (int(part) for part in match.groups())
    if minor > 99 or patch > 99:
        raise ValueError(f"{version}: MINOR and PATCH must be 99 or less")
    return major * 10000 + minor * 100 + patch


def set_version(text: str, version: str) -> tuple[str, int]:
    """Gives the presets text with the new version, and the number of changed lines."""
    code = version_code(version)
    changed = 0

    def replace(match: re.Match) -> str:
        nonlocal changed
        changed += 1
        return match.group(1) + KEYS[match.group(2)](version, code)

    keys = "|".join(re.escape(key) for key in KEYS)
    text = re.sub(rf"^(\s*({keys})\s*=\s*).*$", replace, text, flags=re.MULTILINE)
    return text, changed


def main() -> None:
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    version = sys.argv[1].removeprefix("v")
    path = sys.argv[2] if len(sys.argv) == 3 else PRESETS
    with open(path, encoding="utf-8") as f:
        text, changed = set_version(f.read(), version)
    if changed == 0:
        sys.exit(f"{path} has no version key")
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"set {changed} version values to {version}")


if __name__ == "__main__":
    main()
