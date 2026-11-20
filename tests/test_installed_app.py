import importlib.util
import json
import os
import plistlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

VERIFIER = Path(__file__).resolve().parents[1] / "scripts/verify-installed-app.py"
SPECIFICATION = importlib.util.spec_from_file_location("verify_installed_app", VERIFIER)
installer = importlib.util.module_from_spec(SPECIFICATION)
SPECIFICATION.loader.exec_module(installer)


class InstalledAppTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.contents = self.root / "kiseki-apps/Kiseki.app/Contents"
        self.binary = self.contents / "MacOS/Kiseki"
        self.binary.parent.mkdir(parents=True)
        self.binary.touch()
        self.cask = {
            "full_token": "naviapps/tap/kiseki",
            "version": "1.2.3",
            "depends_on": {"macos": {">=": ["26"]}},
        }
        self.plist = {
            "CFBundleIdentifier": "com.naviapps.Kiseki",
            "CFBundleShortVersionString": "1.2.3",
            "CFBundleExecutable": "Kiseki",
            "LSMinimumSystemVersion": "26.0",
            "SUFeedURL": "https://github.com/naviapps/kiseki/releases/latest/download/appcast.xml",
        }
        self.write_metadata({"casks": [self.cask]})
        self.write_plist()

    def write_metadata(self, value):
        (self.root / "kiseki-cask.json").write_text(json.dumps(value))

    def write_plist(self, **changes):
        (self.contents / "Info.plist").write_bytes(plistlib.dumps(self.plist | changes))

    def test_compares_the_minimum_and_checks_the_declared_executable(self):
        for minimum in ("26", "26.0", "26.0.0"):
            with (
                self.subTest(minimum=minimum),
                patch.object(
                    installer.subprocess, "check_output", return_value="x86_64 arm64\n"
                ) as lipo,
            ):
                self.write_plist(LSMinimumSystemVersion=minimum)
                self.assertEqual(installer.verify(self.root), "1.2.3")
                lipo.assert_called_once_with(
                    ["/usr/bin/lipo", "-archs", str(self.binary)], text=True
                )

    def test_rejects_wrong_bundle_identity_version_or_executable_before_lipo(self):
        for change in (
            {"CFBundleIdentifier": "com.naviapps.Kiseki.QA"},
            {"CFBundleShortVersionString": "1.2.4"},
            {"CFBundleExecutable": "Other"},
            {"CFBundleExecutable": "../Kiseki"},
        ):
            with (
                self.subTest(change=change),
                patch.object(installer.subprocess, "check_output") as lipo,
                self.assertRaisesRegex(
                    ValueError, "Installed (bundle|CFBundleExecutable)"
                ),
            ):
                self.write_plist(**change)
                installer.verify(self.root)
            lipo.assert_not_called()

    def test_rejects_incompatible_or_non_numeric_macos_requirements(self):
        for minimum, error in (
            ("26.1", "match the cask's macOS"),
            ("27", "match the cask's macOS"),
            ("+26", "numeric components"),
            ("26.0.0.0", "numeric components"),
            (26, "numeric components"),
        ):
            with (
                self.subTest(minimum=minimum),
                self.assertRaisesRegex(ValueError, error),
            ):
                self.write_plist(LSMinimumSystemVersion=minimum)
                installer.verify(self.root)

    def test_requires_the_public_update_feed_before_lipo(self):
        plist = self.contents / "Info.plist"
        metadata = self.plist.copy()
        del metadata["SUFeedURL"]
        for feed in (
            {},
            {"SUFeedURL": False},
            {"SUFeedURL": "https://downloads.kisekiapp.com/appcast.xml"},
            {
                "SUFeedURL": "https://github.com/naviapps/kiseki/releases/download/v1.2.3/appcast.xml"
            },
        ):
            with (
                self.subTest(feed=feed),
                patch.object(installer.subprocess, "check_output") as lipo,
                self.assertRaisesRegex(
                    ValueError, "Installed SUFeedURL must match the public update feed"
                ),
            ):
                plist.write_bytes(plistlib.dumps(metadata | feed))
                installer.verify(self.root)
            lipo.assert_not_called()

    def test_requires_single_cask_and_single_minimum(self):
        for metadata, error in (
            ({"casks": [self.cask, self.cask]}, "one Homebrew cask"),
            (
                {"casks": [self.cask | {"full_token": "example/tap/kiseki"}]},
                "naviapps/tap/kiseki",
            ),
            ({"casks": [self.cask | {"depends_on": {}}]}, "one minimum macOS version"),
            (
                {
                    "casks": [
                        self.cask | {"depends_on": {"macos": {">=": ["26", "27"]}}}
                    ]
                },
                "one minimum macOS version",
            ),
        ):
            with (
                self.subTest(metadata=metadata),
                self.assertRaisesRegex(ValueError, error),
            ):
                self.write_metadata(metadata)
                installer.verify(self.root)

    def test_rejects_missing_executable_and_missing_slices(self):
        for architectures in ("arm64", "x86_64", "arm64e x86_64"):
            with (
                self.subTest(architectures=architectures),
                patch.object(
                    installer.subprocess, "check_output", return_value=architectures
                ),
                self.assertRaisesRegex(
                    ValueError, "must contain arm64 and x86_64 slices"
                ),
            ):
                installer.verify(self.root)
        self.binary.unlink()
        with self.assertRaisesRegex(ValueError, "bundle executable is missing"):
            installer.verify(self.root)

    def test_rejects_a_plist_without_a_metadata_dictionary(self):
        (self.contents / "Info.plist").write_bytes(plistlib.dumps(["Kiseki"]))
        with self.assertRaisesRegex(TypeError, "Info.plist must be a dictionary"):
            installer.verify(self.root)

    def test_cli_rejects_wrong_bundle_metadata_even_with_optimization(self):
        self.write_plist(CFBundleIdentifier="com.naviapps.Kiseki.QA")
        result = subprocess.run(
            [sys.executable, "-B", "-O", str(VERIFIER)],
            env=dict(os.environ, RUNNER_TEMP=str(self.root)),
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "Installed bundle identifier must be com.naviapps.Kiseki", result.stderr
        )
