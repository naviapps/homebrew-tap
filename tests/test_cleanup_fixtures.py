import importlib.util
import json
import re
import shutil
import tempfile
import unittest
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[1]
SPECIFICATION = importlib.util.spec_from_file_location(
    "cleanup_fixtures", REPOSITORY / "scripts/cleanup-fixtures.py"
)
cleanup = importlib.util.module_from_spec(SPECIFICATION)
SPECIFICATION.loader.exec_module(cleanup)


class CleanupFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        self.home = self.directory / "home"
        self.root = self.directory / "runner"
        self.root.mkdir()
        cask = (REPOSITORY / "Casks/kiseki.rb").read_text()
        self.trash = re.findall(
            r'^\s*"([^"]+)"', cask.split("zap trash:", 1)[1], re.MULTILINE
        )
        self.write_cask(self.trash)

    def write_cask(self, trash):
        value = {"casks": [{"artifacts": [{"zap": [{"trash": trash}]}]}]}
        (self.root / "kiseki-cask.json").write_text(json.dumps(value))

    def run_stage(self, stage, environment="github-hosted"):
        cleanup.run(stage, self.home, self.root, environment)

    def paths(self):
        return [self.home / path[2:] for path in self.trash]

    def assert_not_seeded(self):
        self.assertFalse((self.root / "exported-gestures.kiseki").exists())
        self.assertFalse(any(self.home.rglob("fixture")))

    def test_seed_requires_a_disposable_host_before_creating_anything(self):
        for environment in (None, "self-hosted"):
            with (
                self.subTest(environment=environment),
                self.assertRaisesRegex(SystemExit, "disposable runner"),
            ):
                self.run_stage("seed", environment)
        self.assertFalse(self.home.exists())
        self.assert_not_seeded()

    def test_cask_paths_must_match_the_exact_reviewed_locations(self):
        changes = (
            [],
            self.trash[:-1],
            [*self.trash[:-1], self.trash[0]],
            [
                self.trash[0].replace("Application Support", "Unrelated"),
                *self.trash[1:],
            ],
            [self.trash[0].replace("Application Support", "*"), *self.trash[1:]],
            [self.trash[0].replace("Library/", "Library/../Library/"), *self.trash[1:]],
        )
        for trash in changes:
            with (
                self.subTest(trash=trash),
                self.assertRaisesRegex(SystemExit, "reviewed production paths"),
            ):
                self.write_cask(trash)
                self.run_stage("seed")
            self.assert_not_seeded()

    def test_expanded_homebrew_paths_still_use_the_same_reviewed_locations(self):
        self.write_cask([str(path) for path in self.paths()])
        self.run_stage("seed")
        self.run_stage("verify-uninstall")

    def test_an_existing_path_prevents_all_fixture_writes(self):
        existing = self.paths()[-1]
        existing.mkdir(parents=True)
        marker = existing / "user-data"
        marker.write_text("existing contents")
        with self.assertRaisesRegex(SystemExit, "Refusing to overwrite"):
            self.run_stage("seed")
        self.assertEqual(marker.read_text(), "existing contents")
        self.assert_not_seeded()

    def test_a_dangling_target_link_is_not_overwritten(self):
        existing = self.paths()[-1]
        existing.parent.mkdir(parents=True)
        existing.symlink_to(self.directory / "missing")
        with self.assertRaises(SystemExit):
            self.run_stage("seed")
        self.assertTrue(existing.is_symlink())
        self.assert_not_seeded()

    def test_a_parent_symlink_cannot_redirect_fixture_creation(self):
        outside = self.directory / "outside"
        outside.mkdir()
        parent = self.paths()[0].parent
        parent.parent.mkdir(parents=True)
        parent.symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(SystemExit, "Unexpected cask cleanup path"):
            self.run_stage("seed")
        self.assertEqual(list(outside.iterdir()), [])
        self.assert_not_seeded()

    def test_uninstall_must_retain_all_app_qa_and_export_fixtures(self):
        self.run_stage("seed")
        self.run_stage("verify-uninstall")
        cleanup.fixture_file(self.paths()[0]).unlink()
        with self.assertRaisesRegex(SystemExit, "Uninstall changed"):
            self.run_stage("verify-uninstall")

    def test_zap_must_remove_only_the_reviewed_app_paths(self):
        self.run_stage("seed")
        with self.assertRaisesRegex(SystemExit, "Zap left"):
            self.run_stage("verify-zap")
        for path in self.paths():
            if path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink()
        self.run_stage("verify-zap")
        (self.root / "exported-gestures.kiseki").write_text("modified")
        with self.assertRaisesRegex(SystemExit, "Zap changed"):
            self.run_stage("verify-zap")

    def test_verification_cannot_silently_redefine_the_seeded_path_set(self):
        self.run_stage("seed")
        self.write_cask([])
        for stage in ("verify-uninstall", "verify-zap"):
            with (
                self.subTest(stage=stage),
                self.assertRaisesRegex(SystemExit, "reviewed production paths"),
            ):
                self.run_stage(stage)

    def test_a_same_content_symlink_is_not_preserved_fixture_data(self):
        self.run_stage("seed")
        fixture = cleanup.fixture_file(self.paths()[0])
        replacement = self.directory / "replacement"
        replacement.write_text(cleanup.FIXTURE_CONTENTS)
        fixture.unlink()
        fixture.symlink_to(replacement)
        with self.assertRaisesRegex(SystemExit, "Uninstall changed"):
            self.run_stage("verify-uninstall")


if __name__ == "__main__":
    unittest.main()
