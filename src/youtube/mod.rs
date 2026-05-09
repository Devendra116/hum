use anyhow::{Context, Result};
use rand::Rng;
use tokio::process::Command;

use crate::types::Track;

/// How many search hits to pull before dedupe/shuffle (radio picks a subset).
const RADIO_SEARCH_DEPTH: u8 = 20;

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
