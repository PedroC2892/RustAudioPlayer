//! Saves the library and playlists as extended M3U files:
//! `<data dir>/library.m3u8` and `<data dir>/playlists/<n>.m3u8`.
//! File names are numeric, so user-typed playlist names never reach the filesystem.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::playlist::Playlist;

/// `$XDG_DATA_HOME`, `%APPDATA%` or `~/.local/share`, plus the app folder.
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("RustAudioPlayer"))
}

pub struct Saved {
    pub name: String,
    pub paths: Vec<PathBuf>,
}

/// Loads the library (always first) followed by the saved playlists.
pub fn load(dir: &Path) -> Vec<Saved> {
    let mut library = read(&dir.join("library.m3u8")).unwrap_or(Saved {
        name: String::new(),
        paths: Vec::new(),
    });
    library.name = "Library".into();

    let mut files: Vec<(usize, PathBuf)> = fs::read_dir(dir.join("playlists"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|p| playlist_number(&p).map(|n| (n, p)))
        .collect();
    files.sort();

    std::iter::once(library)
        .chain(files.iter().filter_map(|(_, p)| read(p)))
        .collect()
}

/// Writes `lists[0]` as the library and the rest as playlists, then deletes
/// files left over from removed playlists.
pub fn save(dir: &Path, lists: &[Playlist]) -> io::Result<()> {
    let playlists_dir = dir.join("playlists");
    fs::create_dir_all(&playlists_dir)?;
    for (i, list) in lists.iter().enumerate() {
        let path = if i == 0 {
            dir.join("library.m3u8")
        } else {
            playlists_dir.join(format!("{i:04}.m3u8"))
        };
        let paths: Vec<&Path> = list.tracks.iter().map(|t| t.path.as_path()).collect();
        write_atomic(&path, &to_m3u(&list.name, &paths))?;
    }
    for entry in fs::read_dir(&playlists_dir)?.flatten() {
        let path = entry.path();
        if playlist_number(&path).is_some_and(|n| n >= lists.len()) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn read(path: &Path) -> Option<Saved> {
    let text = fs::read_to_string(path).ok()?;
    Some(parse_m3u(&text, path.parent().unwrap_or(Path::new("."))))
}

/// `0007.m3u8` -> `Some(7)`; anything else -> `None`.
fn playlist_number(path: &Path) -> Option<usize> {
    if path.extension()? != "m3u8" {
        return None;
    }
    path.file_stem()?.to_str()?.parse().ok()
}

/// Write to a temp file and rename, so a crash never leaves a half-written list.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let tmp = path.with_extension("m3u8.tmp");
    fs::write(&tmp, text)?;
    fs::rename(tmp, path)
}

pub fn to_m3u(name: &str, paths: &[&Path]) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    let mut out = format!("#EXTM3U\n#PLAYLIST:{name}\n");
    // ponytail: non-UTF-8 paths and paths with line breaks are skipped; the
    // M3U line format cannot hold them.
    for p in paths.iter().filter_map(|p| p.to_str()) {
        if !p.contains(['\n', '\r']) {
            out.push_str(p);
            out.push('\n');
        }
    }
    out
}

/// Relative entries are resolved against `base`, absolute ones kept as is.
pub fn parse_m3u(text: &str, base: &Path) -> Saved {
    let mut saved = Saved {
        name: String::new(),
        paths: Vec::new(),
    };
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("#PLAYLIST:") {
            saved.name = name.trim().to_owned();
        } else if !line.trim().is_empty() && !line.starts_with('#') {
            saved.paths.push(base.join(line));
        }
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playlist::Track;

    #[test]
    fn m3u_round_trip() {
        let a = Path::new("/music/a b.flac");
        let bad = Path::new("/music/evil\nname.mp3");
        let text = to_m3u("Late\nnight", &[a, bad]);
        let saved = parse_m3u(&text, Path::new("/base"));
        assert_eq!(saved.name, "Latenight");
        assert_eq!(saved.paths, vec![a.to_path_buf()]);
        let rel = parse_m3u("#EXTM3U\nsong.mp3\n", Path::new("/base"));
        assert_eq!(rel.paths, vec![PathBuf::from("/base/song.mp3")]);
    }

    #[test]
    fn save_then_load_removes_stale_playlists() {
        let dir = std::env::temp_dir().join(format!("rap-store-test-{}", std::process::id()));
        let track = |p: &str| Track::stub(PathBuf::from(p));
        let mut lists = vec![
            Playlist::new("Library"),
            Playlist::new("A"),
            Playlist::new("B"),
        ];
        lists[0].tracks.push(track("/x/1.mp3"));
        lists[2].tracks.push(track("/x/2.mp3"));
        save(&dir, &lists).unwrap();
        lists.remove(1);
        save(&dir, &lists).unwrap();

        let loaded = load(&dir);
        let names: Vec<&str> = loaded.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Library", "B"]);
        assert_eq!(loaded[1].paths, vec![PathBuf::from("/x/2.mp3")]);
        fs::remove_dir_all(dir).unwrap();
    }
}
