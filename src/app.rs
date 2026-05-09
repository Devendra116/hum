use crate::player::Player;
use crate::queue::Queue;
use crate::types::{AppMode, PlaybackState, PlayerStatus, QueueAction, Track};
use anyhow::Result;

pub struct App {
    pub player: Player,
    pub queue: Queue,
    pub status: PlayerStatus,
    pub mode: AppMode,
    pub search_input: String,
    pub choices: Vec<Track>,
    pub message: String,
    pub should_quit: bool,
    pub loading: bool,
    pub spinner_tick: u8,
}

impl App {
    pub async fn new() -> Result<Self> {
        let player = Player::new().await?;
        Ok(Self {
            player,
            queue: Queue::new(),
            status: PlayerStatus::default(),
            mode: AppMode::Normal,
            search_input: String::new(),
            choices: Vec::new(),
            message: String::from("Welcome to hum. Press '/' to search, 'q' to quit."),
            should_quit: false,
            loading: false,
            spinner_tick: 0,
        })
    }

    /// Retreat the queue by one and return what background work is needed.
    pub fn retreat_queue(&mut self) -> QueueAction {
        if self.queue.prev().is_some() {
            if let Some(track) = self.queue.current().cloned() {
                self.loading = true;
                self.status.state = PlaybackState::Loading;
                self.message = format!("Loading: {} — {}", track.title, track.channel);
                return QueueAction::FetchUrl(track);
            }
        }
        QueueAction::None
    }

    pub async fn toggle_pause(&mut self) {
        let _ = self.player.toggle_pause().await;
    }

    pub async fn volume_up(&mut self) {
        let vol = (self.status.volume + 5).min(150);
        let _ = self.player.set_volume(vol).await;
    }

    pub async fn volume_down(&mut self) {
        let vol = (self.status.volume - 5).max(0);
        let _ = self.player.set_volume(vol).await;
    }

    pub async fn seek_forward(&mut self) {
        let _ = self.player.seek_relative(10.0).await;
    }

    pub async fn seek_backward(&mut self) {
        let _ = self.player.seek_relative(-10.0).await;
    }

    pub async fn update_status(&mut self) {
        self.status = self.player.get_status().await;
    }

    pub fn toggle_radio(&mut self) {
        self.queue.radio_mode = !self.queue.radio_mode;
        let state = if self.queue.radio_mode { "ON" } else { "OFF" };
        self.message = format!("Radio mode: {state}");
    }

    pub fn shuffle_queue(&mut self) {
        self.queue.shuffle();
        self.message = "Queue shuffled.".to_string();
    }

    pub async fn shutdown(&mut self) {
        self.player.shutdown().await;
    }

    /// Tick the spinner animation counter — called every event-loop iteration.
    pub fn tick_spinner(&mut self) {
        self.spinner_tick = self.spinner_tick.wrapping_add(1);
    }

    /// Queue a track and start playback from a pre-fetched URL (fast — no network I/O).
    pub async fn start_playing(&mut self, track: Track, url: &str) {
        self.queue.add(track.clone());
        let idx = self.queue.len() - 1;
        self.queue.set_current(idx);
        match self.player.play_url(url).await {
            Ok(()) => self.message = format!("Playing: {} — {}", track.title, track.channel),
            Err(e) => self.message = format!("Playback error: {e}"),
        }
    }

    /// Advance the queue and return what background work is needed next.
    /// Never does network I/O — the caller owns the spawn.
    pub fn advance_queue(&mut self) -> QueueAction {
        if self.queue.next().is_some() {
            if let Some(track) = self.queue.current().cloned() {
                self.loading = true;
                self.status.state = PlaybackState::Loading;
                self.message = format!("Loading: {} — {}", track.title, track.channel);
                return QueueAction::FetchUrl(track);
            }
        }
        if self.queue.radio_mode {
            if let Some(track) = self.queue.current().cloned() {
                self.loading = true;
                self.message = "Loading radio recommendations...".to_string();
                return QueueAction::FetchRadio {
                    title: track.title.clone(),
                    channel: track.channel.clone(),
                };
            }
        }
        self.message = "Queue ended.".to_string();
        self.status.state = PlaybackState::Stopped;
        QueueAction::None
    }
}
