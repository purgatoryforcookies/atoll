<p align="center">
  <img src="docs/icon.png" width="128" alt="Atoll icon">
</p>

<h1 align="center">Atoll</h1>

<p align="center">
  <strong>A Dynamic Island for Claude Code, living in your MacBook's notch.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS-black?logo=apple" alt="macOS">
  <img src="https://img.shields.io/badge/built_with-Rust-orange?logo=rust" alt="Rust">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
</p>

<p align="center">
  <img src="docs/card.png" width="461" alt="The island opened into a card: Writing, +38 −6, the tools called, 4s, 59% context, 1.6k output tokens this turn and since launch, each costing $0.056">
</p>

Stop flipping back to the terminal to check if Claude is done. Atoll shows what Claude is doing, what it's changing and what it's costing, right where your eyes already are, and tells you the moment it's your turn.

No plugins, no extension, no account: one setting points Claude Code at a relay on your own machine.

## At a glance

| State            | Island                                                                                      |
| ---------------- | ------------------------------------------------------------------------------------------- |
| **Thinking**     | <img src="docs/thinking.png" width="461" alt="Thinking, with a violet dot and waveform">    |
| **Editing**      | <img src="docs/editing.png" width="461" alt="Edit, with a blue dot, +17 −3 and a waveform"> |
| **Your turn**    | <img src="docs/your-turn.png" width="461" alt="Your turn, with a green dot and +42 −7">     |
| **Rate limited** | <img src="docs/rate-limit.png" width="461" alt="Back in 1m 33s, with a draining ring">      |
| **External display** | <img src="docs/external.png" width="461" alt="On a screen without a notch: a floating pill reading Edit, +17 −3 and a waveform"> |

## Features

- **Live activity.** A coloured dot and label for thinking, writing or the tool being called, a waveform that moves with the stream, and a running `+42 −7` count of lines added and removed.
- **Click to expand.** Opens a card with the turn's todo step or recent tool calls, turn time, context used, the turn's output tokens and the total since launch with what each cost, then model, effort and files.
- **Several sessions.** Each Claude Code session gets a pill along the bottom; the lit one owns the numbers. A session quiet for 15 minutes drops off the row.
- **Rate limits.** Shows when the limit lifts, with a ring draining down to it.
- **Permission prompts.** With the optional hook, the island turns amber and says "Needs you".
- **Macs without a notch.** Floats as a small pill under the menu bar and follows it across displays.

## How it works

Atoll runs a small relay on `127.0.0.1` that passes every Claude API call through unchanged and reads the streamed answer as it goes by. The island animates from what it reads, then folds into a compact pill when the turn ends.

## Get it

Requires macOS 13 or later and [Claude Code](https://claude.com/claude-code).

1. **Download** the `.dmg` from the [latest release](https://github.com/purgatoryforcookies/atoll/releases/latest), open it and drag Atoll onto Applications. Or [build it from source](docs/INSTALL.md).
2. **Point Claude Code at it** in `~/.claude/settings.json`, plus the optional permission hook ([INSTALL.md](docs/INSTALL.md#3-point-claude-code-at-atoll)):

   ```json
   "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:8788" }
   ```

3. Start a new Claude Code session and watch the notch.

See [INSTALL.md](docs/INSTALL.md) for updating, uninstalling and configuration, and [DEVELOP.md](docs/DEVELOP.md) for running from source, demo scenes and tests.

## Privacy

The relay only listens on `127.0.0.1` and forwards requests upstream as they are, including your API key or login. Atoll reads the streamed responses to animate the island and keeps nothing on disk.

## Credits

- Built on [Rune](https://github.com/anomalyco/rune), the Rust GPU UI framework by [Anomaly](https://github.com/anomalyco), created by Kit Langton with Luke Parker, Dax Raad, James Long and [other contributors](https://github.com/anomalyco/rune/graphs/contributors). Rune is MIT licensed. It draws the island, runs its springs and places the window over the notch.
- Uses [ureq](https://github.com/algesten/ureq) for the upstream connection, [serde_json](https://github.com/serde-rs/json) for parsing the stream and [objc2](https://github.com/madsmtm/objc2) for the macOS screen APIs.

## License

MIT. See [LICENSE](LICENSE).
