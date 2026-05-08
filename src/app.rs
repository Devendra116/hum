use crate::player::Player;
use crate::queue::Queue;
use crate::types::{AppMode, PlaybackState, PlayerStatus, Track};
use crate::youtube;
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
        })
    }

    pub async fn play_query(&mut self, query: &str) {
        self.loading = true;
        self.message = format!("Searching: {query}...");

        match youtube::search(query, 3).await {
            Ok(results) if results.is_empty() => {
                self.message = "No results found.".to_string();
                self.loading = false;
            }
            Ok(results) => {
                if results.len() == 1 || self.is_clear_match(&results) {
                    self.play_track(results[0].clone()).await;
                } else {
                    self.choices = results;
                    self.mode = AppMode::Choosing;
                    self.message = "Multiple matches — press 1, 2, or 3 to pick:".to_string();
                }
                self.loading = false;
            }
            Err(e) => {
                self.message = format!("Search error: {e}");
                self.loading = false;
            }
        }
    }

    fn is_clear_match(&self, results: &[Track]) -> bool {
        if results.len() < 2 {
            return true;
        }
        let first = &results[0].title.to_lowercase();
        let query = self.search_input.to_lowercase();
        first.contains(&query) || query.contains(first.split(" - ").next().unwrap_or(""))
    }

    pub async fn play_track(&mut self, track: Track) {
        self.message = format!("Loading: {} — {}", track.title, track.channel);
        self.status.state = PlaybackState::Loading;

        let video_id = track.id.clone();
        self.queue.add(track);
        let idx = self.queue.len() - 1;
        self.queue.set_current(idx);

        match youtube::get_audio_url(&video_id).await {
            Ok(url) => {
                if let Err(e) = self.player.play_url(&url).await {
                    self.message = format!("Playback error: {e}");
                    return;
                }
                if let Some(t) = self.queue.current() {
                    self.message = format!("Playing: {} — {}", t.title, t.channel);
                }
            }
            Err(e) => {
                self.message = format!("Failed to get audio URL: {e}");
            }
        }
    }

    pub async fn play_current(&mut self) {
        if let Some(track) = self.queue.current().cloned() {
            let video_id = track.id.clone();
            self.message = format!("Loading: {} — {}", track.title, track.channel);
            self.status.state = PlaybackState::Loading;

            match youtube::get_audio_url(&video_id).await {
                Ok(url) => {
                    if let Err(e) = self.player.play_url(&url).await {
                        self.message = format!("Playback error: {e}");
                        return;
                    }
                    self.message = format!("Playing: {} — {}", track.title, track.channel);
                }
                Err(e) => {
                    self.message = format!("Failed to get audio URL: {e}");
                }
            }
        }
    }

    pub async fn next_track(&mut self) {
        if self.queue.next().is_some() {
            self.play_current().await;
        } else if self.queue.radio_mode {
            self.load_radio().await;
        } else {
            self.message = "Queue ended.".to_string();
            self.status.state = PlaybackState::Stopped;
        }
    }

    pub async fn prev_track(&mut self) {
        if self.queue.prev().is_some() {
            self.play_current().await;
        }
    }

    pub async fn load_radio(&mut self) {
        if let Some(track) = self.queue.current().cloned() {
            self.message = "Loading radio recommendations...".to_string();
            match youtube::fetch_mix_playlist(&track.id).await {
                Ok(tracks) if !tracks.is_empty() => {
                    self.queue.add_many(tracks);
                    self.queue.next();
                    self.play_current().await;
                }
                _ => {
                    self.message = "Radio: couldn't find related tracks.".to_string();
                }
            }
        }
    }

    pub async fn handle_choice(&mut self, choice: usize) {
        if choice < self.choices.len() {
            let track = self.choices[choice].clone();
            self.choices.clear();
            self.mode = AppMode::Normal;
            self.play_track(track).await;
        }
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

    pub async fn check_track_ended(&mut self) {
        if self.status.state == PlaybackState::Stopped
            && self.queue.current().is_some()
            && self.status.duration > 0.0
        {
            self.next_track().await;
        }
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
}
