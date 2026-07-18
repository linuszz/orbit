//! Protocol-layer errors. Library crate exposes structured errors so callers can
//! `match` on specific failure modes. Per v5 pre-implementation checklist GAP 4.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("protocol version mismatch: client={client}, server={server}")]
    VersionMismatch { client: u32, server: u32 },

    #[error("message too large: {0} bytes (max {1})")]
    MessageTooLarge(usize, usize),

    #[error("decode failed: {0}")]
    DecodeFailed(String),

    #[error("peer UID {peer} does not match server UID {server}")]
    PeerUidMismatch { peer: u32, server: u32 },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
