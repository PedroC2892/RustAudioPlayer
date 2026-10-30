//! Single instance: a second launch hands its files to the running player over
//! a Unix socket in `$XDG_RUNTIME_DIR` and exits instead of opening a window.
//!
//! Message: absolute paths as raw bytes, one per line; an empty message only
//! asks the running window to come to the front.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

#[cfg(unix)]
use std::{
    ffi::OsString,
    io::{self, Read, Write},
    os::unix::ffi::{OsStrExt, OsStringExt},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    thread,
    time::Duration,
};

/// Upper bound for one message, so a bad client cannot exhaust memory.
#[cfg(unix)]
const MAX_MESSAGE: u64 = 1 << 20;

pub enum Claim {
    /// A running instance took the files; this process should exit.
    Forwarded,
    /// This process is the running instance and serves later launches.
    Primary(Server),
    /// No socket available (no runtime dir, not Unix): run on our own.
    Standalone,
}

pub struct Server {
    #[cfg(unix)]
    listener: UnixListener,
}

pub fn claim(paths: &[PathBuf]) -> Claim {
    #[cfg(unix)]
    if let Some(socket) = socket_path() {
        return claim_at(&socket, paths);
    }
    let _ = paths;
    Claim::Standalone
}

impl Server {
    /// Each later launch arrives as one `Vec` of paths; `wake` runs after each
    /// so the UI can react even when idle.
    pub fn serve(self, wake: impl Fn() + Send + 'static) -> Receiver<Vec<PathBuf>> {
        let (tx, rx) = mpsc::channel();
        #[cfg(unix)]
        thread::spawn(move || {
            for stream in self.listener.incoming() {
                let Ok(stream) = stream else {
                    continue;
                };
                let Ok(paths) = receive(stream) else {
                    continue;
                };
                if tx.send(paths).is_err() {
                    break;
                }
                wake();
            }
        });
        #[cfg(not(unix))]
        let _ = (tx, wake);
        rx
    }
}

#[cfg(unix)]
fn socket_path() -> Option<PathBuf> {
    // The runtime dir is private to the user (mode 0700), so other users
    // cannot connect and push files into the player.
    let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
    dir.is_absolute()
        .then(|| dir.join("rust-audio-player.sock"))
}

#[cfg(unix)]
fn claim_at(socket: &Path, paths: &[PathBuf]) -> Claim {
    match UnixStream::connect(socket) {
        Ok(stream) => match send(stream, paths) {
            Ok(()) => Claim::Forwarded,
            Err(_) => Claim::Standalone,
        },
        Err(_) => {
            // Nobody is listening, so any socket file is left over from a
            // crash. ponytail: two launches in the same instant can both get
            // here; the later one wins the socket. Use a lock file if needed.
            let _ = std::fs::remove_file(socket);
            match UnixListener::bind(socket) {
                Ok(listener) => Claim::Primary(Server { listener }),
                Err(e) => {
                    eprintln!(
                        "Single-instance socket unavailable ({}: {e}); later launches will open their own window.",
                        socket.display()
                    );
                    Claim::Standalone
                }
            }
        }
    }
}

#[cfg(unix)]
fn send(mut stream: UnixStream, paths: &[PathBuf]) -> io::Result<()> {
    stream.write_all(&encode(paths))?;
    stream.shutdown(std::net::Shutdown::Write)
}

#[cfg(unix)]
fn receive(stream: UnixStream) -> io::Result<Vec<PathBuf>> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::new();
    stream.take(MAX_MESSAGE).read_to_end(&mut bytes)?;
    Ok(decode(&bytes))
}

/// Paths are made absolute here because the running instance has another
/// working directory. Paths containing a newline cannot be framed and are skipped.
#[cfg(unix)]
fn encode(paths: &[PathBuf]) -> Vec<u8> {
    let mut out = Vec::new();
    for path in paths {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        let bytes = path.as_os_str().as_bytes();
        if !bytes.contains(&b'\n') {
            out.extend_from_slice(bytes);
            out.push(b'\n');
        }
    }
    out
}

#[cfg(unix)]
fn decode(bytes: &[u8]) -> Vec<PathBuf> {
    bytes
        .split(|&b| b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| PathBuf::from(OsString::from_vec(line.to_vec())))
        .filter(|path| path.is_absolute())
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let paths = [
            PathBuf::from("/music/a b.flac"),
            PathBuf::from("/music/bad\nname.mp3"),
        ];
        assert_eq!(decode(&encode(&paths)), [PathBuf::from("/music/a b.flac")]);
        assert!(decode(b"relative.mp3\n").is_empty());
    }

    #[test]
    fn second_launch_forwards_to_first() {
        let dir = std::env::temp_dir().join(format!("rap-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("test.sock");
        // A stale socket file from a crashed run must not block startup.
        drop(UnixListener::bind(&socket).unwrap());

        let Claim::Primary(server) = claim_at(&socket, &[]) else {
            panic!("first launch should become primary");
        };
        let (woke, wake_rx) = mpsc::channel();
        let opened = server.serve(move || {
            let _ = woke.send(());
        });

        let song = PathBuf::from("/music/song.mp3");
        assert!(matches!(
            claim_at(&socket, std::slice::from_ref(&song)),
            Claim::Forwarded
        ));
        wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(opened.try_recv().unwrap(), [song]);

        assert!(matches!(claim_at(&socket, &[]), Claim::Forwarded));
        wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(opened.try_recv().unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
