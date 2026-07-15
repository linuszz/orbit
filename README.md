# Orbit

A terminal multiplexer with integrated agent monitoring. Built in Rust.

Orbit provides session management, pane splitting, and tabbed workspaces similar
to tmux, with the addition of automatic detection and monitoring for AI coding
agents (Claude Code, Codex, Aider, and others) running inside your sessions.

## Features

**Session management**
- Client-server architecture: sessions persist across client disconnects
- Multi-space support: isolated workspaces with independent tabs and panes
- Horizontal and vertical pane splitting with mouse-drag resizing
- Tab management with drag-to-reorder
- Mouse text selection, copy, and scrollback navigation
- Command palette with fuzzy search (tmux-style prefix key)

**Agent monitoring**
- Automatic detection via process scanning and PTY output pattern matching
- Status tracking: working, blocked, error, idle, completed
- Live resource metrics (CPU, memory)
- Progress extraction from agent output
- Intervention modal: respond to blocked agents without leaving the terminal

**Display**
- Built-in themes: Orbit (default) and Tokyo Night
- Runtime theme switching
- Full mouse support: focus, resize, select, context menus

## Quick start

```bash
git clone https://github.com/linuszz/orbit.git
cd orbit
cargo run -p orbitd    # terminal 1: start the daemon
cargo run -p orbit     # terminal 2: attach the TUI client
```

Requires Rust 1.75+ and Linux or macOS.

## Key bindings

| Key | Action |
|-----|--------|
| `Ctrl+B` | Command palette |
| `Ctrl+B` `h` / `v` | Split horizontal / vertical |
| `Ctrl+B` `c` | New tab |
| `Ctrl+B` `n` / `p` | Next / previous tab |
| `Ctrl+B` `←` `→` `↑` `↓` | Navigate between panes |
| `Ctrl+B` `[` | Scrollback mode (`j`/`k`/`g`/`G`) |
| `Ctrl+B` `a` | Toggle agent panel |
| `Ctrl+B` `b` | Toggle sidebar |
| `Ctrl+B` `T` | Toggle theme |
| `Ctrl+B` `?` | Help |
| `Tab` | Cycle pane focus |

Mouse: click to focus, drag to resize or reorder, right-click for context menu.

## Architecture

```
┌───────────┐   Unix socket   ┌───────────┐
│  orbit    │ ◄──────────────►│  orbitd   │
│  (client) │   bincode 2.x   │ (daemon)  │
└───────────┘                 └───────────┘
```

The client (`orbit`) is a ratatui-based TUI. The daemon (`orbitd`) owns all
PTYs, manages sessions, and performs agent detection. Communication uses
length-prefixed bincode over a Unix domain socket. Both sides maintain
independent VT parsers.

```
crates/
├── orbit/           # TUI client
├── orbitd/          # Daemon
├── orbit-protocol/  # IPC wire types (no tokio dependency)
└── orbit-core/      # VT emulation and cell grid (no I/O)
```

## Roadmap

| Status | Focus |
|--------|-------|
| Done | Terminal workspace: panes, tabs, PTY, IPC |
| Done | Agent detection, monitoring, intervention |
| Planned | Clipboard sync (OSC 52), image rendering, file transfer |
| Planned | Plugin system (WASM), MCP client integration |

## Development

```bash
just qa                 # format check + clippy + tests
just dev / just daemon  # run client / daemon
```

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## License

Orbit is dual-licensed:

- **AGPL-3.0** for open-source and community use. See [LICENSE](LICENSE).
- **Commercial license** available on request for organizations that cannot
  comply with AGPL terms (e.g., embedding Orbit in a closed-source product or
  offering it as a managed service without disclosing modifications). Contact
  the maintainer.

Contributions require signing the [CLA](CLA.md). This allows the project to
remain dual-licensed as it evolves.
