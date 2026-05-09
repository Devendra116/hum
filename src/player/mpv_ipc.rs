use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub struct MpvIpc {
    tx: mpsc::Sender<IpcRequest>,
    _child: Child,
}

struct IpcRequest {
    payload: String,
    request_id: u64,
    reply: oneshot::Sender<Result<Value>>,
}

impl MpvIpc {
    #[cfg(unix)]
    pub async fn spawn(socket_path: PathBuf) -> Result<Self> {
        let _ = std::fs::remove_file(&socket_path);

        let child = Command::new("mpv")
            .arg("--idle=yes")
            .arg("--no-video")
            .arg("--no-terminal")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("failed to spawn mpv – is it installed?")?;

        // Wait for the socket to appear
        for _ in 0..50 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        if !socket_path.exists() {
            bail!("mpv did not create IPC socket at {}", socket_path.display());
        }

        let stream = UnixStream::connect(&socket_path)
            .await
            .context("failed to connect to mpv IPC socket")?;

        let (tx, rx) = mpsc::channel::<IpcRequest>(64);
        tokio::spawn(Self::io_loop(stream, rx));

        Ok(Self { tx, _child: child })
    }

    #[cfg(windows)]
    pub async fn spawn(_socket_path: PathBuf) -> Result<Self> {
        bail!("Windows build is currently experimental: mpv IPC Unix sockets are not supported yet")
    }

    #[cfg(unix)]
    async fn io_loop(stream: UnixStream, mut rx: mpsc::Receiver<IpcRequest>) {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        let mut pending: HashMap<u64, oneshot::Sender<Result<Value>>> = HashMap::new();

        loop {
            tokio::select! {
                req = rx.recv() => {
                    let Some(req) = req else { break };
                    pending.insert(req.request_id, req.reply);
                    let mut data = req.payload.into_bytes();
                    data.push(b'\n');
                    if writer.write_all(&data).await.is_err() {
                        break;
                    }
                }
                line = lines.next_line() => {
                    match line {
                        Ok(Some(text)) => {
                            if let Ok(msg) = serde_json::from_str::<Value>(&text) {
                                if let Some(id) = msg.get("request_id").and_then(|v| v.as_u64()) {
                                    if let Some(reply) = pending.remove(&id) {
                                        let result = if msg.get("error").and_then(|e| e.as_str()) == Some("success") {
                                            Ok(msg.get("data").cloned().unwrap_or(Value::Null))
                                        } else {
                                            let err_msg = msg.get("error")
                                                .and_then(|e| e.as_str())
                                                .unwrap_or("unknown mpv error");
                                            Err(anyhow::anyhow!("mpv: {}", err_msg))
                                        };
                                        let _ = reply.send(result);
                                    }
                                }
                                // Ignore events (no request_id)
                            }
                        }
                        Ok(None) | Err(_) => break,
                    }
                }
            }
        }
    }

    async fn send_raw(&self, mut msg: Value) -> Result<Value> {
        let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        msg["request_id"] = Value::Number(id.into());
        let payload = serde_json::to_string(&msg)?;

        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(IpcRequest {
                payload,
                request_id: id,
                reply: reply_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("mpv IPC channel closed"))?;

        reply_rx.await.map_err(|_| anyhow::anyhow!("mpv reply dropped"))?
    }

    pub async fn command(&self, args: &[&str]) -> Result<()> {
        let cmd = serde_json::json!({
            "command": args,
        });
        self.send_raw(cmd).await?;
        Ok(())
    }

    pub async fn set_property(&self, name: &str, value: Value) -> Result<()> {
        let cmd = serde_json::json!({
            "command": ["set_property", name, value],
        });
        self.send_raw(cmd).await?;
        Ok(())
    }

    pub async fn get_property(&self, name: &str) -> Result<Value> {
        let cmd = serde_json::json!({
            "command": ["get_property", name],
        });
        self.send_raw(cmd).await
    }

    pub async fn get_property_bool(&self, name: &str) -> Result<bool> {
        self.get_property(name)
            .await
            .and_then(|v| v.as_bool().context("expected bool"))
    }

    pub async fn get_property_f64(&self, name: &str) -> Result<f64> {
        self.get_property(name)
            .await
            .and_then(|v| v.as_f64().context("expected number"))
    }

    pub async fn get_property_i64(&self, name: &str) -> Result<i64> {
        self.get_property(name)
            .await
            .and_then(|v| v.as_i64().context("expected integer"))
    }
}
