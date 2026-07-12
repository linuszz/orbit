# PROJECT KNOWLEDGE BASE

**Generated:** 2026-07-12
**Commit:** 72b4371 (agent fleet + multi-space + clipboard)
**Branch:** main

## OVERVIEW
Terminal workspace multiplexer (tmux heritage) + first-class agent runtime. Rust workspace: 4 crates (orbit client, orbitd daemon, orbit-protocol IPC, orbit-core VT emulation). Phase 1 (Mercury) + Phase 2 (Venus agent runtime) in progress.

## STRUCTURE
```
orbit/
├── Cargo.toml / justfile
├── crates/
│   ├── orbit/          # TUI client (Ground Station)
│   │   └── src/
│   │       ├── app.rs / events.rs / ipc.rs / main.rs
│   │       └── tui/
│   │           ├── mod.rs / theme.rs
│   │           └── widgets/           # 9 widget files
│   │               ├── agent_monitor.rs     # Satellites panel
│   │               ├── eclipse_modal.rs     # Blocked-agent intervention
│   │               ├── launch_modal.rs      # Agent type picker
│   │               ├── spaces_sidebar.rs    # Multi-space sidebar
│   │               ├── command_palette.rs   # Flight Deck overlay
│   │               └── status_bar.rs
│   ├── orbitd/         # Daemon (Core) — tokio async
│   │   └── src/
│   │       ├── main.rs / session.rs / pty.rs / ipc.rs
│   │       └── agent.rs               # AgentRegistry + detection + Eclipse
│   ├── orbit-protocol/ # IPC wire types — NO tokio
│   └── orbit-core/     # VT/CellGrid — NO tokio
└── 02_design/          # Design specs (READ-ONLY)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| IPC types/contract | `orbit-protocol/src/messages.rs` | Wire contract — source of truth (28 variants) |
| Protocol encode/decode | `orbit-protocol/src/encoding.rs` | bincode 2.x serde helpers |
| TUI state + events | `orbit/src/app.rs` / `events.rs` | `App` struct: tabs, spaces, agents, modals |
| Server session/PTY | `orbitd/src/session.rs` / `pty.rs` | Tab management, PTY spawn |
| **Agent runtime** | `orbitd/src/agent.rs` | Detection, Eclipse, metrics — `AgentRegistry` |
| Client IPC writer | `orbit/src/ipc.rs` | Background channel → socket task |
| VT emulation | `orbit-core/src/vt/` | Cell grid, escape sequences |
| **Satellites panel UI** | `orbit/src/tui/widgets/agent_monitor.rs` | Card rendering, pulse animations |
| **Eclipse modal UI** | `orbit/src/tui/widgets/eclipse_modal.rs` | Blocked-agent intervention overlay |
| **Multi-space sidebar** | `orbit/src/tui/widgets/spaces_sidebar.rs` | Space cards, fleet badge |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|-------|
| `ClientMessage` | enum | `orbit-protocol/src/messages.rs` | IPC request contract |
| `ServerEvent` | enum | `orbit-protocol/src/messages.rs` | IPC event contract |
| `Cell` | struct | `orbit-protocol/src/types.rs` | 16 bytes — DO NOT grow |
| `App` | struct | `orbit/src/app.rs` | TUI state: tabs, spaces, agents, modals, selection, scroll |
| `SessionState` | struct | `orbitd/src/session.rs` | Server session + tabs + agent wiring |
| `AgentRegistry` | struct | `orbitd/src/agent.rs` | Agent detection, state machine, metrics |
| `AgentRegistry::watch_pane()` | fn | `orbitd/src/agent.rs` | Per-pane async task: /proc scan + PTY scan |
| `EclipseModalState` | struct | `orbit/src/app.rs` | Blocked agent intervention modal state |
| `LaunchModalState` | struct | `orbit/src/app.rs` | Agent launcher picker state |

## CONVENTIONS
- **Prefix key**: `Ctrl+B` tmux-style, intercepted before PTY
- **Aerospace metaphor**: Space/Pane/Agent in code; Deck/Port/Satellite in brand
- **OKLch dark theme**: Orange accent `#d97706`; NO blue
- **3 button states**: Default/Hover/Active only
- **No emoji**: Unicode symbols only (`●○◎◉◌×≡▸`)
- **TOML only**: No YAML config
- **tokio-free libs**: `orbit-protocol` + `orbit-core` — NO `tokio` dep
- **Agent panel keys**: `j`/`k` scroll, `r` restart Error agent, `d` dismiss, `n` launch new
- **Animation**: Lerp-based pulse (Working=slow orange, Blocked=fast gold, Error=red blink)
- **Agent sort**: Blocked-first, then Working, then others; stable on updates

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
- **Satellite Eclipse**: PTY output pattern scanning detects blocked agents (40+ block patterns)
- **Dual agent detection**: `/proc` child scanning (500ms) + PTY output scanning (event-driven)
- **Agent metrics**: CPU% (tick delta), RSS (VmRSS), progress% (regex from output)
- **Multi-space**: Adjective-noun naming generator, per-space agent fleet

## COMMANDS
```bash
nix-shell -p gcc --run "cargo build --workspace"     # build (gcc for NixOS)
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
nix-shell -p gcc --run "cargo test --workspace"
nix-shell -p gcc --run "cargo fmt --all --check"
just dev       # run client
just daemon    # run daemon
just qa        # fmt-check + clippy + test
```

## NOTES
- `orbit-protocol` + `orbit-core` are tokio-free for unit test isolation
- Client never touches PTY — server owns all PTYs
- Both sides run VT parsers (accepted 2x CPU tradeoff)
- `Cell` must stay 16 bytes — grid clone ~160KB
- Socket path: `$XDG_RUNTIME_DIR/orbit.sock` → `$TMPDIR/orbit-<uid>.sock`
- Protocol: length-prefixed bincode (4MB max)
- Async lock rule: scope write guards tight; release before any `await` that reads same state
- Agent detection: `AgentRegistry::watch_pane()` scans last 256 bytes of PTY output for block patterns
- Agent names matched: `claude`, `codex`, `aider`, `gh-copilot`, `cursor` (+ script runners `node`/`npx`/`python`)
- PTY output is ANSI-stripped before display in agent fields (`strip_ansi`)
- `events.rs` is the largest file (1624 lines) — all key/mouse dispatch lives here
