import hashlib
import importlib.util
import io
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

REPOSITORY = Path(__file__).resolve().parents[1]
SPECIFICATION = importlib.util.spec_from_file_location(
    "verify_published_checksum", REPOSITORY / "scripts/verify-published-checksum.py"
)
verifier = importlib.util.module_from_spec(SPECIFICATION)
SPECIFICATION.loader.exec_module(verifier)

ARTIFACT = b"owned DMG fixture"
DIGEST = hashlib.sha256(ARTIFACT).hexdigest()
RELEASE = "https://github.com/naviapps/kiseki/releases/download/v1.2.3"
DMG_ENTRY = f"{DIGEST}  Kiseki.dmg\n".encode()


class Response(io.BytesIO):
    def geturl(self):
        return "https://release-assets.githubusercontent.com/SHA256SUMS"


class PublishedChecksumTests(unittest.TestCase):
    def cask(self, **changes):
        return {
            "full_token": "naviapps/tap/kiseki",
            "version": "1.2.3",
            "sha256": DIGEST,
            "url": f"{RELEASE}/Kiseki.dmg",
        } | changes

    def test_rejects_unbound_sources_before_requesting_the_manifest(self):
        for cask, error in (
            (
                self.cask(full_token="different/tap/kiseki"),
                "Expected the naviapps/tap/kiseki",
            ),
            (self.cask(version="1.2.3-beta.1"), "stable Kiseki version"),
            (self.cask(version="1.2.4"), "exact GitHub release tag"),
            (
                self.cask(url=RELEASE.replace("naviapps/", "example/") + "/Kiseki.dmg"),
                "exact GitHub release tag",
            ),
            (
                self.cask(
                    url=RELEASE.replace("download/v1.2.3", "latest/download")
                    + "/Kiseki.dmg"
                ),
                "exact GitHub release tag",
            ),
            (self.cask(sha256="no_check"), "SHA-256 digest"),
        ):
            with self.subTest(cask=cask), patch.object(verifier, "urlopen") as request:
                with self.assertRaisesRegex(ValueError, error):
                    verifier.verify({"casks": [cask]})
                request.assert_not_called()

    def test_rejects_ambiguous_homebrew_metadata_before_network(self):
        with patch.object(verifier, "urlopen") as request:
            for info in (
                {},
                {"casks": []},
                {"casks": [self.cask()] * 2},
                {"casks": ["kiseki"]},
            ):
                with (
                    self.subTest(info=info),
                    self.assertRaisesRegex(ValueError, "one Homebrew cask"),
                ):
                    verifier.verify(info)
            request.assert_not_called()

    def test_checks_the_exact_release_manifest(self):
        manifest = DMG_ENTRY + ("b" * 64 + "  appcast.xml\n").encode()
        with patch.object(
            verifier, "urlopen", return_value=Response(manifest)
        ) as request:
            self.assertEqual(
                verifier.verify({"casks": [self.cask()]}), ("1.2.3", DIGEST)
            )
            request.assert_called_once_with(f"{RELEASE}/SHA256SUMS", timeout=30)

    def test_rejects_missing_duplicate_malformed_or_mismatching_entries(self):
        for manifest, error in (
            (
                DMG_ENTRY.replace(b"Kiseki.dmg", b"appcast.xml"),
                "exactly one Kiseki.dmg entry",
            ),
            (DMG_ENTRY * 2, "exactly one Kiseki.dmg entry"),
            (DMG_ENTRY + b"invalid Kiseki.dmg\n", "Malformed published SHA256SUMS"),
            (b"a" * 64 + b"  Kiseki.dmg\n", "Cask checksum differs"),
        ):
            with (
                self.subTest(manifest=manifest),
                patch.object(verifier, "urlopen", return_value=Response(manifest)),
                self.assertRaisesRegex(ValueError, error),
            ):
                verifier.verify({"casks": [self.cask()]})

    def test_bounds_the_published_manifest(self):
        with (
            patch.object(
                verifier,
                "urlopen",
                return_value=Response(b" " * (verifier.MAXIMUM_MANIFEST_BYTES + 1)),
            ),
            self.assertRaisesRegex(ValueError, "exceeds the size limit"),
        ):
            verifier.verify({"casks": [self.cask()]})

    def test_rejects_a_final_response_that_downgrades_https(self):
        response = Response(DMG_ENTRY)
        with (
            patch.object(
                response, "geturl", return_value="http://example.test/SHA256SUMS"
            ),
            patch.object(verifier, "urlopen", return_value=response),
            self.assertRaisesRegex(ValueError, "must be transferred over HTTPS"),
        ):
            verifier.verify({"casks": [self.cask()]})

    def test_checks_the_local_dmg_without_installing_it(self):
        with tempfile.TemporaryDirectory() as directory:
            dmg = Path(directory) / "Kiseki.dmg"
            dmg.write_bytes(ARTIFACT)
            with patch.object(verifier, "urlopen", return_value=Response(DMG_ENTRY)):
                self.assertEqual(
                    verifier.verify({"casks": [self.cask()]}, dmg), ("1.2.3", DIGEST)
                )
            dmg.write_bytes(b"different bytes")
            with (
                patch.object(verifier, "urlopen", return_value=Response(DMG_ENTRY)),
                self.assertRaisesRegex(ValueError, "Local DMG checksum differs"),
            ):
                verifier.verify({"casks": [self.cask()]}, dmg)
