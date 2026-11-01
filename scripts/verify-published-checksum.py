import json
import os
from pathlib import Path
from urllib.request import urlopen

cask = json.loads((Path(os.environ["RUNNER_TEMP"]) / "kiseki-cask.json").read_text())[
    "casks"
][0]
url = cask["url"].rsplit("/", 1)[0] + "/SHA256SUMS"
with urlopen(url, timeout=30) as response:
    entries = [line.split() for line in response.read().decode().splitlines()]
hashes = [parts[0] for parts in entries if len(parts) == 2 and parts[1] == "Kiseki.dmg"]
assert hashes == [cask["sha256"]], "Cask checksum differs from published SHA256SUMS"
