use anyhow::{Context, Result};
use rand::Rng;
use tokio::process::Command;

use crate::types::{PlaylistHit, Track};

/// How many search hits to pull before dedupe/shuffle (radio picks a subset).
const RADIO_SEARCH_DEPTH: u8 = 20;

/// Max videos pulled from a single playlist (safety cap).
pub const PLAYLIST_MAX_ENTRIES: usize = 200;

/// YouTube web search "Playlists" filter (`sp` query param).
const PLAYLISTS_TAB_SP: &str = "EgIQAw%253D%253D";

fn is_youtube_video_id(s: &str) -> bool {
    s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn encode_results_query(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for b in query.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                use std::fmt::Write;
                let _ = write!(&mut out, "%{b:02X}");
            }
        }
    }
    out
}

/// Any youtube.com / youtu.be / music.youtube.com link (video, mix, shorts, podcast episode, …).
pub fn looks_like_youtube_url(raw: &str) -> bool {
    let lower = raw.trim().to_lowercase();
    lower.contains("youtube.com/")
        || lower.contains("youtu.be/")
        || lower.contains("music.youtube.com")
}

/// True if pasted text should skip text search and go through yt-dlp URL resolution.
pub fn should_resolve_as_youtube_link(raw: &str) -> bool {
    let s = raw.trim();
    !s.is_empty() && (looks_like_youtube_url(s) || looks_like_playlist_input(s))
}

fn youtube_url_has_playlist_param(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("list=")
        || lower.contains("youtube.com/playlist")
        || lower.contains("music.youtube.com/playlist")
}

/// True if this looks like a YouTube playlist URL or bare list id (`PL…`, `RD…` mix, etc.).
pub fn looks_like_playlist_input(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() {
        return false;
    }
    let lower = s.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return youtube_url_has_playlist_param(s);
    }
    let first_two: Option<&str> = s.get(..2);
    let first_three: Option<&str> = s.get(..3);
    let long_enough = s.len() >= 13;
    // User / mix / radio list ids (RD… is YouTube “start radio” / endless mix)
    (long_enough
        && matches!(
            first_two,
            Some("PL" | "RD" | "UU" | "LL" | "FL" | "OL" | "WL")
        ))
        || (long_enough && matches!(first_three, Some("UUM" | "UUL")))
}

/// Normalize bare list id into a playlist URL.
pub fn normalize_playlist_url(raw: &str) -> String {
    let s = raw.trim();
    if s.starts_with("http://") || s.starts_with("https://") {
        return s.to_string();
    }
    format!("https://www.youtube.com/playlist?list={s}")
}

/// `list=` query value from a YouTube URL, if present.
pub fn playlist_list_id_from_url(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    let search = if let Some(i) = lower.find("list=") {
        &url[i + 5..]
    } else {
        return None;
    };
    let end = search
        .find('&')
        .or_else(|| search.find('#'))
        .unwrap_or(search.len());
    let id = search[..end].trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn extract_video_id_from_text(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    if let Some(i) = lower.find("v=") {
        let rest = &s[i + 2..];
        let end = rest
            .find('&')
            .or_else(|| rest.find('#'))
            .or_else(|| rest.find('?'))
            .unwrap_or(rest.len());
        let id = rest[..end].trim();
        if is_youtube_video_id(id) {
            return Some(id.to_string());
        }
    }
    if let Some(host) = lower.find("youtu.be/") {
        let rest = &s[host + 9..];
        let end = rest
            .find(&['?', '#', '/'][..])
            .unwrap_or(rest.len());
        let id = rest[..end].trim();
        if is_youtube_video_id(id) {
            return Some(id.to_string());
        }
    }
    if lower.contains("/shorts/") {
        if let Some(i) = lower.find("/shorts/") {
            let rest = &s[i + 8..];
            let end = rest.find(&['?', '#', '/'][..]).unwrap_or(rest.len());
            let id = rest[..end].trim();
            if is_youtube_video_id(id) {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn entry_to_track(v: &serde_json::Value) -> Option<Track> {
    let mut id = v["id"].as_str()?.to_string();
    if !is_youtube_video_id(&id) {
        if let Some(u) = v["url"].as_str() {
            id = extract_video_id_from_text(u)?;
        } else {
            return None;
        }
    }
    Some(Track {
        id,
        title: v["title"].as_str().unwrap_or("Unknown").to_string(),
        channel: v["channel"]
            .as_str()
            .or_else(|| v["uploader"].as_str())
            .or_else(|| v["channel_id"].as_str())
            .unwrap_or("Unknown")
            .to_string(),
        duration: v["duration"].as_f64(),
        url: None,
    })
}

fn tracks_from_ytdlp_flat_lines(stdout: &str) -> Vec<Track> {
    let mut seen = std::collections::HashSet::<String>::new();
    let mut tracks = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["_type"].as_str() == Some("playlist") {
            continue;
        }
        let Some(t) = entry_to_track(&v) else { continue };
        if !seen.insert(t.id.clone()) {
            continue;
        }
        tracks.push(t);
    }
    tracks
}

fn tracks_from_ytdlp_single_json(stdout: &str) -> Option<Vec<Track>> {
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
    let mut seen = std::collections::HashSet::<String>::new();
    let mut out = Vec::new();
    if let Some(entries) = v["entries"].as_array() {
        for e in entries {
            if let Some(t) = entry_to_track(e) {
                if seen.insert(t.id.clone()) {
                    out.push(t);
                }
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    entry_to_track(&v).map(|t| vec![t])
}

async fn ytdlp_dump_flat_playlist(url: &str, max_videos: usize) -> Result<String> {
    let end = max_videos.max(1).min(PLAYLIST_MAX_ENTRIES);
    let end_s = end.to_string();
    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--flat-playlist",
            "--yes-playlist",
            "--no-warnings",
            "--playlist-end",
            &end_s,
            url,
        ])
        .output()
        .await
        .context("Failed to run yt-dlp. Is it installed? (pip install yt-dlp)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn ytdlp_dump_full_playlist_json(url: &str, max_videos: usize) -> Result<String> {
    let end = max_videos.max(1).min(PLAYLIST_MAX_ENTRIES);
    let end_s = end.to_string();
    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--yes-playlist",
            "--no-warnings",
            "--playlist-end",
            &end_s,
            url,
        ])
        .output()
        .await
        .context("Failed to run yt-dlp")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Expand a playlist / mix / `watch?v=…&list=…` into tracks — tries several URL shapes and full JSON if flat is thin.
pub async fn expand_playlist(url: &str, max_videos: usize) -> Result<Vec<Track>> {
    let url = url.trim();
    let normalized = if looks_like_youtube_url(url) {
        url.to_string()
    } else {
        normalize_playlist_url(url)
    };

    let mut candidates: Vec<String> = Vec::new();
    if let Some(id) = playlist_list_id_from_url(&normalized) {
        candidates.push(format!("https://www.youtube.com/playlist?list={id}"));
    }
    candidates.push(normalized.clone());

    let mut best: Vec<Track> = Vec::new();
    for try_url in candidates {
        let stdout = match ytdlp_dump_flat_playlist(&try_url, max_videos).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let tracks = tracks_from_ytdlp_flat_lines(&stdout);
        if tracks.len() > best.len() {
            best = tracks;
        }
    }

    if best.len() <= 1 && youtube_url_has_playlist_param(&normalized) {
        for try_url in [
            playlist_list_id_from_url(&normalized)
                .map(|id| format!("https://www.youtube.com/playlist?list={id}")),
            Some(normalized.clone()),
        ]
        .into_iter()
        .flatten()
        {
            if let Ok(stdout) = ytdlp_dump_full_playlist_json(&try_url, max_videos).await {
                let line_tracks = tracks_from_ytdlp_flat_lines(&stdout);
                if line_tracks.len() > best.len() {
                    best = line_tracks;
                }
                if let Some(tracks) = tracks_from_ytdlp_single_json(&stdout) {
                    if tracks.len() > best.len() {
                        best = tracks;
                    }
                }
            }
        }
    }

    if best.is_empty() {
        anyhow::bail!(
            "No tracks extracted from playlist — try updating yt-dlp (`pip install -U yt-dlp`)."
        );
    }
    Ok(best)
}

/// One video (or single tab) from any supported YouTube URL — podcasts, shorts, `/live`, music, etc.
pub async fn resolve_single_youtube_url(url: &str) -> Result<Vec<Track>> {
    let url = url.trim();
    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--flat-playlist",
            "--no-playlist",
            "--no-warnings",
            "--playlist-items",
            "1",
            url,
        ])
        .output()
        .await
        .context("Failed to run yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tracks = tracks_from_ytdlp_flat_lines(&stdout);
    if !tracks.is_empty() {
        return Ok(tracks);
    }
    if let Some(t) = tracks_from_ytdlp_single_json(&stdout) {
        if !t.is_empty() {
            return Ok(t);
        }
    }
    anyhow::bail!("Could not parse video metadata from this URL.");
}

/// Resolve pasted CLI / search input: playlist/mix URLs, bare list ids, or a single video URL.
pub async fn resolve_any_youtube_input(raw: &str) -> Result<Vec<Track>> {
    let s = raw.trim();
    if s.is_empty() {
        anyhow::bail!("empty input");
    }
    if looks_like_youtube_url(s) {
        if youtube_url_has_playlist_param(s) {
            return expand_playlist(s, PLAYLIST_MAX_ENTRIES).await;
        }
        return resolve_single_youtube_url(s).await;
    }
    if looks_like_playlist_input(s) {
        return expand_playlist(s, PLAYLIST_MAX_ENTRIES).await;
    }
    anyhow::bail!("not a recognized YouTube link");
}

/// Search the YouTube "Playlists" tab; returns a few playlist rows to pick from.
pub async fn search_playlists(query: &str, max_results: u8) -> Result<Vec<PlaylistHit>> {
    let enc = encode_results_query(query.trim());
    let url = format!(
        "https://www.youtube.com/results?search_query={enc}&sp={PLAYLISTS_TAB_SP}"
    );
    let end = max_results.max(1).min(15);
    let end_s = end.to_string();

    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--flat-playlist",
            "--no-warnings",
            "--playlist-end",
            &end_s,
            &url,
        ])
        .output()
        .await
        .context("Failed to run yt-dlp for playlist search")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp playlist search failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut hits: Vec<PlaylistHit> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();

    for line in stdout.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let title = match v["title"].as_str() {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => continue,
        };
        let id = v["id"].as_str().or_else(|| v["playlist_id"].as_str());
        let Some(id) = id else { continue };
        if is_youtube_video_id(id) {
            continue;
        }
        if !seen.insert(id.to_string()) {
            continue;
        }
        hits.push(PlaylistHit {
            list_id: id.to_string(),
            title,
        });
        if hits.len() >= max_results as usize {
            break;
        }
    }

    Ok(hits)
}

pub async fn search(query: &str, max_results: u8) -> Result<Vec<Track>> {
    let search_query = format!("ytsearch{max_results}:{query}");

    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--flat-playlist",
            "--no-warnings",
            "--default-search", "ytsearch",
            &search_query,
        ])
        .output()
        .await
        .context("Failed to run yt-dlp. Is it installed? (pip install yt-dlp)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp search failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tracks: Vec<Track> = stdout
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            Some(Track {
                id: v["id"].as_str()?.to_string(),
                title: v["title"].as_str().unwrap_or("Unknown").to_string(),
                channel: v["channel"].as_str()
                    .or_else(|| v["uploader"].as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                duration: v["duration"].as_f64(),
                url: None,
            })
        })
        .collect();

    Ok(tracks)
}

pub async fn get_audio_url(video_id: &str) -> Result<String> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");

    let output = Command::new("yt-dlp")
        .args([
            "--get-url",
            "-f", "bestaudio",
            "--no-warnings",
            &url,
        ])
        .output()
        .await
        .context("Failed to extract audio URL via yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp URL extraction failed: {stderr}");
    }

    let audio_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(audio_url)
}

/// Fetches related tracks via `ytsearch`: not YouTube's Mix API — we pick a
/// random phrasing around the artist/title, search deeply, then the app
/// dedupes against the queue and shuffles picks so repeats are less likely.
pub async fn fetch_radio_tracks(title: &str, channel: &str) -> Result<Vec<Track>> {
    // ThreadRng is not Send — pick the query in a block so it is dropped before `.await`.
    let query = {
        let mut rng = rand::thread_rng();
        if channel != "Unknown" && !channel.is_empty() {
            let opts = [
                format!("{channel} songs"),
                format!("{channel} popular"),
                format!("{channel} greatest hits"),
                format!("best of {channel}"),
            ];
            opts[rng.gen_range(0..opts.len())].clone()
        } else {
            let opts = [
                format!("{title} mix"),
                format!("songs like {title}"),
                format!("{title} similar music"),
            ];
            opts[rng.gen_range(0..opts.len())].clone()
        }
    };
    search(&query, RADIO_SEARCH_DEPTH).await
}
