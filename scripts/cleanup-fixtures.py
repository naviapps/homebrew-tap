import argparse
import json
import os
from pathlib import Path

FIXTURE_CONTENTS = "Homebrew cleanup fixture\n"
PRODUCTION_PATHS = {
    "Application Support/com.naviapps.Kiseki",
    "Caches/com.naviapps.Kiseki",
    "HTTPStorages/com.naviapps.Kiseki",
    "HTTPStorages/com.naviapps.Kiseki.binarycookies",
    "Preferences/com.naviapps.Kiseki.plist",
    "Saved Application State/com.naviapps.Kiseki.savedState",
}


def fixture_file(path):
    if path.suffix in {".plist", ".binarycookies", ".kiseki"}:
        return path
    return path / "nested/fixture"


def present(path):
    return path.exists() or path.is_symlink()


def cleanup_paths(cask, home, root):
    zaps = [artifact["zap"] for artifact in cask["artifacts"] if "zap" in artifact]
    if len(zaps) != 1 or len(zaps[0]) != 1:
        raise SystemExit("Expected one explicit cask cleanup list")
    trash = zaps[0][0]["trash"]
    expected = {home / "Library" / path for path in PRODUCTION_PATHS}
    if not isinstance(trash, list) or not all(isinstance(path, str) for path in trash):
        raise SystemExit("Cask cleanup paths must be an explicit list")
    remove = [
        home / path[2:] if path.startswith("~/") else Path(path) for path in trash
    ]
    if len(remove) != len(expected) or set(remove) != expected:
        raise SystemExit("Cask cleanup paths differ from the reviewed production paths")
    keep = [path.with_name(path.name + ".QA") for path in remove]
    keep.append(root / "exported-gestures.kiseki")
    return remove, keep


def run(stage, home, root, runner_environment):
    if stage == "seed" and runner_environment != "github-hosted":
        raise SystemExit("Requires a disposable runner")
    if stage not in {"seed", "verify-uninstall", "verify-zap"}:
        raise ValueError("Unknown cleanup verification stage")
    casks = json.loads((root / "kiseki-cask.json").read_text())["casks"]
    if len(casks) != 1:
        raise SystemExit("Expected one cask")
    remove, keep = cleanup_paths(casks[0], home, root)

    if stage == "seed":
        for path in remove + keep:
            if path.resolve() != path:
                raise SystemExit(f"Unexpected cask cleanup path: {path}")
            if present(path):
                raise SystemExit(f"Refusing to overwrite {path}")
        for path in map(fixture_file, remove + keep):
            path.parent.mkdir(parents=True, exist_ok=True)
            with path.open("x") as stream:
                stream.write(FIXTURE_CONTENTS)
        return

    if stage == "verify-zap":
        for path in remove:
            if present(path):
                raise SystemExit(f"Zap left {path}")
        expected, action = keep, "Zap"
    else:
        expected, action = remove + keep, "Uninstall"
    for path in map(fixture_file, expected):
        if (
            path.resolve() != path
            or not path.is_file()
            or path.read_text() != FIXTURE_CONTENTS
        ):
            raise SystemExit(f"{action} changed {path}")


def main():
    parser = argparse.ArgumentParser(description="Check the cask cleanup lifecycle.")
    parser.add_argument("stage", choices=("seed", "verify-uninstall", "verify-zap"))
    run(
        parser.parse_args().stage,
        Path.home(),
        Path(os.environ["RUNNER_TEMP"]),
        os.environ.get("RUNNER_ENVIRONMENT"),
    )


if __name__ == "__main__":
    main()
