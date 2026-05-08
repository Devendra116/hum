use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::types::{AppMode, PlaybackState};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_now_playing(frame, app, chunks[0]);
    draw_progress(frame, app, chunks[1]);
    draw_main_area(frame, app, chunks[2]);
    draw_input(frame, app, chunks[3]);
}

fn draw_now_playing(frame: &mut Frame, app: &App, area: Rect) {
    let (title, info) = match app.queue.current() {
        Some(track) => {
            let state_icon = match app.status.state {
                PlaybackState::Playing => "▶",
                PlaybackState::Paused => "⏸",
                PlaybackState::Loading => "◌",
                PlaybackState::Stopped => "■",
            };
            let radio = if app.queue.radio_mode { " [RADIO]" } else { "" };
            (
                format!(" {state_icon} {} ", track.title),
                format!(" {} • Vol: {}%{radio} ", track.channel, app.status.volume),
            )
        }
        None => (
            " hum — terminal music player ".to_string(),
            " No track loaded ".to_string(),
        ),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

    let para = Paragraph::new(Line::from(info))
        .style(Style::default().fg(Color::DarkGray))
        .block(block);

    frame.render_widget(para, area);
}

fn draw_progress(frame: &mut Frame, app: &App, area: Rect) {
    let ratio = if app.status.duration > 0.0 {
        (app.status.position / app.status.duration).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let pos = format_time(app.status.position);
    let dur = format_time(app.status.duration);

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .ratio(ratio)
        .label(format!("{pos} / {dur}"));

    frame.render_widget(gauge, area);
}

fn draw_main_area(frame: &mut Frame, app: &App, area: Rect) {
    match app.mode {
        AppMode::Choosing => draw_choices(frame, app, area),
        _ => draw_queue(frame, app, area),
    }
}

fn draw_choices(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .choices
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", i + 1),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&track.title, Style::default().fg(Color::White)),
                Span::styled(
                    format!(" — {} [{}]", track.channel, track.duration_display()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" Pick a track (1/2/3) "),
        );

    frame.render_widget(list, area);
}

fn draw_queue(frame: &mut Frame, app: &App, area: Rect) {
    let current_idx = app.queue.current_index();
    let items: Vec<ListItem> = app
        .queue
        .tracks()
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = current_idx == Some(i);
            let prefix = if is_current { "▶ " } else { "  " };
            let style = if is_current {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(&track.title, style),
                Span::styled(
                    format!(" — {} [{}]", track.channel, track.duration_display()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!(" Queue ({}) ", app.queue.len());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(title),
        );

    frame.render_widget(list, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let (content, border_color) = match app.mode {
        AppMode::Search => (
            format!(" /{}", app.search_input),
            Color::Green,
        ),
        _ => (
            format!(" {}", app.message),
            Color::DarkGray,
        ),
    };

    let help = match app.mode {
        AppMode::Search => " Enter: play | Esc: cancel ",
        AppMode::Choosing => " 1/2/3: pick | Esc: cancel ",
        AppMode::Normal => " /: search | space: pause | n/p: next/prev | r: radio | q: quit ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(help, Style::default().fg(Color::DarkGray)));

    let para = Paragraph::new(Line::from(content))
        .style(Style::default().fg(Color::White))
        .block(block);

    frame.render_widget(para, area);
}

fn format_time(secs: f64) -> String {
    let total = secs as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}
