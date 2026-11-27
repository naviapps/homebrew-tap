use std::{
    io::{self, Read, Write},
    path::Path,
    process::{Command, Stdio},
};

use crate::release::{Cask, Enclosure, whitespace};

fn curl_command(release: &str, name: &str, maximum: usize) -> Command {
    let mut command = Command::new("/usr/bin/curl");
    command
        .args([
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
        ])
        .arg((maximum + 1).to_string())
        .args(["--", &format!("{release}/{name}")]);
    command
}

fn read_download(command: &mut Command, name: &str, maximum: usize) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not download published {name}: {error}"))?;
    let stdout = child.stdout.take().expect("download output is piped");
    let mut bytes = Vec::new();
    let read = stdout.take(maximum as u64 + 1).read_to_end(&mut bytes);
    let oversized = bytes.len() > maximum;
    if oversized || read.is_err() {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|error| format!("Could not finish downloading published {name}: {error}"))?;
    if oversized || status.code() == Some(63) {
        return Err(format!("Published {name} exceeds the size limit"));
    }
    if status.code() == Some(1) {
        return Err(format!("Published {name} must be transferred over HTTPS"));
    }
    read.map_err(|error| format!("Could not read published {name}: {error}"))?;
    if !status.success() {
        return Err(format!("Could not download published {name}: {status}"));
    }
    Ok(bytes)
}

pub(crate) fn download(release: &str, name: &str, maximum: usize) -> Result<Vec<u8>, String> {
    read_download(&mut curl_command(release, name, maximum), name, maximum)
}

fn plist_output(info: &[u8], arguments: &[&str]) -> io::Result<Option<Vec<u8>>> {
    let mut child = Command::new("/usr/bin/plutil")
        .args(arguments)
        .args(["-o", "-", "--", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let written = child
        .stdin
        .take()
        .expect("plist input is piped")
        .write_all(info);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Ok(None);
    }
    written?;
    Ok(Some(output.stdout))
}

pub(crate) fn plist_string(info: &[u8], key: &str) -> Result<Option<String>, String> {
    plist_output(info, &["-extract", key, "raw", "-expect", "string", "-n"])
        .map_err(|error| format!("Could not read signed bundle metadata: {error}"))?
        .map(|bytes| {
            String::from_utf8(bytes).map_err(|_| "Signed bundle metadata is not UTF-8".to_owned())
        })
        .transpose()
}

pub(crate) fn plist_dictionary(info: &[u8]) -> Result<bool, String> {
    // Ask whether a named key can be written at the root of the stdin snapshot.
    // plutil emits the temporary result only to captured stdout.
    plist_output(info, &["-replace", "CFBundleIdentifier", "-bool", "true"])
        .map(|output| output.is_some())
        .map_err(|error| format!("Could not read installed bundle metadata: {error}"))
}

pub(crate) fn architectures(binary: &Path) -> Result<String, String> {
    let output = Command::new("/usr/bin/lipo")
        .arg("-archs")
        .arg(binary)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| {
            format!("Could not inspect installed executable architectures: {error}")
        })?;
    if !output.status.success() {
        return Err(format!(
            "Could not inspect installed executable architectures: {}",
            output.status
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| "Installed executable architectures are not UTF-8".to_owned())
}

fn crypto_command(dmg: &Path, digest: &str, key: &str, enclosure: &Enclosure) -> Command {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/verify-sparkle-signature.swift");
    let mut command = Command::new("/usr/bin/xcrun");
    command
        .args(["swift", "-warnings-as-errors"])
        .arg(script)
        .arg(dmg)
        .args([digest, key, &enclosure.signature, &enclosure.length]);
    command
}

fn verify_bundle(bundle: &Path, version: &str, build: &str) -> Result<String, String> {
    let bytes = crate::read_file(&bundle.join("Contents/Info.plist"))?;
    let key = plist_string(&bytes, "SUPublicEDKey")?
        .filter(|text| !text.trim_matches(whitespace).is_empty())
        .ok_or("The signed bundle must provide a Sparkle public key in SUPublicEDKey")?;
    if plist_string(&bytes, "CFBundleShortVersionString")?.as_deref() != Some(version) {
        return Err(format!(
            "Signed bundle marketing version must match cask version {version}"
        ));
    }
    if plist_string(&bytes, "CFBundleVersion")?.as_deref() != Some(build) {
        return Err(format!(
            "Signed bundle build must match appcast build {build}"
        ));
    }
    Ok(key)
}

pub(crate) fn verify_signed_image(
    dmg: &Path,
    bundle: &Path,
    cask: &Cask,
    enclosure: &Enclosure,
) -> Result<(), String> {
    let key = verify_bundle(bundle, &cask.version, &enclosure.build)?;
    // Check the digest, length and signature against the same bytes in CryptoKit.
    let result = crypto_command(dmg, &cask.digest, &key, enclosure)
        .status()
        .map_err(|error| {
            format!("Could not start Sparkle update signature verification: {error}")
        })?;
    if !result.success() {
        return Err("Sparkle update signature verification failed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
