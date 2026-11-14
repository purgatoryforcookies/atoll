# Developing Atoll

See [INSTALL.md](INSTALL.md) for requirements. For how the code is laid out and the rules for changing it, see [AGENTS.md](../AGENTS.md).

## Run from source

```sh
cargo run --release
ANTHROPIC_BASE_URL=http://127.0.0.1:8788 claude   # in another terminal
```

Quit an installed Atoll first (`pkill -x atoll`); only one can hold the port. Quit with ⌘Q from the menu bar item or `pkill -x atoll`. "Open at Login" stays off when run from `cargo`; macOS only allows it for an installed app.

## Scenes

Scripted scenes play a turn without the relay, so Claude Code can't reach the port while one runs:

```sh
cargo run --release -- --scene demo    # a scripted turn
cargo run --release -- --scene pill    # the same, as a floating pill
cargo run --release -- --scene card    # with the card open
cargo run --release -- --scene limit   # waiting out a rate limit
```

## Checks

```sh
cargo test
cargo clippy --all-targets
```

CI (`.github/workflows/ci.yml`) runs both on every push to `main` and every pull request, with clippy warnings as errors.

## Releasing

Push a version tag and `.github/workflows/release.yml` tests, builds a universal (Apple silicon and Intel) `Atoll.app` with `scripts/bundle.sh`, signs it with your Developer ID, packs it into a disk image with an Applications shortcut, signs, notarizes and staples the image, and publishes `Atoll-<version>.dmg` as a GitHub release. The tag sets the app's version. To cut one, from a clean `main`:

```sh
scripts/release.sh patch   # or minor, major
```

It bumps the version past the last `v*` tag in `Cargo.toml` and `Cargo.lock`, has `claude -p` draft release notes from the commits since that tag (or lists the commits if `claude` isn't installed), opens them in `$EDITOR` for you to fix, then commits, tags with the notes and pushes. The release uses the tag's notes. Save an empty file to abort.

Only this workflow needs an Apple developer account. Everything else, including `scripts/bundle.sh`, signs ad hoc and needs no setup. It needs these repository secrets (Settings → Secrets and variables → Actions):

| Secret                       | What                                                                                                                                      |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `MACOS_CERTIFICATE`          | Your **Developer ID Application** certificate and its private key, exported from Keychain Access as `.p12`, then `base64 -i cert.p12 \| pbcopy` |
| `MACOS_CERTIFICATE_PASSWORD` | The password you gave the `.p12` export                                                                                                   |
| `APPLE_ID`                   | The Apple ID email of your developer account                                                                                              |
| `APPLE_TEAM_ID`              | Your 10-character team ID, at developer.apple.com → Membership                                                                            |
| `APPLE_APP_PASSWORD`         | An app-specific password from account.apple.com → Sign-In and Security                                                                   |

## Rune

[Rune](https://github.com/anomalyco/rune) draws the island. It comes from git, pinned by `rev` in `Cargo.toml`; it isn't the `rune` crate on crates.io. The pin points at a fork, `purgatoryforcookies/rune`, which carries one fix not yet upstream: `on_displays_changed`, and a nonactivating panel keeping its click-through. Point it back at `anomalyco/rune` once that lands.

## Screenshots

The images in `docs/` are headless renders of the scenes:

```sh
cargo run --release -- --scene card --snap docs/card.png --at 4.5 --scale 2
```

The single-line shots are taken the same way (`demo` at 0.6, 3 and 6 seconds, `limit` at 2) and cropped to their top 72 pixels. `external.png` is `pill` at 3 seconds, cropped to its top 118.

## Icon

`icon.swift` draws the app icon. To redraw it:

```sh
swift icon.swift && iconutil -c icns icon.iconset && rm -r icon.iconset
```

The README's header image is a PNG of the same icon:

```sh
sips -s format png -Z 256 icon.icns --out docs/icon.png
```
