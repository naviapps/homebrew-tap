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
        "RUNNER_TEMP",
    ] {
        assert!(stdout.contains(expected));
    }
    for (arguments, message) in [
        (vec![], "pass --cask-info or set RUNNER_TEMP"),
        (vec!["--verify-appcast=1"], "Unrecognized argument"),
        (vec!["--dmg", "--verify-appcast"], "requires a path"),
    ] {
        let result = verifier().args(arguments).output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8(result.stderr).unwrap().contains(message));
    }
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
