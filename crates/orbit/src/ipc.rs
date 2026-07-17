use anyhow::{bail, Context, Result};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::RecvHalf;
use interprocess::local_socket::GenericFilePath;
use orbit_protocol::{Capabilities, ClientMessage, FullState, ServerEvent, PROTOCOL_VERSION};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tracing::debug;

fn default_socket_path() -> std::path::PathBuf {
    let uid = unsafe { libc::getuid() };

    let runtime = format!("/run/user/{uid}");
    let runtime_dir = std::path::Path::new(&runtime);
    if runtime_dir.exists() {
        return runtime_dir.join("orbit.sock");
    }

    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = std::path::Path::new(&dir);
        if p.exists() {
            return p.join("orbit.sock");
        }
    }

    std::env::temp_dir().join(format!("orbit-{uid}.sock"))
}

async fn write_msg(stream: &mut LocalSocketStream, msg: &ClientMessage) -> Result<()> {
    let bytes = orbit_protocol::encode_message(msg)?;
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn read_event(stream: &mut LocalSocketStream) -> Result<ServerEvent> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > orbit_protocol::MAX_MSG_BYTES {
        bail!("message too large: {len}");
    }
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;
    let (msg, _) = orbit_protocol::decode_message(&data)?;
    Ok(msg)
}

pub struct IpcClient {
    pub stream: LocalSocketStream,
}

impl IpcClient {
    pub async fn connect() -> Result<(Self, FullState)> {
        let path = default_socket_path();
        let name = path
            .to_str()
            .context("socket path not UTF-8")?
            .to_fs_name::<GenericFilePath>()?;

        let mut stream = LocalSocketStream::connect(name)
            .await
            .with_context(|| format!("failed to connect to orbitd at {}", path.display()))?;
        debug!("connected to orbitd");

        write_msg(
            &mut stream,
            &ClientMessage::Hello {
                client_version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: PROTOCOL_VERSION,
                capabilities: Capabilities::default(),
            },
        )
        .await?;

        let event = read_event(&mut stream).await?;
        match event {
            ServerEvent::Welcome { state, .. } => {
                debug!("handshake complete");
                Ok((Self { stream }, state))
            }
            ServerEvent::ProtocolError { code, message } => {
                bail!("orbitd rejected connection (code {code}): {message}");
            }
            other => bail!("expected Welcome, got {other:?}"),
        }
    }

    pub async fn send(&mut self, msg: &ClientMessage) -> Result<()> {
        write_msg(&mut self.stream, msg).await
    }

    pub fn into_split(self) -> (IpcWriter, IpcReader) {
        let (recv, send) = self.stream.split();
        let (tx, rx) = mpsc::channel::<ClientMessage>(1024);

        let send_half = send;
        tokio::spawn(async move {
            let mut send = send_half;
            let mut rx = rx;
            while let Some(msg) = rx.recv().await {
                let bytes = match orbit_protocol::encode_message(&msg) {
                    Ok(b) => b,
                    // Skip oversized messages rather than killing the writer.
                    Err(_) => continue,
                };
                if send.write_all(&bytes).await.is_err() {
                    break;
                }
            }
        });

        (IpcWriter { tx }, IpcReader { stream: recv })
    }
}

pub struct IpcWriter {
    tx: mpsc::Sender<ClientMessage>,
}

impl IpcWriter {
    pub async fn send(&self, msg: ClientMessage) -> Result<()> {
        self.tx.send(msg).await.context("IPC writer closed")?;
        Ok(())
    }
}

pub struct IpcReader {
    stream: RecvHalf,
}

impl IpcReader {
    pub async fn recv(&mut self) -> Result<ServerEvent> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > orbit_protocol::MAX_MSG_BYTES {
            bail!("message too large: {len}");
        }
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data).await?;
        let (msg, _) = orbit_protocol::decode_message(&data)?;
        Ok(msg)
    }
}
