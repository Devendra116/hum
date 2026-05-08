mod mpv_ipc;

use anyhow::Result;
use std::path::PathBuf;

use crate::types::{PlaybackState, PlayerStatus};
pub use mpv_ipc::MpvIpc;

pub struct Player {
    ipc: MpvIpc,
}

impl Player {
    pub async fn new() -> Result<Self> {
        let socket_path = Self::socket_path();
        let ipc = MpvIpc::spawn(socket_path).await?;
        Ok(Self { ipc })
    }

    fn socket_path() -> PathBuf {
        let dir = std::env::temp_dir();
        dir.join(format!("hum-mpv-{}.sock", std::process::id()))
    }

    pub async fn play_url(&mut self, url: &str) -> Result<()> {
        self.ipc.command(&["loadfile", url, "replace"]).await
    }

    pub async fn pause(&mut self) -> Result<()> {
        self.ipc.set_property("pause", serde_json::Value::Bool(true)).await
    }

    pub async fn resume(&mut self) -> Result<()> {
        self.ipc.set_property("pause", serde_json::Value::Bool(false)).await
    }

    pub async fn toggle_pause(&mut self) -> Result<()> {
        let paused = self.ipc.get_property_bool("pause").await.unwrap_or(false);
        if paused {
            self.resume().await
        } else {
            self.pause().await
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.ipc.command(&["stop"]).await
    }

    pub async fn set_volume(&mut self, vol: i64) -> Result<()> {
        let vol = vol.clamp(0, 150);
        self.ipc.set_property("volume", serde_json::Value::Number(vol.into())).await
    }

    pub async fn seek_relative(&mut self, secs: f64) -> Result<()> {
        self.ipc.command(&["seek", &secs.to_string(), "relative"]).await
    }

    pub async fn get_status(&mut self) -> PlayerStatus {
        let paused = self.ipc.get_property_bool("pause").await.unwrap_or(true);
        let idle = self.ipc.get_property_bool("idle-active").await.unwrap_or(true);
        let position = self.ipc.get_property_f64("time-pos").await.unwrap_or(0.0);
        let duration = self.ipc.get_property_f64("duration").await.unwrap_or(0.0);
        let volume = self.ipc.get_property_i64("volume").await.unwrap_or(100);

        let state = if idle {
            PlaybackState::Stopped
        } else if paused {
            PlaybackState::Paused
        } else {
            PlaybackState::Playing
        };

        PlayerStatus {
            state,
            position,
            duration,
            volume,
        }
    }

    pub async fn is_finished(&mut self) -> bool {
        self.ipc.get_property_bool("idle-active").await.unwrap_or(true)
    }

    pub async fn shutdown(&mut self) {
        let _ = self.ipc.command(&["quit"]).await;
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let path = Self::socket_path();
        let _ = std::fs::remove_file(&path);
    }
}
