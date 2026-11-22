use std::{fs, process::Command};

fn verifier() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kiseki-cask-release-checker"));
    command.env_remove("RUNNER_TEMP");
    command
}

#[test]
fn help_and_missing_or_malformed_options_use_the_cli_contract() {
    let help = verifier().arg("--help").output().unwrap();
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    for expected in [
        "--cask-info",
        "--dmg",
        "--verify-appcast",
        "--sparkle-bundle",
        "--verify-installed",
        "--cleanup-fixtures",
        "RUNNER_TEMP",
    ] {
        assert!(stdout.contains(expected));
    }
    for (arguments, message) in [
        (vec![], "pass --cask-info or set RUNNER_TEMP"),
        (vec!["--verify-appcast=1"], "Unrecognized argument"),
        (vec!["--dmg", "--verify-appcast"], "requires a path"),
        (
            vec!["--verify-installed", "--verify-appcast"],
            "--verify-installed takes no other arguments",
        ),
        (
            vec!["--cleanup-fixtures"],
            "--cleanup-fixtures requires one stage",
        ),
        (
            vec!["--cleanup-fixtures", "unknown"],
            "--cleanup-fixtures requires one stage",
        ),
        (
            vec!["--cleanup-fixtures", "seed", "verify-zap"],
            "--cleanup-fixtures requires one stage",
        ),
    ] {
        let result = verifier().args(arguments).output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8(result.stderr).unwrap().contains(message));
    }
}

#[test]
fn installed_mode_requires_the_runner_temp_location() {
    for directory in [None, Some("")] {
        let mut command = verifier();
        if let Some(directory) = directory {
            command.env("RUNNER_TEMP", directory);
        }
        let result = command.arg("--verify-installed").output().unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(
            String::from_utf8(result.stderr).unwrap().trim(),
            "RUNNER_TEMP must hold the cask metadata and test app directory",
        );
    }
}

#[test]
fn cleanup_seed_checks_the_runner_before_writing_to_home_or_temp() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("home");
    let runner = directory.path().join("runner");
    fs::create_dir(&runner).unwrap();
    let result = verifier()
        .args(["--cleanup-fixtures", "seed"])
        .env("HOME", &home)
        .env("RUNNER_TEMP", &runner)
        .env("RUNNER_ENVIRONMENT", "self-hosted")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(result.stderr).unwrap().trim(),
        "Requires a disposable runner",
    );
    assert!(!home.exists());
    assert_eq!(runner.read_dir().unwrap().count(), 0);
}

#[test]
fn runner_temp_default_and_explicit_cask_file_fail_before_network() {
    let root = tempfile::tempdir().unwrap();
    let cask_info = root.path().join("kiseki-cask.json");
    fs::write(&cask_info, b"{}").unwrap();
    let default = verifier().env("RUNNER_TEMP", root.path()).output().unwrap();
    let explicit = verifier()
        .arg("--cask-inf")
        .arg(&cask_info)
        .output()
        .unwrap();
    for result in [default, explicit] {
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(
            String::from_utf8(result.stderr).unwrap().trim(),
            "Expected one Homebrew cask in brew info JSON",
        );
    }
    let missing_dmg = verifier()
        .arg("--cask-info")
        .arg(&cask_info)
        .args(["--sparkle-bundle", "Kiseki.app"])
        .output()
        .unwrap();
    assert_eq!(missing_dmg.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(missing_dmg.stderr).unwrap().trim(),
        "Sparkle signature verification requires a local DMG",
    );
}
