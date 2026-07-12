# Agent Fleet Panel — Implementation Status

> Last updated: 2026-07-12

---

## Done (Phase 1 complete)

Across ~50 commits, the Satellite Monitor panel is functionally complete for Phase 1.

### Rendering

- 5-row card layout: icon+name+status+duration, cwd+model+rss, task/block_msg/recent_output, progress bar+cpu%, buttons
- Animated status colors: Working slow pulse (90 ticks), Blocked fast pulse (48 ticks), Error blink (60 ticks)
- Orange `▸` selection marker for keyboard navigation
- Blocked/Error animated left-border accent (`▌`)
- Eclipse banner: em-dash format `◎ Eclipse — {name}`, block_msg preview, pulsing icon
- Empty-state display when no agents detected
- Pinned footer `[+] Add Satellite`
- Scroll indicators: `▴ N above` / `▾ N more`
- Header badge `SATELLITES [N]`
- Responsive panel width: 22 cols (80-139), 25 cols (>=140), 0 cols (<80)

### Interaction

- Full keyboard nav: Up/Down/j/k (navigate), Tab (cycle), Enter (view/focus pane), r (respond to Blocked / restart Error), s (stop Working/Blocked), d (dismiss Idle/Done/Error), n (launch modal), q/Esc (exit mode)
- Mouse click on all button slots: [View], [Stop], [Chat], [Resp], [Abrt], [Rmov], [Rstr]
- Mouse hover highlights: header [+] and x buttons, eclipse [Respond], card buttons, footer
- Mouse wheel scroll with correct visible-card bounds
- Eclipse modal: type response text, Enter=send, Esc=cancel, click-outside=dismiss
- Launch modal: 3-agent preset picker (claude/codex/aider), Enter=launch, Esc=cancel
- Right-click on panel area absorbed (no spurious pane context menu)
- `sort_agents` preserves keyboard selection through reorders

### Protocol (orbit-protocol)

- `ClientMessage::AgentRestart { agent_id }` — reset Error agent to Idle
- `ClientMessage::AgentLaunch { config }` — split pane + type CLI command
- All existing messages wired: `AgentRespond`, `AgentSkip`, `AgentAbort`, `AgentRemove`

### Server-side (orbitd)

- `/proc`-based agent child-process detection (Linux)
- Block pattern scanning from PTY output (Satellite Eclipse detection)
- Progress percentage extraction from PTY output (regex-based)
- RSS memory + CPU% metrics polling (5-second cycle)
- `restart_agent` (Error -> Idle), `abort_agent` (SIGTERM), `remove_agent` (dismiss)

---

## Remaining TODO

| # | Item | Priority | Notes |
|---|------|----------|-------|
| 1 | macOS agent detection | Medium | `watch_pane` process scan is `#[cfg(target_os = "linux")]` only; macOS needs `sysctl`/`libproc` equivalent |
| 2 | Launch modal text input | Low | Currently a 3-item preset picker; no free-text command entry field. `LaunchModalState` only has `selected: usize` |
| 3 | Agent context menu (right-click on card) | Low | Right-click is absorbed but no `ContextMenuTarget::Agent` variant exists |
| 4 | Button label width-adaptive sizing | Low | Labels fixed at 6 chars (`[Rmov]`/`[Resp]`/`[Abrt]`/`[Rstr]`); spec wants full words (`[Remove]`/`[Respond]`/`[Abort]`/`[Restart]`) at wider panels |
| 5 | Agent panel visibility persistence | Low | `agent_panel_visible` not saved to `config.toml`; always starts hidden |
| 6 | Sort order alignment with spec | Trivial | Code: Blocked > Working > Error > Idle > Done; spec S5.2: Blocked > Working > Idle > Error — decide canonical order |

---

## Key files

| File | Responsibility |
|------|---------------|
| `crates/orbit/src/tui/widgets/agent_monitor.rs` | Card rendering, header, footer, eclipse banner, animations |
| `crates/orbit/src/tui/widgets/eclipse_modal.rs` | Eclipse intervention modal render |
| `crates/orbit/src/tui/widgets/launch_modal.rs` | Launch Satellite picker render |
| `crates/orbit/src/events.rs` | Keyboard + mouse handlers for AgentPanel mode |
| `crates/orbit/src/app.rs` | `AgentHover`, `InputMode::AgentPanel`, `sort_agents`, state fields |
| `crates/orbit-protocol/src/messages.rs` | `ClientMessage::AgentRestart/Launch/Abort/Remove/Respond/Skip` |
| `crates/orbitd/src/agent.rs` | `AgentRegistry`, `watch_pane`, process detection, metrics |
| `crates/orbitd/src/ipc.rs` | Server-side message dispatch |
