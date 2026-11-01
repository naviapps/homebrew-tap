import json
import os
from pathlib import Path

fixtures = json.loads(
    (Path(os.environ["RUNNER_TEMP"]) / "kiseki-cleanup.json").read_text()
)
for path in fixtures["data"]:
    assert Path(path).read_text() == "Homebrew cleanup fixture\n", (
        f"Uninstall changed {path}"
    )
