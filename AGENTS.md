<!-- tallow -->

# Atoll

A macOS menu-bar app that turns the MacBook notch into a Dynamic Island for Claude Code. A localhost relay forwards every Claude API call unchanged and reads the streamed answer; the island animates from what it reads.

## Layout

- `src/main.rs`: app state, the event loop (`setup`, `poll`), the island's shape and drawing (`view`).
- `src/proxy.rs`: the relay. HTTP/1.1 on `127.0.0.1`, forwards upstream byte for byte, parses server-sent events into `Ev`s, and takes the permission hook on `POST /atoll`.
- `src/screen.rs`: which screen, where the notch is, the window's place; the AppKit glue (pointer, menu bar item, Open at Login).
- `src/card.rs`, `src/expanded.rs`: what the expanded card says, and how it draws.
- `src/chat.rs`: one Claude Code session's state. `src/ledger.rs`: token prices.
- `src/demo.rs`: the scripted stream behind `--scene`. `src/tests.rs`: all tests.
- `icon.swift` draws `icon.icns`. `scripts/bundle.sh` builds and signs `Atoll.app`; `.github/workflows/` runs the checks and, on a `v*` tag, the signed release.

## Docs

- [DEVELOP.md](docs/DEVELOP.md): running from source, the scenes, tests and clippy, the Rune pin, screenshots, the icon.
- [INSTALL.md](docs/INSTALL.md): building and installing from source, Claude Code settings, configuration.
- `README.md` is the pitch and a short "get it" for users; detail goes in `docs/`.

Before finishing a change, run `cargo test` and `cargo clippy --all-targets`.

## Dependencies

- [Rune](https://github.com/anomalyco/rune) comes from git, pinned by `rev` in `Cargo.toml` to a fork (`purgatoryforcookies/rune`) carrying one fix not yet upstream. The fork exists for a suspected Rune bug: the pointer sampling meant for overlay windows reset the click-through of an app window adopted as a nonactivating panel, as the island is, undoing the click-through Atoll sets. The same commit adds `on_displays_changed`, so the island can follow screens and the menu bar as they change. Change the rev deliberately and run the tests and every scene after; point it back at `anomalyco/rune` once the fix lands there.
- Don't add a dependency for what a few lines or the standard library can do.

## Changes

- The relay never changes a request or response, never logs or stores headers (they carry the user's API key or login), and only listens on `127.0.0.1`.
- A parsing change gets a test in `src/tests.rs` built from a real stream excerpt.
- Keep Rust files under 800 lines; `main.rs` is close, so move the area you touch into its own module before adding to it.
- macOS-only code lives in `screen.rs` behind `#[cfg(target_os = "macos")]`, with a no-op for other targets so tests build anywhere. Every `unsafe` block gets a `// SAFETY:` comment.
- Keep the README's feature list, INSTALL.md's env var table and DEVELOP.md's scenes in step with the code.

## Performance

The island sits on screen all day, so idle must cost nothing.

- `poll` returns true only when visible state changed. When nothing streams and nothing moves, no frames are drawn.
- Start slow work (the relay, status item, observers) from `setup` or `on_ready`, after the first frame.
- The relay streams through; it never buffers a whole response.

## Attribution

Atoll began as an example app in Rune and is built on it: Rune draws the island, runs its springs and places the window over the notch. Rune is the Rust GPU UI framework by [Anomaly](https://github.com/anomalyco), created by Kit Langton with Luke Parker, Dax Raad, James Long and [other contributors](https://github.com/anomalyco/rune/graphs/contributors), MIT licensed. Keep this credit, and the one in the README, when editing either file.
