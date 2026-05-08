mod app;
mod player;
mod queue;
mod types;
mod ui;
mod youtube;

use anyhow::Result;
use app::App;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::time::Duration;
use types::AppMode;

#[derive(Parser)]
#[command(name = "hum", version, about = "A minimal, ad-free terminal music player")]
struct Cli {
    /// Song name to play immediately (e.g. "bohemian rhapsody")
    query: Vec<String>,

    /// Start with radio mode enabled (auto-play related songs)
    #[arg(long)]
    radio: bool,
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

    if !cli.query.is_empty() {
        let query = cli.query.join(" ");
        app.play_query(&query).await;
    }

    let result = run_loop(&mut terminal, &mut app).await;

    app.shutdown().await;
    terminal::disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.should_quit = true;
                }

                match app.mode {
                    AppMode::Search => handle_search_input(app, key.code).await,
                    AppMode::Choosing => handle_choice_input(app, key.code).await,
                    AppMode::Normal => handle_normal_input(app, key.code).await,
                }
            }
        }

        app.update_status().await;
        app.check_track_ended().await;

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

async fn handle_normal_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('/') => {
            app.mode = AppMode::Search;
            app.search_input.clear();
        }
        KeyCode::Char(' ') => app.toggle_pause().await,
        KeyCode::Char('n') => app.next_track().await,
        KeyCode::Char('p') => app.prev_track().await,
        KeyCode::Char('s') => app.shuffle_queue(),
        KeyCode::Char('r') => app.toggle_radio(),
        KeyCode::Char('+') | KeyCode::Char('=') => app.volume_up().await,
        KeyCode::Char('-') => app.volume_down().await,
        KeyCode::Right => app.seek_forward().await,
        KeyCode::Left => app.seek_backward().await,
        _ => {}
    }
}

async fn handle_search_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.search_input.clear();
        }
        KeyCode::Enter => {
            let query = app.search_input.clone();
            app.mode = AppMode::Normal;
            if !query.is_empty() {
                app.play_query(&query).await;
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

async fn handle_choice_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('1') => app.handle_choice(0).await,
        KeyCode::Char('2') => app.handle_choice(1).await,
        KeyCode::Char('3') => app.handle_choice(2).await,
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.choices.clear();
            app.message = "Cancelled.".to_string();
        }
        _ => {}
    }
}

fn check_dependencies() -> Result<()> {
    use std::process::Command;

    Command::new("yt-dlp")
        .arg("--version")
        .output()
        .map_err(|_| anyhow::anyhow!(
            "yt-dlp not found. Install it:\n  pip install yt-dlp\n  or: sudo apt install yt-dlp"
        ))?;

    Command::new("mpv")
        .arg("--version")
        .output()
        .map_err(|_| anyhow::anyhow!(
            "mpv not found. Install it:\n  sudo apt install mpv\n  or: brew install mpv"
        ))?;

    Ok(())
}
