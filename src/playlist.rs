//! Track metadata, named playlists, and the repeat/shuffle play order.

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use lofty::prelude::*;

pub const EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "oga", "m4a", "mp4", "aac"];

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    /// Technical summary, e.g. "Flac · 44100 Hz · 2 ch · 16-bit · 912 kbps".
    pub info: String,
}

impl Track {
    /// A track titled after its file name, without touching the file.
    /// `MetadataLoader` fills in the rest later.
    pub fn stub(path: PathBuf) -> Self {
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        Track {
            title,
            artist: None,
            album: None,
            duration: None,
            info: String::new(),
            path,
        }
    }

    /// Builds a track and reads its tags. Metadata is best effort: a file lofty
    /// cannot parse is still added and may be playable by the decoder.
    pub fn from_path(path: PathBuf) -> Self {
        let mut track = Track::stub(path);
        let Ok(tagged) = lofty::read_from_path(&track.path) else {
            return track;
        };
        let props = tagged.properties();
        if !props.duration().is_zero() {
            track.duration = Some(props.duration());
        }
        let mut info = vec![format!("{:?}", tagged.file_type())];
        if let Some(rate) = props.sample_rate() {
            info.push(format!("{rate} Hz"));
        }
        if let Some(ch) = props.channels() {
            info.push(format!("{ch} ch"));
        }
        if let Some(bits) = props.bit_depth() {
            info.push(format!("{bits}-bit"));
        }
        if let Some(kbps) = props.audio_bitrate().or(props.overall_bitrate()) {
            info.push(format!("{kbps} kbps"));
        }
        track.info = info.join(" · ");

        if let Some(tag) = tagged.primary_tag().or(tagged.first_tag()) {
            if let Some(title) = tag.title().filter(|t| !t.trim().is_empty()) {
                track.title = title.into_owned();
            }
            track.artist = tag.artist().map(|a| a.into_owned());
            track.album = tag.album().map(|a| a.into_owned());
        }
        track
    }

    /// "Artist · Album", or whichever of the two is known.
    pub fn byline(&self) -> String {
        [&self.artist, &self.album]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Stub tracks for a file, or for every supported file directly inside a
/// folder (sorted by path). Paths are made absolute so saved playlists stay valid.
pub fn scan(path: &Path) -> Vec<Track> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_supported(p))
            .collect();
        files.sort();
        files.into_iter().map(Track::stub).collect()
    } else if path.is_file() {
        vec![Track::stub(path)]
    } else {
        Vec::new()
    }
}

/// Reads tags on a background thread so large libraries and folders never
/// block the UI. Send paths with `request`, collect results with `take_loaded`.
pub struct MetadataLoader {
    jobs: Sender<PathBuf>,
    done: Receiver<Track>,
}

impl MetadataLoader {
    /// `wake` runs after each finished track (e.g. to request a repaint).
    pub fn new(wake: impl Fn() + Send + 'static) -> Self {
        let (jobs, job_rx) = mpsc::channel::<PathBuf>();
        let (done_tx, done) = mpsc::channel();
        thread::spawn(move || {
            for path in job_rx {
                if done_tx.send(Track::from_path(path)).is_err() {
                    break;
                }
                wake();
            }
        });
        Self { jobs, done }
    }

    pub fn request(&self, path: PathBuf) {
        // The worker only stops when this loader is dropped, so a send error
        // just means shutdown is in progress.
        let _ = self.jobs.send(path);
    }

    /// Finished tracks keyed by path (later results for the same path win).
    pub fn take_loaded(&self) -> HashMap<PathBuf, Track> {
        self.done.try_iter().map(|t| (t.path.clone(), t)).collect()
    }
}

pub struct Playlist {
    pub name: String,
    pub tracks: Vec<Track>,
}

impl Playlist {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tracks: Vec::new(),
        }
    }

    /// Sum of the known track lengths.
    pub fn total(&self) -> Duration {
        self.tracks.iter().filter_map(|t| t.duration).sum()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.tracks.iter().any(|t| t.path == path)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Off,
    All,
    One,
}

impl Repeat {
    pub fn cycle(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// How the player moves through a list of `len` tracks.
pub struct Order {
    pub repeat: Repeat,
    pub shuffle: bool,
}

impl Order {
    /// Index to play after `current`. `manual` is true when the user pressed
    /// Next, false when the track ended on its own.
    pub fn next_index(&self, len: usize, current: Option<usize>, manual: bool) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let Some(cur) = current else {
            return Some(0);
        };
        if !manual && self.repeat == Repeat::One {
            return Some(cur);
        }
        // ponytail: shuffle picks at random and never ends; add a play-history
        // queue if "play each track once" matters.
        if self.shuffle && len > 1 {
            let r = (RandomState::new().hash_one(cur) as usize) % (len - 1);
            return Some(if r >= cur { r + 1 } else { r });
        }
        if cur + 1 < len {
            Some(cur + 1)
        } else if manual || self.repeat == Repeat::All {
            Some(0)
        } else {
            None
        }
    }

    pub fn prev_index(&self, len: usize, current: Option<usize>) -> Option<usize> {
        match current {
            _ if len == 0 => None,
            None => Some(0),
            Some(cur) => Some((cur + len - 1) % len),
        }
    }
}

/// Formats a duration as `m:ss`, or `h:mm:ss` from one hour up.
pub fn format_time(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_time() {
        assert_eq!(format_time(Duration::from_secs(0)), "0:00");
        assert_eq!(format_time(Duration::from_secs(65)), "1:05");
        assert_eq!(format_time(Duration::from_secs(3_725)), "1:02:05");
    }

    #[test]
    fn next_respects_repeat() {
        let mut o = Order {
            repeat: Repeat::Off,
            shuffle: false,
        };
        assert_eq!(o.next_index(3, None, false), Some(0));
        assert_eq!(o.next_index(0, None, true), None);
        assert_eq!(o.next_index(3, Some(2), false), None);
        assert_eq!(o.next_index(3, Some(2), true), Some(0));
        o.repeat = Repeat::All;
        assert_eq!(o.next_index(3, Some(2), false), Some(0));
        o.repeat = Repeat::One;
        assert_eq!(o.next_index(3, Some(2), false), Some(2));
        assert_eq!(o.next_index(3, Some(2), true), Some(0));
        assert_eq!(o.prev_index(3, Some(2)), Some(1));
        assert_eq!(o.prev_index(3, Some(0)), Some(2));
        assert_eq!(o.prev_index(0, Some(0)), None);
    }

    #[test]
    fn shuffle_never_repeats_current() {
        let o = Order {
            repeat: Repeat::Off,
            shuffle: true,
        };
        for _ in 0..100 {
            let n = o.next_index(4, Some(1), false).unwrap();
            assert!(n < 4 && n != 1);
        }
    }

    #[test]
    fn loader_returns_requested_tracks() {
        let (woke, wake_rx) = mpsc::channel();
        let loader = MetadataLoader::new(move || {
            let _ = woke.send(());
        });
        let path = PathBuf::from("/nonexistent/song.mp3");
        loader.request(path.clone());
        wake_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("worker should wake the UI");
        let loaded = loader.take_loaded();
        assert_eq!(loaded[&path].title, "song.mp3");
    }

    #[test]
    fn supported_extensions() {
        assert!(is_supported(Path::new("a/b.FLAC")));
        assert!(!is_supported(Path::new("a/b.txt")));
        assert!(!is_supported(Path::new("noext")));
    }
}
