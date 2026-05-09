# hum

A minimal, ad-free terminal music player. No API keys. No subscriptions. No accounts. Just music.



### Terminal UI Preview

```text
$ hum Big Dawgs
┌──────────────────────────────────────────────────────────────────────────────┐
│ hum                                                                          │
├──────────────────────────────────────────────────────────────────────────────┤
│ Now Playing                                                                  │
│ ▶ Big Dawgs — Hanumankind                                                    │
│ 1:12 / 3:35                                                      Vol: 100%   │
│ ████████████──────────────────────────────────────────────────────────        │
│                                                                              │
│ Queue                                                                        │
│  1. Big Dawgs                                                                │
│  2. Millionaire                                                              │
│  3. Midnight City                                                            │
│                                                                              │
│ [Space] Play/Pause  [n/p] Next/Prev  [r] Radio  [q] Quit                    │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Why

- No ads (extracts raw audio stream — ads only exist in YouTube's JS player)
- No API keys, no accounts, no subscriptions
- ~20-35MB RAM total (Rust binary + mpv audio-only)
- Keyboard-driven, distraction-free
- Radio mode for endless related music

## Install

### Quick install (recommended)

**macOS / Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/Devendra116/hum/main/install.sh | sh
```

The installer handles everything — downloads prebuilt binaries for **hum** and **yt-dlp**, verifies SHA-256 checksums, and installs **mpv** via your system package manager. No Rust or Python required.

To install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/Devendra116/hum/main/install.sh | HUM_VERSION=v0.1.0 sh
```

Running the same command again will upgrade to the latest version.

> **Security:** all downloads are over HTTPS from official GitHub repos only, with SHA-256 checksum verification. Binaries install to `~/.local/bin` (no root). You can [inspect the script](install.sh) before running it.
>
> **Platform support:** officially supported on Linux and macOS. Windows support is experimental and not shipped in current releases.

### Other install methods

```bash
# From source (requires Rust toolchain)
git clone https://github.com/Devendra116/hum.git
cd hum
cargo install --path .

# From crates.io (once published)
cargo install hum
```

You can also download binaries directly from [GitHub Releases](https://github.com/Devendra116/hum/releases).

## Usage

```bash
# Play a song immediately
hum Big Dawgs

# Open interactive mode
hum

# Play with radio mode (auto-plays related songs)
hum --radio midnight city

# Queue a YouTube playlist or mix (watch?v=…&list=… works, including RD… radio mixes)
hum --playlist "https://www.youtube.com/playlist?list=PLxxxxxxxx"

# Open a single video, podcast episode, Shorts, or Music link (same as pasting in the TUI)
hum "https://www.youtube.com/watch?v=xxxxxxxxxxx"
```

## Keybindings


| Key       | Action                             |
| --------- | ---------------------------------- |
| `/`       | Song title, **paste any YouTube URL** (video, podcast, Shorts, `watch?v=…&list=RD…` mix), `pl:keywords` for playlist search |
| `Enter`   | Confirm search / play              |
| `Esc`     | Cancel                             |
| `Space`   | Play / Pause                       |
| `n`       | Next track                         |
| `p`       | Previous track                     |
| `s`       | Shuffle queue                      |
| `r`       | Toggle radio mode                  |
| `+` / `=` | Volume up                          |
| `-`       | Volume down                        |
| `→`       | Seek forward 10s                   |
| `←`       | Seek backward 10s                  |
| `q`       | Quit                               |
| `1/2/3`   | Pick a song (when search is ambiguous) |
| `1`–`5`   | Pick a playlist (after a `pl:` search) |


## YouTube playlists and links

- **CLI:** `hum --playlist "<url>"` or paste the same URL as the first argument. Accepts normal playlists (`PL…`), **mix / radio lists** (`RD…`, including links like [`watch?v=…&list=RD…&start_radio=1`](https://www.youtube.com/watch?v=AX1zRInC_TA&list=RDEMLbwywICAYOlC2vOj5cPYjQ&start_radio=1)), `watch?v=…&list=…`, or a bare list id.
- **Single videos:** any `youtube.com`, `youtu.be`, or `music.youtube.com` link without a `list=` parameter is resolved as **one** video (podcasts, live VODs, Shorts, etc.).
- **In the TUI:** press `/`, paste a URL, or type `pl:chill lofi` to search YouTube’s **Playlists** tab and pick **1–5**. You can also use `pl:https://…` to load a link from playlist mode.
- Playback still uses **yt-dlp stream URLs + mpv** (not the site player). Keep **yt-dlp updated** (`pip install -U yt-dlp`) if a link format stops working.

## How It Works

1. You type a song name
2. `yt-dlp` searches YouTube and returns the top matches
3. If the match is clear, it plays immediately. If ambiguous, you pick from max 3 options.
4. `yt-dlp` extracts the direct audio stream URL (no ads in raw streams)
5. `mpv` plays the audio via IPC socket control
6. Radio mode fetches YouTube Mix playlists for endless related tracks

## Radio Mode

Press `r` to toggle. When enabled, after a song finishes, hum automatically
queues and plays related songs using YouTube's Mix playlist feature.
No API key needed — this uses YouTube's public auto-generated playlists.

## Resource Usage


| Component              | RAM                       |
| ---------------------- | ------------------------- |
| hum binary             | ~5-10MB                   |
| mpv (audio-only)       | ~15-25MB                  |
| yt-dlp (brief spikes)  | ~30MB peak, exits quickly |
| **Total steady-state** | **~20-35MB**              |


## License

MIT