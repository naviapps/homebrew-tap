import base64
import hashlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

VERIFIER = (
    Path(__file__).resolve().parents[1] / "scripts/verify-sparkle-signature.swift"
)

# RFC 8032 §7.1 test 2, with the public key from test 1 used for mismatch checks.
MESSAGE = bytes.fromhex("72")
PUBLIC_KEY = bytes.fromhex(
    "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
)
OTHER_KEY = bytes.fromhex(
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
)
SIGNATURE = bytes.fromhex(
    "92a009a9f0d4cab8720e820b5f642540"
    "a2b27b5416503f8fb3762223ebdb69da"
    "085ac1e43e15996e458f3613d0f11d8c"
    "387b2eaeb4302aeeb00d291612bb0c00"
)


@unittest.skipUnless(sys.platform == "darwin", "Apple CryptoKit requires macOS")
class SparkleSignatureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(cls.enterClassContext(tempfile.TemporaryDirectory()))
        cls.binary = root / "verify-sparkle-signature"
        subprocess.run(
            [
                "/usr/bin/xcrun",
                "swiftc",
                "-warnings-as-errors",
                VERIFIER,
                "-o",
                cls.binary,
            ],
            check=True,
        )

    def verify_signature(
        self, *, message=MESSAGE, key=PUBLIC_KEY, signature=SIGNATURE, digest=None
    ):
        with tempfile.TemporaryDirectory() as directory:
            dmg = Path(directory) / "Kiseki.dmg"
            dmg.write_bytes(message)
            return subprocess.run(
                [
                    self.binary,
                    dmg,
                    digest or hashlib.sha256(message).hexdigest(),
                    base64.b64encode(key).decode(),
                    base64.b64encode(signature).decode(),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

    def test_accepts_the_signature_over_the_original_bytes(self):
        result = self.verify_signature()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_a_wrong_published_digest(self):
        result = self.verify_signature(digest="0" * 64)
        self.assertEqual(result.returncode, 1)
        self.assertIn("published SHA-256 digest", result.stderr)

    def test_rejects_a_different_key_or_a_changed_dmg_with_its_own_digest(self):
        for inputs in ({"key": OTHER_KEY}, {"message": b"s"}):
            with self.subTest(inputs=inputs):
                result = self.verify_signature(**inputs)
                self.assertEqual(result.returncode, 1)
                self.assertIn("Sparkle signature does not match", result.stderr)

    def test_rejects_incomplete_key_and_signature_data(self):
        for inputs, explanation in (
            ({"key": PUBLIC_KEY[:-1]}, "valid Ed25519 public key"),
            ({"signature": SIGNATURE[:-1]}, "valid Ed25519 signature"),
        ):
            with self.subTest(inputs=inputs):
                result = self.verify_signature(**inputs)
                self.assertEqual(result.returncode, 1)
                self.assertIn(explanation, result.stderr)

    def test_rejects_non_base64_signature_data(self):
        with tempfile.TemporaryDirectory() as directory:
            dmg = Path(directory) / "Kiseki.dmg"
            dmg.write_bytes(MESSAGE)
            result = subprocess.run(
                [
                    self.binary,
                    dmg,
                    hashlib.sha256(MESSAGE).hexdigest(),
                    base64.b64encode(PUBLIC_KEY).decode(),
                    "invalid base64",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
        self.assertEqual(result.returncode, 1)
        self.assertIn("valid Ed25519 signature", result.stderr)
