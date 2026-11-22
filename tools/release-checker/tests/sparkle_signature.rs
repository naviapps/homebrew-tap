#![cfg(target_os = "macos")]

use sha2::{Digest, Sha256};
use std::{fmt::Write, fs, path::Path, process::Command};

// RFC 8032 `7.1 test 2. The complete vector is verified by the release pipeline test.
const PUBLIC_KEY: &str = "PUAXw+hDiVqStwqnTRt+vJyYLM8uxJaMwM1V8Sr0Zgw=";
const SIGNATURE: &str =
    "kqAJqfDUyrhyDoILX2QlQKKye1QWUD+Ps3YiI+vbadoIWsHkPhWZbkWPNhPQ8R2MOHsurrQwKu6wDSkWErsMAA==";

#[derive(Clone, Copy)]
struct Input {
    image: &'static [u8],
    published_digest: Option<&'static str>,
    key: &'static str,
    signature: &'static str,
}

#[test]
fn swift_verifier_rejects_invalid_digests_images_and_credentials() {
    let root = tempfile::tempdir().unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/verify-sparkle-signature.swift");
    let verifier = root.path().join("verify-sparkle-signature");
    let compile = Command::new("/usr/bin/xcrun")
        .env("HOME", root.path())
        .env("TMPDIR", root.path())
        .args(["swiftc", "-warnings-as-errors", "-module-cache-path"])
        .arg(root.path().join("swift-module-cache"))
        .arg(script)
        .arg("-o")
        .arg(&verifier)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let original = Input {
        image: b"r",
        published_digest: None,
        key: PUBLIC_KEY,
        signature: SIGNATURE,
    };
    let cases = [
        (
            "wrong published digest",
            Input {
                published_digest: Some(
                    "0000000000000000000000000000000000000000000000000000000000000000",
                ),
                ..original
            },
            "published SHA-256 digest",
        ),
        (
            "changed image with its own digest",
            Input {
                image: b"s",
                ..original
            },
            "Sparkle signature does not match",
        ),
        (
            "incomplete key",
            Input {
                key: "PUAXw+hDiVqStwqnTRt+vJyYLM8uxJaMwM1V8Sr0Zg==",
                ..original
            },
            "valid Ed25519 public key",
        ),
        (
            "incomplete signature",
            Input {
                signature: "kqAJqfDUyrhyDoILX2QlQKKye1QWUD+Ps3YiI+vbadoIWsHkPhWZbkWPNhPQ8R2MOHsurrQwKu6wDSkWErsM",
                ..original
            },
            "valid Ed25519 signature",
        ),
        (
            "invalid base64 signature",
            Input {
                signature: "invalid base64",
                ..original
            },
            "valid Ed25519 signature",
        ),
    ];
    let dmg = root.path().join("Kiseki 日本語.dmg");
    for (name, input, diagnostic) in cases {
        fs::write(&dmg, input.image).unwrap();
        let mut actual_digest = String::with_capacity(64);
        for byte in Sha256::digest(input.image) {
            write!(actual_digest, "{byte:02x}").unwrap();
        }
        let output = Command::new(&verifier)
            .env("HOME", root.path())
            .env("TMPDIR", root.path())
            .arg(&dmg)
            .args([
                input.published_digest.unwrap_or(&actual_digest),
                input.key,
                input.signature,
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{name}: {stderr}");
        assert!(stderr.contains(diagnostic), "{name}: {stderr}");
    }
}
