import hashlib
import importlib.util
import io
import plistlib
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import call, patch

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
ENCLOSURE = (
    f'<enclosure url="{RELEASE}/Kiseki.dmg" length="{len(ARTIFACT)}" '
    'sparkle:edSignature="fixture"/>'
)
APPCAST = f"""<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <item>
      <sparkle:version>42</sparkle:version>
      <sparkle:shortVersionString>1.2.3</sparkle:shortVersionString>
      {ENCLOSURE}
    </item>
  </channel>
</rss>""".encode()


def appcast_entry(appcast):
    return f"{hashlib.sha256(appcast).hexdigest()}  appcast.xml\n".encode()


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

    def test_fetches_the_exact_published_appcast(self):
        responses = [Response(DMG_ENTRY + appcast_entry(APPCAST)), Response(APPCAST)]
        with patch.object(verifier, "urlopen", side_effect=responses) as request:
            self.assertEqual(
                verifier.verify({"casks": [self.cask()]}, check_appcast=True),
                ("1.2.3", DIGEST),
            )
            self.assertEqual(
                request.call_args_list,
                [
                    call(f"{RELEASE}/SHA256SUMS", timeout=30),
                    call(f"{RELEASE}/appcast.xml", timeout=30),
                ],
            )

    def test_requires_the_appcast_checksum_before_fetching_its_bytes(self):
        for entries in (b"", appcast_entry(APPCAST) * 2):
            with (
                self.subTest(entries=entries),
                patch.object(
                    verifier, "urlopen", return_value=Response(DMG_ENTRY + entries)
                ) as request,
                self.assertRaisesRegex(ValueError, r"exactly one appcast\.xml entry"),
            ):
                verifier.verify({"casks": [self.cask()]}, check_appcast=True)
            request.assert_called_once_with(f"{RELEASE}/SHA256SUMS", timeout=30)

    def test_rejects_appcast_bytes_that_do_not_match_the_manifest(self):
        responses = [
            Response(DMG_ENTRY + appcast_entry(APPCAST)),
            Response(b"changed bytes"),
        ]
        with (
            patch.object(verifier, "urlopen", side_effect=responses),
            self.assertRaisesRegex(ValueError, r"appcast\.xml checksum differs"),
        ):
            verifier.verify({"casks": [self.cask()]}, check_appcast=True)

    def test_rejects_inconsistent_appcast_metadata_even_with_matching_digests(self):
        for appcast, error in (
            (b"<rss", r"Malformed published appcast\.xml"),
            (b"<!DOCTYPE rss>" + APPCAST, "document type declaration"),
            (APPCAST.replace(b"</item>", b"</item><item/>"), "one release item"),
            (APPCAST.replace(b">1.2.3<", b">1.2.4<"), "marketing version"),
            (
                APPCAST.replace(
                    b"http://www.andymatuschak.org", b"https://invalid.test"
                ),
                "marketing version",
            ),
            (APPCAST.replace(b">42<", b">build<"), "numeric Sparkle build number"),
            (APPCAST.replace(ENCLOSURE.encode(), b""), "exact GitHub release tag"),
            (
                APPCAST.replace(ENCLOSURE.encode(), ENCLOSURE.encode() * 2),
                "exact GitHub release tag",
            ),
            (
                APPCAST.replace(b"download/v1.2.3/", b"download/v1.2.4/"),
                "exact GitHub release tag",
            ),
            (
                APPCAST.replace(b' sparkle:edSignature="fixture"', b""),
                "signature metadata",
            ),
            (
                APPCAST.replace(f'length="{len(ARTIFACT)}"'.encode(), b'length="0"'),
                "positive disk-image length",
            ),
        ):
            responses = [
                Response(DMG_ENTRY + appcast_entry(appcast)),
                Response(appcast),
            ]
            with (
                self.subTest(error=error, appcast=appcast),
                patch.object(verifier, "urlopen", side_effect=responses),
                self.assertRaisesRegex(ValueError, error),
            ):
                verifier.verify({"casks": [self.cask()]}, check_appcast=True)

    def test_bounds_the_published_appcast(self):
        appcast = b" " * (verifier.MAXIMUM_APPCAST_BYTES + 1)
        responses = [Response(DMG_ENTRY + appcast_entry(appcast)), Response(appcast)]
        with (
            patch.object(verifier, "urlopen", side_effect=responses),
            self.assertRaisesRegex(ValueError, r"appcast\.xml exceeds the size limit"),
        ):
            verifier.verify({"casks": [self.cask()]}, check_appcast=True)

    def test_matches_a_local_dmg_length_against_the_enclosure_when_requested(self):
        with tempfile.TemporaryDirectory() as directory:
            dmg = Path(directory) / "Kiseki.dmg"
            dmg.write_bytes(ARTIFACT)
            valid = [Response(DMG_ENTRY + appcast_entry(APPCAST)), Response(APPCAST)]
            with patch.object(verifier, "urlopen", side_effect=valid):
                self.assertEqual(
                    verifier.verify({"casks": [self.cask()]}, dmg, check_appcast=True),
                    ("1.2.3", DIGEST),
                )
            wrong_length = APPCAST.replace(
                f'length="{len(ARTIFACT)}"'.encode(), b'length="1"'
            )
            responses = [
                Response(DMG_ENTRY + appcast_entry(wrong_length)),
                Response(wrong_length),
            ]
            with (
                patch.object(verifier, "urlopen", side_effect=responses),
                self.assertRaisesRegex(ValueError, "Local DMG length differs"),
            ):
                verifier.verify({"casks": [self.cask()]}, dmg, check_appcast=True)

    def test_passes_the_verified_enclosure_and_bundle_key_to_crypto_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dmg = root / "Kiseki.dmg"
            dmg.write_bytes(ARTIFACT)
            bundle = root / "Kiseki.app"
            (bundle / "Contents").mkdir(parents=True)
            (bundle / "Contents/Info.plist").write_bytes(
                plistlib.dumps({"SUPublicEDKey": "fixture-key"})
            )
            responses = [
                Response(DMG_ENTRY + appcast_entry(APPCAST)),
                Response(APPCAST),
            ]
            with (
                patch.object(verifier, "urlopen", side_effect=responses),
                patch.object(verifier.subprocess, "run") as crypto,
            ):
                self.assertEqual(
                    verifier.verify(
                        {"casks": [self.cask()]}, dmg, sparkle_bundle=bundle
                    ),
                    ("1.2.3", DIGEST),
                )
            crypto.assert_called_once_with(
                [
                    "/usr/bin/xcrun",
                    "swift",
                    "-warnings-as-errors",
                    str(REPOSITORY / "scripts/verify-sparkle-signature.swift"),
                    str(dmg),
                    DIGEST,
                    "fixture-key",
                    "fixture",
                ],
                check=True,
            )

    def test_requires_the_signed_bundle_key_and_native_verification_success(self):
        with tempfile.TemporaryDirectory() as directory:
            bundle = Path(directory) / "Kiseki.app"
            (bundle / "Contents").mkdir(parents=True)
            plist = bundle / "Contents/Info.plist"
            for metadata in ({}, {"SUPublicEDKey": 1}, {"SUPublicEDKey": " "}):
                plist.write_bytes(plistlib.dumps(metadata))
                with (
                    self.subTest(metadata=metadata),
                    patch.object(verifier.subprocess, "run") as crypto,
                    self.assertRaisesRegex(ValueError, "provide a Sparkle public key"),
                ):
                    verifier.verify_sparkle_signature(
                        "Kiseki.dmg", DIGEST, "fixture", bundle
                    )
                crypto.assert_not_called()
            plist.write_bytes(plistlib.dumps({"SUPublicEDKey": "fixture-key"}))
            with (
                patch.object(
                    verifier.subprocess,
                    "run",
                    side_effect=subprocess.CalledProcessError(1, "swift"),
                ),
                self.assertRaisesRegex(
                    ValueError, "Sparkle update signature verification failed"
                ),
            ):
                verifier.verify_sparkle_signature(
                    "Kiseki.dmg", DIGEST, "fixture", bundle
                )

    def test_requires_a_dmg_before_requesting_signature_metadata(self):
        with (
            patch.object(verifier, "urlopen") as request,
            self.assertRaisesRegex(ValueError, "requires a local DMG"),
        ):
            verifier.verify({"casks": [self.cask()]}, sparkle_bundle=Path("Kiseki.app"))
        request.assert_not_called()
