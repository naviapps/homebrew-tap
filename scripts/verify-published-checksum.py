import argparse
import hashlib
import json
import os
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import urlopen

RELEASES = "https://github.com/naviapps/kiseki/releases/download"
DMG = "Kiseki.dmg"
APPCAST = "appcast.xml"
SPARKLE = "{http://www.andymatuschak.org/xml-namespaces/sparkle}"
MAXIMUM_MANIFEST_BYTES = 64 * 1024
MAXIMUM_APPCAST_BYTES = 1024 * 1024


def download_asset(release, name, maximum_bytes):
    with urlopen(f"{release}/{name}", timeout=30) as response:
        if urlsplit(response.geturl()).scheme != "https":
            raise ValueError(f"Published {name} must be transferred over HTTPS")
        contents = response.read(maximum_bytes + 1)
    if len(contents) > maximum_bytes:
        raise ValueError(f"Published {name} exceeds the size limit")
    return contents


def manifest_digests(contents, names):
    hashes = {name: [] for name in names}
    for line in contents.decode().splitlines():
        fields = line.split(maxsplit=1)
        if not fields:
            continue
        if len(fields) != 2 or not re.fullmatch(r"[0-9a-fA-F]{64}", fields[0]):
            raise ValueError("Malformed published SHA256SUMS entry")
        digest, filename = fields
        filename = filename.removeprefix("*")
        if filename in hashes:
            hashes[filename].append(digest.lower())
    for name, entries in hashes.items():
        if len(entries) != 1:
            raise ValueError(
                f"Published SHA256SUMS must contain exactly one {name} entry"
            )
    return {name: entries[0] for name, entries in hashes.items()}


def appcast_dmg_length(contents, version, url):
    text = contents.decode("utf-8")
    if "<!DOCTYPE" in text:
        raise ValueError(
            "Published appcast.xml must not contain a document type declaration"
        )
    try:
        feed = ET.fromstring(text)
    except ET.ParseError as error:
        raise ValueError("Malformed published appcast.xml") from error
    channels = feed.findall("channel")
    if feed.tag != "rss" or len(channels) != 1:
        raise ValueError("Published appcast.xml must contain one RSS channel")
    items = channels[0].findall("item")
    if len(items) != 1:
        raise ValueError("Published appcast.xml must contain one release item")
    item = items[0]
    versions = item.findall(SPARKLE + "shortVersionString")
    if [entry.text for entry in versions] != [version]:
        raise ValueError(f"Appcast marketing version must match cask version {version}")
    builds = item.findall(SPARKLE + "version")
    if len(builds) != 1 or not re.fullmatch(r"[1-9][0-9]*", builds[0].text or ""):
        raise ValueError("Appcast must contain one numeric Sparkle build number")
    enclosures = item.findall("enclosure")
    if len(enclosures) != 1 or enclosures[0].get("url") != url:
        raise ValueError(
            "Appcast must reference Kiseki.dmg on its exact GitHub release tag"
        )
    enclosure = enclosures[0]
    if not enclosure.get(SPARKLE + "edSignature", "").strip():
        raise ValueError("Appcast must include Sparkle update signature metadata")
    length = enclosure.get("length", "")
    if not re.fullmatch(r"[1-9][0-9]*", length):
        raise ValueError("Appcast must declare a positive disk-image length")
    return int(length)


def verify(cask_info, dmg=None, *, check_appcast=False):
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
    url = f"{release}/{DMG}"
    if cask.get("url") != url:
        raise ValueError(
            "Cask URL must point to Kiseki.dmg on its exact GitHub release tag"
        )

    expected = cask.get("sha256")
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError("Cask must declare a lowercase SHA-256 digest")

    manifest = download_asset(release, "SHA256SUMS", MAXIMUM_MANIFEST_BYTES)
    digests = manifest_digests(manifest, (DMG, APPCAST) if check_appcast else (DMG,))
    if digests[DMG] != expected:
        raise ValueError("Cask checksum differs from published SHA256SUMS")

    length = None
    if check_appcast:
        appcast = download_asset(release, APPCAST, MAXIMUM_APPCAST_BYTES)
        if hashlib.sha256(appcast).hexdigest() != digests[APPCAST]:
            raise ValueError("Published appcast.xml checksum differs from SHA256SUMS")
        length = appcast_dmg_length(appcast, version, url)

    if dmg is not None:
        with dmg.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
            size = os.fstat(stream.fileno()).st_size
        if actual != expected:
            raise ValueError("Local DMG checksum differs from published SHA256SUMS")
        if length is not None and size != length:
            raise ValueError(
                "Local DMG length differs from the published appcast enclosure"
            )
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
    parser.add_argument(
        "--verify-appcast",
        action="store_true",
        help="Also verify the same release's Sparkle appcast metadata",
    )
    args = parser.parse_args()
    if args.cask_info is None:
        parser.error("pass --cask-info or set RUNNER_TEMP")
    try:
        version, digest = verify(
            json.loads(args.cask_info.read_text()),
            args.dmg,
            check_appcast=args.verify_appcast,
        )
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")
    artifact = " and local DMG" if args.dmg is not None else ""
    print(
        f"Verified Kiseki {version} cask{artifact} against published SHA256SUMS ({digest})."
    )
    if args.verify_appcast:
        print(f"Verified published {APPCAST} checksum and release metadata.")


if __name__ == "__main__":
    main()
