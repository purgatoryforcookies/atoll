# Developing Atoll

See [INSTALL.md](INSTALL.md) for requirements. For how the code is laid out and the rules for changing it, see [AGENTS.md](../AGENTS.md).

## Run from source

```sh
cargo run --release
ANTHROPIC_BASE_URL=http://127.0.0.1:8788 claude   # in another terminal
```

Quit an installed Atoll first (`pkill -x notch`); only one can hold the port. Quit with ⌘Q from the menu bar item or `pkill -x notch`. "Open at Login" stays off when run from `cargo`; macOS only allows it for an installed app.

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

## Rune

[Rune](https://github.com/anomalyco/rune) draws the island. It comes from git, pinned by `rev` in `Cargo.toml`; it isn't the `rune` crate on crates.io. The pin points at a fork, `purgatoryforcookies/rune`, which carries one fix not yet upstream: `on_displays_changed`, and a nonactivating panel keeping its click-through. Point it back at `anomalyco/rune` once that lands.

## Screenshots

The images in `docs/` are headless renders of the scenes:

```sh
cargo run --release -- --scene card --snap docs/card.png --at 4.5 --scale 2
```

The single-line shots are taken the same way (`demo` at 0.6, 3 and 6 seconds, `limit` at 2) and cropped to their top 72 pixels.

## Icon

`icon.swift` draws the app icon. To redraw it:

```sh
swift icon.swift && iconutil -c icns icon.iconset && rm -r icon.iconset
```
