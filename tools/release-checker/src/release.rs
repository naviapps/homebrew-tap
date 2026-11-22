use roxmltree::{Document, Node};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fmt::Write,
    fs::File,
    io::{self, Read},
    path::Path,
};

pub(crate) const DMG: &str = "Kiseki.dmg";
pub(crate) const APPCAST: &str = "appcast.xml";
pub(crate) const MANIFEST: &str = "SHA256SUMS";
const RELEASES: &str = "https://github.com/naviapps/kiseki/releases/download";
const SPARKLE: &str = "http://www.andymatuschak.org/xml-namespaces/sparkle";
const MAXIMUM_MANIFEST_BYTES: usize = 64 * 1024;
const MAXIMUM_APPCAST_BYTES: usize = 1024 * 1024;
// The previous verifier used Python's default decimal conversion limit.
pub(crate) const MAXIMUM_DECIMAL_DIGITS: usize = 4300;

pub(crate) struct Cask {
    pub(crate) version: String,
    pub(crate) digest: String,
    release: String,
    url: String,
}

pub(crate) struct Enclosure {
    pub(crate) length: String,
    pub(crate) signature: String,
    pub(crate) build: String,
}

pub(crate) struct Request<'a> {
    pub(crate) cask_info: &'a Value,
    pub(crate) dmg: Option<&'a Path>,
    pub(crate) verify_appcast: bool,
    pub(crate) bundle: Option<&'a Path>,
}

pub(crate) fn decimal(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn version_number(text: &str) -> bool {
    let parts = text.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| *part == "0" || positive_decimal(part))
}

fn positive_decimal(text: &str) -> bool {
    !text.starts_with('0') && decimal(text)
}

pub(crate) fn cask_entry(info: &Value) -> Result<&Value, String> {
    let Some([entry]) = info["casks"].as_array().map(Vec::as_slice) else {
        return Err("Expected one Homebrew cask in brew info JSON".into());
    };
    if !entry.is_object() {
        return Err("Expected one Homebrew cask in brew info JSON".into());
    }
    if entry["full_token"].as_str() != Some("naviapps/tap/kiseki") {
        return Err("Expected the naviapps/tap/kiseki cask".into());
    }
    Ok(entry)
}

fn cask(info: &Value) -> Result<Cask, String> {
    let entry = cask_entry(info)?;
    let version = entry["version"]
        .as_str()
        .filter(|text| version_number(text))
        .ok_or("Expected a stable Kiseki version in X.Y.Z form")?;
    let release = format!("{RELEASES}/v{version}");
    let url = format!("{release}/{DMG}");
    if entry["url"].as_str() != Some(&url) {
        return Err("Cask URL must point to Kiseki.dmg on its exact GitHub release tag".into());
    }
    let digest = entry["sha256"]
        .as_str()
        .filter(|text| {
            text.len() == 64
                && text
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        })
        .ok_or("Cask must declare a lowercase SHA-256 digest")?;
    Ok(Cask {
        version: version.into(),
        digest: digest.into(),
        release,
        url,
    })
}

pub(crate) fn whitespace(ch: char) -> bool {
    // Keep the Unicode whitespace accepted by the preceding verifier.
    ch.is_whitespace() || matches!(ch, '\u{001c}'..='\u{001f}')
}

fn manifest_digests(
    contents: &[u8],
    include_appcast: bool,
) -> Result<(String, Option<String>), String> {
    let malformed = "Malformed published SHA256SUMS entry";
    let contents = str::from_utf8(contents).map_err(|_| malformed)?;
    let separators = [
        '\n', '\r', '\x0b', '\x0c', '\x1c', '\x1d', '\x1e', '\u{0085}', '\u{2028}', '\u{2029}',
    ];
    let mut dmg = Vec::new();
    let mut appcast = Vec::new();
    for line in contents.split(separators) {
        let line = line.trim_start_matches(whitespace);
        if line.is_empty() {
            continue;
        }
        let (digest, name) = line.split_once(whitespace).ok_or(malformed)?;
        let name = name.trim_start_matches(whitespace);
        if digest.len() != 64
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            || name.is_empty()
        {
            return Err(malformed.into());
        }
        match name.strip_prefix('*').unwrap_or(name) {
            DMG => dmg.push(digest),
            APPCAST if include_appcast => appcast.push(digest),
            _ => {}
        }
    }
    let one = |entries: &[&str], name| match entries {
        [digest] => Ok(digest.to_ascii_lowercase()),
        _ => Err(format!(
            "Published SHA256SUMS must contain exactly one {name} entry"
        )),
    };
    let dmg = one(&dmg, DMG)?;
    let appcast = include_appcast
        .then(|| one(&appcast, APPCAST))
        .transpose()?;
    Ok((dmg, appcast))
}

fn plain(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && node.tag_name().namespace().is_none_or(str::is_empty)
}

fn only_child<'a, 'input>(
    node: Node<'a, 'input>,
    condition: impl Fn(Node<'a, 'input>) -> bool,
) -> Option<Node<'a, 'input>> {
    let mut matches = node.children().filter(|node| condition(*node));
    match (matches.next(), matches.next()) {
        (Some(child), None) => Some(child),
        _ => None,
    }
}

fn leading_text(node: Node<'_, '_>) -> String {
    node.children()
        .take_while(|child| !child.is_element())
        .filter(Node::is_text)
        .filter_map(|child| child.text())
        .collect()
}

fn appcast_enclosure(contents: &[u8], cask: &Cask) -> Result<Enclosure, String> {
    let malformed = "Malformed published appcast.xml";
    let text = str::from_utf8(contents).map_err(|_| malformed)?;
    if text.contains("<!DOCTYPE") {
        return Err("Published appcast.xml must not contain a document type declaration".into());
    }
    let feed = Document::parse(text).map_err(|_| malformed)?;
    let root = feed.root_element();
    let channel = only_child(root, |node| plain(node, "channel"))
        .filter(|_| plain(root, "rss"))
        .ok_or("Published appcast.xml must contain one RSS channel")?;
    let item = only_child(channel, |node| plain(node, "item"))
        .ok_or("Published appcast.xml must contain one release item")?;
    if only_child(item, |node| {
        node.has_tag_name((SPARKLE, "shortVersionString"))
    })
    .is_none_or(|node| leading_text(node) != cask.version)
    {
        return Err(format!(
            "Appcast marketing version must match cask version {}",
            cask.version
        ));
    }
    let build = only_child(item, |node| node.has_tag_name((SPARKLE, "version")))
        .map(leading_text)
        .filter(|value| positive_decimal(value))
        .ok_or("Appcast must contain one numeric Sparkle build number")?;
    let enclosure = only_child(item, |node| plain(node, "enclosure"))
        .filter(|node| node.attribute("url") == Some(&cask.url))
        .ok_or("Appcast must reference Kiseki.dmg on its exact GitHub release tag")?;
    let signature = enclosure
        .attribute((SPARKLE, "edSignature"))
        .unwrap_or("")
        .trim_matches(whitespace);
    if signature.is_empty() {
        return Err("Appcast must include Sparkle update signature metadata".into());
    }
    let length = enclosure
        .attribute("length")
        .filter(|value| positive_decimal(value))
        .ok_or("Appcast must declare a positive disk-image length")?;
    if length.len() > MAXIMUM_DECIMAL_DIGITS {
        return Err("Appcast disk-image length exceeds the decimal digit limit".into());
    }
    Ok(Enclosure {
        length: length.into(),
        signature: signature.into(),
        build,
    })
}

fn hex_digest(digest: &[u8]) -> String {
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(text, "{byte:02x}").expect("writing to a String succeeds");
    }
    text
}

fn disk_image(dmg: &Path) -> io::Result<(String, u64)> {
    let mut file = File::open(dmg)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            let size = file.metadata()?.len();
            return Ok((hex_digest(&hasher.finalize()), size));
        }
        hasher.update(&buffer[..count]);
    }
}

pub(crate) fn verify(
    request: Request<'_>,
    mut download: impl FnMut(&str, &str, usize) -> Result<Vec<u8>, String>,
    verify_signature: impl FnOnce(&Path, &Path, &Cask, &Enclosure) -> Result<(), String>,
) -> Result<Cask, String> {
    if request.bundle.is_some() && request.dmg.is_none() {
        return Err("Sparkle signature verification requires a local DMG".into());
    }
    let cask = cask(request.cask_info)?;
    let check_appcast = request.verify_appcast || request.bundle.is_some();
    let manifest = download(&cask.release, MANIFEST, MAXIMUM_MANIFEST_BYTES)?;
    let (digest, appcast_digest) = manifest_digests(&manifest, check_appcast)?;
    if digest != cask.digest {
        return Err("Cask checksum differs from published SHA256SUMS".into());
    }
    let mut enclosure = None;
    if let Some(expected) = appcast_digest {
        let contents = download(&cask.release, APPCAST, MAXIMUM_APPCAST_BYTES)?;
        if hex_digest(&Sha256::digest(&contents)) != expected {
            return Err("Published appcast.xml checksum differs from SHA256SUMS".into());
        }
        enclosure = Some(appcast_enclosure(&contents, &cask)?);
    }
    if let Some(dmg) = request.dmg {
        let (actual, size) = disk_image(dmg)
            .map_err(|error| format!("Could not read {}: {error}", dmg.display()))?;
        if actual != cask.digest {
            return Err("Local DMG checksum differs from published SHA256SUMS".into());
        }
        if let Some(enclosure) = &enclosure {
            if size.to_string() != enclosure.length {
                return Err("Local DMG length differs from the published appcast enclosure".into());
            }
            if let Some(bundle) = request.bundle {
                verify_signature(dmg, bundle, &cask, enclosure)?;
            }
        }
    }
    Ok(cask)
}

#[cfg(test)]
mod tests;
