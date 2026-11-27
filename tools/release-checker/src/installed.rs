use std::{borrow::Cow, path::Path};

use crate::{
    CASK_INFO_FILE, command, read_cask, read_file,
    release::{MAXIMUM_DECIMAL_DIGITS, cask_entry, decimal, whitespace},
};

const UPDATE_FEED: &str = "https://github.com/naviapps/kiseki/releases/latest/download/appcast.xml";
const INVALID_MINIMUM: &str = "Expected a macOS version with one to three numeric components";
const APPLICATION: &str = "kiseki-apps/Kiseki.app/Contents";
const EXECUTABLE: &str = "Kiseki";
const INVALID_PLIST: &str = "Invalid application Info.plist";

fn macos_version(value: Option<&str>) -> Result<[&str; 3], String> {
    let mut components = value.ok_or(INVALID_MINIMUM)?.split('.');
    let mut normalized = [""; 3];
    for part in &mut normalized {
        let Some(component) = components.next() else {
            break;
        };
        if component.len() > MAXIMUM_DECIMAL_DIGITS || !decimal(component) {
            return Err(INVALID_MINIMUM.into());
        }
        *part = component.trim_start_matches('0');
    }
    if components.next().is_some() {
        return Err(INVALID_MINIMUM.into());
    }
    Ok(normalized)
}

fn recognized_plist(bytes: &[u8]) -> bool {
    // Keep the XML and binary forms recognized by the previous verifier.
    const PREFIXES: &[&[u8]] = &[
        b"bplist00",
        b"<?xml",
        b"<plist",
        b"\xef\xbb\xbf<?xml",
        b"\xef\xbb\xbf<plist",
        b"\xff\xfe<\0?\0x\0m\0l\0",
        b"\xff\xfe<\0p\0l\0i\0s\0t\0",
        b"\xfe\xff\0<\0?\0x\0m\0l",
        b"\xfe\xff\0<\0p\0l\0i\0s\0t",
    ];
    PREFIXES.iter().any(|prefix| bytes.starts_with(prefix))
}

fn plist_snapshot(bytes: &[u8]) -> Cow<'_, [u8]> {
    let (cr, lf) = match bytes {
        [0xff, 0xfe, ..] => (b"\r\0".as_slice(), b"\n\0".as_slice()),
        [0xfe, 0xff, ..] => (b"\0\r".as_slice(), b"\0\n".as_slice()),
        _ if bytes.starts_with(b"bplist00") => return Cow::Borrowed(bytes),
        _ => (b"\r".as_slice(), b"\n".as_slice()),
    };
    if !bytes.chunks_exact(cr.len()).any(|unit| unit == cr) {
        return Cow::Borrowed(bytes);
    }
    // XML normalizes literal line endings. Character references and binary
    // plist strings retain their stored values.
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut units = bytes.chunks(cr.len()).peekable();
    while let Some(unit) = units.next() {
        if unit == cr {
            units.next_if(|next| *next == lf);
            normalized.extend_from_slice(lf);
        } else {
            normalized.extend_from_slice(unit);
        }
    }
    Cow::Owned(normalized)
}

fn verify(
    root: &Path,
    inspect_binary: impl FnOnce(&Path) -> Result<String, String>,
) -> Result<String, String> {
    let info = read_cask(&root.join(CASK_INFO_FILE))?;
    let cask = cask_entry(&info)?;
    let version = cask["version"]
        .as_str()
        .filter(|version| !version.is_empty())
        .ok_or("Cask version must be a nonempty string")?;
    let Some([minimum]) = cask["depends_on"]["macos"][">="]
        .as_array()
        .map(Vec::as_slice)
    else {
        return Err("Expected one minimum macOS version in the cask".into());
    };

    let contents = root.join(APPLICATION);
    let bytes = read_file(&contents.join("Info.plist"))?;
    if !recognized_plist(&bytes) {
        return Err(INVALID_PLIST.into());
    }
    let bytes = plist_snapshot(&bytes);
    let string = |key| command::plist_string(&bytes, key);
    let identifier = string("CFBundleIdentifier")?;
    if identifier.is_none() && !command::plist_dictionary(&bytes)? {
        return Err("App Info.plist must be a dictionary".into());
    }
    if identifier.as_deref() != Some("com.naviapps.Kiseki") {
        return Err("Installed bundle identifier must be com.naviapps.Kiseki".into());
    }
    if string("CFBundleShortVersionString")?.as_deref() != Some(version) {
        return Err(format!(
            "Installed bundle version must match cask version {version}"
        ));
    }
    if string("SUFeedURL")?.as_deref() != Some(UPDATE_FEED) {
        return Err(format!(
            "Installed SUFeedURL must match the public update feed {UPDATE_FEED}"
        ));
    }
    let observed = string("LSMinimumSystemVersion")?;
    let observed = macos_version(observed.as_deref())?;
    let minimum = minimum.as_str().ok_or(INVALID_MINIMUM)?;
    if observed != macos_version(Some(minimum))? {
        return Err(format!(
            "Installed bundle must match the cask's macOS {minimum} minimum"
        ));
    }
    if string("CFBundleExecutable")?.as_deref() != Some(EXECUTABLE) {
        return Err("Installed CFBundleExecutable must be Kiseki".into());
    }
    let binary = contents.join("MacOS").join(EXECUTABLE);
    if !binary.is_file() {
        return Err("Installed bundle executable is missing".into());
    }
    let architectures = inspect_binary(&binary)?;
    if !["arm64", "x86_64"]
        .into_iter()
        .all(|required| architectures.split(whitespace).any(|arch| arch == required))
    {
        return Err("Installed Kiseki executable must contain arm64 and x86_64 slices".into());
    }
    Ok(version.into())
}

pub(crate) fn run(runner_temp: Option<&Path>) -> Result<(), String> {
    let root =
        runner_temp.ok_or("RUNNER_TEMP must hold the cask metadata and test app directory")?;
    let version = verify(root, command::architectures)
        .map_err(|error| format!("Installed bundle verification failed: {error}"))?;
    println!("Verified Kiseki {version} bundle metadata and executable architectures.");
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests;
