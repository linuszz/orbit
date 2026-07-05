# orbit

> Orbit — A universal terminal workspace. Orbit any machine, command every process.
>
> 环绕任何机器，掌控一切进程。

Orbit is a next-generation terminal workspace that unifies human command-line
interaction with AI agent execution environments. It is built on a
client-server architecture (the `orbit` TUI client and the `orbitd` daemon)
inspired by tmux, extended with first-class agent runtime, multi-protocol
image rendering over SSH, OSC 52 clipboard bridging, and a built-in file
transfer channel.

**Status:** Phase 1 — *Mercury* (pre-alpha). Skeleton compiles; functionality
not yet implemented.

## Repository layout

Design specifications live in the separate `02_design/` sibling directory
(one level up from this repo). This repository contains only the
implementation.

```
orbit/
├── Cargo.toml              workspace root
├── crates/
│   ├── orbit/              binary: TUI client (Ground Station)
│   ├── orbitd/             binary: daemon (Core)
│   ├── orbit-protocol/     lib: shared wire types (IPC contract)
│   └── orbit-core/         lib: domain model + VT emulation (no tokio)
├── justfile                convenience build tasks
└── claude.md               project context for AI-assisted development
```

## Quick start

```bash
cargo build --workspace            # build all crates
cargo run -p orbitd                # start the daemon
cargo run -p orbit                 # attach the TUI client
cargo test --workspace             # run the test suite
cargo clippy --workspace -- -D warnings   # lint gate
```

If `just` is installed: `just dev`, `just daemon`, `just qa`.

## Phases

| Phase | Codename  | Scope                                                |
|-------|-----------|------------------------------------------------------|
| 1     | Mercury   | Session/Pane TUI + PTY + IPC + SSH attach (this repo)|
| 2     | Venus     | Agent detection, state machine, Monitor sidebar      |
| 3     | Earth     | OSC 52 clipboard, Kitty/Sixel/iTerm images, files    |
| 4     | Mars      | WASM plugin system, MCP client integration           |

See `claude.md` for the full project context, terminology mapping, and
implementation conventions.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
