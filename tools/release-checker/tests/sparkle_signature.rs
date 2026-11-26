#![cfg(target_os = "macos")]

use std::{fs, path::Path, process::Command};

// RFC 8032 `7.1 test 2. The complete vector is verified by the release pipeline test.
const PUBLIC_KEY: &str = "PUAXw+hDiVqStwqnTRt+vJyYLM8uxJaMwM1V8Sr0Zgw=";
const SIGNATURE: &str =
    "kqAJqfDUyrhyDoILX2QlQKKye1QWUD+Ps3YiI+vbadoIWsHkPhWZbkWPNhPQ8R2MOHsurrQwKu6wDSkWErsMAA==";
const ORIGINAL_DIGEST: &str = "454349e422f05297191ead13e21d3db520e5abef52055e4964b82fb213f593a1";
const CHANGED_DIGEST: &str = "043a718774c572bd8a25adbeb1bfcd5c0256ae11cecf9f9c3f925d0e52beaf89";

#[derive(Clone, Copy)]
struct Input {
    image: &'static [u8],
    published_digest: &'static str,
    key: &'static str,
    signature: &'static str,
    length: Option<&'static str>,
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
        published_digest: ORIGINAL_DIGEST,
        key: PUBLIC_KEY,
        signature: SIGNATURE,
        length: None,
    };
    let cases = [
        (
            "wrong published digest",
            Input {
                published_digest: "0000000000000000000000000000000000000000000000000000000000000000",
                ..original
            },
            "published SHA-256 digest",
        ),
        (
            "wrong enclosure length",
            Input {
                length: Some("2"),
                ..original
            },
            "published appcast enclosure",
        ),
        (
            "changed image with its own digest",
            Input {
                image: b"s",
                published_digest: CHANGED_DIGEST,
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
        let output = Command::new(&verifier)
            .env("HOME", root.path())
            .env("TMPDIR", root.path())
            .arg(&dmg)
            .args([input.published_digest, input.key, input.signature])
            .args(input.length)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{name}: {stderr}");
        assert!(stderr.contains(diagnostic), "{name}: {stderr}");
    }
}
