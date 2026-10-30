# RustAudioPlayer

Desktop audio player written in Rust: `rodio` for decoding/playback, `eframe`/`egui` for the
GUI, `lofty` for tags and audio properties, `rfd` for native file dialogs.

## Features

- Spotify-style layout: sidebar with "All tracks" (the library) and user playlists, main
  area with the selected list as a track table, player bar and a now-playing marquee at the bottom.
- Library and playlists persist as M3U files in `$XDG_DATA_HOME/RustAudioPlayer`
  (`library.m3u8`, `playlists/0001.m3u8`, …). Saved after every change.
- Playlists: create (+ New, or right-click a track > Add to playlist > New playlist), rename,
  delete, add/remove tracks, filter by title/artist/album.
- Adding files (dialog, drag-and-drop, CLI args, folders one level deep) always adds to the
  library, and also to the playlist currently shown.
- Transport: play/pause, stop, previous (restarts the track if past 3 s), next.
- Seek bar with current position and total length; seeking happens on release.
- Volume slider (squared curve) and mute.
- Repeat off / all / one, shuffle.
- Now-playing panel: title, artist, album, format, sample rate, channels, bit depth, bitrate.
- Auto-advance keeps working while the window is minimized (`App::logic`).
- Keyboard: `Space` play/pause, `←`/`→` seek 5 s, `↑`/`↓` volume, `N` next, `P` previous,
  `S` stop, `M` mute, `Ctrl+O` add files.

Supported formats: whatever rodio's default features decode (MP3, FLAC, WAV, Ogg Vorbis,
MP4/AAC). The list of extensions used for folder scans and the file dialog lives in
`playlist::EXTENSIONS`.

## Layout

| File | Role |
|---|---|
| `src/main.rs` | CLI args, window options, `eframe::run_native`. |
| `src/audio.rs` | `Audio`: owns the rodio device sink and `Player`; play/pause/stop/seek/volume and the `State` enum. |
| `src/playlist.rs` | `Track` (metadata via lofty), `scan`, `Playlist` (name + tracks), `Order` (repeat/shuffle next/prev logic), `format_time`. |
| `src/instance.rs` | Single instance over a Unix socket in `$XDG_RUNTIME_DIR`: a second launch forwards its files and exits. |
| `src/store.rs` | M3U load/save of the library and playlists; atomic writes; numeric file names. |
| `src/theme.rs` | DESIGN.md tokens (colors, fonts, pill/ghost widget visuals) and the app icon drawn in code. |
| `src/app.rs` | `App`: layout, sidebar, track table, player bar, marquee, shortcuts, drag-and-drop. |
| `assets/fonts/` | Fraunces (soft serif, body/UI) and Inter Tight Bold (headings), OFL, embedded with `include_bytes!`. |

Design: follow `DESIGN.md`. Two hues only: gray-black (graphite canvas `#161618`, charcoal
placard `#1f1f23`, smoke hairlines `#34343a`, ash text `#e8e6e3`) and one cinnabar red
`#d94a3d` reserved for state (playing row/marker, progress, active toggles, errors) and the
main play button. Serif for all text, bold sans only for headings, ghost/outlined pill
buttons, no shadows, sharp corners except pills. Icons are painted as vector shapes
(`Glyph`), not emoji: the build ships no emoji font.

Build footprint: `Cargo.toml` enables only the needed features (eframe with the `glow`
renderer instead of wgpu, no accesskit/system fonts; rodio without recording/dither).
Dev builds keep dependencies optimized and without debug info; release uses fat LTO,
one codegen unit, `panic = "abort"` and `strip`. Tags are read by `MetadataLoader` on a
background thread, so startup and "Add folder" never block on file I/O.

Opening files from outside (CLI args, file manager, second launch) goes through
`App::open_external`: files join the library, play at once if nothing is playing,
otherwise a modal asks before replacing the current track. Unix socket paths are limited
to 108 bytes, so a very long `$XDG_RUNTIME_DIR` disables single instance (logged on stderr).

Invariants worth knowing:
- `lists[0]` is the library; playlists are `lists[1..]`. `App::playing` is `(list, index)`
  and must be adjusted whenever tracks or playlists are removed.
- Draw code pushes `Action`s and `App::apply` runs them after drawing (avoids borrowing
  `lists` mutably while it is being drawn).
- Every bar/column is placed in an explicit `Rect`; text is truncated to its rect
  (`paint_text`), so widening or shrinking the window never makes content overflow.
- `Audio` declares `player` before `_sink`, so the player drops first. Dropping the sink stops sound.
- `Audio::finished()` is "loaded but the rodio queue is empty"; `App::logic` uses it to advance.
- A track that fails to decode becomes `current` and playback stops, so the error shows and Next skips it.
- Tracks start as `Track::stub` (file name only) and are replaced when `MetadataLoader` returns; track length prefers lofty metadata and falls back to the decoder's `total_duration()`.

## Commands

```sh
cargo run -- <file-or-folder>...     # run with an initial playlist
cargo run --release                  # smoother playback for large files
cargo test                           # order, time format, M3U store, icon tests
XDG_DATA_HOME=/tmp/x cargo run       # run against a throwaway library
./install.sh [--system] [--uninstall] # install binary, icon (assets/icon.svg) and .desktop launcher
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo audit
```

ALSA/JACK messages on stderr at startup are cpal probing backends; harmless.

## Conventions

- All code, comments, identifiers, commit messages and docs in English. Talk to the developer in Portuguese (pt-PT).
- Safe Rust only; no `unsafe`.
- File paths and file contents are untrusted input: no `unwrap()`/`expect()` on them; show errors in the UI status line.
- Keep `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean; run `cargo audit` when dependencies change.
- `PROGRESS.md` is the record of the earlier tutoring sessions (M0–M3); not updated anymore.
