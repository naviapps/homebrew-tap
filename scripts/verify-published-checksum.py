import argparse
import hashlib
import json
import os
import re
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import urlopen

RELEASES = "https://github.com/naviapps/kiseki/releases/download"
DMG = "Kiseki.dmg"
MAXIMUM_MANIFEST_BYTES = 64 * 1024


def verify(cask_info, dmg=None):
    casks = cask_info.get("casks") if isinstance(cask_info, dict) else None
    if not isinstance(casks, list) or len(casks) != 1 or not isinstance(casks[0], dict):
        raise ValueError("Expected one Homebrew cask in brew info JSON")
    cask = casks[0]
    if cask.get("full_token") != "naviapps/tap/kiseki":
        raise ValueError("Expected the naviapps/tap/kiseki cask")

    version = cask.get("version")
    if not isinstance(version, str) or not re.fullmatch(
        r"(?:0|[1-9][0-9]*)(?:\.(?:0|[1-9][0-9]*)){2}", version
    ):
        raise ValueError("Expected a stable Kiseki version in X.Y.Z form")
    release = f"{RELEASES}/v{version}"
    if cask.get("url") != f"{release}/{DMG}":
        raise ValueError(
            "Cask URL must point to Kiseki.dmg on its exact GitHub release tag"
        )

    expected = cask.get("sha256")
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError("Cask must declare a lowercase SHA-256 digest")

    with urlopen(f"{release}/SHA256SUMS", timeout=30) as response:
        if urlsplit(response.geturl()).scheme != "https":
            raise ValueError("Published SHA256SUMS must be transferred over HTTPS")
        manifest = response.read(MAXIMUM_MANIFEST_BYTES + 1)
    if len(manifest) > MAXIMUM_MANIFEST_BYTES:
        raise ValueError("Published SHA256SUMS exceeds the size limit")

    hashes = []
    for line in manifest.decode().splitlines():
        fields = line.split(maxsplit=1)
        if not fields:
            continue
        if len(fields) != 2 or not re.fullmatch(r"[0-9a-fA-F]{64}", fields[0]):
            raise ValueError("Malformed published SHA256SUMS entry")
        digest, filename = fields
        if filename.removeprefix("*") == DMG:
            hashes.append(digest.lower())
    if len(hashes) != 1:
        raise ValueError(
            "Published SHA256SUMS must contain exactly one Kiseki.dmg entry"
        )
    if hashes[0] != expected:
        raise ValueError("Cask checksum differs from published SHA256SUMS")

    if dmg is not None:
        with dmg.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
        if actual != expected:
            raise ValueError("Local DMG checksum differs from published SHA256SUMS")
    return version, expected


def main():
    parser = argparse.ArgumentParser(
        description="Verify the cask against its exact public release."
    )
    runner_temp = os.environ.get("RUNNER_TEMP")
    parser.add_argument(
        "--cask-info",
        type=Path,
        default=Path(runner_temp) / "kiseki-cask.json" if runner_temp else None,
        help="JSON from brew info --json=v2 --cask naviapps/tap/kiseki",
    )
    parser.add_argument(
        "--dmg", type=Path, help="Also verify an already downloaded DMG"
    )
    args = parser.parse_args()
    if args.cask_info is None:
        parser.error("pass --cask-info or set RUNNER_TEMP")
    try:
        version, digest = verify(json.loads(args.cask_info.read_text()), args.dmg)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")
    artifact = " and local DMG" if args.dmg is not None else ""
    print(
        f"Verified Kiseki {version} cask{artifact} against published SHA256SUMS ({digest})."
    )


if __name__ == "__main__":
    main()
