# Orbit

**A terminal workspace for local and remote machines, with built-in AI agent monitoring.**

Orbit (`orbt`) is a terminal multiplexer — like tmux — that also watches the AI coding agents running inside your sessions, lets you respond to them without leaving the terminal, and connects to remote machines over SSH with a single command. It adapts its layout automatically when the terminal is too narrow for a full desktop UI.

---

## Install

### One-liner (Linux / macOS)

```sh
curl -fsSL https://github.com/linuszz/orbt/releases/latest/download/install.sh | sh
```

### Homebrew (macOS / Linux)

```sh
brew install linuszz/orbt/orbt
```

### apt (Debian / Ubuntu)

```sh
curl -fsSL https://apt.orbt.sh/orbt.gpg.pub \
  | sudo gpg --dearmor -o /usr/share/keyrings/orbt.gpg
echo "deb [arch=amd64 signed-by=/usr/share/keyrings/orbt.gpg] https://apt.orbt.sh stable main" \
  | sudo tee /etc/apt/sources.list.d/orbt.list
sudo apt update && sudo apt install orbt
```

### AUR (Arch Linux)

```sh
yay -S orbt-bin     # prebuilt binary — fast
yay -S orbt         # build from source
```

### Scoop (Windows)

```powershell
scoop bucket add orbt https://github.com/linuszz/scoop-orbt
scoop install orbt
```

### winget (Windows)

```powershell
winget install Linus.Orbt
```

### Nix

```sh
# Run without installing
nix run github:linuszz/orbt

# Add to profile
nix profile install github:linuszz/orbt
```

### cargo

```sh
cargo install orbt
```

### Manual download

Prebuilt binaries for every platform are attached to each [GitHub Release](https://github.com/linuszz/orbt/releases/latest):

| Platform | File |
|---|---|
| Linux x86\_64 | `orbt-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `orbt-macos-aarch64.tar.gz` |
| macOS Intel | `orbt-macos-x86_64.tar.gz` |
| Windows x86\_64 | `orbt-windows-x86_64.zip` |

SHA-256 sidecar files (`.sha256`) are included for each archive.

---

## Quick start

```sh
orbt           # start (auto-starts the background daemon on first run)
orbt daemon    # run the daemon in the foreground (for servers / init systems)
```

Orbit is client-server: `orbt` spawns a background daemon (`orbt daemon`) the first time it runs. The daemon keeps all sessions alive — close the client window without losing anything.

---

## Remote connection

Connect Orbit's TUI directly to an `orbtd` daemon running on another machine:

```sh
orbt --remote user@host
orbt --remote user@host:2222     # custom port
```

Orbit opens a native SSH connection (reads your `~/.ssh/id_*` keys and `SSH_AUTH_SOCK`), forwards the remote Unix socket through the tunnel, and attaches as if the daemon were local. The remote daemon must be running (`orbt daemon` or a service unit).

---

## Mobile / narrow terminal

When the terminal is narrower than 80 columns or shorter than 25 rows, Orbit automatically switches to a compact layout designed for phones, iPads, and small terminal windows.

The compact layout has four tabs across the bottom:

| Tab | What it shows |
|---|---|
| **TERMINAL** | Full-screen PTY — the active pane |
| **SPACES** | Two-column switcher: workspaces (left) and tabs (right); tap a tab to go straight to it |
| **COMMAND** | Command palette — search and run any action |
| **AGENTS** | Agent list with status and scroll |

All keyboard shortcuts that work on desktop also work in compact mode. The layout switches back automatically when the window is resized above the threshold.

---

## Features

### Session management

- Daemon-backed sessions that survive client disconnects
- Multiple independent workspaces (spaces), each with its own tabs and panes
- Horizontal and vertical pane splitting
- Mouse-drag pane resizing and tab reordering
- Scrollback with keyboard navigation
- Detach (`Ctrl+B d`) and reattach — the daemon keeps running

### AI agent monitoring

Orbit watches the processes running inside your panes and tracks AI coding agents (Claude Code, Codex, Aider, and others) automatically.

- Status tracking: **Transmitting** (working), **Eclipse** (blocked — needs you), **Standby** (idle), **Debris** (errored), done
- Live CPU / memory metrics and progress extraction from agent output
- **Eclipse modal**: when an agent is blocked, a floating panel lets you send a response without switching away from your work
- Agent Fleet panel: a sidebar listing all active agents with their status and duration
- Status bar pulse: slow for working agents, fast amber for blocked

### Interface

- Command palette with type-to-filter search (`Ctrl+B` to open)
- Two built-in themes: **Orbit** (dark, orange accent) and **Tokyo Night** — switch at runtime with `Ctrl+B T`
- Right-click context menus on panes, tabs, and the sidebar
- Responsive layout with four breakpoints (Compact / Standard / Wide / Ultra)
- Settings panel (`Ctrl+B ,`) persisted to `~/.config/orbt/settings.toml`

---

## Key bindings

All actions are available through the command palette. The most common ones:

| Keys | Action |
|---|---|
| `Ctrl+B` | Open command palette |
| `Ctrl+B h` / `v` | Split pane horizontal / vertical |
| `Ctrl+B x` | Close active pane |
| `Ctrl+B c` | New tab |
| `Ctrl+B n` / `p` | Next / previous tab |
| `Ctrl+B [` | Scrollback mode (`j`/`k` scroll, `Esc` exit) |
| `Ctrl+B a` | Toggle agent panel |
| `Ctrl+B b` | Toggle workspace sidebar |
| `Ctrl+B d` | Detach (Go Dark — session persists) |
| `Ctrl+B T` | Cycle theme |
| `Ctrl+B ,` | Settings |

Mouse: click to focus, drag to resize, drag tabs to reorder, right-click for context menu.

---

## Architecture

```
orbt --remote user@host         orbt (local)
         │                           │
         │ SSH tunnel (russh)         │ Unix socket
         ▼                           ▼
   remote orbtd ◄────────────► local orbtd
   (daemon)                    (daemon)
```

The `orbt` binary contains both the TUI client and the daemon (as `orbt daemon`). The daemon owns all PTYs and persists independently of the client.

```
crates/
├── orbt/            # TUI client + embedded daemon
├── orbt-protocol/   # IPC wire types (no tokio, publishable lib)
└── orbt-core/       # VT emulation and cell grid (no I/O, publishable lib)
```

Communication uses length-prefixed bincode over a Unix domain socket. Both client and daemon maintain independent VT parsers — same tradeoff as tmux.

---

## Building from source

Requires Rust 1.75+ and Linux or macOS (Windows is a secondary target).

```sh
git clone https://github.com/linuszz/orbt.git
cd orbt
cargo build -p orbt --release
```

With `just` installed:

```sh
just dev      # run the TUI client
just daemon   # run the daemon
just qa       # fmt-check + clippy + tests
```

System dependencies (Linux): `pkg-config libssl-dev cmake`

---

## Roadmap

| Status | Phase | Scope |
|---|---|---|
| Done | Mercury | Terminal workspace: panes, tabs, PTY, IPC, detach/reattach |
| Done | Mercury | Remote SSH connection (`orbt --remote`) |
| Done | Venus | Agent detection, monitoring, Eclipse intervention modal |
| Done | Venus | Compact / mobile TUI layout |
| Planned | Earth | Clipboard sync over SSH (OSC 52 bridge) |
| Planned | Earth | Multi-protocol image rendering (Kitty / iTerm / Sixel) |
| Planned | Earth | File transfer channel |
| Planned | Mars | WASM plugin system and MCP client integration |

---

## License

Orbit is dual-licensed:

- **AGPL-3.0** for open-source and community use. See [LICENSE](LICENSE).
- **Commercial license** available on request for organizations that need to embed Orbit in a closed-source product or offer it as a managed service without disclosing modifications. Contact the maintainer.

Contributions require signing the [CLA](CLA.md).
