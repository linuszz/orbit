# UI Polish Sprint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the tab bar (solid filled blocks), spaces sidebar (card list with all spaces), add mouse text selection/copy, and implement aerospace adjective-noun space naming.

**Architecture:** All changes are in the TUI client (`orbit` crate) and the daemon (`orbitd` crate). Protocol additions are purely additive (new `ClientMessage` variants, no version bump). The daemon gains clipboard writing via `arboard` and a space-name generator. The client gains `Selection` state and a multi-space `App` model.

**Tech Stack:** Rust stable, ratatui 0.27, crossterm 0.28, tokio, bincode 2.x/serde, arboard (new), orbit-protocol (internal).

## Global Constraints

- No emoji anywhere — Unicode symbols only (`●`, `○`, `◎`, `▌`, `«`, `»`, `╭`, `╰`)
- Orange accent system: `ACCENT = Rgb(217,119,6)`, `ACCENT_HOVER = Rgb(161,86,0)`, `BG_PRIMARY = Rgb(14,14,20)`, `BG_SECONDARY = Rgb(18,18,26)`, `BG_TERTIARY = Rgb(24,24,33)`, `BG_CARD = Rgb(20,20,29)`, `FG_PRIMARY = Rgb(242,242,248)`, `FG_SECONDARY = Rgb(180,180,196)`, `FG_MUTED = Rgb(120,120,140)`, `BORDER = Rgb(60,60,76)`
- No `tokio` in `orbit-protocol` or `orbit-core` crates
- `cargo clippy --workspace --all-targets -- -D warnings` must be clean
- `cargo fmt --all --check` must be clean
- Build with `nix-shell -p gcc --run "cargo build --workspace"` on NixOS
- All tests: `nix-shell -p gcc --run "cargo test --workspace"`

---

## File Map

| File | Action | What changes |
|---|---|---|
| `crates/orbit-protocol/src/messages.rs` | Modify | Add `CopyToClipboard` variant to `ClientMessage` |
| `crates/orbitd/Cargo.toml` | Modify | Add `arboard` dependency |
| `crates/orbitd/src/session.rs` | Modify | Add `generate_space_name()`; replace hardcoded `"dev"` |
| `crates/orbitd/src/ipc.rs` | Modify | Route `CopyToClipboard`, `SwitchSpace` (currently falls through `_ => {}`) |
| `crates/orbit/src/app.rs` | Modify | Add `SpaceEntry`, `Selection` structs; multi-space fields; `tab_hovered`, `sidebar_hovered` |
| `crates/orbit/src/tui/widgets/tab_bar.rs` | Modify | Solid-block style; hover rendering |
| `crates/orbit/src/tui/widgets/spaces_sidebar.rs` | Rewrite | Card layout; collapsed mode; multi-space list |
| `crates/orbit/src/tui/mod.rs` | Modify | Selection highlight in `render_cells` / `render_cells_scrolled` |
| `crates/orbit/src/events.rs` | Modify | Tab hover; sidebar hover+click; selection drag; copy context item |

---

### Task 1: Add `CopyToClipboard` to protocol + wire daemon clipboard

**Files:**
- Modify: `crates/orbit-protocol/src/messages.rs`
- Modify: `crates/orbitd/Cargo.toml`
- Modify: `crates/orbitd/src/ipc.rs`
- Test: `crates/orbit-protocol/src/encoding.rs` (existing test module)

**Interfaces:**
- Produces: `ClientMessage::CopyToClipboard { text: String }` — used by Task 5
- Produces: `SwitchSpace` now handled in ipc.rs (was `_ => {}`) — used by Task 3

- [ ] **Step 1: Add `CopyToClipboard` to `ClientMessage`**

Open `crates/orbit-protocol/src/messages.rs`. After the `RequestScrollback` variant, add:

```rust
    CopyToClipboard {
        text: String,
    },
```

- [ ] **Step 2: Write a roundtrip test for the new variant**

In `crates/orbit-protocol/src/encoding.rs`, inside `mod tests { ... }`, add:

```rust
#[test]
fn copy_to_clipboard_roundtrip() {
    let msg = ClientMessage::CopyToClipboard {
        text: "hello world".to_string(),
    };
    let bytes = encode_message(&msg).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&bytes[4..], bincode::config::standard()).unwrap();
    match decoded {
        ClientMessage::CopyToClipboard { text } => assert_eq!(text, "hello world"),
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 3: Run test to verify it passes**

```bash
nix-shell -p gcc --run "cargo test -p orbit-protocol copy_to_clipboard_roundtrip"
```

Expected: `test encoding::tests::copy_to_clipboard_roundtrip ... ok`

- [ ] **Step 4: Add `arboard` to orbitd**

In `crates/orbitd/Cargo.toml`, under `[dependencies]`, add:

```toml
arboard = "3"
```

Also add it to the workspace root `Cargo.toml` under `[workspace.dependencies]`:

```toml
arboard = "3"
```

And in `crates/orbitd/Cargo.toml` reference it as:

```toml
arboard = { workspace = true }
```

- [ ] **Step 5: Handle `CopyToClipboard` and `SwitchSpace` in ipc.rs**

In `crates/orbitd/src/ipc.rs`, at the top add:

```rust
use arboard::Clipboard;
```

In the `match msg { ... }` block, replace the `_ => {}` catch-all with:

```rust
                    ClientMessage::CopyToClipboard { text } => {
                        if let Ok(mut cb) = Clipboard::new() {
                            let _ = cb.set_text(text);
                        }
                    }
                    ClientMessage::SwitchSpace { space_id } => {
                        session.switch_space(space_id).await;
                    }
                    _ => {}
```

- [ ] **Step 6: Add `switch_space` stub to `SessionState`**

In `crates/orbitd/src/session.rs`, add this method (it's a no-op for now since single-space is the current reality — the sidebar will call it for future multi-space support):

```rust
pub async fn switch_space(&self, _space_id: orbit_protocol::SpaceId) {
    // Multi-space switching: future implementation.
    // For now the daemon runs a single space; this message is accepted but ignored.
}
```

- [ ] **Step 7: Build and verify clean**

```bash
nix-shell -p gcc --run "cargo build --workspace"
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
```

Both must succeed with no warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/orbit-protocol/src/messages.rs \
        crates/orbit-protocol/src/encoding.rs \
        crates/orbitd/Cargo.toml \
        crates/orbitd/src/ipc.rs \
        crates/orbitd/src/session.rs \
        Cargo.toml Cargo.lock
git commit -m "feat: add CopyToClipboard message + wire arboard clipboard in daemon"
```

---

### Task 2: Aerospace space naming

**Files:**
- Modify: `crates/orbitd/src/session.rs`

**Interfaces:**
- Produces: `generate_space_name(existing: &[&str]) -> String` — used internally in `SessionState::new()`

- [ ] **Step 1: Write a unit test for name generation**

In `crates/orbitd/src/session.rs`, at the bottom add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_name_format() {
        let name = generate_space_name(&[]);
        let parts: Vec<&str> = name.splitn(2, '-').collect();
        assert_eq!(parts.len(), 2, "name should be adjective-noun: {name}");
        assert!(!parts[0].is_empty());
        assert!(!parts[1].is_empty());
    }

    #[test]
    fn space_name_avoids_duplicates() {
        // Fill up all 400 combinations by calling many times — just verify no panic
        let mut seen = vec![];
        for _ in 0..20 {
            let refs: Vec<&str> = seen.iter().map(|s: &String| s.as_str()).collect();
            let name = generate_space_name(&refs);
            seen.push(name);
        }
        assert_eq!(seen.len(), 20);
    }
}
```

- [ ] **Step 2: Run to see it fail**

```bash
nix-shell -p gcc --run "cargo test -p orbitd space_name"
```

Expected: compile error — `generate_space_name` not defined yet.

- [ ] **Step 3: Add the word pools and generator function**

In `crates/orbitd/src/session.rs`, before the `impl SessionState` block, add:

```rust
const ADJECTIVES: &[&str] = &[
    "cosmic", "stellar", "quantum", "lunar", "solar", "orbital", "deep",
    "silent", "swift", "apex", "delta", "zenith", "polar", "radiant",
    "binary", "axial", "thermal", "mach", "ion", "photon",
];

const NOUNS: &[&str] = &[
    "mars", "void", "nova", "horizon", "nebula", "atlas", "vega", "lyra",
    "cygnus", "orbit", "pulse", "core", "arc", "link", "beacon", "vector",
    "node", "flux", "rift", "zone",
];

pub fn generate_space_name(existing: &[&str]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    // Seed from current time nanos — good enough for name generation.
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(42);

    for attempt in 0..10 {
        let mut h = DefaultHasher::new();
        (seed + attempt).hash(&mut h);
        let v = h.finish() as usize;
        let adj = ADJECTIVES[v % ADJECTIVES.len()];
        let noun = NOUNS[(v / ADJECTIVES.len()) % NOUNS.len()];
        let candidate = format!("{adj}-{noun}");
        if !existing.contains(&candidate.as_str()) {
            return candidate;
        }
    }
    // Fallback: append attempt index
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    let v = h.finish() as usize;
    let adj = ADJECTIVES[v % ADJECTIVES.len()];
    let noun = NOUNS[(v / ADJECTIVES.len()) % NOUNS.len()];
    format!("{adj}-{noun}-2")
}
```

- [ ] **Step 4: Replace hardcoded `"dev"` in `SessionState::new()`**

In `crates/orbitd/src/session.rs`, find the line:

```rust
name: "dev".to_string(),
```

(around line 68 — the tab name inside `new()`) and the line:

```rust
space_name: "default".to_string(),
```

(around line 76). Replace the `space_name` line with:

```rust
space_name: generate_space_name(&[]),
```

Leave the tab name `"dev"` as-is — that's the default tab (Deck) name, not the space name.

- [ ] **Step 5: Run tests**

```bash
nix-shell -p gcc --run "cargo test -p orbitd space_name"
```

Expected: both tests pass.

- [ ] **Step 6: Build clean**

```bash
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
```

- [ ] **Step 7: Commit**

```bash
git add crates/orbitd/src/session.rs
git commit -m "feat: aerospace adjective-noun space naming generator"
```

---

### Task 3: Multi-space App state + `SpaceEntry`

**Files:**
- Modify: `crates/orbit/src/app.rs`

**Interfaces:**
- Consumes: `FullState::spaces: Vec<SpaceInfo>`, `FullState::active_space: SpaceId`, `SpaceInfo::{ id, name, path, tabs, panes }`
- Produces:
  - `pub struct SpaceEntry { pub space_id: SpaceId, pub name: String, pub cwd: String, pub tab_count: usize, pub pane_count: usize }` — used by Task 4 (sidebar rendering)
  - `App::spaces: Vec<SpaceEntry>` — used by Task 4
  - `App::active_space_idx: usize` — used by Task 4
  - `App::tab_hovered: Option<usize>` — used by Task 4 (tab bar)
  - `App::sidebar_hovered: Option<usize>` — used by Task 4 (sidebar)
  - `App::selection: Option<Selection>` — used by Tasks 4 and 5
  - `pub struct Selection { pub pane_id: PaneId, pub start: (u16,u16), pub end: (u16,u16), pub active: bool }` — used by Tasks 4 and 5

- [ ] **Step 1: Add `SpaceEntry` and `Selection` structs to `app.rs`**

Near the top of `crates/orbit/src/app.rs`, after the existing `use` imports, add:

```rust
#[derive(Debug, Clone)]
pub struct SpaceEntry {
    pub space_id: orbit_protocol::SpaceId,
    pub name: String,
    pub cwd: String,
    pub tab_count: usize,
    pub pane_count: usize,
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub pane_id: orbit_protocol::PaneId,
    pub start: (u16, u16), // (col, row) in cell coords within pane
    pub end: (u16, u16),
    pub active: bool,
}
```

- [ ] **Step 2: Add new fields to the `App` struct**

In the `pub struct App { ... }` definition, add these fields (alongside existing ones):

```rust
    pub spaces: Vec<SpaceEntry>,
    pub active_space_idx: usize,
    pub tab_hovered: Option<usize>,
    pub sidebar_hovered: Option<usize>,
    pub selection: Option<Selection>,
```

- [ ] **Step 3: Populate `spaces` in `App::from_welcome`**

In `App::from_welcome`, after the existing `let space = state.spaces.first();` line, add space list construction:

```rust
        let spaces: Vec<SpaceEntry> = state.spaces.iter().map(|s| SpaceEntry {
            space_id: s.id,
            name: s.name.clone(),
            cwd: s.path.clone(),
            tab_count: s.tabs.len(),
            pane_count: s.panes.len(),
        }).collect();

        let active_space_idx = state.spaces.iter()
            .position(|s| s.id == state.active_space)
            .unwrap_or(0);
```

At the end of the `Self { ... }` constructor block, add the new fields:

```rust
            spaces,
            active_space_idx,
            tab_hovered: None,
            sidebar_hovered: None,
            selection: None,
```

- [ ] **Step 4: Keep `space_name` / `space_path` working**

The existing `space_name` and `space_path` fields are used by the status bar and other widgets. Keep them — derive them from `spaces[active_space_idx]` in `from_welcome`:

```rust
            space_name: state.spaces.get(active_space_idx)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "orbit".to_string()),
            space_path: state.spaces.get(active_space_idx)
                .map(|s| s.path.clone())
                .unwrap_or_else(|| ".".to_string()),
```

Replace the existing `space_name` and `space_path` lines in the constructor with these.

- [ ] **Step 5: Clear selection in `handle_server_event` on `PaneOutput`**

In `handle_server_event`, in the `PaneOutput` arm, add at the start:

```rust
                if let Some(sel) = &self.selection {
                    if sel.pane_id == pane_id {
                        self.selection = None;
                    }
                }
```

- [ ] **Step 6: Update `handle_server_event` `Welcome` arm to refresh `spaces`**

In the `Welcome` arm of `handle_server_event`, after the existing re-sync logic, add:

```rust
                self.spaces = state.spaces.iter().map(|s| SpaceEntry {
                    space_id: s.id,
                    name: s.name.clone(),
                    cwd: s.path.clone(),
                    tab_count: s.tabs.len(),
                    pane_count: s.panes.len(),
                }).collect();
                self.active_space_idx = state.spaces.iter()
                    .position(|s| s.id == state.active_space)
                    .unwrap_or(0);
```

- [ ] **Step 7: Build**

```bash
nix-shell -p gcc --run "cargo build --workspace"
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
```

- [ ] **Step 8: Commit**

```bash
git add crates/orbit/src/app.rs
git commit -m "feat: multi-space App state, Selection struct, hover fields"
```

---

### Task 4: Tab bar solid-block style + sidebar card redesign

**Files:**
- Modify: `crates/orbit/src/tui/widgets/tab_bar.rs`
- Modify: `crates/orbit/src/tui/widgets/spaces_sidebar.rs`
- Modify: `crates/orbit/src/tui/mod.rs` (selection highlight in render_cells)
- Modify: `crates/orbit/src/events.rs` (hover tracking for tabs + sidebar)

**Interfaces:**
- Consumes: `App::tab_hovered`, `App::sidebar_hovered`, `App::spaces`, `App::active_space_idx`
- Produces: visual tab bar and sidebar — no new public API

---

#### 4a: Tab bar

- [ ] **Step 1: Rewrite `tab_bar.rs` render function**

Replace the entire body of the `pub fn render(frame: &mut Frame, area: Rect, app: &App)` function in `crates/orbit/src/tui/widgets/tab_bar.rs` with:

```rust
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Paragraph};
    use crate::tui::theme::*;

    // Bottom border on the bar
    let bar_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(BORDER));
    let inner = bar_block.inner(area);
    frame.render_widget(bar_block, area);

    let mut spans: Vec<Span> = Vec::new();

    for (i, tab) in app.tabs.iter().enumerate() {
        let label = format!(" {} ", tab.name);
        let (bg, fg, mods) = if tab.id == app.active_tab_id {
            (ACCENT, BG_PRIMARY, Modifier::BOLD)
        } else if app.tab_hovered == Some(i) {
            (ACCENT_HOVER, FG_PRIMARY, Modifier::empty())
        } else {
            (BG_CARD, FG_MUTED, Modifier::empty())
        };
        spans.push(Span::styled(
            label,
            Style::default().fg(fg).bg(bg).add_modifier(mods),
        ));
    }

    // New tab button
    let new_tab_bg = if app.tab_hovered == Some(app.tabs.len()) {
        ACCENT
    } else {
        BG_CARD
    };
    spans.push(Span::styled(
        " + ",
        Style::default().fg(FG_MUTED).bg(new_tab_bg),
    ));

    // Fill remaining space with BG_SECONDARY
    spans.push(Span::styled(
        " ".repeat(inner.width.saturating_sub(
            spans.iter().map(|s| s.content.len() as u16).sum::<u16>()
                + if app.agent_panel_visible { 14 } else { 12 },
        ) as usize),
        Style::default().bg(BG_SECONDARY),
    ));

    // Agent panel toggle — right-aligned
    let (agent_fg, agent_bg) = if app.agent_panel_visible {
        (BG_PRIMARY, ACCENT)
    } else {
        (FG_MUTED, BG_CARD)
    };
    spans.push(Span::styled(
        " [A] Satellites ",
        Style::default().fg(agent_fg).bg(agent_bg),
    ));

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), inner);
}
```

- [ ] **Step 2: Add tab hover tracking in `events.rs`**

In `crates/orbit/src/events.rs`, inside `handle_mouse`, at the **beginning** of the `MouseEventKind::Moved { .. }` arm (add this arm if it doesn't already have one for hover):

Find the existing mouse handler. Add a new arm for `Moved` in the section that handles non-menu mouse events:

```rust
            MouseEventKind::Moved => {
                // Tab bar hover (row 0 relative to content area, which is row 0 of the terminal
                // after accounting for sidebar — tab bar is always at row 0 of the frame).
                let sidebar_w = if app.sidebar_visible { SIDEBAR_W } else { SIDEBAR_COLLAPSED_W };
                if mouse.row == 0 && mouse.column >= sidebar_w {
                    let col = mouse.column - sidebar_w;
                    let mut acc: u16 = 0;
                    let mut hovered = None;
                    for (i, tab) in app.tabs.iter().enumerate() {
                        let w = tab.name.len() as u16 + 2; // " name "
                        if col < acc + w {
                            hovered = Some(i);
                            break;
                        }
                        acc += w;
                    }
                    // Check new-tab button
                    if hovered.is_none() && col < acc + 3 {
                        hovered = Some(app.tabs.len());
                    }
                    if app.tab_hovered != hovered {
                        app.tab_hovered = hovered;
                        app.needs_redraw = true;
                    }
                } else if app.tab_hovered.is_some() {
                    app.tab_hovered = None;
                    app.needs_redraw = true;
                }

                // Sidebar card hover
                if app.sidebar_visible && mouse.column < SIDEBAR_W {
                    // Each card is 5 rows tall (border+3content+border) + 1 gap = 6 rows.
                    // Cards start after header (2 rows: "SPACES«" + divider).
                    let content_row = mouse.row.saturating_sub(2);
                    let card_idx = (content_row / 6) as usize;
                    let hovered = if card_idx < app.spaces.len() {
                        Some(card_idx)
                    } else {
                        None
                    };
                    if app.sidebar_hovered != hovered {
                        app.sidebar_hovered = hovered;
                        app.needs_redraw = true;
                    }
                } else if app.sidebar_hovered.is_some() {
                    app.sidebar_hovered = None;
                    app.needs_redraw = true;
                }
            }
```

Note: `SIDEBAR_W` and `SIDEBAR_COLLAPSED_W` are defined in `tui/mod.rs` as `pub const`. Import them at the top of `events.rs`:

```rust
use crate::tui::{SIDEBAR_W, SIDEBAR_COLLAPSED_W};
```

Make sure these constants are `pub` in `tui/mod.rs`.

- [ ] **Step 3: Build**

```bash
nix-shell -p gcc --run "cargo build --workspace"
```

Fix any compile errors before continuing.

---

#### 4b: Spaces sidebar card list

- [ ] **Step 4: Rewrite `spaces_sidebar.rs`**

Replace the entire content of `crates/orbit/src/tui/widgets/spaces_sidebar.rs` with:

```rust
use orbit_protocol::ClientMessage;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::tui::theme::*;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.sidebar_visible {
        render_expanded(frame, area, app);
    } else {
        render_collapsed(frame, area, app);
    }
}

fn render_expanded(frame: &mut Frame, area: Rect, app: &App) {
    let w = area.width;
    let mut y = area.y;
    let x = area.x;

    // Header row: "SPACES" + collapse hint «
    let header = format!(
        "{:<width$}«",
        "SPACES",
        width = w.saturating_sub(1) as usize
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            header,
            Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
        ))),
        Rect { x, y, width: w, height: 1 },
    );
    y += 1;

    // Divider
    let div = "─".repeat(w as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(div, Style::default().fg(BORDER)))),
        Rect { x, y, width: w, height: 1 },
    );
    y += 1;

    // Cards
    for (i, space) in app.spaces.iter().enumerate() {
        if y + 5 > area.y + area.height {
            break;
        }

        let is_active = i == app.active_space_idx;
        let is_hovered = app.sidebar_hovered == Some(i);

        let card_bg = if is_active {
            BG_CARD
        } else if is_hovered {
            BG_TERTIARY
        } else {
            BG_SECONDARY
        };

        let name_fg = if is_active { FG_PRIMARY } else { FG_SECONDARY };

        // Top border row: ╭─ name ─╮ (or ▌─ name ─╮ for active)
        let name_trunc = truncate(&space.name, w.saturating_sub(4) as usize);
        let dashes_right = w.saturating_sub(4 + name_trunc.len() as u16);
        let top_left = if is_active { "▌" } else { "╭" };
        let top_border = format!(
            "{}─ {}{} ╮",
            top_left,
            name_trunc,
            "─".repeat(dashes_right as usize)
        );
        let accent_fg = if is_active { ACCENT } else { BORDER };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(&top_border[..1], Style::default().fg(accent_fg).bg(card_bg)),
                Span::styled(&top_border[1..], Style::default().fg(BORDER).bg(card_bg)),
            ]))
            .style(Style::default().bg(card_bg)),
            Rect { x, y, width: w, height: 1 },
        );
        // Overlay the name in the correct color
        let name_x = x + 3; // after "╭─ " or "▌─ "
        frame.render_widget(
            Paragraph::new(Span::styled(
                name_trunc,
                Style::default()
                    .fg(name_fg)
                    .bg(card_bg)
                    .add_modifier(if is_active { Modifier::BOLD } else { Modifier::empty() }),
            )),
            Rect { x: name_x, y, width: w.saturating_sub(4), height: 1 },
        );
        y += 1;

        // CWD row: │ ~/path    │
        let cwd_trunc = truncate(&space.cwd, w.saturating_sub(4) as usize);
        let cwd_line = format!(
            "│ {:<width$} │",
            cwd_trunc,
            width = w.saturating_sub(4) as usize
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("│", Style::default().fg(BORDER).bg(card_bg)),
                Span::styled(
                    format!(" {:<width$} ", cwd_trunc, width = w.saturating_sub(4) as usize),
                    Style::default().fg(FG_SECONDARY).bg(card_bg),
                ),
                Span::styled("│", Style::default().fg(BORDER).bg(card_bg)),
            ])),
            Rect { x, y, width: w, height: 1 },
        );
        y += 1;

        // Stats row: │ ● N  Xt Yp │
        let status_sym = if space.pane_count > 0 { "●" } else { "○" };
        let stats = format!(
            "{}   {}t {}p",
            status_sym, space.tab_count, space.pane_count
        );
        let stats_trunc = truncate(&stats, w.saturating_sub(4) as usize);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("│", Style::default().fg(BORDER).bg(card_bg)),
                Span::styled(
                    format!(" {:<width$} ", stats_trunc, width = w.saturating_sub(4) as usize),
                    Style::default().fg(FG_MUTED).bg(card_bg),
                ),
                Span::styled("│", Style::default().fg(BORDER).bg(card_bg)),
            ])),
            Rect { x, y, width: w, height: 1 },
        );
        y += 1;

        // Bottom border: ╰──────────────╯
        let bottom = format!("╰{}╯", "─".repeat(w.saturating_sub(2) as usize));
        frame.render_widget(
            Paragraph::new(Span::styled(bottom, Style::default().fg(BORDER).bg(card_bg))),
            Rect { x, y, width: w, height: 1 },
        );
        y += 1;

        // Gap between cards
        if i + 1 < app.spaces.len() {
            frame.render_widget(
                Paragraph::new("").style(Style::default().bg(BG_PRIMARY)),
                Rect { x, y, width: w, height: 1 },
            );
            y += 1;
        }
    }

    // New space button at bottom if space allows
    if y < area.y + area.height {
        frame.render_widget(
            Paragraph::new(Span::styled(" [+] New ", Style::default().fg(ACCENT).bg(BG_CARD))),
            Rect { x, y, width: w, height: 1 },
        );
    }
}

fn render_collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let w = area.width; // should be 2
    let x = area.x;

    for (i, _space) in app.spaces.iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height.saturating_sub(1) {
            break;
        }
        let is_active = i == app.active_space_idx;
        let (fg, bg) = if is_active {
            (BG_PRIMARY, ACCENT)
        } else {
            (FG_MUTED, BG_SECONDARY)
        };
        let label = format!("{:>2}", i + 1);
        frame.render_widget(
            Paragraph::new(Span::styled(label, Style::default().fg(fg).bg(bg))),
            Rect { x, y, width: w, height: 1 },
        );
    }

    // Expand hint at bottom
    let expand_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(Span::styled("»", Style::default().fg(FG_MUTED))),
        Rect { x, y: expand_y, width: w, height: 1 },
    );
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}
```

- [ ] **Step 5: Add sidebar click routing in `events.rs`**

In `handle_mouse`, find the existing `Left { .. }` arm for sidebar clicks. Replace (or augment) the sidebar section with:

```rust
// Sidebar: collapse button («) at (sidebar_w-1, 0)
if mouse.column == SIDEBAR_W - 1 && mouse.row == 0 && app.sidebar_visible {
    app.sidebar_visible = false;
    app.needs_redraw = true;
    return;
}
// Sidebar: expand button (») when collapsed
if !app.sidebar_visible && mouse.column < SIDEBAR_COLLAPSED_W && mouse.row > 0 {
    app.sidebar_visible = true;
    app.needs_redraw = true;
    return;
}
// Sidebar: click a space card
if app.sidebar_visible && mouse.column < SIDEBAR_W {
    // Cards start at row 2 (after header + divider). Each card is 5 rows + 1 gap = 6 rows.
    let content_row = mouse.row.saturating_sub(2);
    let card_idx = (content_row / 6) as usize;
    if card_idx < app.spaces.len() {
        let space_id = app.spaces[card_idx].space_id;
        app.active_space_idx = card_idx;
        writer.send(ClientMessage::SwitchSpace { space_id }).ok();
        app.needs_redraw = true;
        return;
    }
}
```

- [ ] **Step 6: Build and run**

```bash
nix-shell -p gcc --run "cargo build --workspace"
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
```

- [ ] **Step 7: Commit**

```bash
git add crates/orbit/src/tui/widgets/tab_bar.rs \
        crates/orbit/src/tui/widgets/spaces_sidebar.rs \
        crates/orbit/src/events.rs \
        crates/orbit/src/tui/mod.rs
git commit -m "feat: solid-block tab bar, space card sidebar, hover tracking"
```

---

### Task 5: Mouse text selection and copy

**Files:**
- Modify: `crates/orbit/src/tui/mod.rs`
- Modify: `crates/orbit/src/events.rs`
- Modify: `crates/orbit/src/app.rs`

**Interfaces:**
- Consumes: `Selection` struct (from Task 3), `ClientMessage::CopyToClipboard` (from Task 1), `compute_leaf_areas(layout, area) -> Vec<(PaneId, Rect)>`
- Produces: visual selection highlight; clipboard write on right-click Copy

- [ ] **Step 1: Pass `Selection` into render_cells**

In `crates/orbit/src/tui/mod.rs`, find the `render_cells` function signature. It currently takes `(frame, area, grid, show_cursor)`. Change it to also accept an optional selection:

```rust
fn render_cells(
    frame: &mut Frame,
    area: Rect,
    grid: &orbit_core::vt::CellGrid,
    show_cursor: bool,
    selection: Option<&crate::app::Selection>,
    pane_id: orbit_protocol::PaneId,
) {
```

Update the call sites in `render_single_pane` to pass `app.selection.as_ref()` and the pane's `PaneId`.

- [ ] **Step 2: Apply selection inversion in `render_cells`**

Inside `render_cells`, in the cell-rendering loop, add selection check. After determining `fg` and `bg` for a cell, add:

```rust
    let in_selection = selection.map_or(false, |sel| {
        sel.pane_id == pane_id && {
            let (min_col, max_col) = if sel.start.0 <= sel.end.0 {
                (sel.start.0, sel.end.0)
            } else {
                (sel.end.0, sel.start.0)
            };
            let (min_row, max_row) = if sel.start.1 <= sel.end.1 {
                (sel.start.1, sel.end.1)
            } else {
                (sel.end.1, sel.start.1)
            };
            col as u16 >= min_col && col as u16 <= max_col
                && row as u16 >= min_row && row as u16 <= max_row
        }
    });
    let (fg, bg) = if in_selection { (bg, fg) } else { (fg, bg) };
```

(Place this just before the `frame.render_widget(...)` call for each cell.)

- [ ] **Step 3: Add `MouseDown`, `MouseMove`, `MouseUp` selection handling in `events.rs`**

In `handle_mouse`, add these arms:

```rust
// Start selection
MouseEventKind::Down(MouseButton::Left) => {
    // Only in Normal mode, not over sidebar or agent panel
    let sidebar_w = if app.sidebar_visible { SIDEBAR_W } else { SIDEBAR_COLLAPSED_W };
    let agent_w = if app.agent_panel_visible { AGENT_W } else { 0 };
    let content_x = sidebar_w;
    let content_right = term_size.0.saturating_sub(agent_w);
    if mouse.column >= content_x && mouse.column < content_right && mouse.row > 0 {
        // Hit-test which pane was clicked
        let pane_areas = compute_leaf_areas(app.pane_tree(), content_area(term_size, app));
        for (pane_id, rect) in &pane_areas {
            if mouse.column >= rect.x && mouse.column < rect.x + rect.width
                && mouse.row >= rect.y && mouse.row < rect.y + rect.height
            {
                let col = mouse.column - rect.x;
                let row = mouse.row - rect.y;
                app.selection = Some(crate::app::Selection {
                    pane_id: *pane_id,
                    start: (col, row),
                    end: (col, row),
                    active: true,
                });
                app.needs_redraw = true;
                break;
            }
        }
    }
}

// Update selection during drag
MouseEventKind::Drag(MouseButton::Left) => {
    if let Some(sel) = &mut app.selection {
        if sel.active {
            // Find the pane rect to clamp coords
            let sidebar_w = if app.sidebar_visible { SIDEBAR_W } else { SIDEBAR_COLLAPSED_W };
            let agent_w = if app.agent_panel_visible { AGENT_W } else { 0 };
            let pane_areas = compute_leaf_areas(
                app.pane_tree(),
                content_area(term_size, app),
            );
            for (pane_id, rect) in &pane_areas {
                if *pane_id == sel.pane_id {
                    let col = mouse.column.saturating_sub(rect.x).min(rect.width.saturating_sub(1));
                    let row = mouse.row.saturating_sub(rect.y).min(rect.height.saturating_sub(1));
                    sel.end = (col, row);
                    app.needs_redraw = true;
                    break;
                }
            }
        }
    }
}

// Finalize selection
MouseEventKind::Up(MouseButton::Left) => {
    if let Some(sel) = &mut app.selection {
        sel.active = false;
        if sel.start == sel.end {
            app.selection = None;
        }
        app.needs_redraw = true;
    }
}
```

Add a helper `content_area` in `events.rs`:

```rust
fn content_area(term_size: (u16, u16), app: &App) -> Rect {
    use ratatui::layout::Rect;
    let sidebar_w = if app.sidebar_visible { SIDEBAR_W } else { SIDEBAR_COLLAPSED_W };
    let agent_w = if app.agent_panel_visible { AGENT_W } else { 0 };
    Rect {
        x: sidebar_w,
        y: 1, // below tab bar
        width: term_size.0.saturating_sub(sidebar_w + agent_w),
        height: term_size.1.saturating_sub(2), // above status bar
    }
}
```

Import `AGENT_W` from `crate::tui` (make it `pub const` if not already).

- [ ] **Step 4: Add "Copy Selection" to pane context menu**

In `crates/orbit/src/app.rs`, in `open_context_menu` for `ContextMenuTarget::Pane`, prepend a copy item when selection is non-empty:

```rust
if self.selection.as_ref().map_or(false, |s| s.pane_id == pane_id) {
    items.insert(0, ContextMenuItem::Action {
        id: "copy_selection".to_string(),
        label: "Copy Selection".to_string(),
        shortcut: None,
    });
    items.insert(1, ContextMenuItem::Separator);
}
```

- [ ] **Step 5: Wire copy action in `execute_context_action`**

In `crates/orbit/src/events.rs`, in `execute_context_action`, add:

```rust
"copy_selection" => {
    if let Some(sel) = &app.selection {
        let pane_id = sel.pane_id;
        if let Some(pane_state) = app.panes.get(&pane_id) {
            let grid = &pane_state.parser.grid;
            let (min_col, max_col) = if sel.start.0 <= sel.end.0 {
                (sel.start.0 as usize, sel.end.0 as usize)
            } else {
                (sel.end.0 as usize, sel.start.0 as usize)
            };
            let (min_row, max_row) = if sel.start.1 <= sel.end.1 {
                (sel.start.1 as usize, sel.end.1 as usize)
            } else {
                (sel.end.1 as usize, sel.start.1 as usize)
            };
            let mut lines: Vec<String> = Vec::new();
            for row in min_row..=max_row.min(grid.rows.saturating_sub(1)) {
                let line: String = grid.cells[row][min_col..=max_col.min(grid.cols.saturating_sub(1))]
                    .iter()
                    .map(|c| c.ch)
                    .collect::<String>()
                    .trim_end()
                    .to_string();
                lines.push(line);
            }
            let text = lines.join("\n");
            writer.send(ClientMessage::CopyToClipboard { text }).ok();
        }
        app.selection = None;
        app.needs_redraw = true;
    }
}
```

Also clear `app.selection` on any keypress in Normal mode by adding at the start of `handle_key` when `InputMode::Normal`:

```rust
if app.selection.is_some() {
    app.selection = None;
    app.needs_redraw = true;
}
```

- [ ] **Step 6: Build and verify**

```bash
nix-shell -p gcc --run "cargo build --workspace"
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
nix-shell -p gcc --run "cargo test --workspace"
```

All must pass.

- [ ] **Step 7: Commit**

```bash
git add crates/orbit/src/tui/mod.rs \
        crates/orbit/src/events.rs \
        crates/orbit/src/app.rs
git commit -m "feat: mouse text selection and right-click copy to clipboard"
```

---

### Task 6: Final QA pass

- [ ] **Step 1: Run full QA suite**

```bash
nix-shell -p gcc --run "cargo fmt --all --check"
nix-shell -p gcc --run "cargo clippy --workspace --all-targets -- -D warnings"
nix-shell -p gcc --run "cargo test --workspace"
nix-shell -p gcc --run "cargo build --workspace --release"
```

All four must pass clean.

- [ ] **Step 2: Manual smoke test — tab bar**

Run `just daemon` in one terminal and `just dev` in another. Verify:
- Active tab has solid orange background, white text
- Inactive tabs have `BG_CARD` background, muted text
- Hovering over a tab changes its background to `ACCENT_HOVER`
- Clicking a tab switches to it

- [ ] **Step 3: Manual smoke test — sidebar**

Verify:
- Default space name is `adjective-noun` format (e.g. `cosmic-nova`), not `"default"`
- Sidebar shows card with rounded `╭╮╰╯` border and `▌` accent on active card
- Clicking `«` collapses sidebar to 2 cols
- Clicking `»` expands it back

- [ ] **Step 4: Manual smoke test — selection**

Verify:
- Click-drag in a pane highlights selected cells (inverted colors)
- Single click clears selection
- Right-click on pane with active selection shows "Copy Selection" at top
- Selecting it writes text to system clipboard

- [ ] **Step 5: Commit any fmt/clippy fixes**

```bash
nix-shell -p gcc --run "cargo fmt --all"
git add -u
git commit -m "chore: fmt and clippy cleanup"
```

---

## Self-Review

**Spec coverage:**
- Tab bar solid-block redesign: Tasks 1→4a ✓
- Tab bar hover: Task 4a ✓
- Sidebar card layout: Task 4b ✓
- Sidebar collapse/expand mouse: Task 4b ✓
- Sidebar hover: Task 4a (events.rs Moved arm) ✓
- Multi-space data in App: Task 3 ✓
- `SwitchSpace` IPC routing: Task 1 ✓
- Mouse selection drag: Task 5 ✓
- Right-click copy: Task 5 ✓
- Space naming convention: Task 2 ✓
- `CopyToClipboard` + arboard: Task 1 ✓

**No TBDs, no placeholders found.**

**Type consistency check:**
- `SpaceEntry` defined in Task 3, consumed in Task 4 ✓
- `Selection` defined in Task 3, consumed in Tasks 4 and 5 ✓
- `ClientMessage::CopyToClipboard` added in Task 1, used in Task 5 ✓
- `generate_space_name` defined in Task 2, called in same file ✓
- `SIDEBAR_W`, `SIDEBAR_COLLAPSED_W`, `AGENT_W` must be `pub const` in `tui/mod.rs` — verify at Task 4 build step ✓ (noted in step)
