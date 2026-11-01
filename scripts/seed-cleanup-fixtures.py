import json
import os
from pathlib import Path

assert os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted", (
    "Requires a disposable runner"
)
root = Path(os.environ["RUNNER_TEMP"])
cask = json.loads((root / "kiseki-cask.json").read_text())["casks"][0]
trash = next(
    artifact["zap"][0]["trash"] for artifact in cask["artifacts"] if "zap" in artifact
)
remove = [Path(path).expanduser() for path in trash]
keep = [path.with_name(path.name + ".QA") for path in remove]
keep.append(root / "exported-gestures.kiseki")
for path in remove:
    assert path.is_relative_to(Path.home() / "Library")
    assert path.name in (
        "com.naviapps.Kiseki",
        "com.naviapps.Kiseki.plist",
        "com.naviapps.Kiseki.binarycookies",
        "com.naviapps.Kiseki.savedState",
    )
for path in remove + keep:
    assert not path.exists() and not path.is_symlink(), f"Refusing to overwrite {path}"
data = [
    path
    if path.suffix in (".plist", ".binarycookies", ".kiseki")
    else path / "nested/fixture"
    for path in remove + keep
]
for path in data:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("Homebrew cleanup fixture\n")
(root / "kiseki-cleanup.json").write_text(
    json.dumps(
        {
            "remove": list(map(str, remove)),
            "keep": list(map(str, data[len(remove) :])),
            "data": list(map(str, data)),
        }
    )
)
