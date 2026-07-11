//! Agent runtime: detection, state machine, registry.
//! See `06_tech-design/04-server-architecture.md` §4 and `07-agent-data-model.md`.

use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use orbit_protocol::{AgentDetail, AgentId, AgentInfo, AgentStatus, PaneId, ServerEvent, SpaceId};
use tokio::sync::{broadcast, RwLock};
use tracing::debug;

/// CLI process names that identify known agent tools.
const AGENT_PATTERNS: &[&str] = &[
    "claude",
    "claude-code",
    "codex",
    "aider",
    "gh-copilot",
    "cursor",
];

/// Script runner executables — when detected, also scan their path arguments.
const SCRIPT_RUNNERS: &[&str] = &["node", "npx", "python", "python3", "deno", "bun"];

pub struct AgentRegistry {
    agents: RwLock<Vec<AgentInfo>>,
    /// Maps AgentId → OS PID for abort support.
    pid_map: RwLock<HashMap<AgentId, u32>>,
    next_id: AtomicU32,
    event_bus: broadcast::Sender<ServerEvent>,
}

impl AgentRegistry {
    pub fn new(event_bus: broadcast::Sender<ServerEvent>) -> Arc<Self> {
        Arc::new(Self {
            agents: RwLock::new(Vec::new()),
            pid_map: RwLock::new(HashMap::new()),
            next_id: AtomicU32::new(0),
            event_bus,
        })
    }

    pub async fn get_agents(&self) -> Vec<AgentInfo> {
        self.agents.read().await.clone()
    }

    /// Remove an agent from the visible list immediately (user-driven dismiss).
    pub async fn remove_agent(&self, agent_id: AgentId) {
        self.agents.write().await.retain(|a| a.id != agent_id);
        self.pid_map.write().await.remove(&agent_id);
        let _ = self.event_bus.send(ServerEvent::AgentRemoved(agent_id));
        debug!("agent removed by user: id={agent_id:?}");
    }

    /// Kill the agent process with SIGTERM.
    pub async fn abort_agent(&self, agent_id: AgentId) {
        let pid = {
            let map = self.pid_map.read().await;
            map.get(&agent_id).copied()
        };
        if let Some(pid) = pid {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            debug!("sent SIGTERM to agent {agent_id:?} pid={pid}");
        }
    }

    /// Start a background task that polls for agent child processes of `shell_pid`.
    /// On non-Linux, this is a no-op because /proc is not available.
    pub fn watch_pane(self: Arc<Self>, pane_id: PaneId, space_id: SpaceId, shell_pid: u32) {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (pane_id, space_id, shell_pid);
            return;
        }

        #[cfg(target_os = "linux")]
        tokio::spawn(async move {
            // pid → (AgentId, start_time)
            let mut tracked: HashMap<u32, (AgentId, Instant)> = HashMap::new();
            let mut poll_count: u32 = 0;

            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                poll_count += 1;

                if !process_exists(shell_pid) {
                    break;
                }

                let children = child_processes(shell_pid);
                let child_set: HashSet<u32> = children.iter().copied().collect();

                // Detect newly appeared agents.
                for cpid in &children {
                    if tracked.contains_key(cpid) {
                        continue;
                    }
                    if let Some(name) = agent_name_for(*cpid) {
                        let id = AgentId(self.next_id.fetch_add(1, Ordering::Relaxed));
                        let model = extract_model(*cpid).unwrap_or_default();
                        let task = extract_task(*cpid);
                        let info = AgentInfo {
                            id,
                            name,
                            space_id,
                            pane_id: Some(pane_id),
                            model,
                            status: AgentStatus::Working,
                            detail: Some(AgentDetail {
                                task,
                                ..Default::default()
                            }),
                        };
                        tracked.insert(*cpid, (id, Instant::now()));
                        self.agents.write().await.push(info.clone());
                        self.pid_map.write().await.insert(id, *cpid);
                        let _ = self.event_bus.send(ServerEvent::AgentCreated(info));
                        debug!("agent detected: pid={cpid} id={id:?}");
                    }
                }

                // Detect agents that have exited.
                let exited: Vec<(u32, AgentId)> = tracked
                    .iter()
                    .filter(|(pid, _)| !child_set.contains(pid))
                    .map(|(&pid, &(id, _))| (pid, id))
                    .collect();

                for (cpid, agent_id) in exited {
                    tracked.remove(&cpid);
                    {
                        let mut agents = self.agents.write().await;
                        if let Some(a) = agents.iter_mut().find(|a| a.id == agent_id) {
                            a.status = AgentStatus::Done;
                        }
                    }
                    self.pid_map.write().await.remove(&agent_id);
                    let _ = self.event_bus.send(ServerEvent::AgentStatusChanged {
                        agent_id,
                        new_status: AgentStatus::Done,
                        detail: None,
                    });
                    debug!("agent done: pid={cpid} id={agent_id:?}");

                    // Remove from the visible list after a brief display window.
                    let registry = Arc::clone(&self);
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        registry.agents.write().await.retain(|a| a.id != agent_id);
                        let _ = registry.event_bus.send(ServerEvent::AgentRemoved(agent_id));
                    });
                }

                // Every 10 polls (5s) emit duration updates for active agents.
                if poll_count % 10 == 0 {
                    for (agent_id, start) in tracked.values() {
                        let duration_s = start.elapsed().as_secs() as u32;
                        // Read current status and update duration_s in a single write lock.
                        let (current_status, current_detail) = {
                            let mut agents = self.agents.write().await;
                            let Some(a) = agents.iter_mut().find(|a| a.id == *agent_id) else {
                                continue;
                            };
                            let d = a.detail.get_or_insert_with(Default::default);
                            d.duration_s = duration_s;
                            (a.status.clone(), a.detail.clone().unwrap())
                        };
                        let _ = self.event_bus.send(ServerEvent::AgentStatusChanged {
                            agent_id: *agent_id,
                            new_status: current_status,
                            detail: Some(current_detail),
                        });
                    }
                }
            }

            // Shell exited — mark any remaining tracked agents as done.
            for (cpid, (agent_id, _)) in &tracked {
                {
                    let mut agents = self.agents.write().await;
                    if let Some(a) = agents.iter_mut().find(|a| a.id == *agent_id) {
                        a.status = AgentStatus::Done;
                    }
                }
                self.pid_map.write().await.remove(agent_id);
                let _ = self.event_bus.send(ServerEvent::AgentStatusChanged {
                    agent_id: *agent_id,
                    new_status: AgentStatus::Done,
                    detail: None,
                });
                debug!("agent shell exited: pid={cpid}");
            }
        });
    }
}

/// Returns the canonical agent name for a PID if it is a known agent process.
fn agent_name_for(pid: u32) -> Option<String> {
    let cmdline = read_cmdline(pid)?;
    let args: Vec<&str> = cmdline.split('\0').filter(|s| !s.is_empty()).collect();

    let exe = args.first().copied().unwrap_or("");
    let basename = std::path::Path::new(exe)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(exe);

    // Direct match on executable basename.
    for &pattern in AGENT_PATTERNS {
        if basename == pattern || basename.starts_with(&format!("{pattern}-")) {
            return Some(pattern.to_string());
        }
    }

    // For script runners, scan subsequent args for agent path components.
    if SCRIPT_RUNNERS.contains(&basename) {
        for arg in args.iter().skip(1) {
            if arg.starts_with('-') {
                continue;
            }
            for component in std::path::Path::new(arg).components() {
                if let std::path::Component::Normal(name) = component {
                    if let Some(s) = name.to_str() {
                        for &pattern in AGENT_PATTERNS {
                            if s == pattern || s.starts_with(&format!("{pattern}-")) {
                                return Some(pattern.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Extract the `--model` value from a process cmdline, if present.
fn extract_model(pid: u32) -> Option<String> {
    let cmdline = read_cmdline(pid)?;
    let args: Vec<&str> = cmdline.split('\0').filter(|s| !s.is_empty()).collect();
    for (i, arg) in args.iter().enumerate() {
        if *arg == "--model" || *arg == "-m" {
            return args.get(i + 1).map(|s| s.to_string());
        }
        if let Some(model) = arg.strip_prefix("--model=") {
            return Some(model.to_string());
        }
    }
    None
}

/// Extract a short task description from positional (non-flag) cmdline args.
fn extract_task(pid: u32) -> Option<String> {
    let cmdline = read_cmdline(pid)?;
    let args: Vec<&str> = cmdline.split('\0').filter(|s| !s.is_empty()).collect();
    let positional: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .copied()
        .collect();
    if positional.is_empty() {
        return None;
    }
    Some(
        positional
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(target_os = "linux")]
fn read_cmdline(pid: u32) -> Option<String> {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

#[cfg(not(target_os = "linux"))]
fn read_cmdline(_pid: u32) -> Option<String> {
    None
}

/// Returns the immediate child PIDs of `parent_pid`.
#[cfg(target_os = "linux")]
fn child_processes(parent_pid: u32) -> Vec<u32> {
    // Fast path: kernel exposes direct children in /proc/<pid>/task/<pid>/children.
    let children_path = format!("/proc/{parent_pid}/task/{parent_pid}/children");
    if let Ok(content) = std::fs::read_to_string(&children_path) {
        let pids: Vec<u32> = content
            .split_whitespace()
            .filter_map(|s| s.parse::<u32>().ok())
            .collect();
        if !pids.is_empty() || content.trim().is_empty() {
            return pids;
        }
    }

    // Fallback: scan every /proc/<n>/status for PPid matching parent_pid.
    let mut result = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return result;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let Ok(pid) = name_str.parse::<u32>() else {
            continue;
        };
        if pid == parent_pid {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
            continue;
        };
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("PPid:") {
                if let Ok(ppid) = rest.trim().parse::<u32>() {
                    if ppid == parent_pid {
                        result.push(pid);
                    }
                }
                break;
            }
        }
    }
    result
}

fn process_exists(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}
