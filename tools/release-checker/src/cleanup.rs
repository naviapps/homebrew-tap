use serde_json::Value;
use std::{
    env,
    ffi::OsStr,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

use crate::{CASK_INFO_FILE, read_cask, release::cask_entry};

const FIXTURE_CONTENTS: &[u8] = b"Homebrew cleanup fixture\n";
const PRODUCTION_PATHS: [&str; 6] = [
    "Application Support/com.naviapps.Kiseki",
    "Caches/com.naviapps.Kiseki",
    "HTTPStorages/com.naviapps.Kiseki",
    "HTTPStorages/com.naviapps.Kiseki.binarycookies",
    "Preferences/com.naviapps.Kiseki.plist",
    "Saved Application State/com.naviapps.Kiseki.savedState",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Seed,
    VerifyUninstall,
    VerifyZap,
}

impl Stage {
    pub(crate) fn parse(argument: &OsStr) -> Option<Self> {
        match argument.to_str()? {
            "seed" => Some(Self::Seed),
            "verify-uninstall" => Some(Self::VerifyUninstall),
            "verify-zap" => Some(Self::VerifyZap),
            _ => None,
        }
    }
}

struct FixturePaths {
    remove: Vec<PathBuf>,
    keep: Vec<PathBuf>,
}

impl FixturePaths {
    fn from_cask(info: &Value, home: &Path, root: &Path) -> Result<Self, String> {
        let artifacts = cask_entry(info)?["artifacts"]
            .as_array()
            .ok_or("Expected one explicit cask cleanup list")?;
        let mut zaps = artifacts.iter().filter_map(|artifact| artifact.get("zap"));
        let (Some([zap]), None) = (
            zaps.next().and_then(Value::as_array).map(Vec::as_slice),
            zaps.next(),
        ) else {
            return Err("Expected one explicit cask cleanup list".into());
        };
        let trash_error = "Cask cleanup paths must be an explicit list";
        let remove = zap["trash"]
            .as_array()
            .ok_or(trash_error)?
            .iter()
            .map(|path| {
                let path = path.as_str().ok_or(trash_error)?;
                Ok(path
                    .strip_prefix("~/")
                    .map_or_else(|| PathBuf::from(path), |relative| home.join(relative)))
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let library = home.join("Library");
        if remove.len() != PRODUCTION_PATHS.len()
            || !PRODUCTION_PATHS
                .iter()
                .all(|path| remove.contains(&library.join(path)))
        {
            return Err("Cask cleanup paths differ from the reviewed production paths".into());
        }
        let mut keep = Vec::with_capacity(remove.len() + 1);
        for path in &remove {
            let mut name = path
                .file_name()
                .ok_or("Expected a reviewed cask cleanup file name")?
                .to_os_string();
            name.push(".QA");
            keep.push(path.with_file_name(name));
        }
        keep.push(root.join("exported-gestures.kiseki"));
        Ok(Self { remove, keep })
    }

    fn all(&self) -> impl Iterator<Item = &Path> {
        self.remove.iter().chain(&self.keep).map(PathBuf::as_path)
    }
}

fn fixture_file(path: &Path) -> PathBuf {
    match path.extension().and_then(OsStr::to_str) {
        Some("plist" | "binarycookies" | "kiseki") => path.to_owned(),
        _ => path.join("nested/fixture"),
    }
}

enum CheckedPath {
    Unsafe,
    Safe(Option<fs::Metadata>),
}

fn path_metadata(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn checked_path(path: &Path) -> io::Result<CheckedPath> {
    if !path.is_absolute() {
        return Ok(CheckedPath::Unsafe);
    }
    let mut ancestor = PathBuf::new();
    let mut metadata = None;
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Ok(CheckedPath::Unsafe);
        }
        ancestor.push(component);
        metadata = path_metadata(&ancestor)?;
        if metadata.as_ref().is_some_and(fs::Metadata::is_symlink) {
            return Ok(CheckedPath::Unsafe);
        }
    }
    Ok(CheckedPath::Safe(metadata))
}

fn present(path: &Path) -> Result<bool, String> {
    path_metadata(path)
        .map(|metadata| metadata.is_some())
        .map_err(|error| format!("Could not inspect {}: {error}", path.display()))
}

fn seed(paths: &FixturePaths) -> Result<(), String> {
    // Check every root before creating any fixture.
    for path in paths.all() {
        match checked_path(path)
            .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?
        {
            CheckedPath::Unsafe => {
                return Err(format!("Unexpected cask cleanup path: {}", path.display()));
            }
            CheckedPath::Safe(Some(_)) => {
                return Err(format!("Refusing to overwrite {}", path.display()));
            }
            CheckedPath::Safe(None) => {}
        }
    }
    for path in paths.all().map(fixture_file) {
        let parent = path.parent().ok_or("Expected a fixture parent directory")?;
        fs::create_dir_all(parent)
            .and_then(|()| File::options().write(true).create_new(true).open(&path))
            .and_then(|mut file| file.write_all(FIXTURE_CONTENTS))
            .map_err(|error| format!("Could not create {}: {error}", path.display()))?;
    }
    Ok(())
}

fn verify_preserved(paths: impl Iterator<Item = PathBuf>, action: &str) -> Result<(), String> {
    let body = &FIXTURE_CONTENTS[..FIXTURE_CONTENTS.len() - 1];
    // Read one byte beyond the longest supported marker to detect trailing data.
    let limit = body.len() + 3;
    let mut bytes = Vec::with_capacity(limit);
    for path in paths {
        bytes.clear();
        let ordinary_file = matches!(
            checked_path(&path),
            Ok(CheckedPath::Safe(Some(metadata))) if metadata.is_file()
        );
        if !ordinary_file
            || File::open(&path)
                .and_then(|file| file.take(limit as u64).read_to_end(&mut bytes))
                .is_err()
            || !matches!(bytes.strip_prefix(body), Some(b"\n" | b"\r\n" | b"\r"))
        {
            return Err(format!("{action} changed {}", path.display()));
        }
    }
    Ok(())
}

fn execute(
    stage: Stage,
    home: &Path,
    root: &Path,
    runner_environment: Option<&OsStr>,
) -> Result<(), String> {
    if stage == Stage::Seed && runner_environment != Some(OsStr::new("github-hosted")) {
        return Err("Requires a disposable runner".into());
    }
    let cask = read_cask(&root.join(CASK_INFO_FILE))?;
    let paths = FixturePaths::from_cask(&cask, home, root)?;
    match stage {
        Stage::Seed => seed(&paths),
        Stage::VerifyUninstall => verify_preserved(paths.all().map(fixture_file), "Uninstall"),
        Stage::VerifyZap => {
            for path in &paths.remove {
                if present(path)? {
                    return Err(format!("Zap left {}", path.display()));
                }
            }
            verify_preserved(paths.keep.iter().map(|path| fixture_file(path)), "Zap")
        }
    }
}

pub(crate) fn run(stage: Stage, runner_temp: Option<&Path>) -> Result<(), String> {
    let home = env::home_dir().ok_or("Could not resolve the runner's home directory")?;
    let root = runner_temp.ok_or("RUNNER_TEMP must hold the cask metadata and cleanup fixtures")?;
    execute(
        stage,
        &home,
        root,
        env::var_os("RUNNER_ENVIRONMENT").as_deref(),
    )
}

#[cfg(test)]
mod tests;
