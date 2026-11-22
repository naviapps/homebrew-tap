use super::*;
use serde_json::json;
use std::path::PathBuf;

const METADATA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>com.naviapps.Kiseki</string>
    <key>CFBundleShortVersionString</key><string>1.2.3</string>
    <key>CFBundleExecutable</key><string>Kiseki</string>
    <key>LSMinimumSystemVersion</key><string>26.0</string>
    <key>SUFeedURL</key><string>https://github.com/naviapps/kiseki/releases/latest/download/appcast.xml</string>
    <key>BuildDate</key><date>2026-01-01T00:00:00Z</date>
</dict>
</plist>
"#;

struct Fixture {
    root: tempfile::TempDir,
    contents: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let contents = root.path().join(APPLICATION);
        fs::create_dir_all(contents.join("MacOS")).unwrap();
        fs::write(contents.join("MacOS").join(EXECUTABLE), b"binary fixture").unwrap();
        let fixture = Self { root, contents };
        fixture.write_cask(json!({"casks": [Self::cask()]}));
        fixture.write_plist(METADATA);
        fixture
    }

    fn cask() -> serde_json::Value {
        json!({
            "full_token": "naviapps/tap/kiseki",
            "version": "1.2.3",
            "depends_on": {"macos": {">=": ["26"]}},
        })
    }

    fn write_cask(&self, info: serde_json::Value) {
        fs::write(self.root.path().join("kiseki-cask.json"), info.to_string()).unwrap();
    }

    fn write_plist(&self, metadata: &str) {
        fs::write(self.contents.join("Info.plist"), metadata).unwrap();
    }

    fn verify(
        &self,
        inspect: impl FnOnce(&Path) -> Result<String, String>,
    ) -> Result<String, String> {
        verify(self.root.path(), inspect)
    }

    fn reject_before_lipo(&self, diagnostic: &str) {
        let error = self
            .verify(|_| panic!("lipo ran before rejecting invalid input"))
            .unwrap_err();
        assert!(error.contains(diagnostic), "{error:?}");
    }
}

#[test]
fn equivalent_minimum_versions_use_exact_declared_executable() {
    let fixture = Fixture::new();
    for minimum in ["26", "26.0", "26.0.0", "00026.000.0"] {
        let revised = METADATA.replace("26.0", minimum);
        fixture.write_plist(&revised);
        let version = fixture
            .verify(|binary| {
                assert_eq!(binary, fixture.contents.join("MacOS/Kiseki"));
                Ok("x86_64 arm64\n".into())
            })
            .unwrap();
        assert_eq!(version, "1.2.3");
    }
}

#[test]
fn installed_admission_preserves_nonempty_versions_without_release_fields() {
    let fixture = Fixture::new();
    let mut cask = Fixture::cask();
    cask["version"] = json!("local preview");
    fixture.write_cask(json!({"casks": [cask]}));
    fixture.write_plist(&METADATA.replace("1.2.3", "local preview"));
    assert_eq!(
        fixture
            .verify(|_| Ok("arm64 x86_64 arm64e".into()))
            .unwrap(),
        "local preview"
    );
}

#[test]
fn xml_line_endings_and_references_match_the_previous_reader() {
    let fixture = Fixture::new();
    for (source, version) in [
        ("<![CDATA[preview build]]>", "preview build"),
        ("preview\r\nbuild", "preview\nbuild"),
        ("preview&#13;build", "preview\rbuild"),
    ] {
        let mut cask = Fixture::cask();
        cask["version"] = json!(version);
        fixture.write_cask(json!({"casks": [cask]}));
        let xml = METADATA.replace("1.2.3", source);
        fixture.write_plist(&xml);
        assert_eq!(
            fixture.verify(|_| Ok("arm64 x86_64".into())).unwrap(),
            version
        );
        for (bom, encode) in [
            ([0xff, 0xfe], u16::to_le_bytes as fn(u16) -> [u8; 2]),
            ([0xfe, 0xff], u16::to_be_bytes),
        ] {
            let wide = xml.replace("UTF-8", "UTF-16");
            let bytes: Vec<u8> = bom
                .into_iter()
                .chain(wide.encode_utf16().flat_map(encode))
                .collect();
            fs::write(fixture.contents.join("Info.plist"), bytes).unwrap();
            assert_eq!(
                fixture.verify(|_| Ok("arm64 x86_64".into())).unwrap(),
                version
            );
        }
    }
    let binary = b"bplist00preview\r\nbuild\0";
    assert!(matches!(plist_snapshot(binary), Cow::Borrowed(_)));
}

#[test]
fn mismatched_casks_and_minimum_metadata_are_rejected_before_lipo() {
    let fixture = Fixture::new();
    for (path, value, error) in [
        (
            "full_token",
            json!("example/tap/kiseki"),
            "naviapps/tap/kiseki",
        ),
        (
            "version",
            json!(""),
            "Cask version must be a nonempty string",
        ),
        (
            "version",
            json!(123),
            "Cask version must be a nonempty string",
        ),
        ("depends_on", json!({}), "one minimum macOS version"),
        (
            "depends_on",
            json!({"macos": {">=": ["26", "27"]}}),
            "one minimum macOS version",
        ),
        (
            "depends_on",
            json!({"macos": {">=": [26]}}),
            "numeric components",
        ),
    ] {
        let mut cask = Fixture::cask();
        cask[path] = value;
        fixture.write_cask(json!({"casks": [cask]}));
        fixture.reject_before_lipo(error);
    }
    fixture.write_cask(json!({"casks": [Fixture::cask(), Fixture::cask()]}));
    fixture.reject_before_lipo("one Homebrew cask");
}

#[test]
fn wrong_or_untyped_plist_metadata_fails_before_lipo() {
    let fixture = Fixture::new();
    for (original, revised, error) in [
        (
            "com.naviapps.Kiseki",
            "com.naviapps.Kiseki.QA",
            "Installed bundle identifier",
        ),
        (
            "<string>1.2.3</string>",
            "<integer>123</integer>",
            "Installed bundle version",
        ),
        ("1.2.3", "1.2.4", "Installed bundle version"),
        (
            "<string>Kiseki</string>",
            "<string>../Kiseki</string>",
            "Installed CFBundleExecutable",
        ),
        (
            "<string>Kiseki</string>",
            "<false/>",
            "Installed CFBundleExecutable",
        ),
        (
            UPDATE_FEED,
            "https://downloads.kisekiapp.com/appcast.xml",
            "Installed SUFeedURL",
        ),
        (
            "<key>SUFeedURL</key>",
            "<key>OtherFeedURL</key>",
            "Installed SUFeedURL",
        ),
        (
            "<string>26.0</string>",
            "<integer>26</integer>",
            "numeric components",
        ),
        ("26.0", "26.1", "match the cask's macOS"),
    ] {
        fixture.write_plist(&METADATA.replace(original, revised));
        fixture.reject_before_lipo(error);
    }
    fixture.write_plist("<plist version=\"1.0\"><array><string>Kiseki</string></array></plist>");
    fixture.reject_before_lipo("App Info.plist must be a dictionary");
}

#[test]
fn minimum_version_numbers_keep_their_full_decimal_value() {
    for value in ["26.0.0.0", "+26", " 26", "２６", "26.", "26..0", ""] {
        assert!(macos_version(Some(value)).is_err(), "{value:?}");
    }
    let maximum = "9".repeat(MAXIMUM_DECIMAL_DIGITS);
    assert_eq!(
        macos_version(Some(&maximum)).unwrap(),
        [maximum.as_str(), "", ""]
    );
    let oversized = "0".repeat(MAXIMUM_DECIMAL_DIGITS + 1);
    assert!(macos_version(Some(&oversized)).is_err());
}

#[test]
fn requires_a_binary_with_both_named_slices() {
    let fixture = Fixture::new();
    for arches in ["arm64", "x86_64", "x86_64 arm64e"] {
        assert!(
            fixture
                .verify(|_| Ok(arches.into()))
                .unwrap_err()
                .contains("must contain arm64 and x86_64 slices"),
            "{arches}"
        );
    }
    assert_eq!(
        fixture
            .verify(|_| Err("inspection failed".into()))
            .unwrap_err(),
        "inspection failed"
    );
    fs::remove_file(fixture.contents.join("MacOS/Kiseki")).unwrap();
    fixture.reject_before_lipo("bundle executable is missing");
}

#[test]
fn native_lipo_checks_synthetic_objects_in_the_installed_bundle() {
    use std::process::Command;

    let fixture = Fixture::new();
    let c = fixture.root.path().join("fixture.c");
    fs::write(&c, "int fixture(void) { return 42; }\n").unwrap();
    let objects = ["arm64", "x86_64"].map(|architecture| {
        let object = fixture.root.path().join(format!("{architecture}.o"));
        assert!(
            Command::new("/usr/bin/clang")
                .args(["-arch", architecture, "-c"])
                .arg(&c)
                .arg("-o")
                .arg(&object)
                .status()
                .unwrap()
                .success()
        );
        object
    });
    let binary = fixture.contents.join("MacOS/Kiseki");
    fs::copy(&objects[0], &binary).unwrap();
    assert!(
        fixture
            .verify(command::architectures)
            .unwrap_err()
            .contains("must contain arm64 and x86_64 slices")
    );
    assert!(
        Command::new("/usr/bin/lipo")
            .arg("-create")
            .args(&objects)
            .arg("-output")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(fixture.verify(command::architectures).unwrap(), "1.2.3");
}
