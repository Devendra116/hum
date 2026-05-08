# hum

A minimal, ad-free terminal music player. No API keys. No subscriptions. No accounts. Just music.

## Demo

> Add your recording at `docs/demo.gif` (or update the path below) so the README shows a real run.

hum demo

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

### Prerequisites

```bash
# Install yt-dlp (YouTube audio extraction)
pip install yt-dlp

# Install mpv (audio playback)
sudo apt install mpv        # Debian/Ubuntu
# or
brew install mpv            # macOS
# or
sudo pacman -S mpv          # Arch
```

### Install hum

```bash
# From source
git clone https://github.com/Devendra116/hum.git
cd hum
cargo install --path .

# Or directly from crates.io (once published)
# cargo install hum
```

## Usage

```bash
# Play a song immediately
hum Big Dawgs

# Open interactive mode
hum

# Play with radio mode (auto-plays related songs)
hum --radio midnight city
```

## Keybindings


| Key       | Action                             |
| --------- | ---------------------------------- |
| `/`       | Search for a song                  |
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
| `1/2/3`   | Pick from choices (when ambiguous) |


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