mod app;
mod player;
mod queue;
mod types;
mod ui;
mod youtube;

use anyhow::Result;
use app::App;
use clap::Parser;
use rand::seq::SliceRandom;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::collections::HashSet;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;
use types::{AppMode, PlaybackState, PlaylistHit, QueueAction, Track};

#[derive(Parser)]
#[command(name = "hum", version, about = "A minimal, ad-free terminal music player")]
struct Cli {
    /// Song name to play immediately (e.g. "bohemian rhapsody")
    query: Vec<String>,

    /// Start with radio mode enabled (auto-play related songs)
    #[arg(long)]
    radio: bool,

    /// Load a YouTube playlist URL (or bare list= id) and queue up to 200 tracks
    #[arg(long, value_name = "URL")]
    playlist: Option<String>,
}

/// Results sent back to the main loop from background tasks.
enum BgResult {
    SearchDone {
        query: String,
        result: anyhow::Result<Vec<Track>>,
    },
    UrlFetched {
        track: Track,
        result: anyhow::Result<String>,
    },
    RadioFetched {
        result: anyhow::Result<Vec<Track>>,
    },
    PlaylistLoaded {
        result: anyhow::Result<Vec<Track>>,
    },
    PlaylistSearchDone {
        result: anyhow::Result<Vec<PlaylistHit>>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    check_dependencies()?;

    let mut app = App::new().await?;

    if cli.radio {
        app.queue.radio_mode = true;
    }

    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (bg_tx, bg_rx) = mpsc::channel::<BgResult>(16);

    if let Some(url) = cli.playlist {
        spawn_youtube_resolve(url, bg_tx.clone(), &mut app);
    } else if !cli.query.is_empty() {
        let query = cli.query.join(" ");
        if youtube::should_resolve_as_youtube_link(&query) {
            spawn_youtube_resolve(query, bg_tx.clone(), &mut app);
        } else {
            spawn_search(query, bg_tx.clone(), &mut app);
        }
    }

    let result = run_loop(&mut terminal, &mut app, bg_tx, bg_rx).await;

    app.shutdown().await;
    terminal::disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    bg_tx: mpsc::Sender<BgResult>,
    mut bg_rx: mpsc::Receiver<BgResult>,
) -> Result<()> {
    loop {
        app.tick_spinner();
        terminal.draw(|f| ui::draw(f, app))?;

        // Drain all ready background results without blocking.
        while let Ok(msg) = bg_rx.try_recv() {
            handle_bg_result(app, msg, &bg_tx).await;
        }

        // Short poll so the spinner animates smoothly even during loads.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c')
                {
                    app.should_quit = true;
                }

                match app.mode {
                    AppMode::Search => handle_search_input(app, key.code, &bg_tx),
                    AppMode::Choosing => handle_choice_input(app, key.code, &bg_tx).await,
                    AppMode::ChoosingPlaylist => {
                        handle_playlist_choice_input(app, key.code, &bg_tx).await;
                    }
                    AppMode::Normal => handle_normal_input(app, key.code, &bg_tx).await,
                }
            }
        }

        let prev_state = app.status.state;
        let prev_pos = app.status.position;
        let prev_dur = app.status.duration;
        app.update_status().await;
        check_track_ended(app, &bg_tx, prev_state, prev_pos, prev_dur).await;

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Background result handler
// ---------------------------------------------------------------------------

async fn handle_bg_result(app: &mut App, msg: BgResult, bg_tx: &mpsc::Sender<BgResult>) {
    match msg {
        BgResult::SearchDone { query, result } => {
            app.loading = false;
            match result {
                Err(e) => app.message = format!("Search error: {e}"),
                Ok(tracks) if tracks.is_empty() => {
                    app.message = "No results found.".to_string();
                }
                Ok(tracks) => {
                    if tracks.len() == 1 || is_clear_match(&tracks, &query) {
                        spawn_url_fetch(tracks[0].clone(), bg_tx.clone(), app);
                    } else {
                        app.choices = tracks;
                        app.mode = AppMode::Choosing;
                        app.message =
                            "Multiple matches — press 1, 2, or 3 to pick:".to_string();
                    }
                }
            }
        }

        BgResult::UrlFetched { track, result } => {
            app.loading = false;
            match result {
                Ok(url) => app.start_playing(track, &url).await,
                Err(e) => app.message = format!("Failed to get audio URL: {e}"),
            }
        }

        BgResult::RadioFetched { result } => {
            app.loading = false;
            match result {
                Ok(tracks) if !tracks.is_empty() => {
                    let in_queue: HashSet<_> =
                        app.queue.tracks().iter().map(|t| t.id.as_str()).collect();
                    let mut fresh: Vec<Track> = tracks
                        .into_iter()
                        .filter(|t| !in_queue.contains(t.id.as_str()))
                        .collect();
                    fresh.shuffle(&mut rand::thread_rng());
                    fresh.truncate(5);
                    if fresh.is_empty() {
                        app.message =
                            "Radio: search only returned songs already in queue.".to_string();
                    } else {
                        app.queue.add_many(fresh);
                        app.sync_queue_cursor();
                        let action = app.advance_queue();
                        dispatch_action(app, action, bg_tx);
                    }
                }
                _ => app.message = "Radio: couldn't find related tracks.".to_string(),
            }
        }

        BgResult::PlaylistLoaded { result } => {
            app.loading = false;
            match result {
                Ok(tracks) if !tracks.is_empty() => {
                    app.queue.clear();
                    app.queue.add_many(tracks);
                    app.sync_queue_cursor();
                    let n = app.queue.len();
                    app.message = format!("Queued {n} tracks from playlist.");
                    if let Some(t) = app.queue.current().cloned() {
                        spawn_url_fetch(t, bg_tx.clone(), app);
                    }
                }
                Ok(_) => {
                    app.message = "Playlist is empty or could not be read.".to_string();
                }
                Err(e) => app.message = format!("Playlist error: {e}"),
            }
        }

        BgResult::PlaylistSearchDone { result } => {
            app.loading = false;
            match result {
                Ok(hits) if !hits.is_empty() => {
                    app.playlist_choices = hits;
                    app.mode = AppMode::ChoosingPlaylist;
                    app.message = "Pick a playlist (1–5, Esc to cancel):".to_string();
                }
                Ok(_) => {
                    app.message = "No playlists found for that search.".to_string();
                }
                Err(e) => app.message = format!("Playlist search error: {e}"),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Spawn helpers
// ---------------------------------------------------------------------------

/// Resolve any YouTube URL or bare playlist id (video, mix/RD list, PL playlist, podcast episode, …).
fn spawn_youtube_resolve(raw: String, tx: mpsc::Sender<BgResult>, app: &mut App) {
    app.loading = true;
    app.message = "Resolving YouTube link...".to_string();
    tokio::spawn(async move {
        let result = youtube::resolve_any_youtube_input(&raw).await;
        let _ = tx.send(BgResult::PlaylistLoaded { result }).await;
    });
}

fn spawn_playlist_search(query: String, tx: mpsc::Sender<BgResult>, app: &mut App) {
    app.loading = true;
    app.message = format!("Searching playlists: {query}...");
    tokio::spawn(async move {
        let result = youtube::search_playlists(&query, 5).await;
        let _ = tx.send(BgResult::PlaylistSearchDone { result }).await;
    });
}

fn spawn_search(query: String, tx: mpsc::Sender<BgResult>, app: &mut App) {
    app.loading = true;
    app.message = format!("Searching: {query}...");
    tokio::spawn(async move {
        let result = youtube::search(&query, 3).await;
        let _ = tx.send(BgResult::SearchDone { query, result }).await;
    });
}

fn spawn_url_fetch(track: Track, tx: mpsc::Sender<BgResult>, app: &mut App) {
    app.loading = true;
    app.status.state = PlaybackState::Loading;
    app.message = format!("Loading: {} — {}", track.title, track.channel);
    let video_id = track.id.clone();
    tokio::spawn(async move {
        let result = youtube::get_audio_url(&video_id).await;
        let _ = tx.send(BgResult::UrlFetched { track, result }).await;
    });
}

fn spawn_radio_fetch(title: String, channel: String, tx: mpsc::Sender<BgResult>, app: &mut App) {
    app.loading = true;
    app.message = "Loading radio recommendations...".to_string();
    tokio::spawn(async move {
        let result = youtube::fetch_radio_tracks(&title, &channel).await;
        let _ = tx.send(BgResult::RadioFetched { result }).await;
    });
}

fn dispatch_action(app: &mut App, action: QueueAction, bg_tx: &mpsc::Sender<BgResult>) {
    match action {
        QueueAction::FetchUrl(track) => spawn_url_fetch(track, bg_tx.clone(), app),
        QueueAction::FetchRadio { title, channel } => {
            spawn_radio_fetch(title, channel, bg_tx.clone(), app)
        }
        QueueAction::None => {}
    }
}

// ---------------------------------------------------------------------------
// Track-end auto-advance
// ---------------------------------------------------------------------------

/// True if the previous sample looked like natural EOF (not a mid-track gap).
fn playback_had_reached_end(prev_pos: f64, prev_dur: f64) -> bool {
    if prev_dur <= 0.0 {
        return false;
    }
    let slack = if prev_dur < 3.0 {
        (prev_dur * 0.08).max(0.12)
    } else {
        1.5_f64
    };
    prev_pos >= prev_dur - slack
}

async fn check_track_ended(
    app: &mut App,
    bg_tx: &mpsc::Sender<BgResult>,
    prev_state: PlaybackState,
    prev_pos: f64,
    prev_dur: f64,
) {
    if app.loading {
        return;
    }
    // mpv sets idle-active (Stopped) when a file ends, but duration/time-pos
    // often reset to 0 in that state — so we must use the *previous* tick's
    // position/duration and a Playing→Stopped transition instead of
    // `duration > 0` on the current status.
    if prev_state == PlaybackState::Playing
        && app.status.state == PlaybackState::Stopped
        && app.queue.current().is_some()
        && playback_had_reached_end(prev_pos, prev_dur)
    {
        let action = app.advance_queue();
        dispatch_action(app, action, bg_tx);
    }
}

// ---------------------------------------------------------------------------
// Input handlers (no blocking network calls)
// ---------------------------------------------------------------------------

fn handle_search_input(app: &mut App, key: KeyCode, bg_tx: &mpsc::Sender<BgResult>) {
    match key {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.search_input.clear();
        }
        KeyCode::Enter => {
            let query = app.search_input.trim().to_string();
            app.mode = AppMode::Normal;
            app.search_input.clear();
            if query.is_empty() {
                return;
            }
            if let Some(pl_query) = strip_pl_prefix(&query) {
                let pl_query = pl_query.trim();
                if pl_query.is_empty() {
                    return;
                }
                if youtube::should_resolve_as_youtube_link(pl_query) {
                    spawn_youtube_resolve(pl_query.to_string(), bg_tx.clone(), app);
                } else {
                    spawn_playlist_search(pl_query.to_string(), bg_tx.clone(), app);
                }
            } else if youtube::should_resolve_as_youtube_link(&query) {
                spawn_youtube_resolve(query, bg_tx.clone(), app);
            } else {
                spawn_search(query, bg_tx.clone(), app);
            }
        }
        KeyCode::Backspace => {
            app.search_input.pop();
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
        }
        _ => {}
    }
}

fn strip_pl_prefix(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.len() >= 3 && t[..3].eq_ignore_ascii_case("pl:") {
        Some(t[3..].trim())
    } else {
        None
    }
}

async fn handle_playlist_choice_input(
    app: &mut App,
    key: KeyCode,
    bg_tx: &mpsc::Sender<BgResult>,
) {
    let pick = match key {
        KeyCode::Char('1') => Some(0),
        KeyCode::Char('2') => Some(1),
        KeyCode::Char('3') => Some(2),
        KeyCode::Char('4') => Some(3),
        KeyCode::Char('5') => Some(4),
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.playlist_choices.clear();
            app.message = "Cancelled.".to_string();
            return;
        }
        _ => return,
    };
    if let Some(idx) = pick {
        if idx < app.playlist_choices.len() {
            let url = app.playlist_choices[idx].playlist_url();
            app.playlist_choices.clear();
            app.mode = AppMode::Normal;
            spawn_youtube_resolve(url, bg_tx.clone(), app);
        }
    }
}

async fn handle_choice_input(
    app: &mut App,
    key: KeyCode,
    bg_tx: &mpsc::Sender<BgResult>,
) {
    let pick = match key {
        KeyCode::Char('1') => Some(0),
        KeyCode::Char('2') => Some(1),
        KeyCode::Char('3') => Some(2),
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.choices.clear();
            app.message = "Cancelled.".to_string();
            return;
        }
        _ => return,
    };
    if let Some(idx) = pick {
        if idx < app.choices.len() {
            let track = app.choices[idx].clone();
            app.choices.clear();
            app.mode = AppMode::Normal;
            spawn_url_fetch(track, bg_tx.clone(), app);
        }
    }
}

async fn handle_normal_input(
    app: &mut App,
    key: KeyCode,
    bg_tx: &mpsc::Sender<BgResult>,
) {
    match key {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('/') => {
            app.mode = AppMode::Search;
            app.search_input.clear();
        }
        KeyCode::Char(' ') => app.toggle_pause().await,
        KeyCode::Char('n') => {
            let action = app.advance_queue();
            dispatch_action(app, action, bg_tx);
        }
        KeyCode::Char('p') => {
            let action = app.retreat_queue();
            dispatch_action(app, action, bg_tx);
        }
        KeyCode::Char('s') => app.shuffle_queue(),
        KeyCode::Char('r') => app.toggle_radio(),
        KeyCode::Up => app.move_queue_cursor(-1),
        KeyCode::Down => app.move_queue_cursor(1),
        KeyCode::Enter => {
            let action = app.play_selected();
            dispatch_action(app, action, bg_tx);
        }
        KeyCode::Char('+') | KeyCode::Char('=') => app.volume_up().await,
        KeyCode::Char('-') => app.volume_down().await,
        KeyCode::Right => app.seek_forward().await,
        KeyCode::Left => app.seek_backward().await,
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_clear_match(results: &[Track], query: &str) -> bool {
    if results.len() < 2 {
        return true;
    }
    let first = results[0].title.to_lowercase();
    let q = query.to_lowercase();
    first.contains(&q) || q.contains(first.split(" - ").next().unwrap_or(""))
}

fn check_dependencies() -> Result<()> {
    use std::process::Command;

    Command::new("yt-dlp")
        .arg("--version")
        .output()
        .map_err(|_| {
            anyhow::anyhow!(
                "yt-dlp not found. Install it:\n  pip install yt-dlp\n  or: sudo apt install yt-dlp"
            )
        })?;

    Command::new("mpv")
        .arg("--version")
        .output()
        .map_err(|_| {
            anyhow::anyhow!(
                "mpv not found. Install it:\n  sudo apt install mpv\n  or: brew install mpv"
            )
        })?;

    Ok(())
}
