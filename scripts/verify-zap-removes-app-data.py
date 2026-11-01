import json
import os
from pathlib import Path

fixtures = json.loads(
    (Path(os.environ["RUNNER_TEMP"]) / "kiseki-cleanup.json").read_text()
)
for path in fixtures["remove"]:
    assert not Path(path).exists(), f"Zap left {path}"
for path in fixtures["keep"]:
    assert Path(path).read_text() == "Homebrew cleanup fixture\n", f"Zap changed {path}"
