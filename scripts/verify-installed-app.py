import json
import os
import plistlib
import re
import subprocess
from pathlib import Path

UPDATE_FEED = "https://github.com/naviapps/kiseki/releases/latest/download/appcast.xml"


def macos_version(value):
    if not isinstance(value, str) or not re.fullmatch(
        r"[0-9]+(?:\.[0-9]+){0,2}", value
    ):
        raise ValueError(
            "Expected a macOS version with one to three numeric components"
        )
    parts = [int(part) for part in value.split(".")]
    return parts + [0] * (3 - len(parts))


def verify(root):
    metadata = json.loads((root / "kiseki-cask.json").read_text())
    casks = metadata.get("casks") if isinstance(metadata, dict) else None
    if not isinstance(casks, list) or len(casks) != 1 or not isinstance(casks[0], dict):
        raise ValueError("Expected one Homebrew cask in brew info JSON")
    cask = casks[0]
    if cask.get("full_token") != "naviapps/tap/kiseki":
        raise ValueError("Expected the naviapps/tap/kiseki cask")
    version = cask.get("version")
    if not isinstance(version, str) or not version:
        raise ValueError("Cask version must be a nonempty string")
    try:
        minimum = cask["depends_on"]["macos"][">="]
    except (KeyError, TypeError) as error:
        raise ValueError("Expected one minimum macOS version in the cask") from error
    if not isinstance(minimum, list) or len(minimum) != 1:
        raise ValueError("Expected one minimum macOS version in the cask")

    contents = root / "kiseki-apps/Kiseki.app/Contents"
    with (contents / "Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    if not isinstance(info, dict):
        raise TypeError("App Info.plist must be a dictionary")
    if info.get("CFBundleIdentifier") != "com.naviapps.Kiseki":
        raise ValueError("Installed bundle identifier must be com.naviapps.Kiseki")
    if info.get("CFBundleShortVersionString") != version:
        raise ValueError(f"Installed bundle version must match cask version {version}")
    if info.get("SUFeedURL") != UPDATE_FEED:
        raise ValueError(
            f"Installed SUFeedURL must match the public update feed {UPDATE_FEED}"
        )
    if macos_version(info.get("LSMinimumSystemVersion")) != macos_version(minimum[0]):
        raise ValueError(
            f"Installed bundle must match the cask's macOS {minimum[0]} minimum"
        )
    executable = info.get("CFBundleExecutable")
    if executable != "Kiseki":
        raise ValueError("Installed CFBundleExecutable must be Kiseki")

    binary = contents / "MacOS" / executable
    if not binary.is_file():
        raise ValueError("Installed bundle executable is missing")
    architectures = subprocess.check_output(
        ["/usr/bin/lipo", "-archs", str(binary)], text=True
    ).split()
    if not {"arm64", "x86_64"}.issubset(architectures):
        raise ValueError(
            "Installed Kiseki executable must contain arm64 and x86_64 slices"
        )
    return version


def main():
    runner_temp = os.environ.get("RUNNER_TEMP")
    if not runner_temp:
        raise SystemExit(
            "RUNNER_TEMP must hold the cask metadata and test app directory"
        )
    try:
        version = verify(Path(runner_temp))
    except (OSError, TypeError, ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"Installed bundle verification failed: {error}") from error
    print(f"Verified Kiseki {version} bundle metadata and executable architectures.")


if __name__ == "__main__":
    main()
