use std::time::Duration;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: Option<f64>,
    pub url: Option<String>,
}

impl Track {
    pub fn duration_display(&self) -> String {
        match self.duration {
            Some(secs) => {
                let d = Duration::from_secs_f64(secs);
                let m = d.as_secs() / 60;
                let s = d.as_secs() % 60;
                format!("{m}:{s:02}")
            }
            None => "?:??".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Loading,
}

#[derive(Debug, Clone)]
pub struct PlayerStatus {
    pub state: PlaybackState,
    pub position: f64,
    pub duration: f64,
    pub volume: i64,
}

impl Default for PlayerStatus {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            position: 0.0,
            duration: 0.0,
            volume: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Search,
    Choosing,
}

/// Action returned by queue-advance logic — tells the main loop what background
/// work (if any) needs to be kicked off next.
#[derive(Debug)]
pub enum QueueAction {
    FetchUrl(Track),
    FetchRadio { title: String, channel: String },
    None,
}
