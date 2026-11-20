# Navi Apps Homebrew Tap

Official Navi Apps packaging for [Kiseki](https://kisekiapp.com), a gesture app
for mouse and trackpad. Requires macOS 26 or later on Apple silicon or Intel Macs.

## Install

With [Homebrew](https://brew.sh) installed:

```sh
brew install --cask naviapps/tap/kiseki
```

Open Kiseki from Applications and follow its Accessibility permission guide.
Homebrew installs the same signed and notarized disk image available on the
[Kiseki releases page](https://github.com/naviapps/kiseki/releases).

For plans, license activation, and purchases, see [Kiseki billing help](https://kisekiapp.com/billing).

## Updates and removal

Kiseki checks for updates through Sparkle. Homebrew marks it as `auto_updates`,
so ordinary `brew upgrade` does not also update it. To explicitly update through
Homebrew:

```sh
brew update
brew upgrade --cask --greedy naviapps/tap/kiseki
```

Before uninstalling, turn off Launch at Login in Kiseki if enabled. To free a Pro
activation, deactivate the license in Kiseki before uninstalling.
To uninstall the app while keeping its settings and Keychain license record:

```sh
brew uninstall --cask naviapps/tap/kiseki
```

To also move Kiseki's stored gesture configuration, recovery backups, preferences,
caches, cookies and saved window state to the Trash:

```sh
brew uninstall --cask --zap naviapps/tap/kiseki
```

Both commands ask a running Kiseki to quit normally so it can save pending edits.
`--zap` leaves exported preset files, the Keychain license record, macOS permission
grants and other apps' data alone. It does not deactivate a license or release an
activation. To clean up after an ordinary uninstall, use the same zap command with
`--force`.

If you already installed Kiseki manually, keep using its built-in updater, or quit
Kiseki and remove only its application bundle before installing through Homebrew.
Do not force-install over an existing copy.

## Support

For app issues, use [Kiseki support](https://github.com/naviapps/kiseki/issues).
For billing or private account details, email support@kisekiapp.com.

## Packaging maintenance

After publishing a new stable Kiseki release, update `version` and `sha256` in
`Casks/kiseki.rb` using the final, stapled public DMG. Keep a versioned URL and
verify it against the published `SHA256SUMS`. Do not replace existing release
assets. `brew livecheck --cask naviapps/tap/kiseki` checks the latest stable release.

Review `mise.toml`, then prepare and run the pinned static checks from this checkout:

```sh
mise trust mise.toml
mise install
mise run check
```

`check` uses dprint, actionlint with ShellCheck, and Ruff for the Python helpers.
It does not need Homebrew or Python. On macOS with Homebrew, check the cask's style:

```sh
mise run lint:cask
```

`lint:cask` checks `./Casks/kiseki.rb` using Homebrew's style rules with auto-update disabled.
The `cask:python` task pins Python separately for the verification helpers that run
in the hosted macOS job.

```sh
mise run test:helpers
```

This tests the release checksum, installed bundle metadata, and cleanup fixtures
without network access, Homebrew or the current user's Library. CI verifies exact
production cleanup paths, refuses existing or symlinked fixture destinations, and
checks ordinary uninstall and zap separately. On macOS the tests also use the
selected Swift toolchain to exercise Ed25519 verification with public fixtures.

Release validation additionally uses:

```sh
brew audit --strict --online --cask naviapps/tap/kiseki
brew fetch --cask naviapps/tap/kiseki
brew --cache --cask naviapps/tap/kiseki
```

Run these commands against the modified checkout at `brew --repository naviapps/tap`;
Homebrew does not audit a separate clone by file path. CI taps the proposed checkout.
To verify the versioned release manifest and an already downloaded DMG without
installing the cask, save the metadata from the active tap and run the checksum helper:

```sh
cask_info=$(mktemp)
HOMEBREW_NO_AUTO_UPDATE=1 brew info --json=v2 --cask naviapps/tap/kiseki > "$cask_info"
mise run cask:python scripts/verify-published-checksum.py \
  --cask-info "$cask_info" --dmg /path/to/Kiseki.dmg --verify-appcast
```

`--verify-appcast` checks the same tag's Sparkle feed against the release manifest
and verifies that its update points to the Cask's versioned DMG. With `--dmg`, it
also compares the downloaded file size with the update enclosure. Omit both
options to check the cask's release URL and published checksum alone.
With `--dmg`, add `--sparkle-bundle /path/to/Kiseki.app` to verify the update
signature using the supplied app's public key. Use a copy of the app whose code
signature has been verified. This check uses CryptoKit via the macOS Swift
toolchain.
Inspect `brew generate-zap naviapps/tap/kiseki` suggestions against the app's actual storage
before changing cleanup paths: name matches can include QA apps, tests and CI
workers. Keep paths specific to the production bundle ID; do not use bundle-prefix
wildcards, remove shared directories or delete Keychain records with a script.

CI checks the proposed checkout and versioned Sparkle feed against the published
checksum manifest. Before installing, it verifies the checksum and Sparkle file
size for the DMG fetched by Homebrew, validates its stapled ticket, and assesses
the disk image with Gatekeeper. After installing into a temporary app directory,
CI checks the bundle identifier, version, minimum macOS version, configured
Sparkle update feed, and both required slices in the executable named by the
bundle. It separately verifies the installed app's signature and Gatekeeper
status, then uses the app's public key to verify the Sparkle signature over the
original DMG. CI checks that
ordinary uninstall preserves fixture data and explicit zap removes only the
declared app data.
Cleanup fixtures run only on disposable GitHub-hosted runners; Kiseki is never
launched. Do not run zap against an existing installation as a packaging test.
This repository contains packaging metadata only. Its MIT license does not apply
to the Kiseki application, which is covered by [its own terms](https://kisekiapp.com/terms).
