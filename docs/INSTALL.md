# Installing Atoll

This builds Atoll from source. To just use it, download the `.dmg` from the [latest release](https://github.com/purgatoryforcookies/atoll/releases/latest) instead, then do [step 3](#3-point-claude-code-at-atoll).

## Requirements

- macOS 13 or later, ideally a MacBook with a notch
- [Rust](https://rustup.rs) via rustup; `rust-toolchain.toml` picks the right version (1.99.1) on first build
- [Claude Code](https://claude.com/claude-code)

## 1. Get the code

```sh
git clone https://github.com/purgatoryforcookies/atoll.git
cd atoll
```

## 2. Build and install the app

This builds Atoll and installs it as `~/Applications/Atoll.app`:

```sh
scripts/bundle.sh
open ~/Applications/Atoll.app
```

The first build takes a few minutes. The app is signed ad hoc, which is enough to run it on the Mac that built it. Atoll has no Dock icon; it shows up as a small capsule in the menu bar. Tick **Open at Login** in that menu to start it with your Mac.

## 3. Point Claude Code at Atoll

Add this to `~/.claude/settings.json` (all projects) or a project's `.claude/settings.local.json`:

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

Merge it with what's already there. Start a new Claude Code session and the island comes alive.

- `env` sends Claude Code's API calls through Atoll. While it's set, Claude Code can't connect unless Atoll is running.
- The `Notification` hook is optional. Nothing reaches the API while Claude Code waits for your approval, so this hook is how the island learns to show "Needs you" in amber. It stays amber until the next request.

To try it for one session without touching settings: `ANTHROPIC_BASE_URL=http://127.0.0.1:8788 claude`.

## Updating

```sh
git pull
pkill -x notch
```

Then run step 2 again.

## Uninstalling

Untick **Open at Login**, quit Atoll from its menu (⌘Q), delete `~/Applications/Atoll.app`, and remove the `ANTHROPIC_BASE_URL` and hook entries from your Claude Code settings.

## Configuration

The binary, environment variables and hook endpoint still use the project's working name, `notch`.

| Variable         | Default                     | Effect                                                    |
| ---------------- | --------------------------- | --------------------------------------------------------- |
| `NOTCH_PORT`     | `8788`                      | Port the relay listens on                                 |
| `NOTCH_UPSTREAM` | `https://api.anthropic.com` | API the relay forwards to                                 |
| `NOTCH_PILL`     | unset                       | `1` shows the floating pill even on a screen with a notch |

These are read from the shell, so they only apply when you start the binary from a terminal (`NOTCH_PORT=9000 ~/Applications/Atoll.app/Contents/MacOS/notch`), not when the app is opened from Finder or at login. If you change the port, change it in the Claude Code settings too.
