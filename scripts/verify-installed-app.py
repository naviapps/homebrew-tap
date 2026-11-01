import json
import os
import plistlib
import subprocess
from pathlib import Path

root = Path(os.environ["RUNNER_TEMP"])
cask = json.loads((root / "kiseki-cask.json").read_text())["casks"][0]
with (root / "kiseki-apps/Kiseki.app/Contents/Info.plist").open("rb") as stream:
    info = plistlib.load(stream)
assert info["CFBundleIdentifier"] == "com.naviapps.Kiseki"
assert info["CFBundleShortVersionString"] == cask["version"]
minimum = list(map(int, info["LSMinimumSystemVersion"].split(".")))
declared = list(map(int, cask["depends_on"]["macos"][">="][0].split(".")))
assert minimum + [0] * (3 - len(minimum)) == declared + [0] * (3 - len(declared))
binary = root / "kiseki-apps/Kiseki.app/Contents/MacOS/Kiseki"
architectures = subprocess.check_output(
    ["lipo", "-archs", str(binary)], text=True
).split()
assert {"arm64", "x86_64"}.issubset(architectures), architectures
