use super::*;
use serde_json::json;

fn cask_info(artifacts: Value) -> Value {
    json!({"casks": [{"full_token": "naviapps/tap/kiseki", "artifacts": artifacts}]})
}

struct Fixture {
    _directory: tempfile::TempDir,
    home: PathBuf,
    runner: PathBuf,
    trash: Vec<String>,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let physical = directory.path().canonicalize().unwrap();
        let runner = physical.join("runner");
        fs::create_dir(&runner).unwrap();
        let cask = include_str!("../../../../Casks/kiseki.rb");
        let (_, zap) = cask.split_once("zap trash:").unwrap();
        let trash = zap
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix('"')?.split('"').next())
            .map(str::to_owned)
            .collect();
        let fixture = Self {
            _directory: directory,
            home: physical.join("home"),
            runner,
            trash,
        };
        fixture.write_trash(&fixture.trash);
        fixture
    }

    fn write_info(&self, cask: Value) {
        fs::write(self.runner.join("kiseki-cask.json"), cask.to_string()).unwrap();
    }

    fn write_trash(&self, trash: &[String]) {
        self.write_info(cask_info(json!([{"zap": [{"trash": trash}]}])));
    }

    fn paths(&self) -> FixturePaths {
        let info = read_cask(&self.runner.join("kiseki-cask.json")).unwrap();
        FixturePaths::from_cask(&info, &self.home, &self.runner).unwrap()
    }

    fn run(&self, stage: Stage, environment: Option<&str>) -> Result<(), String> {
        execute(stage, &self.home, &self.runner, environment.map(OsStr::new))
    }

    fn seed(&self) {
        self.run(Stage::Seed, Some("github-hosted")).unwrap();
    }

    fn no_seeded_files(&self, paths: &FixturePaths) {
        for fixture in paths.all().map(fixture_file) {
            assert!(!present(&fixture).unwrap(), "created {}", fixture.display());
        }
    }
}

#[test]
fn seed_checks_the_disposable_runner_before_creating_files() {
    let fixture = Fixture::new();
    let paths = fixture.paths();
    fs::remove_file(fixture.runner.join("kiseki-cask.json")).unwrap();
    for environment in [None, Some("self-hosted"), Some("GitHub-hosted")] {
        assert_eq!(
            fixture.run(Stage::Seed, environment).unwrap_err(),
            "Requires a disposable runner"
        );
    }
    assert!(!present(&fixture.home).unwrap());
    fixture.no_seeded_files(&paths);
}

#[test]
fn cleanup_paths_require_one_cask_and_one_explicit_list() {
    let fixture = Fixture::new();
    for (info, message) in [
        (
            json!({"casks": []}),
            "Expected one Homebrew cask in brew info JSON",
        ),
        (
            cask_info(json!([{"zap": []}])),
            "Expected one explicit cask cleanup list",
        ),
        (
            cask_info(json!([{"zap": [{"trash": "~/Library"}]}])),
            "Cask cleanup paths must be an explicit list",
        ),
        (
            cask_info(json!([{"zap": [{"trash": [false]}]}])),
            "Cask cleanup paths must be an explicit list",
        ),
    ] {
        fixture.write_info(info);
        assert_eq!(
            fixture.run(Stage::Seed, Some("github-hosted")).unwrap_err(),
            message
        );
        assert!(!present(&fixture.home).unwrap());
    }
}

#[test]
fn another_cask_cannot_seed_kiseki_cleanup_paths() {
    let fixture = Fixture::new();
    let paths = fixture.paths();
    let mut info = read_cask(&fixture.runner.join("kiseki-cask.json")).unwrap();
    info["casks"][0]["full_token"] = json!("example/tap/kiseki");
    fixture.write_info(info);
    assert_eq!(
        fixture.run(Stage::Seed, Some("github-hosted")).unwrap_err(),
        "Expected the naviapps/tap/kiseki cask"
    );
    fixture.no_seeded_files(&paths);
}

#[test]
fn exact_reviewed_locations_are_required_before_seeding() {
    let fixture = Fixture::new();
    let paths = fixture.paths();
    let mut many = fixture.trash.clone();
    let first_path = &fixture.trash[0];
    many.push(first_path.clone());
    let with_first_path = |path| {
        let mut trash = fixture.trash.clone();
        trash[0] = path;
        trash
    };
    for trash in [
        vec![],
        fixture.trash[..fixture.trash.len() - 1].to_vec(),
        many,
        with_first_path(fixture.trash[1].clone()),
        with_first_path(first_path.replace("Application Support", "Unrelated")),
        with_first_path(first_path.replace("Application Support", "*")),
        with_first_path(first_path.replace("Library/", "Library/../Library/")),
    ] {
        fixture.write_trash(&trash);
        let error = fixture.run(Stage::Seed, Some("github-hosted")).unwrap_err();
        assert!(error.contains("reviewed production paths"), "{error}");
        fixture.no_seeded_files(&paths);
    }
}

#[test]
fn reordered_expanded_paths_seed_production_qa_and_export_fixtures() {
    let fixture = Fixture::new();
    let expected = fixture.paths();
    let mut paths: Vec<_> = expected
        .remove
        .iter()
        .map(|path| path.to_str().unwrap().to_owned())
        .collect();
    paths.reverse();
    fixture.write_trash(&paths);
    fixture.seed();
    for path in expected.all().map(fixture_file) {
        assert_eq!(
            fs::read(&path).unwrap(),
            FIXTURE_CONTENTS,
            "{}",
            path.display()
        );
    }
    for path in [
        fixture
            .home
            .join("Library/Preferences/com.naviapps.Kiseki.plist"),
        fixture
            .home
            .join("Library/Preferences/com.naviapps.Kiseki.plist.QA/nested/fixture"),
        fixture
            .home
            .join("Library/Application Support/com.naviapps.Kiseki/nested/fixture"),
        fixture.runner.join("exported-gestures.kiseki"),
    ] {
        assert!(path.is_file(), "{}", path.display());
    }
    fixture.run(Stage::VerifyUninstall, None).unwrap();
}

#[test]
fn an_existing_protected_root_prevents_all_fixture_writes() {
    let fixture = Fixture::new();
    let paths = fixture.paths();
    let existing = &paths.keep[paths.keep.len() - 2];
    fs::create_dir_all(existing).unwrap();
    let marker = existing.join("user-data");
    fs::write(&marker, b"existing contents").unwrap();
    let error = fixture.run(Stage::Seed, Some("github-hosted")).unwrap_err();
    assert!(error.contains("Refusing to overwrite"), "{error}");
    assert_eq!(fs::read(marker).unwrap(), b"existing contents");
    fixture.no_seeded_files(&paths);
}

#[cfg(unix)]
#[test]
fn dangling_targets_and_parent_symlinks_are_refused_before_writes() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let paths = fixture.paths();
    let target = paths.remove.last().unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    symlink(fixture.runner.join("missing"), target).unwrap();
    let error = fixture.run(Stage::Seed, Some("github-hosted")).unwrap_err();
    assert!(error.contains("Unexpected cask cleanup path"), "{error}");
    assert!(target.is_symlink());
    fixture.no_seeded_files(&paths);

    fs::remove_file(target).unwrap();
    let redirect = paths.remove[0].parent().unwrap();
    let outside = fixture.runner.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, redirect).unwrap();
    let error = fixture.run(Stage::Seed, Some("github-hosted")).unwrap_err();
    assert!(error.contains("Unexpected cask cleanup path"), "{error}");
    assert_eq!(outside.read_dir().unwrap().count(), 0);
    fixture.no_seeded_files(&paths);
}

#[test]
fn uninstall_requires_every_fixture_and_preserves_legacy_line_endings() {
    let fixture = Fixture::new();
    let paths = fixture.paths();
    fixture.seed();
    let retained = fixture_file(&paths.keep[0]);
    for bytes in [
        b"Homebrew cleanup fixture\r\n".as_slice(),
        b"Homebrew cleanup fixture\r",
    ] {
        fs::write(&retained, bytes).unwrap();
        fixture
            .run(Stage::VerifyUninstall, Some("self-hosted"))
            .unwrap();
    }
    fs::write(&retained, b"Homebrew cleanup fixture\r\nx").unwrap();
    let error = fixture.run(Stage::VerifyUninstall, None).unwrap_err();
    assert!(error.contains("Uninstall changed"), "{error}");
    fs::write(&retained, FIXTURE_CONTENTS).unwrap();
    fs::remove_file(fixture_file(&paths.remove[0])).unwrap();
    let error = fixture.run(Stage::VerifyUninstall, None).unwrap_err();
    assert!(error.contains("Uninstall changed"), "{error}");
}

#[test]
fn zap_requires_production_roots_to_be_absent_and_controls_to_survive() {
    let fixture = Fixture::new();
    let paths = fixture.paths();
    fixture.seed();
    let error = fixture.run(Stage::VerifyZap, None).unwrap_err();
    assert!(error.contains("Zap left"), "{error}");
    for path in &paths.remove {
        if path.is_dir() {
            fs::remove_dir_all(path).unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
    }
    fixture.run(Stage::VerifyZap, None).unwrap();
    fs::write(fixture.runner.join("exported-gestures.kiseki"), b"modified").unwrap();
    let error = fixture.run(Stage::VerifyZap, None).unwrap_err();
    assert!(error.contains("Zap changed"), "{error}");
}

#[test]
fn verification_cannot_redefine_the_seeded_paths() {
    let fixture = Fixture::new();
    fixture.seed();
    fixture.write_trash(&[]);
    for stage in [Stage::VerifyUninstall, Stage::VerifyZap] {
        let error = fixture.run(stage, None).unwrap_err();
        assert!(error.contains("reviewed production paths"), "{error}");
    }
}

#[cfg(unix)]
#[test]
fn a_same_content_symlink_is_not_preserved_fixture_data() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.seed();
    let path = fixture_file(&fixture.paths().remove[0]);
    let replacement = fixture.runner.join("replacement");
    fs::write(&replacement, FIXTURE_CONTENTS).unwrap();
    fs::remove_file(&path).unwrap();
    symlink(replacement, path).unwrap();
    let error = fixture.run(Stage::VerifyUninstall, None).unwrap_err();
    assert!(error.contains("Uninstall changed"), "{error}");
}
