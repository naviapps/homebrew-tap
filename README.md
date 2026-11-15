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

## Packaging maintenance

See [the maintainer guide](docs/MAINTENANCE.md) for release metadata, local checks,
and CI installation and cleanup validation.

For app issues, use [Kiseki support](https://github.com/naviapps/kiseki/issues).
For billing or private account details, email support@kisekiapp.com.
