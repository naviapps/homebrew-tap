use super::*;
use serde_json::json;
use std::{cell::RefCell, fs};

const VERSION: &str = "1.2.3";
const RELEASE: &str = "https://github.com/naviapps/kiseki/releases/download/v1.2.3";
const URL: &str = "https://github.com/naviapps/kiseki/releases/download/v1.2.3/Kiseki.dmg";
const IMAGE: &[u8] = b"owned DMG fixture";
const DIGEST: &str = "d380130e372cd3c980076117a5f81044950bb72e73bfcc42144a02a45d6951d8";

fn cask_info() -> Value {
    json!({
        "casks": [{
            "full_token": "naviapps/tap/kiseki",
            "version": VERSION,
            "sha256": DIGEST,
            "url": URL,
        }]
    })
}

fn feed() -> String {
    format!(
        r#"<rss xmlns:s="{SPARKLE}"><channel><item>
<s:version>42</s:version>
<s:shortVersionString>{VERSION}</s:shortVersionString>
<enclosure url="{URL}" length="{}" s:edSignature="fixture"/>
</item></channel></rss>"#,
        IMAGE.len()
    )
}

fn manifest(feed: Option<&[u8]>) -> Vec<u8> {
    let mut manifest = format!("{DIGEST}  {DMG}\n");
    if let Some(feed) = feed {
        let digest = hex_digest(&Sha256::digest(feed));
        writeln!(manifest, "{digest}  {APPCAST}").unwrap();
    }
    manifest.into_bytes()
}

fn fetch(
    calls: &RefCell<Vec<&'static str>>,
    manifest: Vec<u8>,
    feed: Vec<u8>,
) -> impl FnMut(&str, &str, usize) -> Result<Vec<u8>, String> {
    move |release, name, limit| {
        assert_eq!(release, RELEASE);
        match name {
            MANIFEST => {
                assert_eq!(limit, 64 * 1024);
                calls.borrow_mut().push(MANIFEST);
                Ok(manifest.clone())
            }
            APPCAST => {
                assert_eq!(limit, 1024 * 1024);
                calls.borrow_mut().push(APPCAST);
                Ok(feed.clone())
            }
            _ => panic!("unexpected asset: {name}"),
        }
    }
}

fn request(info: &Value) -> Request<'_> {
    Request {
        cask_info: info,
        dmg: None,
        verify_appcast: false,
        bundle: None,
    }
}

fn no_signature(_: &Path, _: &Path, _: &Cask, _: &Enclosure) -> Result<(), String> {
    panic!("unexpected signature verification");
}

fn check_error(result: Result<Cask, String>, expected: &str) {
    let Err(error) = result else {
        panic!("expected {expected}, but verification succeeded");
    };
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

#[test]
fn unbound_and_ambiguous_casks_fail_before_downloading() {
    for (key, value, expected) in [
        (
            "full_token",
            "different/tap/kiseki",
            "Expected the naviapps/tap/kiseki",
        ),
        ("version", "1.2.3-beta.1", "stable Kiseki version"),
        ("version", "01.2.3", "stable Kiseki version"),
        ("version", "1.2.4", "exact GitHub release tag"),
        (
            "url",
            "https://github.com/example/kiseki/releases/download/v1.2.3/Kiseki.dmg",
            "exact GitHub release tag",
        ),
        (
            "url",
            "https://github.com/naviapps/kiseki/releases/latest/download/Kiseki.dmg",
            "exact GitHub release tag",
        ),
        ("sha256", "no_check", "lowercase SHA-256 digest"),
        (
            "sha256",
            &DIGEST.to_ascii_uppercase(),
            "lowercase SHA-256 digest",
        ),
    ] {
        let mut info = cask_info();
        info["casks"][0][key] = json!(value);
        check_error(
            verify(
                request(&info),
                |_, _, _| panic!("unexpected download"),
                no_signature,
            ),
            expected,
        );
    }
    for info in [
        json!(null),
        json!({}),
        json!({"casks":[]}),
        json!({"casks":["kiseki"]}),
        json!({"casks":[cask_info()["casks"][0], cask_info()["casks"][0]]}),
    ] {
        check_error(
            verify(
                request(&info),
                |_, _, _| panic!("unexpected download"),
                no_signature,
            ),
            "one Homebrew cask",
        );
    }
}

#[test]
fn manifest_only_and_appcast_modes_download_the_exact_assets() {
    for verify_appcast in [false, true] {
        let info = cask_info();
        let feed = feed().into_bytes();
        let calls = RefCell::new(Vec::new());
        let actual = verify(
            Request {
                verify_appcast,
                ..request(&info)
            },
            fetch(&calls, manifest(Some(&feed)), feed),
            no_signature,
        )
        .unwrap();
        assert_eq!(
            (actual.version.as_str(), actual.digest.as_str()),
            (VERSION, DIGEST)
        );
        let expected: &[_] = if verify_appcast {
            &[MANIFEST, APPCAST]
        } else {
            &[MANIFEST]
        };
        assert_eq!(calls.into_inner(), expected);
    }
}

#[test]
fn manifest_requires_one_well_formed_entry_per_selected_asset() {
    let feed = feed().into_bytes();
    let valid = manifest(Some(&feed));
    let dmg_only = manifest(None);
    for (contents, require_appcast, expected) in [
        (Vec::new(), false, "exactly one Kiseki.dmg entry"),
        (dmg_only.repeat(2), false, "exactly one Kiseki.dmg entry"),
        (dmg_only.clone(), true, "exactly one appcast.xml entry"),
        (
            dmg_only
                .iter()
                .copied()
                .chain(valid[dmg_only.len()..].repeat(2))
                .collect(),
            true,
            "exactly one appcast.xml entry",
        ),
        (
            dmg_only
                .iter()
                .copied()
                .chain(b"invalid unrelated".iter().copied())
                .collect(),
            false,
            "Malformed published SHA256SUMS",
        ),
        (
            dmg_only.iter().copied().chain([0xff]).collect(),
            false,
            "Malformed published SHA256SUMS",
        ),
        (
            format!("{}  Kiseki.dmg\n", "a".repeat(64)).into_bytes(),
            false,
            "Cask checksum differs",
        ),
    ] {
        let calls = RefCell::new(Vec::new());
        check_error(
            verify(
                Request {
                    verify_appcast: require_appcast,
                    ..request(&cask_info())
                },
                fetch(&calls, contents, feed.clone()),
                no_signature,
            ),
            expected,
        );
        assert_eq!(calls.into_inner(), [MANIFEST]);
    }
    let manifest = format!(
        "\u{2028}\t{}\u{2003}*Kiseki.dmg\r\n{DIGEST}\tappcast.xml\u{0085}{DIGEST}  appcast.xml",
        DIGEST.to_ascii_uppercase(),
    );
    assert_eq!(
        manifest_digests(manifest.as_bytes(), false),
        Ok((DIGEST.into(), None))
    );
    let trailing = format!("{DIGEST}\tKiseki.dmg \n");
    assert!(
        manifest_digests(trailing.as_bytes(), false)
            .unwrap_err()
            .contains("exactly one Kiseki.dmg entry")
    );
}

#[test]
fn appcast_bytes_must_match_before_xml_parsing() {
    let calls = RefCell::new(Vec::new());
    check_error(
        verify(
            Request {
                verify_appcast: true,
                ..request(&cask_info())
            },
            fetch(
                &calls,
                manifest(Some(feed().as_bytes())),
                b"changed bytes".to_vec(),
            ),
            no_signature,
        ),
        "appcast.xml checksum differs",
    );
    assert_eq!(calls.into_inner(), [MANIFEST, APPCAST]);
}

#[test]
fn appcast_validates_exact_structure_namespace_and_metadata() {
    let feed = feed();
    for (xml, expected) in [
        ("<rss".into(), "Malformed published appcast.xml"),
        (
            format!("<!-- <!DOCTYPE rss> -->{feed}"),
            "document type declaration",
        ),
        (
            feed.replace("<rss ", "<rss xmlns=\"https://example.invalid\" "),
            "one RSS channel",
        ),
        (
            feed.replace("</channel>", "</channel><channel/>"),
            "one RSS channel",
        ),
        (
            feed.replace("</item>", "</item><item/>"),
            "one release item",
        ),
        (feed.replace(">1.2.3<", ">1.2.4<"), "marketing version"),
        (
            feed.replace(SPARKLE, "https://example.invalid"),
            "marketing version",
        ),
        (
            feed.replace(">42<", "> 42<"),
            "numeric Sparkle build number",
        ),
        (feed.replace(">42<", ">0<"), "numeric Sparkle build number"),
        (
            feed.replace("<s:version>", "<s:version>8</s:version><s:version>"),
            "numeric Sparkle build number",
        ),
        (
            feed.replace(
                "<enclosure ",
                "<enclosure xmlns=\"https://example.invalid\" ",
            ),
            "exact GitHub release tag",
        ),
        (
            feed.replace("<enclosure ", "<enclosure/><enclosure "),
            "exact GitHub release tag",
        ),
        (
            feed.replace("download/v1.2.3/", "download/v1.2.4/"),
            "exact GitHub release tag",
        ),
        (
            feed.replace(" s:edSignature=\"fixture\"", ""),
            "signature metadata",
        ),
        (
            feed.replace(" s:edSignature=\"fixture\"", " edSignature=\"fixture\""),
            "signature metadata",
        ),
        (
            feed.replace(&format!("length=\"{}\"", IMAGE.len()), "length=\"0\""),
            "positive disk-image length",
        ),
    ] {
        let calls = RefCell::new(Vec::new());
        check_error(
            verify(
                Request {
                    verify_appcast: true,
                    ..request(&cask_info())
                },
                fetch(&calls, manifest(Some(xml.as_bytes())), xml.into_bytes()),
                no_signature,
            ),
            expected,
        );
        assert_eq!(calls.into_inner(), [MANIFEST, APPCAST]);
    }
}

#[test]
fn appcast_preserves_leading_text_and_non_machine_integer_lengths() {
    let cask = cask(&cask_info()).unwrap();
    let feed = feed().replace(">42<", ">4<!-- ignore --><?pi x?><![CDATA[2]]><");
    for prefix in [
        "\u{feff}",
        "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n",
    ] {
        let xml = format!("{prefix}{feed}");
        assert_eq!(
            appcast_enclosure(xml.as_bytes(), &cask).unwrap().build,
            "42"
        );
    }
    let xml = feed.replace("</s:version>", "<nested/>99</s:version>");
    assert_eq!(
        appcast_enclosure(xml.as_bytes(), &cask).unwrap().build,
        "42"
    );
    let xml = feed.replace("<rss ", "<rss xmlns=\"\" ");
    assert_eq!(
        appcast_enclosure(xml.as_bytes(), &cask).unwrap().build,
        "42"
    );
    let length = "9".repeat(319);
    let suffix = format!("length=\"{}\"", IMAGE.len());
    let xml = feed.replace(&suffix, &format!("length=\"{length}\""));
    assert_eq!(
        appcast_enclosure(xml.as_bytes(), &cask).unwrap().length,
        length
    );
    let xml = feed.replace(&suffix, &format!("length=\"{}\"", "9".repeat(4301)));
    assert!(appcast_enclosure(xml.as_bytes(), &cask).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn verified_bundle_drives_the_existing_swift_signature_verifier() {
    // RFC 8032 §7.1 test 2. The first test vector's public key deliberately mismatches.
    let key = "PUAXw+hDiVqStwqnTRt+vJyYLM8uxJaMwM1V8Sr0Zgw=";
    let other_key = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
    let signature =
        "kqAJqfDUyrhyDoILX2QlQKKye1QWUD+Ps3YiI+vbadoIWsHkPhWZbkWPNhPQ8R2MOHsurrQwKu6wDSkWErsMAA==";
    let root = tempfile::tempdir().unwrap();
    let dmg = root.path().join("Kiseki 日本語.dmg");
    let bundle = root.path().join("Kiseki 日本語.app");
    fs::create_dir_all(bundle.join("Contents")).unwrap();
    fs::write(&dmg, b"r").unwrap();
    let digest = hex_digest(&Sha256::digest(b"r"));
    let mut info = cask_info();
    info["casks"][0]["sha256"] = json!(digest);
    let xml = feed()
        .replace(&format!("length=\"{}\"", IMAGE.len()), "length=\"1\"")
        .replace(
            "s:edSignature=\"fixture\"",
            &format!("s:edSignature=\"{signature}\""),
        );
    let appcast_digest = hex_digest(&Sha256::digest(xml.as_bytes()));
    let manifest = format!("{digest}  {DMG}\n{appcast_digest}  {APPCAST}\n");
    for candidate in [key, other_key] {
        fs::write(
            bundle.join("Contents/Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
<key>CFBundleShortVersionString</key><string>{VERSION}</string>
<key>CFBundleVersion</key><string>42</string>
<key>SUPublicEDKey</key><string>{candidate}</string>
</dict></plist>"#
            ),
        )
        .unwrap();
        let calls = RefCell::new(Vec::new());
        let result = verify(
            Request {
                dmg: Some(&dmg),
                bundle: Some(&bundle),
                ..request(&info)
            },
            fetch(
                &calls,
                manifest.as_bytes().to_vec(),
                xml.as_bytes().to_vec(),
            ),
            crate::command::verify_signature,
        );
        if candidate == key {
            assert!(result.is_ok());
        } else {
            check_error(result, "Sparkle update signature verification failed");
        }
        assert_eq!(calls.into_inner(), [MANIFEST, APPCAST]);
    }
}

#[test]
fn local_dmg_and_optional_signature_use_the_verified_release() {
    let root = tempfile::tempdir().unwrap();
    let dmg = root.path().join("Kiseki 日本語.dmg");
    let bundle = root.path().join("Kiseki.app");
    fs::write(&dmg, IMAGE).unwrap();
    assert_eq!(
        disk_image(&dmg).unwrap(),
        (DIGEST.into(), IMAGE.len() as u64)
    );
    let info = cask_info();
    let calls = RefCell::new(Vec::new());
    let feed = feed();
    verify(
        Request {
            dmg: Some(&dmg),
            bundle: Some(&bundle),
            ..request(&info)
        },
        fetch(
            &calls,
            manifest(Some(feed.as_bytes())),
            feed.as_bytes().to_vec(),
        ),
        |passed_dmg, passed_bundle, cask, proof| {
            assert_eq!(
                (passed_dmg, passed_bundle),
                (dmg.as_path(), bundle.as_path())
            );
            assert_eq!(
                (cask.version.as_str(), cask.digest.as_str()),
                (VERSION, DIGEST)
            );
            assert_eq!(
                (proof.build.as_str(), proof.signature.as_str()),
                ("42", "fixture")
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(calls.into_inner(), [MANIFEST, APPCAST]);

    let wrong_size = feed.replace(&format!("length=\"{}\"", IMAGE.len()), "length=\"1\"");
    let calls = RefCell::new(Vec::new());
    check_error(
        verify(
            Request {
                dmg: Some(&dmg),
                verify_appcast: true,
                ..request(&info)
            },
            fetch(
                &calls,
                manifest(Some(wrong_size.as_bytes())),
                wrong_size.into_bytes(),
            ),
            no_signature,
        ),
        "Local DMG length differs",
    );
    fs::write(&dmg, b"different bytes").unwrap();
    let calls = RefCell::new(Vec::new());
    check_error(
        verify(
            Request {
                dmg: Some(&dmg),
                ..request(&info)
            },
            fetch(&calls, manifest(None), Vec::new()),
            no_signature,
        ),
        "Local DMG checksum differs",
    );
    assert_eq!(calls.into_inner(), [MANIFEST]);
    check_error(
        verify(
            Request {
                bundle: Some(&bundle),
                ..request(&info)
            },
            |_, _, _| panic!("unexpected download"),
            no_signature,
        ),
        "requires a local DMG",
    );
}
