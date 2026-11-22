#![forbid(unsafe_code)]

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

mod command;
mod installed;
mod release;

const USAGE: &str = "Usage: kiseki-cask-release-checker [--cask-info PATH] [--dmg PATH] [--verify-appcast] [--sparkle-bundle PATH]\n\
       kiseki-cask-release-checker --verify-installed\n\
Verify the cask against its exact public release or a disposable installation.\n\n\
  --cask-info PATH       JSON from brew info --json=v2 --cask naviapps/tap/kiseki\n\
                         Defaults to kiseki-cask.json in RUNNER_TEMP\n\
  --dmg PATH             Also verify an already downloaded DMG\n\
  --verify-appcast       Also verify the release's Sparkle appcast metadata\n\
  --sparkle-bundle PATH  Match the appcast to the signed bundle and verify the signature. Requires --dmg.\n\
  --verify-installed    Check the bundle and cask metadata in RUNNER_TEMP";

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

fn read_cask(path: &Path) -> Result<serde_json::Value, String> {
    let contents =
        fs::read(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_slice(&contents)
        .map_err(|error| format!("Could not parse Homebrew cask JSON: {error}"))
}

fn run(arguments: Arguments) -> Result<(), String> {
    let info = read_cask(&arguments.cask_info)?;
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

fn finish(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1).peekable();
    let runner_temp = env::var_os("RUNNER_TEMP");
    if args
        .next_if(|argument| argument == "--verify-installed")
        .is_some()
    {
        if args.next().is_some() {
            eprintln!("{USAGE}\n--verify-installed takes no other arguments");
            return ExitCode::from(2);
        }
        return finish(installed::run(runner_temp));
    }
    match parse_arguments(args, runner_temp) {
        Ok(Some(arguments)) => finish(run(arguments)),
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
