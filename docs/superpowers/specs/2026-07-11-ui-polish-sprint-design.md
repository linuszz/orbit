# UI Polish Sprint — Design Spec

**Date:** 2026-07-11
**Status:** Approved
**Scope:** Single PR (Approach A) covering tab bar redesign, spaces sidebar redesign, mouse selection/copy, and space naming convention.
**Out of scope:** Alt-screen / TUI garbling fix (tracked separately).

---

## 1. Tab Bar Redesign

### 1.1 Visual

Every tab is a solid filled block spanning the full 1-row height of the tab bar. No underlines. No separators — color contrast is the divider.

| State    | Background     | Text color   |
|----------|----------------|--------------|
| Active   | `ACCENT`       | `BG_PRIMARY` |
| Inactive | `BG_CARD`      | `FG_MUTED`   |
| Hovered  | `ACCENT_HOVER` | `FG_PRIMARY` |

Layout per tab: `" {name} "` (1-space padding each side). Tabs are adjacent with no gap.

The `+` new-tab button: `FG_MUTED`/`BG_CARD` default, hover → `ACCENT`/`BG_CARD`.

The `[A] Satellites` toggle button on the right: `ACCENT` when panel visible, `FG_MUTED` otherwise. Same hover handling.

### 1.2 Hover state

Crossterm emits `MouseEventKind::Moved` events (mouse capture already enabled). `handle_mouse` hit-tests the tab bar row using accumulated label widths — same logic already used for clicks. On move, updates `app.tab_hovered: Option<usize>`. `needs_redraw` is set **only when hover state changes** to avoid redraws on every pixel move over the pane area.

### 1.3 Changes required

- `app.rs`: add `tab_hovered: Option<usize>` field.
- `tab_bar.rs`: replace current mixed-span style with solid block style; read `app.tab_hovered` for hover rendering.
- `events.rs` `handle_mouse`: add `MouseMove` branch for tab bar row that updates `tab_hovered`.

---

## 2. Spaces Sidebar Redesign

### 2.1 Card layout (expanded, width=14)

Each space is rendered as a rounded card using box-drawing characters:

```
╭─ cosmic-mars ─╮   ← active: BG_CARD bg, ACCENT ▌ left-col bar
│ ~/projects    │   ← cwd truncated, FG_SECONDARY
│ ● 2   3t 2p  │   ← agent status + tab/pane count, FG_MUTED
╰───────────────╯
                    ← 1-row gap between cards
╭─ stellar-void ╮   ← inactive: BG_SECONDARY bg, FG_SECONDARY name
│ ~/web         │
│ ○ 0   1t 1p  │
╰───────────────╯
```

**Active card:** `BG_CARD` background across all 3 content rows. The left column of the first `╭` char is replaced with `▌` in `ACCENT` color to provide the active accent bar.

**Inactive card:** `BG_SECONDARY` background. Name in `FG_SECONDARY`.

**Hovered card:** `BG_TERTIARY` background.

Card height: 3 rows of content + top/bottom border = 5 rows total. 1 blank row between cards.

### 2.2 Collapsed mode (width=2)

Shows space index number centered vertically in a 2-col column. Active space: `ACCENT` bg + `BG_PRIMARY` text. Inactive: `BG_SECONDARY` + `FG_MUTED`. `»` expand hint at the bottom.

### 2.3 Collapse/expand button

- `«` rendered in top-right corner of the expanded sidebar header. Clickable → collapses.
- `»` rendered near bottom of collapsed sidebar. Clickable → expands.
- `prefix+b` shortcut already wired in `events.rs` — no change needed.

### 2.4 Mouse support

- Click a space card → switch to that space. Sends `ClientMessage::SwitchSpace(space_id)` (new message variant — see §5).
- Click `«`/`»` → toggle `app.sidebar_visible`.
- Hover over card → update `app.sidebar_hovered: Option<usize>`, set `needs_redraw` only on change.
- Right-click on card → open Space context menu (already wired: rename/close/move).

### 2.5 Multi-space data in App

`App::from_welcome` currently reads only `state.spaces.first()`. Change to read all spaces and store:

```rust
pub spaces: Vec<SpaceEntry>,   // ordered list
pub active_space_idx: usize,
```

Where `SpaceEntry` holds `{ space_id, name, tab_count, pane_count, cwd }`.

`cwd` is added to `SpaceInfo` in `orbit-protocol` (see §5). Server reads it from the active pane's PTY process working directory (`/proc/{pid}/cwd` on Linux, `proc_pidinfo` on macOS).

### 2.6 Changes required

- `orbit-protocol/src/messages.rs`: add `cwd: String` to `SpaceInfo`.
- `orbit-protocol/src/messages.rs`: add `ClientMessage::SwitchSpace { space_id: SpaceId }`.
- `orbitd/src/session.rs`: populate `cwd` in `collect_space_info()`; handle `SwitchSpace` IPC message.
- `orbitd/src/ipc.rs`: route `SwitchSpace` to session handler.
- `app.rs`: replace single-space fields with `spaces: Vec<SpaceEntry>` + `active_space_idx`; add `sidebar_hovered: Option<usize>`.
- `spaces_sidebar.rs`: full rewrite to render card list.
- `events.rs`: add hover tracking and click routing for sidebar cards and collapse/expand buttons.

---

## 3. Mouse Selection and Copy

### 3.1 Selection state

Added to `App`:

```rust
pub selection: Option<Selection>,

pub struct Selection {
    pub pane_id: PaneId,
    pub start: (u16, u16),   // (col, row) in cell coords within the pane
    pub end: (u16, u16),
    pub active: bool,        // true while drag in progress
}
```

### 3.2 Interaction flow

1. `MouseDown(Left)` on pane area in `Normal` mode: compute cell coords from pixel position and pane rect (via `compute_leaf_areas`). Set `app.selection = Some(Selection { pane_id, start, end: start, active: true })`.
2. `MouseMove` while `selection.active`: update `selection.end`, set `needs_redraw`.
3. `MouseUp(Left)`: set `selection.active = false`. If `start == end`, clear selection (was a click, not a drag).
4. Any keypress, tab switch, or `PaneOutput` for the selected pane: clear `selection`.

Selection is **not** started in `Scroll` mode — drag events there continue adjusting scroll offset.

### 3.3 Rendering

In `render_single_pane` / `render_cells`: when a `Selection` exists for this pane, cells within the selected rectangle have fg and bg inverted (same inversion as the cursor). Selection rectangle is defined by `(min(start.col, end.col), min(start.row, end.row))` to `(max(...), max(...))`.

### 3.4 Copy action

When `selection` is non-empty, the pane right-click context menu gains a `Copy Selection` item at the top. Selecting it:
1. Extracts text from `PaneState` cell grid rows in the selection range (cells joined by empty string, rows joined by `\n`, trailing spaces stripped per row).
2. Sends `ClientMessage::CopyToClipboard { text: String }` to daemon (new message variant — see §5).
3. Daemon writes to system clipboard via `arboard` crate.
4. Clears `app.selection`.

### 3.5 Future config hook

`copy_on_select: bool` under `[ui]` in `config.toml`. When `true`, clipboard write happens at `MouseUp` (step 3) rather than requiring the explicit copy menu action. Default `false`. Not implemented in this sprint — config key reserved, behavior not wired.

### 3.6 Changes required

- `orbit-protocol/src/messages.rs`: add `ClientMessage::CopyToClipboard { text: String }`.
- `orbitd/Cargo.toml`: add `arboard` dependency.
- `orbitd/src/ipc.rs`: handle `CopyToClipboard` — write to clipboard via `arboard`.
- `app.rs`: add `Selection` struct + `selection: Option<Selection>` field; add clear-on-keypress/tab-switch/output logic.
- `tui/mod.rs` `render_cells`: apply selection inversion.
- `events.rs`: add `MouseDown`/`MouseMove`/`MouseUp` selection tracking; add `Copy Selection` item to pane context menu when selection non-empty.

---

## 4. Space Naming Convention

### 4.1 Format

Default space names are generated as `{adjective}-{noun}`, both lowercase, hyphen-separated. Examples from design docs: `cosmic-mars`, `stellar-void`, `quantum-web`.

### 4.2 Word pools

**Adjectives (aerospace/physics themed):**
`cosmic`, `stellar`, `quantum`, `lunar`, `solar`, `orbital`, `deep`, `silent`, `swift`, `apex`, `delta`, `zenith`, `polar`, `radiant`, `binary`, `axial`, `thermal`, `mach`, `ion`, `photon`

**Nouns (celestial/space themed):**
`mars`, `void`, `nova`, `horizon`, `nebula`, `atlas`, `vega`, `lyra`, `cygnus`, `orbit`, `pulse`, `core`, `arc`, `link`, `beacon`, `vector`, `node`, `flux`, `rift`, `zone`

Generates 400 unique combinations. Uniqueness checked against existing space names at creation time; if collision, retry up to 10 times before falling back to `{adjective}-{noun}-{n}`.

### 4.3 Changes required

- `orbitd/src/session.rs`: replace hardcoded `"dev"` with `generate_space_name(existing: &[&str]) -> String` function containing the word pools.
- The first space created when no name is given uses this generator. Named spaces (`orbit dev`, `orbit my-project`) bypass it.

---

## 5. Protocol Changes Summary

All additive — no `PROTOCOL_VERSION` bump required; add `Capabilities` flag if needed.

| Change | Location | Type |
|---|---|---|
| `SpaceInfo::cwd: String` | `orbit-protocol/src/messages.rs` | Additive field |
| `ClientMessage::SwitchSpace { space_id }` | `orbit-protocol/src/messages.rs` | New variant |
| `ClientMessage::CopyToClipboard { text }` | `orbit-protocol/src/messages.rs` | New variant |

---

## 6. Files Changed Summary

| File | Change |
|---|---|
| `orbit-protocol/src/messages.rs` | Add `cwd` to `SpaceInfo`; add `SwitchSpace`, `CopyToClipboard` variants |
| `orbitd/Cargo.toml` | Add `arboard` |
| `orbitd/src/session.rs` | Populate `cwd`; `generate_space_name`; handle `SwitchSpace` |
| `orbitd/src/ipc.rs` | Route `SwitchSpace`, `CopyToClipboard` |
| `orbit/src/app.rs` | Add `spaces`, `active_space_idx`, `tab_hovered`, `sidebar_hovered`, `Selection` |
| `orbit/src/tui/widgets/tab_bar.rs` | Full visual redesign; hover rendering |
| `orbit/src/tui/widgets/spaces_sidebar.rs` | Full rewrite; card layout; collapsed mode |
| `orbit/src/tui/mod.rs` | Selection inversion in `render_cells` |
| `orbit/src/events.rs` | Mouse hover for tabs + sidebar; selection tracking; copy action |

---

## 7. Verification Checklist

- `cargo fmt --all --check` clean
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `cargo test --workspace` passes
- Tab bar: active tab solid ACCENT block, inactive BG_CARD, hover ACCENT_HOVER — verified at all 4 breakpoints
- Sidebar: card list renders, active card has `▌` accent bar, collapsed shows index numbers
- Sidebar: click card switches space, click `«`/`»` toggles collapse
- Mouse selection: drag selects, right-click shows Copy option, clipboard receives text
- New space name is `adjective-noun` format, not `"dev"`
