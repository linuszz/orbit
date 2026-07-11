# PROJECT KNOWLEDGE BASE

**Generated:** 2026-07-11
**Commit:** dc18bc5 (tabs + deadlock fix)
**Branch:** main

## OVERVIEW
Terminal workspace multiplexer (tmux heritage) + first-class agent runtime. Rust workspace: 4 crates (orbit client, orbitd daemon, orbit-protocol IPC, orbit-core VT emulation).

## STRUCTURE
```
orbit/
├── Cargo.toml / justfile
├── crates/
│   ├── orbit/          # TUI client (Ground Station)
│   ├── orbitd/         # Daemon (Core) — tokio async
│   ├── orbit-protocol/ # IPC wire types — NO tokio
│   └── orbit-core/     # VT/CellGrid — NO tokio
└── 02_design/          # Design specs (READ-ONLY)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| IPC types/contract | `orbit-protocol/src/messages.rs` | Wire contract — source of truth |
| Protocol encode/decode | `orbit-protocol/src/encoding.rs` | bincode 2.x serde helpers |
| TUI state + events | `orbit/src/app.rs` / `events.rs` | `App::update()` only mutation path |
| Server session/PTY | `orbitd/src/session.rs` / `pty.rs` | Tab management lives here |
| Client IPC writer | `orbit/src/ipc.rs` | Background channel → socket task |
| VT emulation | `orbit-core/src/vt/` | Cell grid, escape sequences |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|-------|
| `ClientMessage` | enum | `orbit-protocol/src/messages.rs` | IPC request contract |
| `ServerEvent` | enum | `orbit-protocol/src/messages.rs` | IPC event contract |
| `Cell` | struct | `orbit-protocol/src/types.rs` | 16 bytes — DO NOT grow |
| `App` | struct | `orbit/src/app.rs` | TUI state — single source of truth |
| `SessionState` | struct | `orbitd/src/session.rs` | Server session + tabs |

## CONVENTIONS
- **Prefix key**: `Ctrl+B` tmux-style, intercepted before PTY
- **Aerospace metaphor**: Space/Pane/Agent in code; Deck/Port/Satellite in brand
- **OKLch dark theme**: Orange accent `#d97706`; NO blue
- **3 button states**: Default/Hover/Active only
- **No emoji**: Unicode symbols only
- **TOML only**: No YAML config
- **tokio-free libs**: `orbit-protocol` + `orbit-core` — NO `tokio` dep

## ANTI-PATTERNS (THIS PROJECT)
- **NEVER** `as any` / `@ts-ignore` / `unwrap()` in library public APIs
- **NEVER** suppress clippy without justifying comment
- **NEVER** busy-loop render — redraw on demand (`needs_redraw`)
- **NEVER** hold `RwLock.write()` across `await` that reads same lock (self-deadlock)
- **NEVER** add heap fields to `Cell` (16 byte invariant)
- **NEVER** touch PTY from client — all server-side

## UNIQUE STYLES
- Aerospace metaphor branding: "Orbit/Ground Station/Core", "Port/Deck/Satellite"
- OKLch color tokens over RGB
- Command Palette via `Flight Deck` (prefix key opens)
- Server-side tabs: `TabId`, `new_tab`/`switch_tab`/`close_tab` survive reconnect

## COMMANDS
```bash
nix-shell -p gcc --run "cargo build --workspace"     # build (gcc for NixOS)
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
nix-shell -p gcc --run "cargo test --workspace"
nix-shell -p gcc --run "cargo fmt --all --check"
just dev       # run client
just daemon    # run daemon
just qa        # fmt + clippy + test
```

## NOTES
- `orbit-protocol` + `orbit-core` are tokio-free for unit test isolation
- Client never touches PTY — server owns all PTYs
- Both sides run VT parsers (accepted 2x CPU tradeoff)
- `Cell` must stay 16 bytes — grid clone ~160KB
- Socket path: `$XDG_RUNTIME_DIR/orbit.sock` → `$TMPDIR/orbit-<uid>.sock`
- Protocol: length-prefixed bincode (4MB max)
- Async lock rule: scope write guards tight; release before any `await` that reads same state
