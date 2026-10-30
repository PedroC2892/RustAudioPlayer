//! Playback engine: a thin wrapper around a rodio `Player` that tracks
//! whether a track is loaded, paused or stopped.

use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{Decoder, DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink, Player, Source};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stopped,
    Playing,
    Paused,
}

pub struct Audio {
    // Field order matters: `player` is dropped before the device sink it feeds.
    player: Player,
    _sink: MixerDeviceSink,
    state: State,
    volume: f32,
    muted: bool,
}

impl Audio {
    pub fn new() -> Result<Self, DeviceSinkError> {
        let mut sink = DeviceSinkBuilder::open_default_sink()?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());
        let audio = Self {
            player,
            _sink: sink,
            state: State::Stopped,
            volume: 0.8,
            muted: false,
        };
        audio.apply_volume();
        Ok(audio)
    }

    /// Replaces whatever is playing with `path` and starts playback.
    /// Returns the duration reported by the decoder, if any.
    pub fn play_file(&mut self, path: &Path) -> Result<Option<Duration>, Box<dyn Error>> {
        // Decode before touching the player so a bad file keeps the current track.
        let source = Decoder::try_from(File::open(path)?)?;
        let total = source.total_duration();
        self.player.clear();
        self.player.append(source);
        self.player.play();
        self.state = State::Playing;
        Ok(total)
    }

    pub fn pause(&mut self) {
        if self.state == State::Playing {
            self.player.pause();
            self.state = State::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == State::Paused {
            self.player.play();
            self.state = State::Playing;
        }
    }

    pub fn stop(&mut self) {
        self.player.clear();
        self.state = State::Stopped;
    }

    pub fn seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        if self.state == State::Stopped {
            return Ok(());
        }
        self.player.try_seek(pos)
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn position(&self) -> Duration {
        if self.state == State::Stopped {
            Duration::ZERO
        } else {
            self.player.get_pos()
        }
    }

    /// True once a loaded track has played to its end.
    pub fn finished(&self) -> bool {
        self.state != State::Stopped && self.player.empty()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.muted = false;
        self.apply_volume();
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        self.apply_volume();
    }

    fn apply_volume(&self) {
        let v = if self.muted { 0.0 } else { self.volume };
        // Squared curve: perceived loudness is closer to linear on the slider.
        self.player.set_volume(v * v);
    }
}
