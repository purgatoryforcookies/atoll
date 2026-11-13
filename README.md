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
- **Permission prompts.** With the optional hook, the island says "Needs you" in amber while Claude Code waits for your approval.
- **Macs without a notch.** On a screen without a notch, Atoll floats as a small pill under the menu bar that grows the same way. It follows the menu bar when displays are connected or disconnected.

## Get it

You need macOS, [Rust](https://rustup.rs) and [Claude Code](https://claude.com/claude-code).

1. **Clone:** `git clone https://github.com/purgatoryforcookies/atoll.git && cd atoll`
2. **Build and install** it as `~/Applications/Atoll.app`, with the commands in [INSTALL.md](docs/INSTALL.md#2-build-and-install-the-app).
3. **Point Claude Code at it** by adding `"env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:8788" }` to `~/.claude/settings.json`, plus the optional hook for permission prompts ([INSTALL.md](docs/INSTALL.md#3-point-claude-code-at-atoll)).

Start a new Claude Code session and watch the notch. [INSTALL.md](docs/INSTALL.md) also covers updating, uninstalling and configuration; [DEVELOP.md](docs/DEVELOP.md) covers running from source, the demo scenes and tests.

## Privacy

The relay only listens on `127.0.0.1` and forwards requests to the upstream API as they are, including your API key or login. Atoll reads the streamed responses to animate the island and keeps nothing on disk.

## Credits

- It is built on [Rune](https://github.com/anomalyco/rune), the Rust GPU UI framework by [Anomaly](https://github.com/anomalyco), created by Kit Langton with Luke Parker, Dax Raad, James Long and [other contributors](https://github.com/anomalyco/rune/graphs/contributors). Rune is MIT licensed. It draws the island, runs its springs and places the window over the notch.
- It uses [ureq](https://github.com/algesten/ureq) for the upstream connection, [serde_json](https://github.com/serde-rs/json) for parsing the stream and [objc2](https://github.com/madsmtm/objc2) for the macOS screen APIs.

## License

MIT. See [LICENSE](LICENSE).
