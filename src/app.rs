use crate::player::Player;
use crate::queue::Queue;
use crate::types::{AppMode, PlaybackState, PlayerStatus, PlaylistHit, QueueAction, Track};
use anyhow::Result;

pub struct App {
    pub player: Player,
    pub queue: Queue,
    pub status: PlayerStatus,
    pub mode: AppMode,
    pub search_input: String,
    pub choices: Vec<Track>,
    pub playlist_choices: Vec<PlaylistHit>,
    pub message: String,
    pub should_quit: bool,
    pub loading: bool,
    pub spinner_tick: u8,
    pub queue_cursor: Option<usize>,
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
            playlist_choices: Vec::new(),
            message: String::from(
                "Welcome to hum. Press '/' to search, paste any YouTube link, or pl:… for playlist search.",
            ),
            should_quit: false,
            loading: false,
            spinner_tick: 0,
            queue_cursor: None,
        })
    }

    /// Retreat the queue by one and return what background work is needed.
    pub fn retreat_queue(&mut self) -> QueueAction {
        if self.queue.prev().is_some() {
            if let Some(track) = self.queue.current().cloned() {
                self.queue_cursor = self.queue.current_index();
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
        self.queue_cursor = self.queue.current_index();
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
        self.queue_cursor = Some(idx);
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
                self.queue_cursor = self.queue.current_index();
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

    /// Keep queue cursor valid when queue content changes.
    pub fn sync_queue_cursor(&mut self) {
        let len = self.queue.len();
        self.queue_cursor = match (self.queue_cursor, len) {
            (_, 0) => None,
            (Some(i), _) if i < len => Some(i),
            _ => self.queue.current_index().or(Some(0)),
        };
    }

    /// Move queue selection cursor up/down for large playlists.
    pub fn move_queue_cursor(&mut self, delta: isize) {
        let len = self.queue.len();
        if len == 0 {
            self.queue_cursor = None;
            return;
        }
        let cur = self
            .queue_cursor
            .or(self.queue.current_index())
            .unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, (len - 1) as isize) as usize;
        self.queue_cursor = Some(next);
    }

    /// Start playing the currently selected queue row.
    pub fn play_selected(&mut self) -> QueueAction {
        let Some(idx) = self.queue_cursor.or(self.queue.current_index()) else {
            return QueueAction::None;
        };
        self.queue.set_current(idx);
        self.queue_cursor = Some(idx);
        if let Some(track) = self.queue.current().cloned() {
            self.loading = true;
            self.status.state = PlaybackState::Loading;
            self.message = format!("Loading: {} — {}", track.title, track.channel);
            return QueueAction::FetchUrl(track);
        }
        QueueAction::None
    }
}
