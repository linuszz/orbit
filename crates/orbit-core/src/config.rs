use orbit_protocol::SplitDir;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub ui: UiConfig,
    pub agent: AgentConfig,
    pub ssh: SshConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub prefix_key: String,
    pub mouse: bool,
    pub true_color: bool,
    pub flight_deck_mode: FlightDeckMode,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            prefix_key: "ctrl+b".to_string(),
            mouse: true,
            true_color: true,
            flight_deck_mode: FlightDeckMode::Expanded,
        }
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlightDeckMode {
    #[default]
    Expanded,
    Minimal,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub image_protocol: ImageProtocolConfig,
    pub image_max_inline_height: u16,
    pub sidebar_width: u16,
    pub agent_panel_width: u16,
    pub scrollback_lines: u32,
    pub scrollback_persistence: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            image_protocol: ImageProtocolConfig::Auto,
            image_max_inline_height: 20,
            sidebar_width: 14,
            agent_panel_width: 20,
            scrollback_lines: 10_000,
            scrollback_persistence: false,
        }
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageProtocolConfig {
    #[default]
    Auto,
    Kitty,
    Iterm,
    Sixel,
    Blocks,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub auto_detect: bool,
    pub poll_interval_ms: u64,
    pub block_patterns: Vec<String>,
    pub default_model: String,
    pub history_retention_days: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            poll_interval_ms: 500,
            block_patterns: Vec::new(),
            default_model: String::new(),
            history_retention_days: 30,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SshConfig {
    pub clipboard_bridge: bool,
    pub image_bridge: bool,
    pub file_transfer: bool,
    pub tunnel_compression: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaneLayout {
    pub root: PaneLayoutNode,
}

#[derive(Debug, Clone, Deserialize)]
pub enum PaneLayoutNode {
    Single(orbit_protocol::PaneId),
    Split {
        direction: SplitDir,
        first_pct: u32,
        first: Box<PaneLayoutNode>,
        second: Box<PaneLayoutNode>,
    },
}

impl Config {
    pub fn load() -> Result<Self, std::io::Error> {
        let path = config_dir().join("config.toml");
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            toml::from_str(&text)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        } else {
            Ok(Self::default())
        }
    }
}

pub fn config_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("ORBIT_CONFIG_DIR") {
        return std::path::PathBuf::from(dir);
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".orbit");
    }
    std::path::PathBuf::from(".orbit")
}
