//! Integration test: connects to a running orbitd, exercises space/tab/pane operations.

use interprocess::local_socket::GenericFilePath;
use orbit_protocol::*;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

fn socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let rt = format!("/run/user/{uid}");
    if std::path::Path::new(&rt).exists() {
        return PathBuf::from(rt).join("orbit.sock");
    }
    std::env::temp_dir().join(format!("orbit-{uid}.sock"))
}

struct Conn {
    stream: interprocess::local_socket::tokio::Stream,
}

impl Conn {
    async fn connect() -> anyhow::Result<(Self, FullState)> {
        let path = socket_path();
        let name = path.to_str().unwrap().to_fs_name::<GenericFilePath>()?;
        use interprocess::local_socket::tokio::prelude::*;
        let mut stream = interprocess::local_socket::tokio::Stream::connect(name).await?;

        let hello = ClientMessage::Hello {
            client_version: "0.1.0-test".to_string(),
            protocol_version: PROTOCOL_VERSION,
            capabilities: Capabilities::default(),
        };
        let bytes = encode_message(&hello)?;
        stream.write_all(&bytes).await?;

        let event = Self::read_raw(&mut stream).await?;
        match event {
            ServerEvent::Welcome { state, .. } => Ok((Self { stream }, state)),
            other => anyhow::bail!("Expected Welcome, got {other:?}"),
        }
    }

    async fn send(&mut self, msg: &ClientMessage) -> anyhow::Result<()> {
        let bytes = encode_message(msg)?;
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    async fn read_raw(
        stream: &mut interprocess::local_socket::tokio::Stream,
    ) -> anyhow::Result<ServerEvent> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_MSG_BYTES {
            anyhow::bail!("msg too large: {len}");
        }
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;
        let (msg, _) = decode_message(&data)?;
        Ok(msg)
    }

    /// Wait for a specific event type, filtering out PaneOutput noise.
    async fn wait_for<F>(&mut self, deadline_ms: u64, pred: F) -> Option<ServerEvent>
    where
        F: Fn(&ServerEvent) -> bool,
    {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(deadline_ms);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match timeout(remaining, Self::read_raw(&mut self.stream)).await {
                Ok(Ok(ev)) => {
                    if pred(&ev) {
                        return Some(ev);
                    }
                    // Skip PaneOutput and other irrelevant events
                }
                _ => return None,
            }
        }
    }

    /// Get fresh full state from server (direct response, not broadcast).
    async fn full_state(&mut self) -> anyhow::Result<FullState> {
        self.send(&ClientMessage::RequestFullState).await?;
        match self.wait_for(3000, |e| matches!(e, ServerEvent::Welcome { .. })).await {
            Some(ServerEvent::Welcome { state, .. }) => Ok(state),
            _ => anyhow::bail!("No Welcome from RequestFullState"),
        }
    }
}

macro_rules! check {
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            eprintln!("  FAIL: {}", format!($($arg)*));
            return false;
        }
    };
}

async fn test_initial_state(state: &FullState) -> bool {
    println!("[TEST] Initial state validation");
    check!(!state.spaces.is_empty(), "No spaces");
    let space = &state.spaces[0];
    check!(!space.tabs.is_empty(), "No tabs in space {}", space.name);
    check!(!space.panes.is_empty(), "No panes in space {}", space.name);
    for tab in &space.tabs {
        let leaves = tab.layout.leaves();
        check!(!leaves.is_empty(), "Tab {:?} has empty layout", tab.id);
        for pid in &leaves {
            check!(
                space.panes.iter().any(|p| p.id == *pid),
                "Pane {:?} in layout but missing from panes list",
                pid
            );
        }
    }
    println!("  Spaces: {}", state.spaces.len());
    for s in &state.spaces {
        println!("    {} ({:?}): {} tabs, {} panes", s.name, s.id, s.tabs.len(), s.panes.len());
    }
    println!("  PASS");
    true
}

async fn test_request_full_state(conn: &mut Conn) -> bool {
    println!("[TEST] RequestFullState roundtrip");
    match conn.full_state().await {
        Ok(state) => {
            check!(!state.spaces.is_empty(), "Empty state from RequestFullState");
            println!("  PASS");
            true
        }
        Err(e) => {
            eprintln!("  FAIL: {e}");
            false
        }
    }
}

async fn test_new_tab_and_close(conn: &mut Conn) -> bool {
    println!("[TEST] New tab creation and close");

    let state = conn.full_state().await.unwrap();
    let space = &state.spaces[0];
    let initial_tabs = space.tabs.len();

    conn.send(&ClientMessage::NewTab { name: Some("test-newtab".to_string()) })
        .await
        .unwrap();

    // Wait for SpaceUpdated confirming new tab
    let got = conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    check!(got.is_some(), "No SpaceUpdated after NewTab");

    let state = conn.full_state().await.unwrap();
    let space = &state.spaces[0];
    check!(
        space.tabs.len() == initial_tabs + 1,
        "Expected {} tabs, got {}",
        initial_tabs + 1,
        space.tabs.len()
    );

    // Close the new tab's pane
    let new_tab = space.tabs.iter().find(|t| t.name == "test-newtab");
    check!(new_tab.is_some(), "test-newtab not found");
    let new_tab = new_tab.unwrap();
    conn.send(&ClientMessage::CloseTab { tab_id: new_tab.id })
        .await
        .unwrap();
    let got = conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    check!(got.is_some(), "No SpaceUpdated after ClosePane");

    let state = conn.full_state().await.unwrap();
    check!(
        state.spaces[0].tabs.len() == initial_tabs,
        "After close: expected {} tabs, got {}",
        initial_tabs,
        state.spaces[0].tabs.len()
    );

    println!("  PASS");
    true
}

async fn test_split_and_close(conn: &mut Conn) -> bool {
    println!("[TEST] Split pane and close");

    let state = conn.full_state().await.unwrap();
    let space = &state.spaces[0];
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
    let active_pane = tab.active_pane;
    let initial_panes = space.panes.len();

    conn.send(&ClientMessage::SplitPane {
        tab_id: tab.id,
        pane_id: active_pane,
        direction: SplitDir::Horizontal,
    }).await.unwrap();

    let got = conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    check!(got.is_some(), "No SpaceUpdated after SplitPane");

    let state = conn.full_state().await.unwrap();
    let space = &state.spaces[0];
    check!(
        space.panes.len() == initial_panes + 1,
        "Split: expected {} panes, got {}",
        initial_panes + 1,
        space.panes.len()
    );

    // Find and close the new pane
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
    let new_pane = tab.layout.leaves().into_iter().find(|p| *p != active_pane);
    check!(new_pane.is_some(), "No new pane found after split");
    let new_pane = new_pane.unwrap();

    conn.send(&ClientMessage::ClosePane { tab_id: tab.id, pane_id: new_pane }).await.unwrap();
    let got = conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    check!(got.is_some(), "No SpaceUpdated after closing split");

    let state = conn.full_state().await.unwrap();
    check!(
        state.spaces[0].panes.len() == initial_panes,
        "After close: expected {} panes, got {}",
        initial_panes,
        state.spaces[0].panes.len()
    );

    println!("  PASS");
    true
}

async fn test_space_switch(conn: &mut Conn) -> bool {
    println!("[TEST] Space switching");

    let state = conn.full_state().await.unwrap();
    if state.spaces.len() < 2 {
        println!("  SKIP: only 1 space");
        return true;
    }

    let current = state.active_space;
    let other = state.spaces.iter().find(|s| s.id != current).unwrap().id;

    conn.send(&ClientMessage::SwitchSpace { space_id: other }).await.unwrap();
    let got = conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    check!(got.is_some(), "No SpaceUpdated after SwitchSpace");

    // Switch back
    conn.send(&ClientMessage::SwitchSpace { space_id: current }).await.unwrap();
    conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    println!("  PASS");
    true
}

async fn test_tab_switch(conn: &mut Conn) -> bool {
    println!("[TEST] Tab switching");

    let state = conn.full_state().await.unwrap();
    let space = &state.spaces[0];
    if space.tabs.len() < 2 {
        println!("  SKIP: only 1 tab");
        return true;
    }

    let other_tab = space.tabs.iter().find(|t| t.id != space.active_tab).unwrap();
    conn.send(&ClientMessage::SwitchTab { tab_id: other_tab.id }).await.unwrap();
    let got = conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    check!(got.is_some(), "No SpaceUpdated after SwitchTab");

    // Switch back
    conn.send(&ClientMessage::SwitchTab { tab_id: space.active_tab }).await.unwrap();
    conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;

    println!("  PASS");
    true
}

async fn test_pane_input_output(conn: &mut Conn) -> bool {
    println!("[TEST] Pane input/output (echo)");

    let state = conn.full_state().await.unwrap();
    let space = &state.spaces[0];
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
    let pane_id = tab.active_pane;

    let marker = "ORBIT_INTEG_7391";
    let cmd = format!("echo {marker}\r");
    conn.send(&ClientMessage::PaneInput {
        tab_id: tab.id,
        pane_id,
        data: cmd.into_bytes(),
    }).await.unwrap();

    // Wait for output containing our marker
    let got = conn.wait_for(5000, |e| {
        if let ServerEvent::PaneOutput { data, .. } = e {
            String::from_utf8_lossy(data).contains(marker)
        } else {
            false
        }
    }).await;
    check!(got.is_some(), "Did not receive echo output");

    println!("  PASS");
    true
}

async fn test_resize_pane(conn: &mut Conn) -> bool {
    println!("[TEST] Resize pane");
    let state = conn.full_state().await.unwrap();
    let space = &state.spaces[0];
    let tab = space.tabs.iter().find(|t| t.id == space.active_tab).unwrap();
    conn.send(&ClientMessage::ResizePane {
        tab_id: tab.id,
        pane_id: tab.active_pane,
        cols: 80,
        rows: 24,
    }).await.unwrap();
    // No crash = pass. Give it a moment.
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!("  PASS");
    true
}

async fn test_create_space(conn: &mut Conn) -> bool {
    println!("[TEST] Create space");
    conn.send(&ClientMessage::CreateSpace { name: Some("test-integ-space".to_string()) })
        .await
        .unwrap();
    let got = conn.wait_for(3000, |e| matches!(e, ServerEvent::SpaceCreated(_))).await;
    check!(got.is_some(), "No SpaceCreated event");

    // Switch back to first space
    let state = conn.full_state().await.unwrap();
    if let Some(first) = state.spaces.first() {
        conn.send(&ClientMessage::SwitchSpace { space_id: first.id }).await.unwrap();
        conn.wait_for(2000, |e| matches!(e, ServerEvent::SpaceUpdated(_))).await;
    }

    println!("  PASS");
    true
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Orbit Integration Tests ===\n");
    println!("Connecting to orbitd...");

    let (mut conn, state) = Conn::connect().await?;
    println!("Connected. Running tests...\n");

    let mut passed = 0u32;
    let mut failed = 0u32;

    macro_rules! run {
        ($f:expr) => {
            if $f { passed += 1; } else { failed += 1; }
        };
    }

    run!(test_initial_state(&state).await);
    run!(test_request_full_state(&mut conn).await);
    run!(test_space_switch(&mut conn).await);
    run!(test_tab_switch(&mut conn).await);
    run!(test_pane_input_output(&mut conn).await);
    run!(test_resize_pane(&mut conn).await);
    run!(test_split_and_close(&mut conn).await);
    run!(test_new_tab_and_close(&mut conn).await);
    run!(test_create_space(&mut conn).await);

    println!("\n=== Results: {passed} passed, {failed} failed ===");
    if failed > 0 { std::process::exit(1); }
    Ok(())
}
