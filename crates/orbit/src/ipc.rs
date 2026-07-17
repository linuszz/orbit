use anyhow::{bail, Context, Result};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::GenericFilePath;
use orbit_protocol::{Capabilities, ClientMessage, FullState, ServerEvent, PROTOCOL_VERSION};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf};
use tokio::sync::mpsc;

pub use orbit_protocol::default_socket_path;

pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> AsyncStream for T {}

type DynStream = Box<dyn AsyncStream>;

async fn write_frame(stream: &mut (impl AsyncWrite + Unpin), msg: &ClientMessage) -> Result<()> {
    let bytes = orbit_protocol::encode_message(msg)?;
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn read_frame(stream: &mut (impl AsyncRead + Unpin)) -> Result<ServerEvent> {
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
    stream: DynStream,
}

impl IpcClient {
    pub async fn connect() -> Result<(Self, FullState)> {
        let path = default_socket_path();
        let name = path
            .to_str()
            .context("socket path not UTF-8")?
            .to_fs_name::<GenericFilePath>()?;

        let stream = LocalSocketStream::connect(name)
            .await
            .with_context(|| format!("failed to connect to orbitd at {}", path.display()))?;

        Self::connect_with(Box::new(stream)).await
    }

    pub async fn connect_with(stream: DynStream) -> Result<(Self, FullState)> {
        let mut client = Self { stream };

        write_frame(
            &mut client.stream,
            &ClientMessage::Hello {
                client_version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: PROTOCOL_VERSION,
                capabilities: Capabilities::default(),
            },
        )
        .await?;

        let event = read_frame(&mut client.stream).await?;
        match event {
            ServerEvent::Welcome { state, .. } => Ok((client, state)),
            ServerEvent::ProtocolError { code, message } => {
                bail!("orbitd rejected connection (code {code}): {message}");
            }
            other => bail!("expected Welcome, got {other:?}"),
        }
    }

    pub fn into_split(self) -> (IpcWriter, IpcReader) {
        let (read_half, write_half) = tokio::io::split(self.stream);
        let (tx, rx) = mpsc::channel::<ClientMessage>(1024);

        tokio::spawn(async move {
            let mut write_half = write_half;
            let mut rx = rx;
            while let Some(msg) = rx.recv().await {
                let bytes = match orbit_protocol::encode_message(&msg) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                if write_half.write_all(&bytes).await.is_err() {
                    break;
                }
            }
        });

        (IpcWriter { tx }, IpcReader { read_half })
    }
}

#[derive(Clone)]
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
    read_half: ReadHalf<DynStream>,
}

impl IpcReader {
    pub async fn recv(&mut self) -> Result<ServerEvent> {
        read_frame(&mut self.read_half).await
    }
}
