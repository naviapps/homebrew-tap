# Navi Apps Homebrew Tap

Official Navi Apps packaging for [Kiseki](https://kisekiapp.com), a mouse gesture
app for macOS 26 and later on Apple silicon and Intel Macs.

## Install

With [Homebrew](https://brew.sh) installed:

```sh
brew install --cask naviapps/tap/kiseki
```

Open Kiseki from Applications and follow its Accessibility permission guide.
Homebrew installs the same signed and notarized disk image available on the
[Kiseki releases page](https://github.com/naviapps/kiseki/releases).

Free includes global gesture rules and three active app profiles, with unlimited
saved profiles. Pro licensing and purchases remain with [Kiseki](https://kisekiapp.com/#pricing).

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

## Maintenance

After publishing a new stable Kiseki release, update `version` and `sha256` in
`Casks/kiseki.rb` using the final, stapled public DMG. Keep a versioned URL and
verify it against the published `SHA256SUMS`. Do not replace existing release
assets. `brew livecheck --cask naviapps/tap/kiseki` checks the latest stable release.

Validate a change with:

```sh
brew style --cask naviapps/tap/kiseki
brew audit --strict --online --cask naviapps/tap/kiseki
brew fetch --cask naviapps/tap/kiseki
```

Run these commands against the modified checkout at `brew --repository naviapps/tap`;
Homebrew does not audit a separate clone by file path. CI taps the proposed checkout.
Inspect `brew generate-zap naviapps/tap/kiseki` suggestions against the app's actual storage
before changing cleanup paths: name matches can include QA apps, tests and CI
workers. Keep paths specific to the production bundle ID; do not use bundle-prefix
wildcards, remove shared directories or delete Keychain records with a script.

CI checks the proposed checkout against the published checksum manifest, installs
into a temporary app directory, and verifies app metadata, both architectures,
signatures and Gatekeeper acceptance. It then checks that ordinary uninstall
preserves fixture data and explicit zap removes only the declared app data.
Cleanup fixtures run only on disposable GitHub-hosted runners; Kiseki is never
launched. Do not run zap against an existing installation as a packaging test.
This repository contains packaging metadata only. Its MIT license does not apply
to the Kiseki application, which is covered by [its own terms](https://kisekiapp.com/terms).

For app issues, use [Kiseki support](https://github.com/naviapps/kiseki/issues).
For billing or private account details, email support@kisekiapp.com.
