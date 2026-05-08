use anyhow::{Context, Result};
use tokio::process::Command;

use crate::types::Track;

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

pub async fn fetch_mix_playlist(video_id: &str) -> Result<Vec<Track>> {
    let mix_url = format!(
        "https://www.youtube.com/watch?v={video_id}&list=RD{video_id}",
    );

    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--flat-playlist",
            "--no-warnings",
            &mix_url,
        ])
        .output()
        .await
        .context("Failed to fetch YouTube Mix playlist")?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tracks: Vec<Track> = stdout
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            let id = v["id"].as_str()?.to_string();
            if id == video_id {
                return None;
            }
            Some(Track {
                id,
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
