use anyhow::{bail, Context, Result};
use orbt_protocol::FullState;
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::Arc;

use russh::client::AuthResult;
use russh::keys::known_hosts;

use crate::ipc::{IpcClient, IpcReader, IpcWriter};

pub struct RemoteSpec {
    pub user: String,
    pub host: String,
    pub port: u16,
}

impl RemoteSpec {
    pub fn parse(s: &str) -> Result<Self> {
        // Formats: user@host  or  user@host:port
        let (userhost, port) = if let Some((h, p)) = s.rsplit_once(':') {
            let port: u16 = p.parse().with_context(|| format!("invalid port: {p}"))?;
            (h, port)
        } else {
            (s, 22u16)
        };
        let (user, host) = userhost
            .split_once('@')
            .with_context(|| format!("expected user@host, got: {userhost}"))?;
        if user.is_empty() || host.is_empty() {
            bail!("user and host must be non-empty in: {s}");
        }
        Ok(RemoteSpec {
            user: user.to_string(),
            host: host.to_string(),
            port,
        })
    }
}

// ─── russh client handler ────────────────────────────────────────────────────

struct SshHandler {
    host: String,
    port: u16,
}

impl russh::client::Handler for SshHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool> {
        check_known_hosts_interactive(&self.host, self.port, server_public_key)
    }
}

fn check_known_hosts_interactive(
    host: &str,
    port: u16,
    key: &russh::keys::PublicKey,
) -> Result<bool> {
    match known_hosts::check_known_hosts(host, port, key) {
        Ok(true) => return Ok(true),
        Ok(false) => {}
        Err(russh::keys::Error::KeyChanged { line }) => {
            bail!(
                "SSH host key mismatch for {host}:{port} (known_hosts line {line})!\n\
                 If you trust this host, remove the old entry and retry."
            );
        }
        Err(e) => bail!("known_hosts check error: {e}"),
    }

    // Unknown host — prompt user.
    let fingerprint = key.fingerprint(Default::default());
    eprint!(
        "The authenticity of host '{host}' (port {port}) can't be established.\n\
         Key fingerprint is {fingerprint}.\n\
         Are you sure you want to continue connecting (yes/no)? "
    );
    std::io::stderr().flush().ok();

    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .context("failed to read user input")?;
    let answer = answer.trim().to_lowercase();

    if answer == "yes" || answer == "y" {
        known_hosts::learn_known_hosts(host, port, key)
            .context("failed to save host key to known_hosts")?;
        eprintln!("Warning: Permanently added '{host}:{port}' to known hosts.");
        Ok(true)
    } else {
        Ok(false)
    }
}

// ─── Auth helpers ─────────────────────────────────────────────────────────────

async fn authenticate(handle: &mut russh::client::Handle<SshHandler>, user: &str) -> Result<()> {
    // 1. Try SSH agent via SSH_AUTH_SOCK (Unix only)
    #[cfg(unix)]
    if let Ok(sock_path) = std::env::var("SSH_AUTH_SOCK") {
        if let Ok(stream) = tokio::net::UnixStream::connect(&sock_path).await {
            let mut agent = russh::keys::agent::client::AgentClient::connect(stream);
            if let Ok(identities) = agent.request_identities().await {
                for identity in identities {
                    let pub_key = identity.public_key().into_owned();
                    let hash_alg = handle
                        .best_supported_rsa_hash()
                        .await
                        .ok()
                        .flatten()
                        .flatten();
                    let res = handle
                        .authenticate_publickey_with(user, pub_key, hash_alg, &mut agent)
                        .await;
                    match res {
                        Ok(AuthResult::Success) => return Ok(()),
                        Ok(AuthResult::Failure { .. }) => {}
                        Err(_) => {}
                    }
                }
            }
        }
    }

    // 2. Try key files
    let home = dirs_home();
    for filename in &["id_ed25519", "id_ecdsa", "id_rsa"] {
        let key_path = home.join(".ssh").join(filename);
        if !key_path.exists() {
            continue;
        }
        if let Ok(key_pair) = russh::keys::load_secret_key(&key_path, None) {
            let hash_alg = handle
                .best_supported_rsa_hash()
                .await
                .ok()
                .flatten()
                .flatten();
            let res = handle
                .authenticate_publickey(
                    user,
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg),
                )
                .await;
            match res {
                Ok(AuthResult::Success) => return Ok(()),
                Ok(AuthResult::Failure { .. }) => continue,
                Err(e) => {
                    tracing::debug!("auth with {filename} failed: {e:#}");
                }
            }
        }
    }

    bail!("SSH authentication failed for '{user}' — ensure an SSH key is loaded or SSH_AUTH_SOCK is set");
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/root"))
}

// ─── Remote socket path resolution ───────────────────────────────────────────

async fn resolve_remote_socket(handle: &mut russh::client::Handle<SshHandler>) -> Result<String> {
    let mut channel = handle
        .channel_open_session()
        .await
        .context("failed to open exec channel for socket path resolution")?;
    channel
        .exec(
            true,
            r#"echo "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/orbt.sock""#,
        )
        .await
        .context("failed to exec remote socket path command")?;

    let mut path = String::new();
    loop {
        match channel.wait().await {
            Some(russh::ChannelMsg::Data { data }) => {
                if let Ok(s) = std::str::from_utf8(&data) {
                    path.push_str(s);
                }
            }
            Some(russh::ChannelMsg::ExitStatus { .. }) | None => break,
            _ => {}
        }
    }

    let path = path.trim().to_string();
    if path.is_empty() {
        bail!("failed to resolve remote orbit socket path");
    }
    Ok(path)
}

// ─── Public API ───────────────────────────────────────────────────────────────

pub async fn connect_remote(spec: &RemoteSpec) -> Result<(IpcWriter, IpcReader, FullState)> {
    let config = Arc::new(russh::client::Config::default());
    let handler = SshHandler {
        host: spec.host.clone(),
        port: spec.port,
    };

    let mut handle = russh::client::connect(config, (spec.host.as_str(), spec.port), handler)
        .await
        .with_context(|| format!("SSH connect failed to {}:{}", spec.host, spec.port))?;

    authenticate(&mut handle, &spec.user).await?;

    let remote_socket = resolve_remote_socket(&mut handle).await?;
    tracing::debug!("remote orbit socket: {remote_socket}");

    let channel = handle
        .channel_open_direct_streamlocal(&remote_socket)
        .await
        .with_context(|| format!("failed to open direct-streamlocal channel to {remote_socket}"))?;

    let stream = channel.into_stream();

    // Keep SSH session alive for the duration.
    tokio::spawn(async move {
        let _keep_alive = handle;
        std::future::pending::<()>().await;
    });

    let (ipc_client, state) = IpcClient::connect_with(Box::new(stream))
        .await
        .context("IPC handshake with remote orbtd failed")?;

    let (writer, reader) = ipc_client.into_split();
    Ok((writer, reader, state))
}
