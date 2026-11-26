use super::*;

#[test]
fn curl_ignores_user_configuration_and_bounds_https_redirects() {
    let command = curl_command("https://example.invalid/v1", "SHA256SUMS", 65536);
    assert_eq!(command.get_program(), "/usr/bin/curl");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "--disable",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-redirs",
            "10",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "30",
            "--max-filesize",
            "65537",
            "--",
            "https://example.invalid/v1/SHA256SUMS",
        ],
    );
}

#[test]
fn read_requires_complete_successful_response_within_the_byte_limit() {
    for name in ["SHA256SUMS", "appcast.xml"] {
        for (script, expected, message) in [
            ("printf 1234", Some(b"1234".as_slice()), ""),
            ("printf 12345", None, "exceeds the size limit"),
            ("printf partial; exit 63", None, "exceeds the size limit"),
            (
                "printf 1234; exit 1",
                None,
                "must be transferred over HTTPS",
            ),
            ("printf 1234; exit 22", None, "Could not download published"),
        ] {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            let output = read_download(&mut command, name, 4);
            if let Some(expected) = expected {
                assert_eq!(output.unwrap(), expected);
            } else {
                assert!(output.unwrap_err().contains(message), "{script}");
            }
        }
    }
}

#[test]
fn crypto_receives_the_original_script_and_exact_metadata() {
    let dmg = Path::new("/tmp/Kiseki 日本語.dmg");
    let enclosure = Enclosure {
        length: "123".into(),
        signature: "signature".into(),
        build: "42".into(),
    };
    let command = crypto_command(dmg, "sha", " key ", &enclosure);
    assert_eq!(command.get_program(), "/usr/bin/xcrun");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(&args[..2], &["swift", "-warnings-as-errors"]);
    assert!(Path::new(args[2]).ends_with("scripts/verify-sparkle-signature.swift"));
    assert_eq!(
        &args[3..],
        &[
            dmg.as_os_str(),
            "sha".as_ref(),
            " key ".as_ref(),
            "signature".as_ref(),
            "123".as_ref(),
        ]
    );
}

#[cfg(target_os = "macos")]
#[test]
fn signed_bundle_checks_exact_string_types_in_xml_and_binary_plists() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("Kiseki 日本語.app");
    std::fs::create_dir_all(bundle.join("Contents")).unwrap();
    let info = bundle.join("Contents/Info.plist");
    let valid = r#"<key>CFBundleShortVersionString</key><string>1.2.3</string>
<key>CFBundleVersion</key><string>42</string>
<key>SUPublicEDKey</key><string>fixture-key</string>"#;
    let contents = |body: &str| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>{body}</dict></plist>"#
        )
    };
    std::fs::write(&info, contents(valid)).unwrap();
    assert_eq!(
        verify_bundle(&bundle, "1.2.3", "42"),
        Ok("fixture-key".into())
    );
    let status = Command::new("/usr/bin/plutil")
        .args(["-convert", "binary1", "--"])
        .arg(&info)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        verify_bundle(&bundle, "1.2.3", "42"),
        Ok("fixture-key".into())
    );

    for (from, to, error) in [
        (
            "<string>42</string>",
            "<integer>42</integer>",
            "bundle build must match",
        ),
        (
            "<string>1.2.3</string>",
            "<string>1.2.4</string>",
            "marketing version must match",
        ),
        (
            "<string>fixture-key</string>",
            "<integer>1</integer>",
            "provide a Sparkle public key",
        ),
        (
            "<string>fixture-key</string>",
            "<string> \n </string>",
            "provide a Sparkle public key",
        ),
    ] {
        std::fs::write(&info, contents(&valid.replace(from, to))).unwrap();
        assert!(
            verify_bundle(&bundle, "1.2.3", "42")
                .unwrap_err()
                .contains(error)
        );
    }
}
