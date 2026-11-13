# Atoll

**A Dynamic Island for Claude Code, living in your MacBook's notch.**

Stop flipping back to the terminal to see if Claude is done. Atoll shows what Claude is doing, what it's changing and what it's costing, right where your eyes already are, and tells you the moment it's your turn.

<p align="center"><img src="docs/card.png" width="461" alt="The island opened into a card: Writing, +38 −6, the files read and edited, 4s, 59% context, 1.6k output tokens, $0.056"></p>

| | |
| --- | --- |
| Thinking | <img src="docs/thinking.png" width="461" alt="Thinking, with a violet dot and waveform"> |
| Editing | <img src="docs/editing.png" width="461" alt="Edit, with a blue dot, +17 −3 and a waveform"> |
| Your turn | <img src="docs/your-turn.png" width="461" alt="Your turn, with a green dot and +42 −7"> |
| Rate limited | <img src="docs/rate-limit.png" width="461" alt="Back in 1m 33s, with a draining ring"> |

No plugins, no extension, no account: one setting points Claude Code at a relay on your own machine, and Atoll animates what streams through it.

## How it works

It runs a small relay on localhost that passes every Claude API call through unchanged and watches the stream as it goes by. While Claude answers, the island grows two wings around the notch. The left wing has a dot coloured by what Claude is doing and a label saying what that is: thinking, writing, or the name of the tool it called. The right wing has a waveform that moves with the stream, and beside it the lines Claude's edits add and remove (`+42 −7`), counting up as each edit lands. When the turn ends, the island folds into a compact pill that says it's your turn.

- **Click to expand.** Click the island while Claude works and it opens into a card with the tool's argument, how long the turn has run, context used, output, the turn's cost, the model and effort, and what the session has cost so far. Click again to fold it back.
- **Several sessions.** Each Claude Code session going through the relay gets a small pill along the bottom of the island. The lit pill shows which session the numbers belong to.
- **Rate limits.** When the API turns a request away for a rate limit, the island says when the limit lifts and a ring drains down to that time.
- **Permission prompts.** With the hook below, the island says "Needs you" in amber while Claude Code waits for your approval.
- **Macs without a notch.** On a screen without a notch, Atoll floats as a small pill under the menu bar that grows the same way. It follows the menu bar when displays are connected or disconnected.

## Requirements

- macOS, ideally a MacBook with a notch
- Rust 1.99.1 (`rust-toolchain.toml` selects it under rustup)
- Claude Code

[Rune](https://github.com/anomalyco/rune) comes from git, pinned in `Cargo.toml`; it isn't the `rune` crate on crates.io.

## Run

```sh
cargo run --release
```

Then start Claude Code pointed at the relay:

```sh
ANTHROPIC_BASE_URL=http://127.0.0.1:8788 claude
```

To make that permanent, set it in `~/.claude/settings.json` (all projects) or in a project's `.claude/settings.local.json`:

```json
{ "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:8788" } }
```

While this is set, Claude Code can't connect unless Atoll is running. Quit Atoll from its menu bar menu (⌘Q) or with `pkill -x notch`.

### Install as an app

To keep Atoll running without a terminal and start it at login, wrap the build in an app bundle in `~/Applications`:

```sh
APP=~/Applications/Atoll.app
cargo build --release
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/notch "$APP/Contents/MacOS/notch"
cp icon.icns "$APP/Contents/Resources/icon.icns"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Atoll</string>
  <key>CFBundleDisplayName</key><string>Atoll</string>
  <key>CFBundleIdentifier</key><string>dev.atoll.app</string>
  <key>CFBundleExecutable</key><string>notch</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.0.1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP"
open "$APP"
```

Then tick **Open at Login** in its menu bar menu. macOS only allows that for an installed app; run from `cargo`, the tick stays off.

To update, quit Atoll (`pkill -x notch`) and run the same commands again.

### "Needs you" on permission prompts

Nothing is sent to the API while Claude Code waits for you to approve something, so the relay can't see a permission prompt on its own. Add a `Notification` hook to tell it:

```json
{
  "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:8788" },
  "hooks": {
    "Notification": [
      {
        "matcher": "permission_prompt",
        "hooks": [{ "type": "command", "command": "curl -s -m 1 -X POST http://127.0.0.1:8788/notch -d @- >/dev/null" }]
      }
    ]
  }
}
```

The island stays amber until the next request.

## Configuration

The binary, environment variables and hook endpoint still use the project's working name, `notch`.

| Variable         | Default                     | Effect                                                    |
| ---------------- | --------------------------- | --------------------------------------------------------- |
| `NOTCH_PORT`     | `8788`                      | Port the relay listens on                                 |
| `NOTCH_UPSTREAM` | `https://api.anthropic.com` | API the relay forwards to                                 |
| `NOTCH_PILL`     | unset                       | `1` shows the floating pill even on a screen with a notch |

These are read from the shell, so they don't reach the app when it is opened from Finder.

## Develop

Scripted scenes play a turn without the relay, so Claude Code can't reach the port while one runs:

```sh
cargo run --release -- --scene demo    # a scripted turn
cargo run --release -- --scene pill    # the same, as a floating pill
cargo run --release -- --scene card    # with the card open
cargo run --release -- --scene limit   # waiting out a rate limit
cargo test
```

The screenshots in `docs/` are headless renders of the scenes, for example:

```sh
cargo run --release -- --scene card --snap docs/card.png --at 4.5 --scale 2
```

The single-line shots are taken the same way (`demo` at 0.6, 3 and 6 seconds, `limit` at 2) and cropped to their top 72 pixels.

The app icon is drawn by `icon.swift`. To redraw it:

```sh
swift icon.swift && iconutil -c icns icon.iconset && rm -r icon.iconset
```

## Privacy

The relay only listens on `127.0.0.1` and forwards requests to the upstream API as they are, including your API key or login. Atoll reads the streamed responses to animate the island and keeps nothing on disk.

## Credits

- It is built on [Rune](https://github.com/anomalyco/rune), the Rust GPU UI framework by [Anomaly](https://github.com/anomalyco), created by Kit Langton with Luke Parker, Dax Raad, James Long and [other contributors](https://github.com/anomalyco/rune/graphs/contributors). Rune is MIT licensed. It draws the island, runs its springs and places the window over the notch.
- It uses [ureq](https://github.com/algesten/ureq) for the upstream connection, [serde_json](https://github.com/serde-rs/json) for parsing the stream and [objc2](https://github.com/madsmtm/objc2) for the macOS screen APIs.

## License

MIT. See [LICENSE](LICENSE).
