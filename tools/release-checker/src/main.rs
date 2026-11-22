#![forbid(unsafe_code)]

use std::{env, ffi::OsString, fs, path::PathBuf, process::ExitCode};

mod command;
mod release;

const USAGE: &str = "Usage: kiseki-cask-release-checker [--cask-info PATH] [--dmg PATH] [--verify-appcast] [--sparkle-bundle PATH]\n\
Verify the cask against its exact public release.\n\n\
  --cask-info PATH       JSON from brew info --json=v2 --cask naviapps/tap/kiseki\n\
                         Defaults to kiseki-cask.json in RUNNER_TEMP\n\
  --dmg PATH             Also verify an already downloaded DMG\n\
  --verify-appcast       Also verify the release's Sparkle appcast metadata\n\
  --sparkle-bundle PATH  Match the appcast to the signed bundle and verify the signature. Requires --dmg.";

#[derive(Default)]
struct Arguments {
    cask_info: PathBuf,
    dmg: Option<PathBuf>,
    verify_appcast: bool,
    bundle: Option<PathBuf>,
}

fn parse_arguments(
    mut args: std::iter::Peekable<impl Iterator<Item = OsString>>,
    runner_temp: Option<OsString>,
) -> Result<Option<Arguments>, String> {
    let mut arguments = Arguments::default();
    let mut cask_info = None;
    while let Some(argument) = args.next() {
        if argument == "-h" {
            return Ok(None);
        }
        let Some(arg) = argument.to_str() else {
            return Err(format!("Unrecognized argument: {:?}", argument));
        };
        let (name, inline) = arg
            .split_once('=')
            .map_or((arg, None), |(name, value)| (name, Some(value)));
        let name = [
            "--help",
            "--cask-info",
            "--dmg",
            "--sparkle-bundle",
            "--verify-appcast",
        ]
        .into_iter()
        .find(|flag| name.len() > 2 && flag.starts_with(name))
        .unwrap_or(name);
        match name {
            "--help" if inline.is_none() => return Ok(None),
            "--cask-info" | "--dmg" | "--sparkle-bundle" => {
                let argument = inline
                    .map(OsString::from)
                    .or_else(|| args.next_if(|next| !next.as_encoded_bytes().starts_with(b"-")))
                    .ok_or_else(|| format!("{name} requires a path"))?;
                let path = PathBuf::from(argument);
                match name {
                    "--cask-info" => cask_info = Some(path),
                    "--dmg" => arguments.dmg = Some(path),
                    _ => arguments.bundle = Some(path),
                }
            }
            "--verify-appcast" if inline.is_none() => arguments.verify_appcast = true,
            _ => return Err(format!("Unrecognized argument: {arg}")),
        }
    }
    arguments.cask_info = cask_info
        .or_else(|| {
            runner_temp
                .filter(|path| !path.is_empty())
                .map(|path| PathBuf::from(path).join("kiseki-cask.json"))
        })
        .ok_or("pass --cask-info or set RUNNER_TEMP")?;
    Ok(Some(arguments))
}

fn run(arguments: Arguments) -> Result<(), String> {
    let contents = fs::read(&arguments.cask_info)
        .map_err(|error| format!("Could not read {}: {error}", arguments.cask_info.display()))?;
    let info = serde_json::from_slice(&contents)
        .map_err(|error| format!("Could not parse Homebrew cask JSON: {error}"))?;
    let cask = release::verify(
        release::Request {
            cask_info: &info,
            dmg: arguments.dmg.as_deref(),
            verify_appcast: arguments.verify_appcast,
            bundle: arguments.bundle.as_deref(),
        },
        command::download,
        command::verify_signature,
    )?;
    let artifact = if arguments.dmg.is_some() {
        " and local DMG"
    } else {
        ""
    };
    println!(
        "Verified Kiseki {} cask{artifact} against published SHA256SUMS ({}).",
        cask.version, cask.digest
    );
    if arguments.verify_appcast || arguments.bundle.is_some() {
        println!(
            "Verified published {} checksum and release metadata.",
            release::APPCAST
        );
    }
    if arguments.bundle.is_some() {
        println!(
            "Verified the Sparkle update signature over the DMG using the supplied bundle's public key."
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse_arguments(
        env::args_os().skip(1).peekable(),
        env::var_os("RUNNER_TEMP"),
    ) {
        Ok(Some(arguments)) => match run(arguments) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Ok(None) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{USAGE}\n{error}");
            ExitCode::from(2)
        }
    }
}
