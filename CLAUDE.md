# claude.md

> Project context for AI-assisted development of **Orbit**.
> Read this before any code work on the project. The single source of truth
> for terminology, conventions, and architecture. Design rationale lives in
> the sibling `02_design/` directory.

---

## 1. What Orbit is

**Orbit** is a terminal workspace that unifies human CLI interaction with AI
agent execution environments. It is a **terminal multiplexer** (tmux/zellij
heritage) with a **first-class agent runtime** (herdr heritage) and a unique
**I/O bridge** that makes images, clipboard, and files work seamlessly over
SSH — none of which tmux/zellij/herdr provide.

**One-liner**: *Orbit — A universal terminal workspace. Orbit any machine,
command every process.*

**Key differentiators** (the product's "护城河" — do not trade these away):

- Multi-protocol image rendering over SSH (Kitty → iTerm → Sixel → Unicode blocks)
- OSC 52 clipboard bridge over SSH ("Beacon")
- Built-in file transfer channel ("Payload")
- Agent state machine with user-intervention flow ("Satellite Eclipse")
- MCP (Model Context Protocol) client integration (Phase 4+)

**Not Orbit** (explicitly out of scope):

- Not a terminal emulator (runs inside iTerm2/Kitty/WezTerm, doesn't replace them)
- Not a SaaS — local-first, all data on the user's machine
- Not an LLM — Orbit hosts and manages agents, it doesn't run inference
- Not an IDE replacement
- Not Windows-first (Linux/macOS priority)

---

## 2. Terminology (MANDATORY — use these terms in code AND docs)

Orbit uses a **space/aerospace metaphor**. The codebase uses **generic
terms** (`Space`, `Pane`, `Agent`, `Session`); the brand layer maps them to
aerospace metaphors for CLI commands and user-facing copy. Use the Orbit
term, never the generic alternative, in user-facing strings.

### 2.1 Concept mapping (canonical)

| Generic concept | Code identifier | Orbit brand term | CLI hint |
|---|---|---|---|
| Session | `Space` (`SpaceId`) | **Orbit** / Constellation | `orbit dev` attaches to dev |
| Daemon / Server | `orbitd` binary | **Core** | the space station |
| Client / TUI | `orbit` binary | **Ground Station** | the CLI binary |
| Window | (within `Space`) | **Deck** | `orbit deck list` |
| Pane | `Pane` (`PaneId`) | **Port** | `orbit port split h` |
| PTY | (within `Pane`) | — | the observation aperture |
| Agent | `Agent` (`AgentId`) | **Satellite** | `orbit sat list` |
| Agent Working | `AgentStatus::Working` | **Transmitting** | ● |
| Agent Blocked | `AgentStatus::Blocked` | **Eclipse** | ◎ (Satellite Eclipse modal) |
| Agent Idle | `AgentStatus::Idle` | **Standby** | ○ |
| Agent Error | `AgentStatus::Error` | **Debris** | ◉ |
| Agent Done | `AgentStatus::Done` | (completed) | ◌ |
| SSH attach | — | **Link-up** | `orbit --remote user@host` |
| Detach | — | **Go Dark** | session persists without client |
| Clipboard sync | (Beacon channel) | **Beacon** | `orbit beacon sync` |
| Image transfer | (Downlink channel) | **Downlink** | `orbit downlink last` |
| File transfer | (Payload channel) | **Payload** | `orbit payload up file.txt` |
| Command Palette | `Overlay::CommandPalette` | **Flight Deck** | triggered by prefix key |
| Plugin | `OrbitPlugin` trait | **Module** | `orbit module list` |
| MCP tool | — | **Instrument** | `~/.orbit/instruments/` |

**Hierarchy**: `Session/Space → Window/Deck → Pane/Port → PTY`. Mirrors tmux.

### 2.2 Why "Orbit" (not Nexus / Tether / Pulsar)

Decision locked: **Orbit** won over Nexus, Tether, Meridian, Astral, Pulsar.
Reasoning: 5 letters, 2 syllables (`/ˈɔːrbɪt/`), strong verb-ization
(`orbit dev` ≈ `ssh dev`), and a complete aerospace metaphor ecosystem to
draw from. See `02_design/NAMING.md`.

### 2.3 Phase codenames (solar-system bodies)

| Phase | Codename | Scope | Crates involved |
|---|---|---|---|
| 1 | **Mercury** | Session/Pane TUI + PTY + IPC + SSH attach | orbit, orbitd, orbit-protocol, orbit-core |
| 2 | **Venus** | Agent detection, state machine, Monitor sidebar | + agent module in orbitd |
| 3 | **Earth** | OSC 52 clipboard, multi-protocol images, file transfer | + io module |
| 4 | **Mars** | WASM plugin system (wasmtime), MCP integration | + orbit-plugin |

Release codenames: v0.1=Mercury, v0.2=Venus, v0.3=Earth, v0.4=Mars,
v0.5=Jupiter (MCP), v1.0=Sol.

---

## 3. Repository layout

```
orbit/                              # this repo — implementation only
├── Cargo.toml                      # workspace root, unified deps
├── Cargo.lock                      # TRACKED in git (application project)
├── justfile                        # convenience build tasks
├── rust-toolchain.toml             # pins stable + rustfmt + clippy
├── clippy.toml
├── .cargo/config.toml
├── .github/workflows/ci.yml        # fmt + clippy + test + build (Linux+macOS)
├── crates/
│   ├── orbit/                      # BIN: TUI client (Ground Station)
│   │   └── src/
│   │       ├── {main,app,events,ipc}.rs
│   │       └── tui/
│   │           ├── mod.rs          # layout, render, terminal setup
│   │           ├── theme.rs        # ThemeColors struct (Orbit + Tokyo Night palettes)
│   │           └── widgets/        # UI component modules
│   │               ├── agent_monitor.rs   # Agent Fleet Panel (cards, scroll, keyboard)
│   │               ├── command_palette.rs # Flight Deck (searchable, grouped, scrollable)
│   │               ├── context_menu.rs    # Right-click context menus
│   │               ├── eclipse_modal.rs   # Satellite Eclipse intervention modal
│   │               ├── launch_modal.rs    # New space launch dialog
│   │               ├── settings_modal.rs  # Settings overlay (theme, sidebar, panel)
│   │               ├── spaces_sidebar.rs  # Left sidebar (space cards, stats)
│   │               ├── status_bar.rs      # Bottom status bar (mode, agent pulse)
│   │               └── tab_bar.rs         # Top tab bar (overflow, drag reorder)
│   ├── orbitd/                     # BIN: daemon (Core)
│   │   └── src/{main,session,pty,agent,io,ipc}.rs
│   ├── orbit-protocol/             # LIB: shared wire types (IPC contract)
│   │   └── src/{lib,messages,types,encoding,error}.rs
│   └── orbit-core/                 # LIB: domain model + VT emulation (no tokio)
│       └── src/{lib,config,error,vt/}.rs
├── LICENSE                         # AGPL-3.0-only
├── CLA.md                          # Contributor License Agreement
└── CLAUDE.md                       # this file

02_design/                          # sibling — design specs (READ-ONLY reference)
├── ARCHITECTURE.md                 # high-level system architecture
├── BRAND_ORBIT.md                  # terminology + CLI command system + brand voice
├── NAMING.md                       # why "Orbit" was chosen
├── PRODUCT_ARCHITECTURE_REVIEW.md  # business-perspective review
├── UI_DESIGN_BRIEF.md              # UI/UX spec overview
├── TUI_DESIGN.md / TUI_DESIGN_MOBILE.md  # desktop + mobile TUI specs
├── 05_UI-UX-design/                # detailed Chinese-language design docs (15 files)
│   ├── 01-09: design system, components, interactions
│   └── critics/REVIEW.md
└── 06_tech-design/                 # technical design specs (8 docs + critics)
    ├── 01-tech-stack-and-workspace.md
    ├── 02-tui-architecture.md
    ├── 03-ipc-protocol.md
    ├── 04-server-architecture.md
    ├── 05-vt-emulation.md
    ├── 06-input-routing-and-modes.md
    ├── 07-agent-data-model.md
    ├── 08-scrollback-and-history.md
    └── critics/                    # v2-v5 review rounds (audit trail)
```

---

## 4. Build, test, verify

```bash
cargo build --workspace                          # debug build
cargo build --workspace --release                # release build (LTO=thin, strip=symbols)
cargo check --workspace --all-targets            # fast type-check
cargo test --workspace                           # unit + integration tests
cargo clippy --workspace --all-targets -- -D warnings   # lint gate (zero tolerance)
cargo fmt --all --check                          # format gate
```

With `just` installed: `just dev` (run client), `just daemon` (run daemon),
`just qa` (fmt-check + clippy + test).

### Toolchain

- **Rust**: stable channel (pinned via `rust-toolchain.toml`), MSRV 1.75
- **Edition**: 2021
- **License**: AGPL-3.0-only
- **Targets**: Linux + macOS first-class; Windows is a future compatibility target
- **System deps**: pkg-config, libssl-dev (Linux)

### CI

`.github/workflows/ci.yml` runs on every push/PR with a Linux + macOS matrix:
fmt → clippy → test → release build. All must pass.

---

## 5. Architecture

### 5.1 Client-server model

```
orbit (client / Ground Station)        orbitd (daemon / Core)
  crossterm events                            session manager
  → Action enum                               PTY manager (one task per pane)
  → App state update                          agent runtime
  → ratatui render                            VT parser (one per PTY, server-side)
                                              EventBus broadcast
              ▲                                 |
              | ServerEvent (bincode)           | ClientMessage (bincode)
              └─────── Unix domain socket ──────┘
                     (length-prefixed)
```

**Hard rules:**

- **Client never touches PTY.** All PTYs are owned by `orbitd`. Client only
  renders the cell grid it receives from the server.
- **Both sides run VT parsers** on the same raw PTY byte stream — this is an
  accepted tradeoff (2× CPU under heavy output) for client rendering
  independence and low latency. Same tradeoff tmux makes. See §11 below.
- **Session persists across client disconnects.** Detach (`Go Dark`) leaves
  the daemon running; reattach (`Reacquire`) re-syncs full state.

### 5.2 Crate responsibilities

| Crate | Type | Depends on tokio? | Owns |
|---|---|---|---|
| `orbit-protocol` | lib | No | `ClientMessage`, `ServerEvent`, `Capabilities`, `Cell`, `CellGrid`, `TermColor`, ID newtypes, `ProtocolError` |
| `orbit-core` | lib | **No** (pure sync) | `Config`, `VtParser`, `CellGrid` ops, `VtError`/`GridError` |
| `orbitd` | bin | Yes | session/pty/agent/io/ipc modules |
| `orbit` | bin | Yes | app state, tui (render/layout/theme/widgets), events (keyboard/mouse/drag), ipc client, settings persistence |

`orbit-protocol` and `orbit-core` MUST remain tokio-free so they can be unit
tested in isolation.

### 5.3 Data flow (input → render)

```
User keypress
  → crossterm capture
  → events::keyboard::handle() returns Action
  → App::update(action)                          # only mutation entry point
  → if needs_redraw: terminal.draw(render)
  → if pending_image_render: inject_image()      # AFTER ratatui flush

Pane output path:
  PTY bytes → orbitd spawn_blocking loop
            → VtParser.process() (updates server-side CellGrid)
            → EventBus::send(ServerEvent::PaneOutput{ raw_bytes })
  Client receives PaneOutput
    → app.spaces[i].terminal_state.parser.process(bytes)  # client-side VtParser
    → next redraw renders the updated CellGrid
```

---

## 6. Design principles (NON-NEGOTIABLE)

These apply to ALL UI work. Violations must be flagged in review.

### 6.1 No emoji anywhere

Use Unicode symbols or pure text labels. **No exceptions.**

| State / action | Symbol | Unicode |
|---|---|---|
| Working | `●` | U+25CF |
| Idle | `○` | U+25CB |
| Blocked (Eclipse) | `◎` | U+25CE |
| Error (Debris) | `◉` | U+25C9 |
| Done | `◌` | U+25CC |
| Close | `×` | U+00D7 |
| Menu | `≡` | U+2261 |
| Expand | `▸` | U+25B8 |

`[A] Agent Fleet` not `🤖 Agent Fleet`. `[View]` not `👁 View`.

### 6.2 Color system (OKLch dark theme, orange accent)

The canonical palette (from `UI_DESIGN_BRIEF.md` §3 and
`06_tech-design/02-tui-architecture.md` §6). Orange is the brand color;
do NOT regress to blue/other palettes from older design docs.

| Token | OKLch | TrueColor RGB | Usage |
|---|---|---|---|
| `BG_PRIMARY` | `oklch(15% 0.008 250)` | `#0e0e14` | main background |
| `BG_SECONDARY` | `oklch(18% 0.006 250)` | `#12121a` | secondary surface |
| `BG_TERTIARY` | `oklch(22% 0.004 250)` | `#181821` | tertiary surface |
| `BG_CARD` | `oklch(20% 0.005 250)` | `#14141d` | card background |
| `FG_PRIMARY` | `oklch(95% 0.002 250)` | `#f2f2f8` | main text |
| `FG_SECONDARY` | `oklch(75% 0.012 250)` | `#b4b4c4` | secondary text |
| `FG_MUTED` | `oklch(55% 0.018 250)` | `#78788c` | muted text, default button |
| `ACCENT` | `oklch(65% 0.18 45)` | `#d97706` | orange — active/primary |
| `ACCENT_HOVER` | `oklch(55% 0.14 45)` | `#a15600` | all button hovers |
| `ACCENT_BRIGHT` | — | `#fba028` | pulse animation bright end |
| `ACCENT_DIM` | — | `#783c00` | pulse animation dim end |
| `ACCENT_IDLE` | `oklch(58% 0.08 250)` | `#60789e` | cyan — idle |
| `ACCENT_BLOCKED` | `oklch(75% 0.15 60)` | `#d9ac00` | yellow — Blocked |
| `ACCENT_ERROR` | `oklch(65% 0.20 25)` | `#c8321e` | red — Error |
| `BORDER` | `oklch(35% 0.008 250)` | `#3c3c4c` | borders |

RGB constants live in `crates/orbit/src/tui/theme.rs` as `ThemeColors::orbit()`.
The theme system supports runtime switching via `set_theme("orbit"|"tokyo-night")`
with thread-local accessor functions (`bg_primary()`, `accent()`, etc.).

### 6.3 Button states (three only)

| State | Background | Text | Border |
|---|---|---|---|
| Default | transparent | `FG_MUTED` | `BORDER` |
| Hover | `ACCENT_HOVER` | `BG_PRIMARY` | `ACCENT` |
| Active | `ACCENT` | `BG_PRIMARY` | `ACCENT` |

### 6.4 Aesthetic

"Aerospace control console information density + IDE precision."
Precise, restrained, engineered, calm. **No gradients. No shadows. No
decorative animation.** Animation exists only for state feedback
(0.15s–0.3s transitions, slow pulse for Working, fast pulse for Blocked).

### 6.5 Redraw-on-demand (do NOT busy-loop)

The event loop must block on `tokio::select!` when idle. `terminal.draw()`
runs only when `needs_redraw == true`. A 16ms tick timer is scheduled ONLY
when at least one of these is true: (a) an agent is Working/Blocked, (b)
sidebar/modal animation is in progress, (c) a post-input transition frame
is pending. Otherwise CPU should be ~0%.

This is the v2-review F1 fix. The naive "always tick at 60fps" pattern is a
bug.

### 6.6 UI layout tree

```
┌─────────────────────────────────────────────────────────┐
│  Tab Bar (横跨全宽)                                       │
├──────────┬──────────────────────────────┬───────────────┤
│  Spaces  │                              │  Agent/       │
│  Sidebar │       Main Pane Area         │  Satellite    │
│  (全局)   │       (PTY / Terminal)       │  Monitor      │
│          │                              │  (临时边栏)    │
├──────────┴──────────────────────────────┴───────────────┤
│  Status Bar (横跨全宽)                                   │
└─────────────────────────────────────────────────────────┘
        [Floating Layer: Flight Deck / Context Menu / Eclipse Modal / Settings / Launch]
```

Layout invariants:
- Spaces Sidebar is a global container (full height, left)
- Tab Bar and Status Bar span the full width (not scoped to a Space)
- Agent Monitor is a temporary sidebar (toggle via prefix + `a`)
- Spaces Sidebar collapses to 2 cols showing space numbers

### 6.7 Responsive breakpoints

| Mode | Cols | Sidebar | Agent Panel |
|---|---|---|---|
| Ultra | ≥ 140 | expanded (14 cols) | expanded (25 cols) |
| Wide | 100–139 | expanded (14 cols) | expanded (20 cols) |
| Standard | 80–99 | animated (default 14) | animated |
| Compact | < 80 | collapsed (2 cols) | hidden (0 cols) |

---

## 7. Input model (prefix-key system)

Orbit uses a tmux-style **prefix key** so PTY programs (vim, emacs, bash,
fzf) get unrestricted use of nearly every key combination. Only the prefix
is intercepted; everything else passes through to the PTY in Normal mode.

### 7.1 Prefix key

- **Default**: `Ctrl+B` (same as tmux default, easy migration)
- **Override**: `ORBIT_PREFIX_KEY=ctrl+space` env var, or
  `prefix_key = "ctrl+b"` in `~/.orbit/config.toml` `[general]`
- Pressing the prefix enters **COMMAND mode** (a.k.a. opens the Flight Deck
  in expanded mode, or just shows a status-bar hint in minimal mode)
- Pressing the prefix again inside COMMAND mode cancels (returns to Normal)

### 7.2 Input modes (`InputMode` enum)

| Mode | Entry | Behavior |
|---|---|---|
| `Normal` | default | All keys passthrough to PTY; only prefix is intercepted |
| `CommandPalette { search, selected, search_focused }` | press prefix (expanded mode) | Searchable command list; Enter executes, Esc cancels |
| `Scroll { offset }` | prefix + `[` | Arrow keys scroll scrollback; no PTY forwarding |
| `AgentPanel { selected }` | prefix + `a` | Keyboard navigation of Agent Fleet panel (j/k/n/r/s/d) |

Note: `Prefix` mode from the design docs is now subsumed by
`CommandPalette` in expanded mode (the default). In minimal mode, the
prefix key opens the status-bar hint then processes one key — but the code
routes through `CommandPalette` with `search_focused: false`.

Eclipse modal, Settings modal, and Context Menu are NOT `InputMode` variants
— they live in overlay/state fields on `App` and intercept input on a
separate priority path before mode dispatch.

### 7.3 COMMAND mode key table (implemented)

These are registered in `COMMANDS` array (`app.rs`) and dispatched via the
Command Palette. Direct shortcut keys work when the palette is open:

| Key | Command ID | Action |
|---|---|---|
| `h` | `split_h` | Split pane horizontal |
| `v` | `split_v` | Split pane vertical |
| `x` | `close_pane` | Close active pane |
| `[` | `scroll_mode` | Enter scroll mode |
| `c` | `new_tab` | New tab |
| `n` / `p` | `next_tab` / `prev_tab` | Next / Previous tab |
| `b` | `toggle_sidebar` | Toggle spaces sidebar |
| `a` | `toggle_agent` | Toggle Agent Fleet panel (enters AgentPanel mode) |
| `d` | `detach` | Detach session (Go Dark) |
| `T` | `toggle_theme` | Cycle theme (Orbit / Tokyo Night) |
| `,` | `settings` | Open Settings modal |
| `?` | `help` | Show help overlay |
| `k` / `j` | `agent_scroll_up/down` | Scroll Agent Fleet panel |
| `Esc` | — | Cancel / close palette |

**AgentPanel mode keys** (when in `InputMode::AgentPanel`):

| Key | Action |
|---|---|
| `j` / `k` | Navigate agent cards |
| `n` | Cycle to next agent |
| `r` | Restart errored agent |
| `s` | Stop agent |
| `d` | Dismiss completed agent |
| `Esc` / `q` | Exit AgentPanel mode |

Full key tables (including Scroll / Copy modes) live in
`02_design/06_tech-design/06-input-routing-and-modes.md` §5. That document
is the design source of truth; the `COMMANDS` array in code is the
implementation source of truth.

### 7.4 Flight Deck (Command Palette)

Currently always in expanded mode. Prefix key opens the searchable command
list with grouped commands (Pane, Tab, View, Session, Satellite, Help).
Features: type-to-filter, scroll with group header accounting, mouse wheel,
click to execute, shortcut key display.

---

## 8. Key types and contracts

These are already defined in `orbit-protocol` (the IPC contract). Do not
diverge.

### 8.1 IDs (newtypes — prevents mixing)

```rust
pub struct SpaceId(pub u32);
pub struct PaneId(pub u32);
pub struct AgentId(pub u32);
pub struct ImageId(pub u32);
```

All `Copy + Hash + Eq + Serialize + Deserialize`.

### 8.2 IPC wire format

Length-prefixed bincode:

```
┌─────────────────┬─────────────────────────────────────┐
│ length: u32 LE  │ bincode payload (variable length)   │
│  (4 bytes)      │                                     │
└─────────────────┴─────────────────────────────────────┘
```

- **Max message size**: 4 MB (`MAX_MSG_BYTES` constant in
  `orbit-protocol::encoding`). Reject larger — defends against OOM.
- Typical `PaneOutput` delta < 4 KB. Full `CellGrid` snapshot ≈ 160 KB
  (200×50 cells × 16 bytes).

### 8.3 Handshake

```
client → server:  ClientMessage::Hello { client_version, protocol_version, capabilities }
server checks SO_PEERCRED UID == server UID; rejects otherwise
server → client:  ServerEvent::Welcome { server_version, protocol_version, capabilities∩, FullState }
```

If `client.protocol_version != server.protocol_version`, server replies
`ServerEvent::ProtocolError { code: 1 }` and closes.

### 8.4 `ClientMessage` and `ServerEvent`

Full variant set in `crates/orbit-protocol/src/messages.rs`. Treat as the
wire contract — adding fields requires either a `Capabilities` flag
(additive) or a `PROTOCOL_VERSION` bump (breaking).

### 8.5 Cell (16 bytes — DO NOT grow)

```rust
pub struct Cell {           // 16 bytes total
    pub ch: char,           // 4 bytes
    pub fg: TermColor,      // 4 bytes
    pub bg: TermColor,      // 4 bytes
    pub flags: CellFlags,   // 1 byte (bitfield) + 3 padding
}
```

Never add `String`/`Vec` to `Cell`. 200×50 cells × 16 B = 160 KB per pane
snapshot, ~640 KB for 4 panes — must stay clonable in < 1 ms.

### 8.6 `App` struct and `Action` enum (TUI state)

All state in a single `App` struct. **All mutations go through
`App::update(action: Action)`** — Redux-style single entry point. Field
visibility: future code should expose `App` fields as `pub(crate)` with
accessor methods, not blanket `pub` (v2 review M5). The skeleton currently
has `pub` for ergonomics; tighten as the module matures.

`Action` enum includes `Noop` (placeholder for unmatched keys — does NOT
trigger redraw) and `Tick` (drives animations; only triggers redraw when
an animation is actually running).

### 8.7 Agent state machine

```
        ┌──────────┐  task assigned   ┌──────────┐
        │  Idle    │ ───────────────► │ Working  │
        └──────────┘                  └────┬─────┘
                                           │ block detected
                                           ▼
                                      ┌──────────┐
                          ┌───────────│ Blocked  │
                          │           │ (Eclipse)│
                          │ user      └──────────┘
                          │ responds
                          ▼
        ┌──────────┐  completed      ┌──────────┐
        │  Done    │ ◄────────────── │ Working  │
        └──────────┘                 └──────────┘
```

Server-side state in `AgentState` (richer: carries `Instant`, reason
strings). Wire-side simplified to `AgentStatus` enum. Mapping via
`AgentState::to_protocol_status()` / `to_detail()`.

---

## 9. Implementation conventions

### 9.1 Error layering (v5 GAP 4)

| Crate | Error strategy |
|---|---|
| `orbit-protocol` | `thiserror` — `ProtocolError` enum (callers can `match` on `VersionMismatch`/`MessageTooLarge`/etc.) |
| `orbit-core` | `thiserror` — `VtError`, `GridError` enums |
| `orbitd` (binary) | `anyhow::Result` in `main`, propagates to exit code + log |
| `orbit` (binary) | `anyhow::Result` in `main` |

Library crates MUST NOT use `anyhow` in their public API — it erases type
info that callers need.

### 9.2 bincode 2.x (the footgun)

Message types derive ONLY `serde::Serialize, Deserialize`. **Do not**
derive `bincode::Encode`. Encode/decode through the `bincode::serde` module:

```rust
let bytes = bincode::serde::encode_to_vec(&msg, bincode::config::standard())?;
let (msg, _): (MyType, usize) =
    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
```

Helpers `encode_message` / `decode_message` already in
`orbit-protocol::encoding`.

### 9.3 interprocess 2.x

`LocalSocketStream::connect` and `LocalSocketListener::bind` take a `Name`,
not a `&str`. Convert via:

```rust
use interprocess::local_socket::GenericFilePath;
let name = socket_path.to_fs_name::<GenericFilePath>()?;
```

### 9.4 Socket path + auth (F3 fix — DO NOT regress to /tmp)

```rust
// Priority: $XDG_RUNTIME_DIR/orbit.sock → $TMPDIR/orbit-<uid>.sock
pub fn default_socket_path() -> PathBuf { ... }
```

Server checks `SO_PEERCRED` UID on every accepted connection (Linux).
macOS equivalent via `getpeereid`. Windows: Named Pipe ACL (handled by
`interprocess`). Reject any connection from a different UID.

### 9.5 VT emulation

- Library: `vte` crate (push-based parser, `Perform` trait)
- Grid ops: hand-rolled in `orbit-core::vt::grid` (we control `Cell` size)
- Server-side `VtParser` runs in `tokio::task::spawn_blocking` (CPU-bound,
  no async locks held)
- Client-side `VtParser` runs in an async task on receipt of `PaneOutput`
- Phase 1 escape sequences to support: CSI A/B/C/D/H/G/d/J/K/L/M/@/P/m/r,
  CSI ?25h/l (cursor visibility), CSI ?1049h/l (alt screen), CSI r
  (DECSTBM), OSC 0/2 (title), ESC M (RI). Full P0/P1/P2 list in
  `06_tech-design/05-vt-emulation.md` §4.

### 9.6 PTY batch window

`orbitd` PTY read loop batches reads within an 8 ms window before emitting
a `PaneOutput` event. Prevents `cat huge_file` from spamming hundreds of
events per frame.

### 9.7 Logging

`tracing` + `tracing-subscriber` with `EnvFilter`. Controlled by
`ORBIT_LOG_LEVEL` env var (default `info` for orbitd, `warn` for orbit).
Logs to `~/.orbit/logs/orbitd-<date>.log`. Never log in the PTY I/O hot
path (latency-sensitive). PTY I/O logs at `trace` level only.

### 9.8 Comments and docstrings

Be stingy with comments. Module-level `//!` doc may reference the design
doc section. Inline `//` comments only for: non-obvious algorithm steps,
security-sensitive code, performance-critical decisions, regex patterns,
bincode API footguns, `#[cfg(unix)]` gating. Self-documenting code first,
comment second. NO emoji in any string or comment.

### 9.9 Async lock scoping (deadlock prevention)

`tokio::sync::RwLock` does **not** support recursive or read-after-write
locking from the same task. Holding a `write()` guard across an `await`
that internally calls `read()` on the **same** lock will self-deadlock.

Pattern to use:

```rust
{
    let mut active = self.active_tab.write().await;
    *active = new_id;
} // drop write guard here before any await
let info = self.collect_space_info().await; // may read active_tab
```

Scope write guards as tightly as possible, especially before calling
`collect_space_info()` or any other helper that may read the same state.

---

## 10. Configuration

### 10.1 File locations (`~/.orbit/`)

```
~/.orbit/
├── config.toml              # TOML — main config
├── constellations/          # Session state (was: sessions/)
├── decks/                   # Layout templates (KDL format, Phase 2)
├── satellites/              # Agent configs (TOML, was: agents/)
├── beacons/                 # Clipboard history
├── downlinks/               # Cached images
├── instruments/             # MCP server configs (Phase 4)
├── modules/                 # Plugins (native/ + wasm/)
└── logs/                    # tracing log files
```

### 10.2 Environment variables

| Var | Effect |
|---|---|
| `ORBIT_CONFIG_DIR` | Override `~/.orbit/` |
| `ORBIT_DEFAULT_CONSTELLATION` | Default session name |
| `ORBIT_PREFIX_KEY` | Override prefix key (e.g. `ctrl+space`) |
| `ORBIT_MOBILE` | Force mobile TUI mode (Phase 4+) |
| `ORBIT_NO_BEACON` | Disable clipboard bridge |
| `ORBIT_LOG_LEVEL` | tracing filter (e.g. `debug`, `orbitd=trace`) |
| `ORBIT_INSTRUMENTS` | Comma-separated MCP server names to enable (Phase 4) |
| `ORBIT_PANE_ID` | **Set by orbitd on every PTY child** — agent detection reads this |

### 10.3 `config.toml` shape (planned full config)

```toml
[general]
prefix_key = "ctrl+b"             # default; "ctrl+space" / "ctrl+a" also supported
mouse = true
true_color = true
flight_deck_mode = "expanded"     # or "minimal"

[ui]
image_protocol = "auto"           # auto | kitty | iterm | sixel | blocks
image_max_inline_height = 20
sidebar_width = 14
agent_panel_width = 20
scrollback_lines = 10000
scrollback_persistence = false    # Phase 2

[agent]
auto_detect = true
poll_interval_ms = 500
block_patterns = []               # user-supplied Blocked patterns
default_model = ""
history_retention_days = 30

[ssh]
clipboard_bridge = true
image_bridge = true
file_transfer = true
tunnel_compression = true
```

**Format: TOML only.** (Older design docs reference YAML — that's been
superseded; see `v2-technical-review.md` C6.)

### 10.4 `settings.toml` (user preferences, persisted by client)

Located at `$XDG_CONFIG_HOME/orbit/settings.toml` (or `~/.config/orbit/settings.toml`).
Managed by `app::load_settings()` / `app::save_settings()`. Contains
UI-level preferences that the client controls independently of the daemon:

```toml
theme = "orbit"                   # "orbit" | "tokyo-night"
sidebar_visible = true
agent_panel_visible = false
```

---

## 11. Accepted tradeoffs (known costs)

These decisions have been explicitly accepted. Do not "fix" them without
re-opening the design.

1. **VtParser dual-instance** (server + client each parse the same byte
   stream). 2× CPU under heavy PTY output, but buys client rendering
   independence + low latency. `CellGridDelta` is a reserved Phase-2+
   escape hatch if CPU ever becomes the bottleneck. Same tradeoff tmux
   makes. (`05-vt-emulation.md` §6)

2. **`broadcast::Sender::send` is lossy** by design when a subscriber lags.
   Recovery: `RequestFullState` → server replies `Welcome`/`PaneSnapshot`
   with full state. Loss is detected via tokio's lagged-recv error.

3. **Agent detection by process-name + output-pattern matching** is
   heuristic and will produce false positives/negatives. Phase 2 path
   forward: white-list + explicit OSC status protocol. (`04-server-architecture.md` §4)

4. **Cell grid clone on every PTY read** in `orbitd` (snapshot prep). ~160
   KB copy, target < 50 µs. Acceptable for Phase 1.

5. **Per-pane scrollback default 10,000 lines** (~32 MB/pane, ~128 MB for
   4 panes). Tunable via `ui.scrollback_lines`. Phase 2 will add disk
   persistence; Phase 3 may add RLE compression.

---

## 12. Implementation status

### 12.1 Pre-implementation gaps (all resolved)

| GAP | Status | Action |
|---|---|---|
| **GAP 1**: workspace deps complete | ✅ Done | All deps declared in `Cargo.toml [workspace.dependencies]` |
| **GAP 2**: test strategy | ✅ Partial | IPC roundtrip test + `split_area` guards + `extract_progress`/`strip_ansi` unit tests. Still need: VT golden-file tests, `proptest` for CellGrid invariants. |
| **GAP 3**: daemon lifecycle | ⚠ Open | Add signal handling (SIGTERM/SIGINT → graceful PTY shutdown → unlink socket), PID/lock file at `$XDG_RUNTIME_DIR/orbit.lock`, `prctl(PR_SET_PDEATHSIG)` for PTY children. |
| **GAP 4**: error layering | ✅ Done | `thiserror` in `orbit-protocol`/`orbit-core`; `anyhow` in `orbit`/`orbitd` mains. |
| **GAP 5**: CI scaffold | ✅ Done | `.github/workflows/ci.yml` + `rust-toolchain.toml`. |
| **GAP 6**: Vertical Slice 0 | ✅ Done | Full Mercury slice operational. |
| **GAP 7**: README frame-rate wording | ✅ Done | Redraw-on-demand documented and implemented. |

### 12.2 Phase 1 Mercury — complete

- IPC: Hello/Welcome handshake, PTY lifecycle, PaneInput/PaneOutput
- TUI: full layout (sidebar + tab bar + pane area + status bar + agent panel)
- Tabs: server-side TabId/TabInfo, new/switch/close, survive reconnect
- Panes: split h/v, resize (mouse drag + IPC), close, navigation (arrows)
- Input: prefix-key system, Normal/Prefix/Scroll modes, PTY passthrough
- Detach/reattach: session persists, full state resync on reconnect

### 12.3 Phase 2 Venus — in progress

Implemented:
- ✅ Agent detection (process-name + output-pattern heuristics in orbitd)
- ✅ Agent state machine (Working/Blocked/Idle/Error/Done) with IPC events
- ✅ Agent Fleet Panel (full-featured sidebar): status cards, progress bars,
  duration tracking, scroll, keyboard navigation (j/k/n/r/s/d shortcuts)
- ✅ Eclipse modal: blocked-agent intervention with context display
- ✅ Eclipse banner in status bar (click opens modal)
- ✅ Agent pulse animation in status bar (Working = slow, Blocked = fast)
- ✅ Fleet status badge on tab bar when panel hidden

Remaining:
- Agent restart/stop IPC (wire messages exist, server handlers partial)
- Agent output history / scrollback in panel

### 12.4 TUI features (cross-phase)

- ✅ Theme system: `ThemeColors` struct with Orbit (orange) and Tokyo Night palettes,
  runtime-switchable via `set_theme()`, thread-local accessor functions
- ✅ Settings persistence: `~/.config/orbit/settings.toml` (theme, sidebar, panel)
- ✅ Settings modal: theme selection, toggle sidebar/agent panel visibility
- ✅ Command Palette (Flight Deck): searchable, grouped by category, scroll with
  group header accounting, mouse wheel support
- ✅ Context menus: right-click on tabs, panes, sidebar items
- ✅ Mouse interactions: pane resize drag, tab reorder drag, hover states
- ✅ Spaces Sidebar: space cards with stats (pane count, agent fleet badge),
  bottom separator + bg_secondary content area, click to switch
- ✅ Tab Bar: overflow handling, drag reorder, close buttons, numbering
- ✅ Responsive layout: all four breakpoints (Compact/Standard/Wide/Ultra)

### 12.5 Consistency items

- ✅ No emoji (codebase clean)
- ✅ Orange color system canonical (encoded in `ThemeColors::orbit()`)
- ✅ TOML config format (no YAML)
- ✅ Prefix key default Ctrl+B
- ✅ Code uses generic terms; brand terms in UI strings only
- ✅ License: AGPL-3.0-only (switched from unlicensed, 2026-07-13)

---

## 13. Working on this codebase — guidance for AI agents

### 13.1 Before writing code

1. Identify which crate owns the change (§5.2)
2. Read the relevant `06_tech-design/` doc section
3. If UI: read `UI_DESIGN_BRIEF.md` + the relevant `05_UI-UX-design/` doc
4. Check §6 design principles — violations are review-blockers
5. Check §12 open items — don't reintroduce a closed gap

### 13.2 Non-negotiables

- **No emoji** in code, comments, strings, docs, commit messages
- **No `as Any`-style type erasure** (Rust equivalent: no `.unwrap()` in
  library public APIs without justification; no `todo!()`/`unimplemented!()`
  in committed code without an issue link)
- **No suppressing clippy** (`#[allow(...)]`) without a justifying comment
- **Library crates stay tokio-free** (`orbit-protocol`, `orbit-core`)
- **`App::update(Action)` is the only state mutation path** in the TUI
- **Cell stays 16 bytes** — never add heap-allocated fields
- **Client never touches PTY directly**
- **All IPC messages ≤ 4 MB** — enforce at encode site
- **TOML only** for config — no YAML
- **Match the existing module structure** — don't reorganize without a plan

### 13.3 Verification before declaring work done

- `cargo fmt --all --check` clean
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `cargo test --workspace` passes
- `cargo check --workspace --all-targets` succeeds
- For UI changes: verify in at least one terminal size from each breakpoint
  (Compact <80, Standard 80–99, Wide 100–139, Ultra ≥140)
- For protocol changes: bump `PROTOCOL_VERSION` if breaking; add a
  `Capabilities` flag if additive
- **Run the scenario test suite** against a fresh daemon and confirm 0 failed:
  `cd tools && cargo run --bin scenario_test`

### 13.5 Regression tests — policy

Any bug the user reports becomes a scenario test in `tools/scenario_test.rs`
(or a unit test if it's widget/logic level). The test must fail before the
fix and pass after. This prevents re-regressions.

To add a scenario:
1. Write an `async fn scen_<name>(conn: &mut Conn) -> ScenResult` function in
   `tools/scenario_test.rs`.
2. Register it in `main()` with `run!("name", scen_name);` under the relevant
   section header.
3. Fix the underlying bug.
4. Verify `cargo run --bin scenario_test` passes with 0 failures.

Known regressions that have been covered (do not remove these tests):
- `space_lifecycle` — CreateSpace/CloseSpace IPC round-trip (was: CloseSpace
  handler missing; close_space context menu action was empty stub)
- `state_invariants` — active_tab fallback was TabId(0) which could be absent
  from the tabs map after close_tab (now uses TabId(u32::MAX) sentinel)

### 13.4 Where to look for answers

| Question | Look in |
|---|---|
| What does this widget look like? | `02_design/05_UI-UX-design/03-主工作区设计.md` or `04-Agent-Monitor设计.md` |
| What's the wire format / message variant? | `crates/orbit-protocol/src/messages.rs` (canonical) or `02_design/06_tech-design/03-ipc-protocol.md` |
| What escape sequences must I support? | `02_design/06_tech-design/05-vt-emulation.md` §4 |
| What does this key do? | `COMMANDS` array in `app.rs` (implemented), or `02_design/06_tech-design/06-input-routing-and-modes.md` §5 (design) |
| What's the agent state machine? | `02_design/06_tech-design/04-server-architecture.md` §4.2 |
| Where does scrollback data come from? | `02_design/06_tech-design/08-scrollback-and-history.md` |
| What fields does the Agent card need? | `02_design/06_tech-design/07-agent-data-model.md` §1 |
| What's the CLI command for X? | `02_design/BRAND_ORBIT.md` "CLI Command System" |
| Why was decision Y made? | `02_design/06_tech-design/critics/v2-technical-review.md` and successors |
| What's still open? | §12 above |
| What theme tokens / colors exist? | `crates/orbit/src/tui/theme.rs` (`ThemeColors` struct + accessor fns) |
| How does settings persistence work? | `crates/orbit/src/app.rs` (`UserSettings`, `load_settings`, `save_settings`) |

---

## 14. Glossary quick-reference

- **Beacon** — clipboard sync (OSC 52 bridge)
- **Constellation** — session grouping / session list
- **Deck** — window (a tab in the Tab Bar)
- **Downlink** — image transfer from server to client
- **Eclipse** — agent Blocked state (needs user intervention)
- **Flight Deck** — Command Palette overlay
- **Go Dark** — detach (client exits, daemon keeps session)
- **Ground Station** — the `orbit` TUI client
- **Instrument** — MCP tool
- **Link-up** — SSH attach
- **Module** — plugin (native or WASM)
- **Orbit** — the product; also a Session conceptually
- **Payload** — file transfer
- **Port** — pane (a rectangular PTY region)
- **Satellite** — agent (Claude Code, Codex, Copilot CLI, etc.)
- **Standby** — agent Idle
- **Transmitting** — agent Working
- **Debris** — agent Error

---

*This document is the canonical project context. Update it when design
decisions change; do not let it drift from the code.*
