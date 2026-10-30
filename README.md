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

To uninstall the app while keeping its settings and license data:

```sh
brew uninstall --cask naviapps/tap/kiseki
```

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

CI checks the proposed checkout, installs into a temporary app directory, verifies
Gatekeeper acceptance and uninstalls without launching Kiseki or deleting user data.
This repository contains packaging metadata only. Its MIT license does not apply
to the Kiseki application, which is covered by [its own terms](https://kisekiapp.com/terms).

For app issues, use [Kiseki support](https://github.com/naviapps/kiseki/issues).
For billing or private account details, email support@kisekiapp.com.
