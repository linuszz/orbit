//! TUI scenario tests: daily shell usage, pane/tab/space workflows, rapid ops, render snapshots.
//!
//! Requires a running orbtd (`just daemon` or `cargo run -p orbtd`).
//! Optionally dumps visual render snapshots via render_preview.
//!
//! Usage:
//!   cargo run --bin scenario_test                # all scenarios
//!   cargo run --bin scenario_test -- --snap      # with render snapshots at key points
//!   cargo run --bin scenario_test -- --plain     # no color in output

use interprocess::local_socket::GenericFilePath;
use orbt_protocol::*;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

// ─────────────────────────────────────────────── connection helper

fn socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let rt = format!("/run/user/{uid}");
    if std::path::Path::new(&rt).exists() {
        return PathBuf::from(rt).join("orbt.sock");
    }
    std::env::temp_dir().join(format!("orbt-{uid}.sock"))
}

struct Conn {
    stream: interprocess::local_socket::tokio::Stream,
}

impl Conn {
    async fn connect() -> anyhow::Result<(Self, FullState)> {
        let path = socket_path();
        let name = path.to_str().unwrap().to_fs_name::<GenericFilePath>()?;
        use interprocess::local_socket::tokio::prelude::*;
        let stream = interprocess::local_socket::tokio::Stream::connect(name)
            .await
            .map_err(|e| anyhow::anyhow!("Cannot connect to orbtd at {path:?}: {e}"))?;
        let mut this = Self { stream };
        let hello = ClientMessage::Hello {
            client_version: "0.1.0-scenario".to_string(),
            protocol_version: PROTOCOL_VERSION,
            capabilities: Capabilities::default(),
        };
        this.send_msg(&hello).await?;
        let ev = this.read_one().await?;
        match ev {
            ServerEvent::Welcome { state, .. } => Ok((this, state)),
            other => anyhow::bail!("Expected Welcome, got {other:?}"),
        }
    }

    async fn send_msg(&mut self, msg: &ClientMessage) -> anyhow::Result<()> {
        let bytes = encode_message(msg)?;
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    async fn read_one(&mut self) -> anyhow::Result<ServerEvent> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_MSG_BYTES {
            anyhow::bail!("message too large: {len}");
        }
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data).await?;
        let (msg, _) = decode_message(&data)?;
        Ok(msg)
    }

    /// Wait up to `ms` for an event matching `pred`, ignoring others.
    async fn wait_for<F>(&mut self, ms: u64, pred: F) -> Option<ServerEvent>
    where
        F: Fn(&ServerEvent) -> bool,
    {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match timeout(remaining, self.read_one()).await {
                Ok(Ok(ev)) if pred(&ev) => return Some(ev),
                Ok(Ok(_)) => {} // skip irrelevant
                _ => return None,
            }
        }
    }

    async fn full_state(&mut self) -> anyhow::Result<FullState> {
        self.send_msg(&ClientMessage::RequestFullState).await?;
        match self.wait_for(4000, |e| matches!(e, ServerEvent::Welcome { .. })).await {
            Some(ServerEvent::Welcome { state, .. }) => Ok(state),
            _ => anyhow::bail!("No Welcome from RequestFullState"),
        }
    }

    /// Active space in full state.
    async fn active_space(&mut self) -> anyhow::Result<SpaceInfo> {
        let state = self.full_state().await?;
        state
            .spaces
            .into_iter()
            .find(|s| s.id == state.active_space)
            .ok_or_else(|| anyhow::anyhow!("active space missing from state"))
    }

    /// Send raw bytes to the active pane of the active space.
    async fn type_in(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let space = self.active_space().await?;
        let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
        self.send_msg(&ClientMessage::PaneInput {
            tab_id: tab.id,
            pane_id: tab.active_pane,
            data: data.to_vec(),
        })
        .await
    }

    /// Send a shell command (appends \r) and wait for output containing `expect`.
    async fn run_cmd(&mut self, cmd: &str, expect: &str, ms: u64) -> bool {
        let full = format!("{cmd}\r");
        if self.type_in(full.as_bytes()).await.is_err() {
            return false;
        }
        let needle = expect.to_string();
        self.wait_for(ms, |e| {
            matches!(e, ServerEvent::PaneOutput { data, .. }
                if String::from_utf8_lossy(data).contains(&needle))
        })
        .await
        .is_some()
    }
}

// ─────────────────────────────────────────────── test harness

struct Results {
    passed: u32,
    failed: u32,
    skipped: u32,
    snap: bool,
}

impl Results {
    fn new(snap: bool) -> Self {
        Self { passed: 0, failed: 0, skipped: 0, snap }
    }

    fn record(&mut self, name: &str, result: ScenResult) {
        match result {
            ScenResult::Pass => {
                println!("  \u{25CF} PASS  {name}");
                self.passed += 1;
            }
            ScenResult::Fail(why) => {
                println!("  \u{25C9} FAIL  {name}: {why}");
                self.failed += 1;
            }
            ScenResult::Skip(why) => {
                println!("  \u{25CC} SKIP  {name}: {why}");
                self.skipped += 1;
            }
        }
    }
}

enum ScenResult {
    Pass,
    Fail(String),
    Skip(String),
}

macro_rules! pass { () => { return ScenResult::Pass }; }
macro_rules! fail { ($($t:tt)*) => { return ScenResult::Fail(format!($($t)*)) }; }
macro_rules! skip { ($($t:tt)*) => { return ScenResult::Skip(format!($($t)*)) }; }
macro_rules! ensure {
    ($e:expr, $($t:tt)*) => {
        if !$e { return ScenResult::Fail(format!($($t)*)); }
    };
}

/// Dump a render snapshot (requires render_preview binary to be built).
fn snap(label: &str) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/debug/render_preview");
    if !bin.exists() {
        return;
    }
    println!("\n--- render snapshot: {label} ---");
    let _ = std::process::Command::new(&bin)
        .args(["160", "40", "--plain"])
        .status();
    println!("--- end snapshot ---\n");
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 1: Initial state sanity
// ═══════════════════════════════════════════════════════════════════
async fn scen_initial_state(conn: &mut Conn) -> ScenResult {
    let state = match conn.full_state().await {
        Ok(s) => s,
        Err(e) => fail!("{e}"),
    };
    ensure!(!state.spaces.is_empty(), "no spaces");
    for space in &state.spaces {
        ensure!(!space.tabs.is_empty(), "space {} has no tabs", space.name);
        ensure!(!space.panes.is_empty(), "space {} has no panes", space.name);
        for tab in &space.tabs {
            let leaves = tab.layout.leaves();
            ensure!(!leaves.is_empty(), "tab {:?} empty layout", tab.id);
            for pid in &leaves {
                ensure!(
                    space.panes.iter().any(|p| p.id == *pid),
                    "pane {:?} in layout but absent from panes list",
                    pid
                );
            }
        }
    }
    println!("    {} space(s):", state.spaces.len());
    for s in &state.spaces {
        println!("      {:?} \"{}\"  {} tab(s)  {} pane(s)", s.id, s.name, s.tabs.len(), s.panes.len());
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 2: Echo command — basic PTY round-trip
// ═══════════════════════════════════════════════════════════════════
async fn scen_echo(conn: &mut Conn) -> ScenResult {
    let marker = format!("ORBT_SCEN_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_millis());
    if !conn.run_cmd(&format!("echo {marker}"), &marker, 5000).await {
        fail!("echo output not received");
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 3: pwd — verify shell is alive
// ═══════════════════════════════════════════════════════════════════
async fn scen_pwd(conn: &mut Conn) -> ScenResult {
    if !conn.run_cmd("pwd", "/", 5000).await {
        fail!("pwd output not received");
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 4: Multi-line output (ls)
// ═══════════════════════════════════════════════════════════════════
async fn scen_ls(conn: &mut Conn) -> ScenResult {
    // ls / produces multiple lines; check we get something back
    if !conn.run_cmd("ls /", "usr", 5000).await {
        fail!("ls / output not received");
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 5: Long output (seq)
// ═══════════════════════════════════════════════════════════════════
async fn scen_long_output(conn: &mut Conn) -> ScenResult {
    // Generate 200 lines; verify last line arrives
    if !conn.run_cmd("seq 1 200", "200", 8000).await {
        fail!("seq output not received");
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 6: Environment variable roundtrip
// ═══════════════════════════════════════════════════════════════════
async fn scen_env_var(conn: &mut Conn) -> ScenResult {
    let val = "ORBT_TEST_VAL_42";
    if !conn.run_cmd(&format!("export ORBT_TESTVAR={val}; echo $ORBT_TESTVAR"), val, 5000).await {
        fail!("env var not echoed back");
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 7: Horizontal split, type in new pane, close it
// ═══════════════════════════════════════════════════════════════════
async fn scen_split_h(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();
    let orig_pane = tab.active_pane;
    let orig_count = space.panes.len();

    conn.send_msg(&ClientMessage::SplitPane {
        tab_id: tab.id,
        pane_id: orig_pane,
        direction: SplitDir::Horizontal,
    }).await.unwrap();

    ensure!(
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await.is_some(),
        "no SpaceUpdated after SplitPane H"
    );

    let space = conn.active_space().await.unwrap();
    ensure!(space.panes.len() == orig_count + 1, "pane count unchanged after split");

    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
    let new_pane = tab.layout.leaves().into_iter().find(|p| *p != orig_pane);
    ensure!(new_pane.is_some(), "new pane not in layout");

    // Type in the new pane
    let marker = "SPLIT_H_OK";
    conn.send_msg(&ClientMessage::PaneInput {
        tab_id: tab.id,
        pane_id: new_pane.unwrap(),
        data: format!("echo {marker}\r").into_bytes(),
    }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::PaneOutput { data, .. }
        if String::from_utf8_lossy(data).contains(marker))).await;

    // Close new pane
    conn.send_msg(&ClientMessage::ClosePane { tab_id: tab.id, pane_id: new_pane.unwrap() })
        .await.unwrap();
    ensure!(
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await.is_some(),
        "no SpaceUpdated after ClosePane"
    );

    let space = conn.active_space().await.unwrap();
    ensure!(space.panes.len() == orig_count, "pane not removed");
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 8: Vertical split, verify layout, close
// ═══════════════════════════════════════════════════════════════════
async fn scen_split_v(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();
    let orig_pane = tab.active_pane;
    let orig_count = space.panes.len();

    conn.send_msg(&ClientMessage::SplitPane {
        tab_id: tab.id,
        pane_id: orig_pane,
        direction: SplitDir::Vertical,
    }).await.unwrap();
    ensure!(
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await.is_some(),
        "no SpaceUpdated after SplitPane V"
    );

    let space = conn.active_space().await.unwrap();
    ensure!(space.panes.len() == orig_count + 1, "pane count unchanged after V split");

    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
    let new_pane = tab.layout.leaves().into_iter().find(|p| *p != orig_pane);
    ensure!(new_pane.is_some(), "new pane not found");

    conn.send_msg(&ClientMessage::ClosePane { tab_id: tab.id, pane_id: new_pane.unwrap() })
        .await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    ensure!(space.panes.len() == orig_count, "pane not closed after V split");
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 9: Multiple splits — build a 1+2 layout in one tab
// ═══════════════════════════════════════════════════════════════════
async fn scen_multi_split(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();
    let tab_id = tab.id;
    let p0 = tab.active_pane;
    // Count leaves only in this tab (not the whole space, which has panes from other tabs)
    let orig_tab_leaves = tab.layout.leaves().len();

    // Split H → p0 | p1
    conn.send_msg(&ClientMessage::SplitPane { tab_id, pane_id: p0, direction: SplitDir::Horizontal }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == tab_id).unwrap();
    let p1 = tab.layout.leaves().into_iter().find(|p| *p != p0);
    ensure!(p1.is_some(), "p1 not created");
    let p1 = p1.unwrap();

    // Split V on p0 → p0 / p2
    conn.send_msg(&ClientMessage::SplitPane { tab_id, pane_id: p0, direction: SplitDir::Vertical }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == tab_id).unwrap();
    let all = tab.layout.leaves();
    let expected = orig_tab_leaves + 2;
    ensure!(all.len() == expected, "layout has {} leaves, expected {}", all.len(), expected);

    // Send a distinct echo to each pane in this tab, verify output
    let panes: Vec<PaneId> = all.clone();
    for (i, pid) in panes.iter().enumerate() {
        let marker = format!("MULTI_SPLIT_PANE_{i}");
        conn.send_msg(&ClientMessage::PaneInput {
            tab_id,
            pane_id: *pid,
            data: format!("echo {marker}\r").into_bytes(),
        }).await.unwrap();
        let m2 = marker.clone();
        let ok = conn.wait_for(4000, |e| matches!(e, ServerEvent::PaneOutput { data, .. }
            if String::from_utf8_lossy(data).contains(&m2))).await.is_some();
        ensure!(ok, "pane {i} ({pid:?}) echo not received");
    }

    // Close all except original (p2 first, then p1)
    for pid in panes.iter().filter(|p| **p != p0 && **p != p1) {
        conn.send_msg(&ClientMessage::ClosePane { tab_id, pane_id: *pid }).await.unwrap();
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    }
    conn.send_msg(&ClientMessage::ClosePane { tab_id, pane_id: p1 }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == tab_id).unwrap();
    ensure!(
        tab.layout.leaves().len() == orig_tab_leaves,
        "not all extra panes closed: {} leaves left",
        tab.layout.leaves().len()
    );
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 10: New tab, switch, type, switch back, close
// ═══════════════════════════════════════════════════════════════════
async fn scen_tab_lifecycle(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let orig_active = space.active_tab;
    let orig_tabs = space.tabs.len();

    conn.send_msg(&ClientMessage::NewTab { name: Some("scen-tab".to_string()) }).await.unwrap();
    ensure!(
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await.is_some(),
        "no SpaceUpdated after NewTab"
    );

    let space = conn.active_space().await.unwrap();
    ensure!(space.tabs.len() == orig_tabs + 1, "tab not created");

    let new_tab = space.tabs.iter().find(|t| t.name == "scen-tab");
    ensure!(new_tab.is_some(), "scen-tab not found");
    let new_tab_id = new_tab.unwrap().id;

    // Switch to new tab
    conn.send_msg(&ClientMessage::SwitchTab { tab_id: new_tab_id }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    // Type in new tab
    if !conn.run_cmd("echo NEWTAB_ALIVE", "NEWTAB_ALIVE", 5000).await {
        fail!("echo in new tab failed");
    }

    // Switch back
    conn.send_msg(&ClientMessage::SwitchTab { tab_id: orig_active }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    ensure!(space.active_tab == orig_active, "active tab not restored");

    // Close new tab
    conn.send_msg(&ClientMessage::CloseTab { tab_id: new_tab_id }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    ensure!(space.tabs.len() == orig_tabs, "tab count not restored after close");
    ensure!(space.tabs.iter().all(|t| t.id != new_tab_id), "closed tab still present");
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 11: Multiple tabs — create 3, cycle through each
// ═══════════════════════════════════════════════════════════════════
async fn scen_multi_tab(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let orig_active = space.active_tab;
    let orig_tabs = space.tabs.len();
    let mut created: Vec<TabId> = Vec::new();

    for i in 0..3 {
        conn.send_msg(&ClientMessage::NewTab { name: Some(format!("mt-{i}")) }).await.unwrap();
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
        let space = conn.active_space().await.unwrap();
        if let Some(t) = space.tabs.iter().find(|t| t.name == format!("mt-{i}")) {
            created.push(t.id);
        }
    }
    ensure!(created.len() == 3, "only {} tabs created", created.len());

    // Cycle each: switch, echo, verify
    for (i, tid) in created.iter().enumerate() {
        conn.send_msg(&ClientMessage::SwitchTab { tab_id: *tid }).await.unwrap();
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
        let marker = format!("MT_TAB_{i}");
        if !conn.run_cmd(&format!("echo {marker}"), &marker, 5000).await {
            fail!("echo in tab {i} failed");
        }
    }

    // Close all created tabs
    for tid in &created {
        conn.send_msg(&ClientMessage::CloseTab { tab_id: *tid }).await.unwrap();
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    }

    // Switch back to original
    conn.send_msg(&ClientMessage::SwitchTab { tab_id: orig_active }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    ensure!(space.tabs.len() == orig_tabs, "tabs not cleaned up: {} left", space.tabs.len());
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 12: Space switch (needs ≥2 spaces)
// ═══════════════════════════════════════════════════════════════════
async fn scen_space_switch(conn: &mut Conn) -> ScenResult {
    let state = conn.full_state().await.unwrap();
    if state.spaces.len() < 2 {
        skip!("only 1 space (need ≥2)")
    }

    let orig = state.active_space;
    let other = state.spaces.iter().find(|s| s.id != orig).unwrap().id;

    conn.send_msg(&ClientMessage::SwitchSpace { space_id: other }).await.unwrap();
    ensure!(
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await.is_some(),
        "no SpaceUpdated after switch to other space"
    );

    // Type in switched space to verify it's live
    if !conn.run_cmd("echo SPACE_SWITCHED", "SPACE_SWITCHED", 5000).await {
        fail!("echo after space switch failed");
    }

    conn.send_msg(&ClientMessage::SwitchSpace { space_id: orig }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let state = conn.full_state().await.unwrap();
    ensure!(state.active_space == orig, "active space not restored");
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 13: Create new space, use it, close it
// ═══════════════════════════════════════════════════════════════════
async fn scen_space_lifecycle(conn: &mut Conn) -> ScenResult {
    let state = conn.full_state().await.unwrap();
    let orig_space = state.active_space;
    let orig_count = state.spaces.len();

    conn.send_msg(&ClientMessage::CreateSpace { name: Some("scen-newspace".to_string()) })
        .await.unwrap();

    let ev = conn.wait_for(4000, |e| matches!(e, ServerEvent::SpaceCreated(_))).await;
    ensure!(ev.is_some(), "no SpaceCreated event");

    let new_id = match ev.unwrap() {
        ServerEvent::SpaceCreated(info) => info.id,
        _ => unreachable!(),
    };

    // Switch into it and verify shell works
    conn.send_msg(&ClientMessage::SwitchSpace { space_id: new_id }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    if !conn.run_cmd("echo NEWSPACE_OK", "NEWSPACE_OK", 6000).await {
        fail!("shell not alive in new space");
    }

    // Switch back then close
    conn.send_msg(&ClientMessage::SwitchSpace { space_id: orig_space }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    conn.send_msg(&ClientMessage::CloseSpace { space_id: new_id }).await.unwrap();
    let closed = conn.wait_for(4000, |e| matches!(e, ServerEvent::SpaceClosed(_))).await;
    ensure!(closed.is_some(), "no SpaceClosed event");

    let state = conn.full_state().await.unwrap();
    ensure!(state.spaces.len() == orig_count, "space count not restored");
    ensure!(state.spaces.iter().all(|s| s.id != new_id), "closed space still present");
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 14: Tab reorder
// ═══════════════════════════════════════════════════════════════════
async fn scen_tab_reorder(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    if space.tabs.len() < 2 {
        // Create a second tab to reorder
        conn.send_msg(&ClientMessage::NewTab { name: Some("reorder-b".to_string()) }).await.unwrap();
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    }

    let space = conn.active_space().await.unwrap();
    ensure!(space.tabs.len() >= 2, "not enough tabs to reorder");

    let first_tab = space.tabs[0].id;
    let was_added = space.tabs.iter().any(|t| t.name == "reorder-b");

    conn.send_msg(&ClientMessage::ReorderTab { tab_id: first_tab, to_index: 1 })
        .await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    ensure!(space.tabs.len() >= 2, "tabs disappeared after reorder");

    // Cleanup if we created a tab
    if was_added {
        if let Some(t) = space.tabs.iter().find(|t| t.name == "reorder-b") {
            conn.send_msg(&ClientMessage::CloseTab { tab_id: t.id }).await.unwrap();
            conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
        }
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 15: Rapid split + close (stress test — 5 rounds)
// ═══════════════════════════════════════════════════════════════════
async fn scen_rapid_splits(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();
    let tab_id = tab.id;
    let orig_pane = tab.active_pane;
    let orig_count = space.panes.len();

    for round in 0..5 {
        let dir = if round % 2 == 0 { SplitDir::Horizontal } else { SplitDir::Vertical };
        conn.send_msg(&ClientMessage::SplitPane { tab_id, pane_id: orig_pane, direction: dir })
            .await.unwrap();
        conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

        let space = conn.active_space().await.unwrap();
        let tab = space.tabs.iter().find(|t| t.id == tab_id).unwrap();
        let new_pane = tab.layout.leaves().into_iter().find(|p| *p != orig_pane);
        if let Some(np) = new_pane {
            conn.send_msg(&ClientMessage::ClosePane { tab_id, pane_id: np }).await.unwrap();
            conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
        }
    }

    let space = conn.active_space().await.unwrap();
    ensure!(
        space.panes.len() == orig_count,
        "after rapid splits: expected {} panes, got {}",
        orig_count, space.panes.len()
    );
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 16: Rapid tab create + close (stress test — 5 rounds)
// ═══════════════════════════════════════════════════════════════════
async fn scen_rapid_tabs(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let orig_count = space.tabs.len();
    let orig_active = space.active_tab;

    for i in 0..5 {
        conn.send_msg(&ClientMessage::NewTab { name: Some(format!("rapid-{i}")) }).await.unwrap();
        conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

        let space = conn.active_space().await.unwrap();
        if let Some(t) = space.tabs.iter().find(|t| t.name == format!("rapid-{i}")) {
            let tid = t.id;
            conn.send_msg(&ClientMessage::CloseTab { tab_id: tid }).await.unwrap();
            conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
        }
    }

    // Restore original active tab
    conn.send_msg(&ClientMessage::SwitchTab { tab_id: orig_active }).await.unwrap();
    conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    ensure!(
        space.tabs.len() == orig_count,
        "after rapid tabs: expected {} tabs, got {}",
        orig_count, space.tabs.len()
    );
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 17: Pane resize IPC — no crash
// ═══════════════════════════════════════════════════════════════════
async fn scen_resize(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
    let tab_id = tab.id;
    let pane_id = tab.active_pane;

    for (cols, rows) in [(40, 12), (80, 24), (120, 40), (200, 50), (20, 5)] {
        conn.send_msg(&ClientMessage::ResizePane { tab_id, pane_id, cols, rows }).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Restore sensible size
    conn.send_msg(&ClientMessage::ResizePane { tab_id, pane_id, cols: 80, rows: 24 }).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 18: FocusPane — switch focus across panes
// ═══════════════════════════════════════════════════════════════════
async fn scen_focus_pane(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();
    let tab_id = tab.id;
    let orig_pane = tab.active_pane;

    // Create a split so we have something to focus
    conn.send_msg(&ClientMessage::SplitPane { tab_id, pane_id: orig_pane, direction: SplitDir::Horizontal }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == tab_id).unwrap();
    let new_pane = tab.layout.leaves().into_iter().find(|p| *p != orig_pane);
    ensure!(new_pane.is_some(), "split failed for focus test");
    let new_pane = new_pane.unwrap();

    // Focus new, then original
    conn.send_msg(&ClientMessage::FocusPane { tab_id, pane_id: new_pane }).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    conn.send_msg(&ClientMessage::FocusPane { tab_id, pane_id: orig_pane }).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Clean up
    conn.send_msg(&ClientMessage::ClosePane { tab_id, pane_id: new_pane }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 19: Concurrent pane output — two panes emitting simultaneously
// ═══════════════════════════════════════════════════════════════════
async fn scen_concurrent_output(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();
    let tab_id = tab.id;
    let p0 = tab.active_pane;

    conn.send_msg(&ClientMessage::SplitPane { tab_id, pane_id: p0, direction: SplitDir::Horizontal }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == tab_id).unwrap();
    let p1 = tab.layout.leaves().into_iter().find(|p| *p != p0);
    ensure!(p1.is_some(), "split failed for concurrent test");
    let p1 = p1.unwrap();

    // Send long output to both panes simultaneously
    let cmd0 = "seq 1 50\r";
    let cmd1 = "seq 51 100\r";
    conn.send_msg(&ClientMessage::PaneInput { tab_id, pane_id: p0, data: cmd0.as_bytes().to_vec() }).await.unwrap();
    conn.send_msg(&ClientMessage::PaneInput { tab_id, pane_id: p1, data: cmd1.as_bytes().to_vec() }).await.unwrap();

    // Wait for both outputs
    let mut saw_50 = false;
    let mut saw_100 = false;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(6000);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() { break; }
        match timeout(remaining, conn.read_one()).await {
            Ok(Ok(ServerEvent::PaneOutput { data, .. })) => {
                let s = String::from_utf8_lossy(&data);
                if s.contains("50\n") || s.contains("50\r") { saw_50 = true; }
                if s.contains("100\n") || s.contains("100\r") { saw_100 = true; }
                if saw_50 && saw_100 { break; }
            }
            _ => break,
        }
    }

    ensure!(saw_50, "pane0 seq 1-50 output not received");
    ensure!(saw_100, "pane1 seq 51-100 output not received");

    // Cleanup
    conn.send_msg(&ClientMessage::ClosePane { tab_id, pane_id: p1 }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 20: RequestFullState consistency — layout invariants
// ═══════════════════════════════════════════════════════════════════
async fn scen_state_invariants(conn: &mut Conn) -> ScenResult {
    let state = conn.full_state().await.unwrap();

    let mut issues: Vec<String> = Vec::new();
    for space in &state.spaces {
        // active_tab must exist in the tabs list (skip empty spaces — they are
        // transitional states where all tabs/panes have been closed)
        if space.tabs.is_empty() {
            continue;
        }
        if !space.tabs.iter().any(|t| t.id == space.active_tab) {
            issues.push(format!("space {:?} active_tab {:?} not in tabs list", space.id, space.active_tab));
        }

        // every pane in every tab layout must exist in panes list
        let pane_ids: std::collections::HashSet<PaneId> = space.panes.iter().map(|p| p.id).collect();
        for tab in &space.tabs {
            for pid in tab.layout.leaves() {
                if !pane_ids.contains(&pid) {
                    issues.push(format!("tab {:?} layout has pane {:?} missing from panes", tab.id, pid));
                }
            }
            // active_pane must be in layout
            if !tab.layout.leaves().contains(&tab.active_pane) {
                issues.push(format!("tab {:?} active_pane {:?} not in layout", tab.id, tab.active_pane));
            }
        }

        // no orphaned panes (in panes list but no tab references them)
        for pane in &space.panes {
            let referenced = space.tabs.iter().any(|t| t.layout.leaves().contains(&pane.id));
            if !referenced {
                issues.push(format!("pane {:?} in space {:?} not referenced by any tab", pane.id, space.id));
            }
        }
    }

    if !issues.is_empty() {
        fail!("state invariant violations:\n{}", issues.join("\n    "));
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 21: Close non-active space does NOT detach
// Regression: SpaceClosed was triggering should_quit unconditionally.
// ═══════════════════════════════════════════════════════════════════
async fn scen_close_nonactive_space_no_detach(conn: &mut Conn) -> ScenResult {
    let state = conn.full_state().await.unwrap();
    let orig_space = state.active_space;
    let orig_count = state.spaces.len();

    // Create a second space
    conn.send_msg(&ClientMessage::CreateSpace { name: Some("close-test".to_string()) })
        .await.unwrap();
    let ev = conn.wait_for(4000, |e| matches!(e, ServerEvent::SpaceCreated(_))).await;
    ensure!(ev.is_some(), "no SpaceCreated event");
    let new_id = match ev.unwrap() {
        ServerEvent::SpaceCreated(info) => info.id,
        _ => unreachable!(),
    };

    // Switch back to the original space so we are NOT in the space we'll close
    conn.send_msg(&ClientMessage::SwitchSpace { space_id: orig_space }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    // Close the non-active space
    conn.send_msg(&ClientMessage::CloseSpace { space_id: new_id }).await.unwrap();
    let closed = conn.wait_for(5000, |e| matches!(e, ServerEvent::SpaceClosed(_))).await;
    ensure!(closed.is_some(), "no SpaceClosed event after close");

    // The connection must still be alive — verify by echoing in the original space
    if !conn.run_cmd("echo STILL_ALIVE", "STILL_ALIVE", 5000).await {
        fail!("session detached after closing non-active space");
    }

    let state = conn.full_state().await.unwrap();
    ensure!(state.spaces.len() == orig_count, "space count not restored");
    ensure!(state.active_space == orig_space, "active space changed unexpectedly");
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 22: cursor_visible survives SpaceUpdated (hide via PTY output)
// Regression: SpaceUpdated was resetting cursor_visible to snapshot value,
// overwriting client-side VT state (e.g. CSI ?25l hide cursor).
// ═══════════════════════════════════════════════════════════════════
async fn scen_cursor_visible_survives_space_updated(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();
    let tab_id = tab.id;
    let pane_id = tab.active_pane;

    // Make the PTY *output* CSI ?25l so the server VT parser processes it.
    // printf is universally available and outputs raw escape sequences.
    let hide_cmd = "printf '\\033[?25l'\r";
    conn.send_msg(&ClientMessage::PaneInput {
        tab_id,
        pane_id,
        data: hide_cmd.as_bytes().to_vec(),
    }).await.unwrap();
    // Give the server time to process PTY output
    tokio::time::sleep(Duration::from_millis(400)).await;

    // Trigger a SpaceUpdated by creating+closing a tab in the same space
    conn.send_msg(&ClientMessage::NewTab { name: Some("cursor-probe".to_string()) }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    let space = conn.active_space().await.unwrap();
    if let Some(t) = space.tabs.iter().find(|t| t.name == "cursor-probe") {
        conn.send_msg(&ClientMessage::CloseTab { tab_id: t.id }).await.unwrap();
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    }
    conn.send_msg(&ClientMessage::SwitchTab { tab_id }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    // The server-side VT now has cursor_visible=false for that pane.
    // Verify via full_state: cursor_visible in the pane's cell_grid must be false.
    let state = conn.full_state().await.unwrap();
    let active_space = state.spaces.iter().find(|s| s.id == state.active_space).unwrap();
    let pane_info = active_space.panes.iter().find(|p| p.id == pane_id);
    ensure!(pane_info.is_some(), "active pane not found in full state");
    ensure!(
        !pane_info.unwrap().cell_grid.cursor_visible,
        "cursor_visible should be false after CSI ?25l, got true"
    );

    // Restore cursor so subsequent tests aren't affected
    conn.send_msg(&ClientMessage::PaneInput {
        tab_id,
        pane_id,
        data: "printf '\\033[?25h'\r".as_bytes().to_vec(),
    }).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 23b: erase-with-background-color (CSI K and CSI J use current_bg)
// Regression: erase_line/erase_display filled cells with Cell::default() (no
// background) instead of current_bg. This caused selection highlights in yazi,
// status bars in vim, etc. to be invisible — only explicitly-drawn characters
// with colored fg appeared, while the background fill was lost.
// ═══════════════════════════════════════════════════════════════════
async fn scen_erase_with_background(conn: &mut Conn) -> ScenResult {
    let space = conn.active_space().await.unwrap();
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap().clone();

    // Run a shell command that: sets bg=red (SGR 41), erases to EOL (CSI K),
    // then resets. If erase_line preserves current_bg, the server grid will
    // have bg=Ansi(1) cells in that row.
    // We use printf to output raw escape sequences as PTY output.
    let cmd = r#"printf '\033[41m\033[K\033[0m'"#;
    conn.send_msg(&ClientMessage::PaneInput {
        tab_id: tab.id,
        pane_id: tab.active_pane,
        data: format!("{cmd}\r").into_bytes(),
    }).await.unwrap();
    // Wait for PTY to process and server VT to update
    tokio::time::sleep(Duration::from_millis(400)).await;

    // Check server state: the row where printf ran should have bg=Ansi(1) on erased cells.
    // After the command, the cursor moves to a new prompt line — so we check all rows
    // for any row that has Ansi(1) background (the erased row keeps its color).
    let state = conn.full_state().await.unwrap();
    let active = state.spaces.iter().find(|s| s.id == state.active_space).unwrap();
    let pane = active.panes.iter().find(|p| p.id == tab.active_pane).unwrap();
    let g = &pane.cell_grid;
    let cols = g.cols as usize;
    let rows = g.rows as usize;
    let has_red_bg = (0..rows).any(|row| {
        let row_cells = &g.cells[row * cols..(row + 1) * cols];
        row_cells.iter().any(|c| c.bg == orbt_protocol::TermColor::Ansi(1))
    });
    ensure!(has_red_bg, "erase_line did not preserve background color: no Ansi(1) cells found anywhere in pane");
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 24: Welcome loads panes from the ACTIVE space, not spaces[0].
// Regression: from_welcome() called state.spaces.first() instead of finding
// the space matching state.active_space — if the active space is not the
// first in the list, panes from the wrong space were loaded into self.panes,
// and PaneOutput for the actual active space was silently dropped (cursor
// never updated, screen never redrawn, cursor invisible).
// ═══════════════════════════════════════════════════════════════════
async fn scen_welcome_loads_active_space_panes(conn: &mut Conn) -> ScenResult {
    // Create a second space (which becomes active) and verify we can echo into it
    conn.send_msg(&ClientMessage::CreateSpace {
        name: Some("active-space-test".to_string()),
    }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let state = conn.full_state().await.unwrap();
    let active_space = state.spaces.iter().find(|s| s.id == state.active_space).unwrap();
    ensure!(active_space.name == "active-space-test", "active space should be the newly created one");

    // The active space's pane must be visible — run echo and get output
    let got_output = conn.run_cmd("echo active-pane-ok", "active-pane-ok", 3000).await;
    ensure!(got_output, "echo in newly created (active) space did not produce output — panes from wrong space were loaded");

    // Connect a SECOND client, verify it also sees the active space panes correctly
    match Conn::connect().await {
        Err(e) => fail!("second connect failed: {e}"),
        Ok((_conn2, state2)) => {
            let active2 = state2.spaces.iter().find(|s| s.id == state2.active_space).unwrap();
            ensure!(active2.name == "active-space-test", "second client sees wrong active space: {:?}", active2.name);
        }
    }

    // Clean up: switch back and close the test space
    if let Some(orig) = state.spaces.iter().find(|s| s.id != state.active_space) {
        conn.send_msg(&ClientMessage::SwitchSpace { space_id: orig.id }).await.unwrap();
        conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    }
    conn.send_msg(&ClientMessage::CloseSpace { space_id: active_space.id }).await.unwrap();
    conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceClosed(_))).await;
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Scenario 25: Render snapshot — visual state after all ops
// ═══════════════════════════════════════════════════════════════════
async fn scen_render_snapshot(_conn: &mut Conn) -> ScenResult {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/debug/render_preview");
    if !bin.exists() {
        skip!("render_preview binary not built (run cargo build first)")
    }
    let out = std::process::Command::new(&bin)
        .args(["160", "40", "--plain"])
        .output();
    match out {
        Err(e) => fail!("render_preview failed: {e}"),
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            fail!("render_preview exited non-zero: {stderr}");
        }
        Ok(output) => {
            let rendered = String::from_utf8_lossy(&output.stdout);
            // Basic sanity: should contain sidebar and pane border characters
            let ok = rendered.contains('│') || rendered.contains("SPACES");
            ensure!(ok, "render output missing expected TUI structure");
            println!("    render size: {} chars, {} lines", rendered.len(), rendered.lines().count());
        }
    }
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// Agent detection
// ═══════════════════════════════════════════════════════════════════

/// Run a background agent command and wait for AgentCreated, then verify
/// the expected protocol classification.  Returns (agent_id, protocol).
async fn run_agent_cmd(
    conn: &mut Conn,
    cmd: &str,
    timeout_ms: u64,
) -> Option<(AgentId, AgentProtocol)> {
    let full = format!("{cmd}\r");
    if conn.type_in(full.as_bytes()).await.is_err() {
        return None;
    }
    let ev = conn
        .wait_for(timeout_ms, |e| matches!(e, ServerEvent::AgentCreated(_)))
        .await?;
    match ev {
        ServerEvent::AgentCreated(info) => Some((info.id, info.protocol)),
        _ => None,
    }
}

async fn scen_agent_detection_claude(conn: &mut Conn) -> ScenResult {
    // Verify `claude` is available on PATH
    if !conn
        .run_cmd("command -v claude && echo FOUND", "FOUND", 3000)
        .await
    {
        skip!("claude not found on PATH")
    }

    // Launch claude as a background job so the shell stays interactive
    let result =
        run_agent_cmd(conn, "claude --print 'say hi in one word' &", 8000).await;

    match result {
        None => fail!("no AgentCreated event within 8 s — detection not working"),
        Some((_id, protocol)) => {
            ensure!(
                protocol == AgentProtocol::Heuristic,
                "claude should be Heuristic, got {protocol:?}"
            );
            println!("    detected claude with protocol={protocol:?}");
        }
    }

    // Clean up: let the background job finish (wait for shell prompt to return)
    tokio::time::sleep(Duration::from_millis(3000)).await;
    pass!()
}

async fn scen_agent_detection_opencode(conn: &mut Conn) -> ScenResult {
    // Verify opencode is available
    if !conn
        .run_cmd("command -v opencode && echo FOUND", "FOUND", 3000)
        .await
    {
        skip!("opencode not found on PATH")
    }

    // Launch opencode's version check as a background job
    let result = run_agent_cmd(conn, "opencode --version &", 6000).await;

    match result {
        None => fail!("no AgentCreated event within 6 s — opencode detection not working"),
        Some((_id, protocol)) => {
            ensure!(
                protocol == AgentProtocol::AcpCapable,
                "opencode should be AcpCapable, got {protocol:?}"
            );
            println!("    detected opencode with protocol={protocol:?}");
        }
    }

    tokio::time::sleep(Duration::from_millis(2000)).await;
    pass!()
}

// ═══════════════════════════════════════════════════════════════════
// main
// ═══════════════════════════════════════════════════════════════════
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let do_snap = args.contains(&"--snap".to_string());

    println!("=== Orbit Scenario Tests ===\n");
    println!("Connecting to orbtd...");
    let (mut conn, _) = Conn::connect().await?;
    println!("Connected.\n");

    let mut r = Results::new(do_snap);

    macro_rules! run {
        ($label:expr, $f:expr) => {{
            print!("  [{:>2}] {:.<55}", r.passed + r.failed + r.skipped + 1, $label);
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let result = $f(&mut conn).await;
            println!();
            r.record($label, result);
            if r.snap { snap($label); }
        }};
    }

    // ── shell basics ────────────────────────────────────────────────
    println!("[ Shell basics ]");
    run!("initial_state",       scen_initial_state);
    run!("echo_roundtrip",      scen_echo);
    run!("pwd",                 scen_pwd);
    run!("ls_multiline",        scen_ls);
    run!("long_output_seq200",  scen_long_output);
    run!("env_var_roundtrip",   scen_env_var);

    println!("\n[ Pane management ]");
    run!("split_horizontal",    scen_split_h);
    run!("split_vertical",      scen_split_v);
    run!("multi_split_2x2",     scen_multi_split);
    run!("focus_pane",          scen_focus_pane);
    run!("resize_pane",         scen_resize);
    run!("concurrent_output",   scen_concurrent_output);

    println!("\n[ Tab management ]");
    run!("tab_lifecycle",       scen_tab_lifecycle);
    run!("multi_tab_3",         scen_multi_tab);
    run!("tab_reorder",         scen_tab_reorder);

    println!("\n[ Space management ]");
    run!("space_switch",                     scen_space_switch);
    run!("space_lifecycle",                  scen_space_lifecycle);
    run!("close_nonactive_space_no_detach",  scen_close_nonactive_space_no_detach);
    run!("cursor_visible_survives_update",   scen_cursor_visible_survives_space_updated);

    println!("\n[ Stability / stress ]");
    run!("rapid_splits_5x",     scen_rapid_splits);
    run!("rapid_tabs_5x",       scen_rapid_tabs);
    run!("state_invariants",    scen_state_invariants);

    run!("erase_preserves_background", scen_erase_with_background);
    run!("welcome_loads_active_space", scen_welcome_loads_active_space_panes);

    println!("\n[ Agent detection ]");
    run!("agent_detection_claude",    scen_agent_detection_claude);
    run!("agent_detection_opencode",  scen_agent_detection_opencode);

    println!("\n[ Visual rendering ]");
    run!("render_snapshot",     scen_render_snapshot);

    let total = r.passed + r.failed + r.skipped;
    println!("\n═══════════════════════════════════════════════════════");
    println!("  Results: {total} total  ● {} passed  ◉ {} failed  ◌ {} skipped",
             r.passed, r.failed, r.skipped);
    println!("═══════════════════════════════════════════════════════");

    if r.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
