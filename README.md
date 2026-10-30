# Rust Audio Player

[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](https://www.rust-lang.org)
[![GUI](https://img.shields.io/badge/GUI-egui%20%2F%20eframe-blue.svg)](https://github.com/emilk/egui)
[![Audio](https://img.shields.io/badge/Audio-rodio-red.svg)](https://github.com/RustAudio/rodio)
[![Platform](https://img.shields.io/badge/Platform-Linux%20(X11%20%2F%20Wayland)-green.svg)]()
[![License](https://img.shields.io/badge/License-MIT%2FApache--2.0-lightgrey.svg)]()

A fast, lightweight, and distraction-free desktop audio player written in pure safe Rust. Built around high-fidelity sound, instant startup, minimal resource consumption, and a bespoke "listening room after dark" visual identity.

---

## Features

- **Spotify-Style Two-Pane Layout**:
  - Left sidebar with unified library ("All tracks") and custom user playlists.
  - Interactive track table with sorting, real-time filtering (by title, artist, or album), and right-click context menus.
  - Floating bottom player bar with scrubber and technical now-playing marquee.
- **Audio Fidelity & Transport**:
  - Full transport controls: Play / Pause, Stop, Previous (restarts if past 3 seconds), Next.
  - Seeking with position scrubber and accurate time readouts (`mm:ss` / `hh:mm:ss`).
  - Smooth volume control with perceptual squared curve and instant mute toggle.
  - Playback modes: Repeat (Off / All / One) and non-repeating Shuffle.
  - Uninterrupted auto-advance: continues playing seamlessly when minimized or in the background.
- **Asynchronous Non-Blocking Metadata**:
  - Reads audio tags and stream technical details via `lofty` on a dedicated background worker thread.
  - Instant UI startup with stub titles that enrich smoothly without stalling file I/O.
  - Displays codec details: file type, sample rate (Hz), channel count, bit depth, and bitrate (kbps).
- **Persistent Playlists & Library**:
  - Auto-saved to `$XDG_DATA_HOME/RustAudioPlayer/` (or `~/.local/share/RustAudioPlayer/`) using standard extended M3U8 playlists (`library.m3u8`, `playlists/0001.m3u8`, ...).
  - Atomic writes (`.tmp` rename) ensure playlists are never corrupted during sudden termination.
  - Drag-and-drop tracks or folders directly into the window.
- **Single Instance IPC**:
  - Managed via Unix domain socket in `$XDG_RUNTIME_DIR`.
  - Opening files from a file manager or a second terminal forward the paths to the running player window, brings it to focus, and prompts gracefully if music is already playing.
- **Optimized Footprint**:
  - Hardware-accelerated OpenGL (`glow`) backend instead of bulky wgpu pipelines.
  - Embedded OFL typography (Fraunces & Inter Tight) — zero system font or emoji dependencies.
  - Release binaries built with Fat LTO, single codegen unit, stripped debug symbols, and `panic = "abort"`.

---

## Design Philosophy

Inspired by vintage high-end hi-fi systems:
> *"A listening room after dark, one red light on the amplifier."*

- **Color Palette**:
  - **Graphite** (`#161618`): Canvas for every panel, frame, and sidebar.
  - **Charcoal** (`#1f1f23`): Raised surfaces, input fields, and track tables.
  - **Smoke** (`#34343a`): Hairlines, subtle dividers, and pill-button borders.
  - **Ash** (`#e8e6e3`): Warm typography and clean vector icons (never pure white).
  - **Cinnabar Red** (`#d94a3d`): The single indicator accent for active playback, progress bars, toggle states, and error alerts.
- **Typography**:
  - **Fraunces Soft Serif**: Used across all UI body copy, track titles, and navigation labels.
  - **Inter Tight Bold**: Geometric display sans for headings and wordmarks.

---

## Supported Audio Formats

Format decoding is powered by `rodio` and `symphonia`:

| Format | File Extensions |
|---|---|
| **FLAC** | `.flac` |
| **MP3** | `.mp3` |
| **WAV** | `.wav` |
| **Ogg Vorbis** | `.ogg`, `.oga` |
| **MP4 / AAC** | `.m4a`, `.mp4`, `.aac` |

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| <kbd>Space</kbd> | Play / Pause toggle |
| <kbd>→</kbd> | Seek forward 5 seconds |
| <kbd>←</kbd> | Seek backward 5 seconds |
| <kbd>↑</kbd> | Volume up (+5%) |
| <kbd>↓</kbd> | Volume down (-5%) |
| <kbd>N</kbd> | Next track |
| <kbd>P</kbd> | Previous track (restarts track if past 3s) |
| <kbd>S</kbd> | Stop playback |
| <kbd>M</kbd> | Toggle mute |
| <kbd>Ctrl</kbd> + <kbd>O</kbd> | Open file dialog to add files |

---

## Architecture & Project Structure

```
RustAudioPlayer/
├── assets/
│   ├── fonts/               # Embedded Fraunces & Inter Tight TTF fonts
│   └── icon.svg             # Vector application icon
├── src/
│   ├── main.rs              # CLI arguments, window setup, single-instance dispatch
│   ├── app.rs               # egui UI, layout, track table, player bar, drag & drop
│   ├── audio.rs             # rodio audio engine, sink/player lifecycle, state management
│   ├── instance.rs          # Single-instance Unix socket server and client IPC
│   ├── playlist.rs          # Track metadata, background loader, repeat/shuffle order
│   ├── store.rs             # M3U8 persistence and atomic file I/O
│   └── theme.rs             # DESIGN.md color tokens, typography, and procedural icon rasterizer
├── Cargo.toml               # Minimal features, dev profile overrides, and release LTO
└── install.sh               # Desktop integration and installation script
```

---

## Getting Started

### Prerequisites

You will need the **Rust** toolchain (2024 edition or newer) and standard audio/window development libraries:

**Debian / Ubuntu / Pop!_OS:**
```bash
sudo apt update
sudo apt install build-essential pkg-config libasound2-dev libx11-dev libwayland-dev
```

**Fedora:**
```bash
sudo dnf install alsa-lib-devel libX11-devel wayland-devel
```

**Arch Linux:**
```bash
sudo pacman -S base-devel alsa-lib libx11 wayland
```

---

### Building & Running

1. **Clone the repository:**
   ```bash
   git clone https://github.com/PedroC2892/RustAudioPlayer.git
   cd RustAudioPlayer
   ```

2. **Run in development mode:**
   ```bash
   # Run without arguments
   cargo run

   # Or open files / folders immediately
   cargo run -- ~/Music/album/
   ```

3. **Run with release optimizations (smoother playback for large libraries):**
   ```bash
   cargo run --release
   ```

4. **Run tests:**
   ```bash
   cargo test
   ```

---

### Desktop Installation

A helper script is provided to compile an optimized release binary and install the desktop launcher and icon:

```bash
# Install for current user (~/.local/bin and ~/.local/share/applications)
./install.sh

# Or install system-wide (/usr/local)
sudo ./install.sh

# Uninstall
./install.sh --uninstall
```

Once installed, **Rust Audio Player** can be launched from your desktop application menu or by right-clicking audio files in your file manager ("Open With...").

---

## License

This project is licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](http://www.apache.org/licenses/LICENSE-2.0))
- MIT license ([LICENSE-MIT](http://opensource.org/licenses/MIT))

at your option. Embedded fonts are licensed under the [SIL Open Font License](http://scripts.sil.org/OFL).
