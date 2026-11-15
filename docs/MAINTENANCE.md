# Packaging maintenance

For installation, updates, and removal, see the [README](../README.md).

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

This uses the pinned Python to test cleanup fixtures in temporary directories,
without Homebrew or the current user's Library. CI runs these tests before cask
validation. Seeding requires the exact reviewed production cleanup paths and
refuses existing paths or symlinked destinations. Verification rejects changed
cleanup paths and fixture files replaced by symlinks; an empty cleanup list cannot
turn a failed uninstall or zap into a passing check.

Release validation additionally uses:

```sh
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
