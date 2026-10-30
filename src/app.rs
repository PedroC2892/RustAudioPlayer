//! egui front end in the DESIGN.md style: library and playlists in a left
//! sidebar, the track table on a frost placard, a player bar and a
//! now-playing marquee fixed to the bottom.
//!
//! Every region is laid out inside an explicit rect (columns, rows, bars), so
//! nothing asks for more width than the window gives it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Key, Layout, Margin, Modifiers, Painter, Rect,
    RichText, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureOptions, UiBuilder, pos2,
    vec2,
};
use eframe::epaint::text::{LayoutJob, TextFormat, TextWrapping};

use crate::audio::{Audio, State};
use crate::playlist::{
    self, EXTENSIONS, MetadataLoader, Order, Playlist, Repeat, Track, format_time,
};
use crate::store;
use crate::theme::{self, ASH, CHARCOAL, GRAPHITE, PILL, RED, SMOKE, muted};

const SEEK_STEP: Duration = Duration::from_secs(5);
const VOLUME_STEP: f32 = 0.05;
/// "Previous" restarts the track instead when past this point.
const RESTART_THRESHOLD: Duration = Duration::from_secs(3);
/// `lists[LIBRARY]` holds every track ever added; the rest are user playlists.
const LIBRARY: usize = 0;
const ROW_HEIGHT: f32 = 48.0;
const MARQUEE_SPEED: f64 = 40.0; // px per second

/// A request made while drawing, applied once drawing is done so the draw
/// code only needs shared access to the lists.
enum Action {
    Select(usize),
    Play(usize, usize),
    PlayView { shuffle: bool },
    RemoveTrack(usize, usize),
    AddToPlaylist { track: (usize, usize), to: usize },
    NewPlaylist { with: Option<(usize, usize)> },
    DeletePlaylist(usize),
    StartRename(usize),
    Rename(usize, String),
    CancelRename,
    AddFiles,
    AddFolder,
}

pub struct App {
    audio: Audio,
    lists: Vec<Playlist>,
    order: Order,
    /// List shown in the main area.
    view: usize,
    /// (list, track) that is loaded in the player.
    playing: Option<(usize, usize)>,
    /// Length of the loaded track: tag metadata first, decoder as fallback.
    total: Option<Duration>,
    /// Seek fraction while the user drags the bar; seeking happens on release.
    seek_drag: Option<f32>,
    filter: String,
    /// (list, edited name, focus requested on next frame).
    renaming: Option<(usize, String, bool)>,
    status: Option<String>,
    data_dir: Option<PathBuf>,
    logo: TextureHandle,
    meta: MetadataLoader,
    /// Files sent by later launches of the app (see `instance`).
    opened: Option<Receiver<Vec<PathBuf>>>,
    /// Opened file waiting for the user to allow replacing what is playing.
    ask_play: Option<PathBuf>,
}

impl App {
    pub fn new(
        ctx: &egui::Context,
        audio: Audio,
        paths: &[PathBuf],
        opened: Option<Receiver<Vec<PathBuf>>>,
    ) -> Self {
        let data_dir = store::data_dir();
        // Lists open instantly with file names; tags arrive from the loader.
        let lists: Vec<Playlist> = match &data_dir {
            Some(dir) => store::load(dir)
                .into_iter()
                .map(|saved| Playlist {
                    name: saved.name,
                    tracks: saved.paths.into_iter().map(Track::stub).collect(),
                })
                .collect(),
            None => vec![Playlist::new("Library")],
        };
        let size = 128;
        let logo = ctx.load_texture(
            "logo",
            egui::ColorImage::from_rgba_unmultiplied([size, size], &theme::icon_rgba(size as u32)),
            TextureOptions::LINEAR,
        );
        let repaint = ctx.clone();
        let meta = MetadataLoader::new(move || repaint.request_repaint());
        let mut unique = HashSet::new();
        for t in lists.iter().flat_map(|p| &p.tracks) {
            if unique.insert(&t.path) {
                meta.request(t.path.clone());
            }
        }
        let mut app = Self {
            audio,
            lists,
            order: Order {
                repeat: Repeat::Off,
                shuffle: false,
            },
            view: LIBRARY,
            playing: None,
            total: None,
            seek_drag: None,
            filter: String::new(),
            renaming: None,
            status: None,
            data_dir,
            logo,
            meta,
            opened,
            ask_play: None,
        };
        if !paths.is_empty() {
            app.open_external(ctx, paths);
        }
        app
    }

    // ---- state changes -------------------------------------------------

    fn current_track(&self) -> Option<&Track> {
        self.playing
            .and_then(|(l, i)| self.lists.get(l)?.tracks.get(i))
    }

    /// Swaps stub tracks for the ones the loader finished reading.
    fn apply_loaded_metadata(&mut self) {
        let loaded = self.meta.take_loaded();
        if loaded.is_empty() {
            return;
        }
        for track in self.lists.iter_mut().flat_map(|p| p.tracks.iter_mut()) {
            if let Some(full) = loaded.get(&track.path) {
                track.clone_from(full);
            }
        }
        if self.total.is_none() {
            self.total = self.current_track().and_then(|t| t.duration);
        }
    }

    fn save(&mut self) {
        if let Some(dir) = &self.data_dir
            && let Err(e) = store::save(dir, &self.lists)
        {
            self.status = Some(format!("Could not save playlists: {e}"));
        }
    }

    /// Adds files/folders to the shown list (and always to the library), and
    /// starts playback if the player is idle.
    fn add_paths<P: AsRef<Path>>(&mut self, paths: &[P]) {
        if let Some((list, i)) = self.add_to(self.view, paths)
            && self.audio.state() == State::Stopped
        {
            self.play(list, i);
        }
    }

    /// Files opened from outside (file manager, second launch): they go to the
    /// library and play right away, unless something is playing; then the
    /// user is asked first.
    fn open_external(&mut self, ctx: &egui::Context, paths: &[PathBuf]) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        let Some((list, i)) = self.add_to(LIBRARY, paths) else {
            return;
        };
        if self.audio.state() == State::Playing {
            self.ask_play = Some(self.lists[list].tracks[i].path.clone());
        } else {
            self.ask_play = None;
            self.play(list, i);
        }
    }

    /// Adds files/folders to the library and, if `target` is a playlist, to
    /// that playlist too. Returns where the first added track sits in `target`.
    fn add_to<P: AsRef<Path>>(&mut self, target: usize, paths: &[P]) -> Option<(usize, usize)> {
        let new: Vec<Track> = paths
            .iter()
            .flat_map(|p| playlist::scan(p.as_ref()))
            .collect();
        let Some(first_path) = new.first().map(|t| t.path.clone()) else {
            self.status = Some("No playable files found".into());
            return None;
        };
        // ponytail: linear `contains` per new track; use a path index if
        // libraries reach tens of thousands of tracks.
        for t in &new {
            self.meta.request(t.path.clone());
            if !self.lists[LIBRARY].contains(&t.path) {
                self.lists[LIBRARY].tracks.push(t.clone());
            }
        }
        let first = if target == LIBRARY {
            self.lists[LIBRARY]
                .tracks
                .iter()
                .position(|t| t.path == first_path)
        } else {
            let start = self.lists[target].tracks.len();
            self.lists[target].tracks.extend(new);
            Some(start)
        };
        self.save();
        first.map(|i| (target, i))
    }

    /// Answer to the "play this file now?" question.
    fn answer_ask_play(&mut self, play: bool) {
        let Some(path) = self.ask_play.take() else {
            return;
        };
        // Looked up by path: the library may have changed while asking.
        if play
            && let Some(i) = self.lists[LIBRARY]
                .tracks
                .iter()
                .position(|t| t.path == path)
        {
            self.play(LIBRARY, i);
        }
    }

    fn play(&mut self, list: usize, index: usize) {
        let Some(track) = self.lists.get(list).and_then(|p| p.tracks.get(index)) else {
            return;
        };
        // Point at the track even on failure, so Next moves past a broken file.
        self.playing = Some((list, index));
        self.seek_drag = None;
        match self.audio.play_file(&track.path) {
            Ok(decoder_total) => {
                self.total = track.duration.or(decoder_total);
                self.status = None;
            }
            Err(e) => {
                self.audio.stop();
                self.total = None;
                self.status = Some(format!("Cannot play \"{}\": {e}", track.title));
            }
        }
    }

    fn toggle_play(&mut self) {
        match self.audio.state() {
            State::Playing => self.audio.pause(),
            State::Paused => self.audio.resume(),
            State::Stopped => match self.playing {
                Some((l, i)) => self.play(l, i),
                None => self.play(self.view, 0),
            },
        }
    }

    fn stop(&mut self) {
        self.audio.stop();
        self.seek_drag = None;
    }

    /// The list the transport works on, its length and the current index.
    fn queue(&self) -> (usize, usize, Option<usize>) {
        let (list, current) = match self.playing {
            Some((l, i)) => (l, Some(i)),
            None => (self.view, None),
        };
        let len = self.lists.get(list).map_or(0, |p| p.tracks.len());
        (list, len, current)
    }

    fn next(&mut self, manual: bool) {
        let (list, len, current) = self.queue();
        match self.order.next_index(len, current, manual) {
            Some(i) => self.play(list, i),
            None => self.stop(),
        }
    }

    fn prev(&mut self) {
        if self.audio.position() > RESTART_THRESHOLD {
            self.seek_to(Duration::ZERO);
            return;
        }
        let (list, len, current) = self.queue();
        if let Some(i) = self.order.prev_index(len, current) {
            self.play(list, i);
        }
    }

    fn seek_to(&mut self, pos: Duration) {
        let pos = self.total.map_or(pos, |t| pos.min(t));
        if let Err(e) = self.audio.seek(pos) {
            self.status = Some(format!("Seek failed: {e}"));
        }
    }

    fn seek_by(&mut self, forward: bool) {
        let pos = self.audio.position();
        self.seek_to(if forward {
            pos + SEEK_STEP
        } else {
            pos.saturating_sub(SEEK_STEP)
        });
    }

    /// Forgets the loaded track (it was removed from its list).
    fn unload(&mut self) {
        self.stop();
        self.playing = None;
        self.total = None;
    }

    fn remove_track(&mut self, list: usize, index: usize) {
        let Some(p) = self.lists.get_mut(list) else {
            return;
        };
        if index >= p.tracks.len() {
            return;
        }
        p.tracks.remove(index);
        match self.playing {
            Some((l, i)) if l == list && i == index => self.unload(),
            Some((l, i)) if l == list && i > index => self.playing = Some((l, i - 1)),
            _ => {}
        }
        self.save();
    }

    fn new_playlist(&mut self, with: Option<(usize, usize)>) {
        let name = format!("Playlist {}", self.lists.len());
        let mut list = Playlist::new(name.clone());
        if let Some(t) = with.and_then(|(l, i)| self.lists.get(l)?.tracks.get(i)) {
            list.tracks.push(t.clone());
        }
        self.lists.push(list);
        let index = self.lists.len() - 1;
        if with.is_none() {
            self.view = index;
            self.filter.clear();
        }
        self.renaming = Some((index, name, true));
        self.save();
    }

    fn delete_playlist(&mut self, list: usize) {
        if list == LIBRARY || list >= self.lists.len() {
            return;
        }
        self.lists.remove(list);
        match self.playing {
            Some((l, _)) if l == list => self.unload(),
            Some((l, i)) if l > list => self.playing = Some((l - 1, i)),
            _ => {}
        }
        if self.view == list {
            self.view = LIBRARY;
        } else if self.view > list {
            self.view -= 1;
        }
        self.renaming = None;
        self.save();
    }

    // ponytail: dialogs block the UI thread while open (audio keeps playing);
    // move to rfd::AsyncFileDialog if that ever matters.
    fn open_files_dialog(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("Audio", EXTENSIONS)
            .pick_files()
        {
            self.add_paths(&files);
        }
    }

    fn open_folder_dialog(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            self.add_paths(&[dir]);
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Select(l) => {
                self.view = l;
                self.filter.clear();
            }
            Action::Play(l, i) => self.play(l, i),
            Action::PlayView { shuffle } => {
                self.order.shuffle = shuffle;
                let len = self.lists[self.view].tracks.len();
                let start = if shuffle {
                    self.order.next_index(len, Some(0), true).unwrap_or(0)
                } else {
                    0
                };
                self.play(self.view, start);
            }
            Action::RemoveTrack(l, i) => self.remove_track(l, i),
            Action::AddToPlaylist { track: (l, i), to } => {
                let track = self.lists.get(l).and_then(|p| p.tracks.get(i)).cloned();
                if let (Some(t), Some(p)) = (track, self.lists.get_mut(to)) {
                    p.tracks.push(t);
                    self.save();
                }
            }
            Action::NewPlaylist { with } => self.new_playlist(with),
            Action::DeletePlaylist(l) => self.delete_playlist(l),
            Action::StartRename(l) => {
                if let Some(p) = self.lists.get(l) {
                    self.renaming = Some((l, p.name.clone(), true));
                }
            }
            Action::Rename(l, name) => {
                self.renaming = None;
                let name = name.trim();
                if let Some(p) = self.lists.get_mut(l)
                    && !name.is_empty()
                {
                    p.name = name.to_owned();
                    self.save();
                }
            }
            Action::CancelRename => self.renaming = None,
            Action::AddFiles => self.open_files_dialog(),
            Action::AddFolder => self.open_folder_dialog(),
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_paths(&dropped);
        }

        // Typing in the filter or a playlist name must not trigger shortcuts,
        // and neither must keys pressed while the play question is open.
        if ctx.egui_wants_keyboard_input() || self.ask_play.is_some() {
            return;
        }
        let key = |k| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        if key(Key::Space) {
            self.toggle_play();
        }
        if key(Key::ArrowRight) {
            self.seek_by(true);
        }
        if key(Key::ArrowLeft) {
            self.seek_by(false);
        }
        if key(Key::ArrowUp) {
            self.audio.set_volume(self.audio.volume() + VOLUME_STEP);
        }
        if key(Key::ArrowDown) {
            self.audio.set_volume(self.audio.volume() - VOLUME_STEP);
        }
        if key(Key::N) {
            self.next(true);
        }
        if key(Key::P) {
            self.prev();
        }
        if key(Key::S) {
            self.stop();
        }
        if key(Key::M) {
            self.audio.toggle_mute();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.open_files_dialog();
        }
    }

    // ---- drawing -------------------------------------------------------

    fn sidebar(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.add(
                egui::Image::from_texture(egui::load::SizedTexture::from_handle(&self.logo))
                    .fit_to_exact_size(vec2(30.0, 30.0)),
            );
            ui.add(egui::Label::new(theme::display("Rust Audio", 20.0).color(ASH)).truncate());
        });
        ui.add_space(28.0);

        ui.label(RichText::new("Your library").small().color(muted(0.6)));
        let playing_list = self.playing.map(|(l, _)| l);
        let library = &self.lists[LIBRARY];
        let resp = nav_item(
            ui,
            "All tracks",
            library.tracks.len(),
            self.view == LIBRARY,
            playing_list == Some(LIBRARY),
        );
        if resp.clicked() {
            acts.push(Action::Select(LIBRARY));
        }

        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Playlists").small().color(muted(0.6)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button("+ New")
                    .on_hover_text("Create a playlist")
                    .clicked()
                {
                    acts.push(Action::NewPlaylist { with: None });
                }
            });
        });

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if self.lists.len() == 1 {
                    ui.label(
                        RichText::new("No playlists yet.\nRight-click a track to add it to one.")
                            .size(13.0)
                            .color(muted(0.5)),
                    );
                }
                for i in 1..self.lists.len() {
                    if let Some((r, name, focus)) = &mut self.renaming
                        && *r == i
                    {
                        let edit = ui.add(
                            egui::TextEdit::singleline(name)
                                .desired_width(f32::INFINITY)
                                .margin(vec2(12.0, 6.0)),
                        );
                        if std::mem::take(focus) {
                            edit.request_focus();
                        }
                        if ui.input(|inp| inp.key_pressed(Key::Escape)) {
                            acts.push(Action::CancelRename);
                        } else if edit.lost_focus() {
                            acts.push(Action::Rename(i, name.clone()));
                        }
                        continue;
                    }
                    let list = &self.lists[i];
                    let resp = nav_item(
                        ui,
                        &list.name,
                        list.tracks.len(),
                        self.view == i,
                        playing_list == Some(i),
                    );
                    if resp.clicked() {
                        acts.push(Action::Select(i));
                    }
                    if resp.double_clicked() && !list.tracks.is_empty() {
                        acts.push(Action::Play(i, 0));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Play").clicked() {
                            acts.push(Action::Play(i, 0));
                        }
                        if ui.button("Rename").clicked() {
                            acts.push(Action::StartRename(i));
                        }
                        if ui.button("Delete").clicked() {
                            acts.push(Action::DeletePlaylist(i));
                        }
                    });
                }
            });
    }

    fn main_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let view = self.view.min(self.lists.len() - 1);
        let list = &self.lists[view];
        let heading = if view == LIBRARY {
            "All tracks"
        } else {
            &list.name
        };

        // Section heading with the signature trailing ellipsis.
        ui.add(egui::Label::new(theme::display(format!("{heading}…"), 52.0)).truncate());
        ui.horizontal(|ui| {
            let n = list.tracks.len();
            let meta = format!(
                "{n} track{} · {}",
                if n == 1 { "" } else { "s" },
                format_time(list.total())
            );
            ui.label(RichText::new(meta).size(13.0).color(muted(0.7)));
            if let Some(msg) = &self.status {
                ui.add(egui::Label::new(RichText::new(msg).size(13.0).color(RED)).truncate());
            }
        });
        ui.add_space(12.0);

        ui.horizontal_wrapped(|ui| {
            let has_tracks = !list.tracks.is_empty();
            if ui
                .add_enabled(has_tracks, egui::Button::new("Play"))
                .clicked()
            {
                acts.push(Action::PlayView { shuffle: false });
            }
            if ui
                .add_enabled(has_tracks, egui::Button::new("Shuffle"))
                .clicked()
            {
                acts.push(Action::PlayView { shuffle: true });
            }
            if ui.button("Add files…").on_hover_text("Ctrl+O").clicked() {
                acts.push(Action::AddFiles);
            }
            if ui.button("Add folder…").clicked() {
                acts.push(Action::AddFolder);
            }
            if view != LIBRARY {
                if ui.button("Rename").clicked() {
                    acts.push(Action::StartRename(view));
                }
                if ui.button("Delete").clicked() {
                    acts.push(Action::DeletePlaylist(view));
                }
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Filter…")
                    .desired_width(180.0)
                    .margin(vec2(14.0, 5.0)),
            );
        });
        ui.add_space(16.0);

        // The track table is a frost placard: sharp corners, hairline border.
        let needle = self.filter.to_lowercase();
        let rows: Vec<usize> = (0..list.tracks.len())
            .filter(|&i| needle.is_empty() || matches_filter(&list.tracks[i], &needle))
            .collect();
        let card = ui.available_rect_before_wrap();
        ui.painter().rect_filled(card, 0.0, CHARCOAL);
        ui.painter()
            .rect_stroke(card, 0.0, Stroke::new(1.0, SMOKE), StrokeKind::Inside);
        let header = Rect::from_min_size(card.min, vec2(card.width(), 34.0));
        let cols = Columns::new(header);
        let painter = ui.painter();
        let caption = FontId::proportional(11.0);
        let faint = muted(0.55);
        paint_text(
            painter,
            cols.num,
            "#",
            caption.clone(),
            faint,
            Align2::LEFT_CENTER,
        );
        paint_text(
            painter,
            cols.title,
            "Title",
            caption.clone(),
            faint,
            Align2::LEFT_CENTER,
        );
        paint_text(
            painter,
            cols.album,
            "Album",
            caption.clone(),
            faint,
            Align2::LEFT_CENTER,
        );
        paint_text(
            painter,
            cols.time,
            "Time",
            caption,
            faint,
            Align2::RIGHT_CENTER,
        );
        painter.hline(header.x_range(), header.bottom(), Stroke::new(1.0, SMOKE));

        let body = Rect::from_min_max(pos2(card.left(), header.bottom() + 1.0), card.max);
        ui.scope_builder(UiBuilder::new().max_rect(body), |ui| {
            if rows.is_empty() {
                let msg = if list.tracks.is_empty() {
                    "Drop audio files or folders here, or use Add files…".to_owned()
                } else {
                    format!("No tracks match \"{}\"", self.filter)
                };
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new(msg).color(muted(0.6)));
                });
                return;
            }
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().auto_shrink(false).show_rows(
                ui,
                ROW_HEIGHT,
                rows.len(),
                |ui, range| {
                    for &i in &rows[range] {
                        let current = self.playing == Some((view, i));
                        let resp = track_row(ui, &list.tracks[i], i, current);
                        if resp.double_clicked() {
                            acts.push(Action::Play(view, i));
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Play").clicked() {
                                acts.push(Action::Play(view, i));
                            }
                            ui.menu_button("Add to playlist", |ui| {
                                if ui.button("New playlist").clicked() {
                                    acts.push(Action::NewPlaylist {
                                        with: Some((view, i)),
                                    });
                                }
                                for (j, p) in self.lists.iter().enumerate().skip(1) {
                                    if j != view && ui.button(&p.name).clicked() {
                                        acts.push(Action::AddToPlaylist {
                                            track: (view, i),
                                            to: j,
                                        });
                                    }
                                }
                            });
                            let remove = if view == LIBRARY {
                                "Remove from library"
                            } else {
                                "Remove from playlist"
                            };
                            if ui.button(remove).clicked() {
                                acts.push(Action::RemoveTrack(view, i));
                            }
                        });
                    }
                },
            );
        });
    }

    fn player_bar(&mut self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        let side = (full.width() * 0.3).min(340.0);
        let left = Rect::from_min_max(full.min, pos2(full.left() + side, full.bottom()));
        let right = Rect::from_min_max(pos2(full.right() - side, full.top()), full.max);
        let center = Rect::from_min_max(
            pos2(left.right() + 16.0, full.top()),
            pos2(right.left() - 16.0, full.bottom()),
        );

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(left)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| self.now_playing(ui),
        );
        self.transport(ui, center);
        self.volume(ui, right);
    }

    fn now_playing(&self, ui: &mut egui::Ui) {
        ui.add(
            egui::Image::from_texture(egui::load::SizedTexture::from_handle(&self.logo))
                .fit_to_exact_size(vec2(52.0, 52.0)),
        );
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            match self.current_track() {
                Some(t) => {
                    ui.add(egui::Label::new(theme::display(&t.title, 16.0).color(ASH)).truncate());
                    let byline = t.byline();
                    if !byline.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(byline).size(13.0).color(ASH))
                                .truncate(),
                        );
                    }
                    if !t.info.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(&t.info).small().color(muted(0.6)))
                                .truncate(),
                        );
                    }
                }
                None => {
                    ui.add(egui::Label::new(theme::display("Nothing playing", 16.0)).truncate());
                    ui.label(
                        RichText::new("Pick a track to start")
                            .size(13.0)
                            .color(muted(0.6)),
                    );
                }
            }
        });
    }

    fn transport(&mut self, ui: &mut egui::Ui, area: Rect) {
        let playing = self.audio.state() == State::Playing;
        let cy = area.top() + 24.0;
        let at = |dx: f32, size: f32| {
            Rect::from_center_size(pos2(area.center().x + dx, cy), vec2(size, size))
        };

        let shuffle = self.order.shuffle;
        if glyph_button(
            ui,
            at(-112.0, 30.0),
            "shuffle",
            Glyph::Shuffle,
            false,
            shuffle,
            "Shuffle",
        ) {
            self.order.shuffle = !shuffle;
        }
        if glyph_button(
            ui,
            at(-58.0, 34.0),
            "prev",
            Glyph::Prev,
            false,
            false,
            "Previous (P)",
        ) {
            self.prev();
        }
        let main = if playing { Glyph::Pause } else { Glyph::Play };
        if glyph_button(
            ui,
            at(0.0, 44.0),
            "play",
            main,
            true,
            false,
            "Play/Pause (Space)",
        ) {
            self.toggle_play();
        }
        if glyph_button(
            ui,
            at(58.0, 34.0),
            "next",
            Glyph::Next,
            false,
            false,
            "Next (N)",
        ) {
            self.next(true);
        }
        let repeat = self.order.repeat;
        let hint = match repeat {
            Repeat::Off => "Repeat: off",
            Repeat::All => "Repeat: all",
            Repeat::One => "Repeat: one",
        };
        let on = repeat != Repeat::Off;
        if glyph_button(
            ui,
            at(112.0, 30.0),
            "repeat",
            Glyph::Repeat {
                one: repeat == Repeat::One,
            },
            false,
            on,
            hint,
        ) {
            self.order.repeat = repeat.cycle();
        }

        // Seek row: elapsed, bar, total.
        let row = Rect::from_min_max(pos2(area.left(), cy + 30.0), pos2(area.right(), cy + 50.0));
        let label_w = 52.0;
        let painter = ui.painter();
        let small = FontId::proportional(12.0);
        let total = self.total.map(|t| t.as_secs_f32()).filter(|t| *t > 0.0);
        let frac = self
            .seek_drag
            .unwrap_or_else(|| total.map_or(0.0, |t| self.audio.position().as_secs_f32() / t));
        let elapsed = match (self.seek_drag, total) {
            (Some(f), Some(t)) => Duration::from_secs_f32(f * t),
            _ => self.audio.position(),
        };
        let elapsed_rect = Rect::from_min_size(row.min, vec2(label_w, row.height()));
        let total_rect = Rect::from_min_max(pos2(row.right() - label_w, row.top()), row.max);
        paint_text(
            painter,
            elapsed_rect,
            &format_time(elapsed),
            small.clone(),
            muted(0.7),
            Align2::LEFT_CENTER,
        );
        let total_text = self.total.map_or("-:--".into(), format_time);
        paint_text(
            painter,
            total_rect,
            &total_text,
            small,
            muted(0.7),
            Align2::RIGHT_CENTER,
        );

        let bar = Rect::from_min_max(
            pos2(elapsed_rect.right(), row.top()),
            pos2(total_rect.left(), row.bottom()),
        );
        let can_seek = total.is_some() && self.audio.state() != State::Stopped;
        let (resp, pointer) = line_bar(ui, bar, "seek", frac, can_seek);
        if resp.dragged() {
            self.seek_drag = pointer;
        }
        if (resp.drag_stopped() || resp.clicked())
            && let (Some(f), Some(t)) = (pointer.or(self.seek_drag), total)
        {
            self.seek_drag = None;
            self.seek_to(Duration::from_secs_f32(f * t));
        }
    }

    fn volume(&mut self, ui: &mut egui::Ui, area: Rect) {
        let cy = area.center().y;
        let bar = Rect::from_min_max(
            pos2(area.right() - 110.0, cy - 10.0),
            pos2(area.right(), cy + 10.0),
        );
        let icon = Rect::from_center_size(pos2(bar.left() - 22.0, cy), vec2(30.0, 30.0));
        let silent = self.audio.muted() || self.audio.volume() == 0.0;
        let glyph = Glyph::Volume { muted: silent };
        if glyph_button(ui, icon, "mute", glyph, false, false, "Mute (M)") {
            self.audio.toggle_mute();
        }
        let level = if self.audio.muted() {
            0.0
        } else {
            self.audio.volume()
        };
        let (resp, pointer) = line_bar(ui, bar, "volume", level, true);
        if (resp.dragged() || resp.clicked())
            && let Some(v) = pointer
        {
            self.audio.set_volume(v);
        }
    }

    /// Modal shown when a file is opened while music plays. Returns the answer
    /// once given: `Some(true)` play it now, `Some(false)` keep listening.
    fn ask_play_dialog(&self, ctx: &egui::Context) -> Option<bool> {
        let path = self.ask_play.as_ref()?;
        let title_of = |p: &Path| {
            self.lists[LIBRARY]
                .tracks
                .iter()
                .find(|t| t.path == p)
                .map_or_else(|| p.display().to_string(), |t| t.title.clone())
        };
        let opened = title_of(path);
        let current = self.current_track().map(|t| t.title.clone());

        let mut answer = None;
        let frame = Frame::NONE
            .fill(CHARCOAL)
            .stroke(Stroke::new(1.0, SMOKE))
            .inner_margin(Margin::same(24));
        let modal = egui::Modal::new(egui::Id::unique("ask-play"))
            .frame(frame)
            .backdrop_color(GRAPHITE.gamma_multiply(0.7))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(theme::display("Play this file now?", 24.0));
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new(&opened).color(ASH)).truncate());
                if let Some(current) = &current {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("This stops \"{current}\"."))
                                .size(13.0)
                                .color(muted(0.6)),
                        )
                        .truncate(),
                    );
                }
                ui.label(
                    RichText::new("It was added to your library either way.")
                        .size(13.0)
                        .color(muted(0.6)),
                );
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let play = egui::Button::new(RichText::new("Play now").color(RED))
                        .stroke(Stroke::new(1.0, RED));
                    if ui.add(play).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        answer = Some(true);
                    }
                    if ui.button("Keep listening").clicked() {
                        answer = Some(false);
                    }
                });
            });
        if modal.should_close() && answer.is_none() {
            answer = Some(false);
        }
        answer
    }

    /// Bottom ticker, always present. Scrolls while music plays.
    fn marquee(&self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let text = match self.current_track() {
            Some(t) => {
                let mut parts = vec![format!("Now playing: {}", t.title)];
                parts.extend(
                    [t.byline(), t.info.clone()]
                        .into_iter()
                        .filter(|s| !s.is_empty()),
                );
                parts.join("   ·   ")
            }
            None => {
                "Rust Audio Player   ·   Drop audio files or folders anywhere to start listening"
                    .to_owned()
            }
        };
        let painter = ui.painter_at(rect);
        let galley = painter.layout_no_wrap(
            format!("{text}            "),
            FontId::proportional(13.0),
            ASH,
        );
        let width = galley.size().x;
        let y = rect.center().y - galley.size().y / 2.0;
        let playing = self.audio.state() == State::Playing;
        let offset = if playing {
            (ui.input(|i| i.time) * MARQUEE_SPEED % f64::from(width)) as f32
        } else {
            0.0
        };
        let mut x = rect.left() + 20.0 - offset;
        while x < rect.right() {
            painter.galley(pos2(x, y), galley.clone(), ASH);
            x += width;
        }
        if playing {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
    }
}

impl eframe::App for App {
    /// Runs even while the window is hidden, so playback advances when minimized.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let opened: Vec<Vec<PathBuf>> = self
            .opened
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for paths in opened {
            self.open_external(ctx, &paths);
        }
        self.apply_loaded_metadata();
        if self.audio.finished() {
            self.next(false);
        }
        if self.audio.state() == State::Playing {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx);
        let mut acts = Vec::new();

        egui::Panel::bottom("marquee")
            .exact_size(28.0)
            .resizable(false)
            .frame(Frame::NONE.fill(GRAPHITE))
            .show(ui, |ui| self.marquee(ui));
        egui::Panel::bottom("player")
            .exact_size(104.0)
            .resizable(false)
            .frame(
                Frame::NONE
                    .fill(GRAPHITE)
                    .inner_margin(Margin::symmetric(20, 12)),
            )
            .show(ui, |ui| self.player_bar(ui));
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(240.0)
            .size_range(190.0..=360.0)
            .frame(Frame::NONE.fill(GRAPHITE).inner_margin(Margin::same(20)))
            .show(ui, |ui| self.sidebar(ui, &mut acts));
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(GRAPHITE).inner_margin(Margin {
                left: 40,
                right: 40,
                top: 28,
                bottom: 20,
            }))
            .show(ui, |ui| self.main_view(ui, &mut acts));

        for action in acts {
            self.apply(action);
        }
        if let Some(play) = self.ask_play_dialog(&ctx) {
            self.answer_ask_play(play);
        }
    }
}

fn matches_filter(t: &Track, needle: &str) -> bool {
    [Some(&t.title), t.artist.as_ref(), t.album.as_ref()]
        .into_iter()
        .flatten()
        .any(|s| s.to_lowercase().contains(needle))
}

/// Column rects of the track table for one row.
struct Columns {
    num: Rect,
    title: Rect,
    album: Rect,
    time: Rect,
}

impl Columns {
    fn new(row: Rect) -> Self {
        let inner = row.shrink2(vec2(20.0, 0.0));
        let num_w = 36.0;
        let time_w = 64.0;
        let flex = (inner.width() - num_w - time_w).max(0.0);
        let title_w = flex * 0.58;
        let x = |offset: f32, w: f32| {
            Rect::from_min_size(
                pos2(inner.left() + offset, inner.top()),
                vec2(w, inner.height()),
            )
        };
        Self {
            num: x(0.0, num_w),
            title: x(num_w, title_w - 16.0),
            album: x(num_w + title_w, flex - title_w - 16.0),
            time: x(num_w + flex, time_w),
        }
    }
}

fn track_row(ui: &mut egui::Ui, t: &Track, index: usize, current: bool) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    let painter = ui.painter();
    if current {
        painter.rect_filled(rect, 0.0, RED.gamma_multiply(0.14));
    } else if resp.hovered() {
        painter.rect_filled(rect, 0.0, SMOKE.gamma_multiply(0.6));
    }
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, SMOKE));

    let cols = Columns::new(rect);
    let soft = muted(0.6);
    if current {
        let tip = pos2(cols.num.left() + 10.0, cols.num.center().y);
        arrow_head(painter, tip, 1.0, 22.0, RED);
    } else {
        let n = (index + 1).to_string();
        paint_text(
            painter,
            cols.num,
            &n,
            FontId::proportional(13.0),
            soft,
            Align2::LEFT_CENTER,
        );
    }
    let (top, bottom) = cols.title.split_top_bottom_at_fraction(0.55);
    paint_text(
        painter,
        top,
        &t.title,
        FontId::proportional(15.0),
        ASH,
        Align2::LEFT_BOTTOM,
    );
    let artist = t.artist.as_deref().unwrap_or("Unknown artist");
    paint_text(
        painter,
        bottom,
        artist,
        FontId::proportional(12.0),
        soft,
        Align2::LEFT_TOP,
    );
    let album = t.album.as_deref().unwrap_or("");
    paint_text(
        painter,
        cols.album,
        album,
        FontId::proportional(13.0),
        soft,
        Align2::LEFT_CENTER,
    );
    let time = t.duration.map_or(String::new(), format_time);
    paint_text(
        painter,
        cols.time,
        &time,
        FontId::proportional(13.0),
        soft,
        Align2::RIGHT_CENTER,
    );
    resp
}

/// Sidebar entry: ghost text link, outlined pill when selected.
fn nav_item(
    ui: &mut egui::Ui,
    name: &str,
    count: usize,
    selected: bool,
    playing: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_stroke(
            rect,
            PILL,
            Stroke::new(1.0, muted(0.55)),
            StrokeKind::Inside,
        );
    }
    let hot = selected || resp.hovered();
    let color = if hot { ASH } else { muted(0.8) };
    let inner = rect.shrink2(vec2(14.0, 0.0));
    let meta_w = if playing { 56.0 } else { 32.0 };
    let name_rect = Rect::from_min_max(inner.min, pos2(inner.right() - meta_w, inner.bottom()));
    let meta_rect = Rect::from_min_max(pos2(inner.right() - meta_w, inner.top()), inner.max);
    let text_rect = paint_text(
        painter,
        name_rect,
        name,
        FontId::proportional(15.0),
        color,
        Align2::LEFT_CENTER,
    );
    if resp.hovered() && !selected {
        painter.hline(
            text_rect.x_range(),
            text_rect.bottom(),
            Stroke::new(1.0, ASH),
        );
    }
    let count_rect = paint_text(
        painter,
        meta_rect,
        &count.to_string(),
        FontId::proportional(12.0),
        if playing { RED } else { muted(0.55) },
        Align2::RIGHT_CENTER,
    );
    if playing {
        painter.circle_filled(pos2(count_rect.left() - 8.0, rect.center().y), 3.0, RED);
    }
    resp
}

#[derive(Clone, Copy)]
enum Glyph {
    Play,
    Pause,
    Prev,
    Next,
    Shuffle,
    Repeat { one: bool },
    Volume { muted: bool },
}

/// Small filled triangle pointing right (`dir` 1.0) or left (-1.0), tip at `tip`.
fn arrow_head(painter: &Painter, tip: egui::Pos2, dir: f32, s: f32, color: Color32) {
    let back = 0.35 * s * dir;
    painter.add(Shape::convex_polygon(
        vec![tip, tip + vec2(-back, -0.3 * s), tip + vec2(-back, 0.3 * s)],
        color,
        Stroke::NONE,
    ));
}

/// Icon button painted inside `rect`. `ring` draws the outlined circle used
/// for the main play button; `active` marks an enabled toggle with a dot.
fn glyph_button(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &str,
    glyph: Glyph,
    ring: bool,
    active: bool,
    hint: &str,
) -> bool {
    let resp = ui
        .interact(rect, ui.scope_id().with(id), Sense::click())
        .on_hover_text(hint);
    let hot = resp.hovered();
    let color = if active || ring {
        RED
    } else if hot {
        ASH
    } else {
        muted(0.7)
    };
    let painter = ui.painter();
    let c = rect.center();
    if ring {
        if hot {
            painter.circle_filled(c, rect.width() / 2.0, RED.gamma_multiply(0.15));
        }
        painter.circle_stroke(c, rect.width() / 2.0 - 0.5, Stroke::new(1.0, color));
    }
    let s = rect.width() * if ring { 0.2 } else { 0.24 };
    let poly = |pts: Vec<egui::Pos2>| Shape::convex_polygon(pts, color, Stroke::NONE);
    match glyph {
        Glyph::Play => {
            painter.add(poly(vec![
                c + vec2(-0.7 * s, -s),
                c + vec2(-0.7 * s, s),
                c + vec2(1.1 * s, 0.0),
            ]));
        }
        Glyph::Pause => {
            for dx in [-0.65 * s, 0.25 * s] {
                let bar = Rect::from_min_size(c + vec2(dx, -s), vec2(0.4 * s, 2.0 * s));
                painter.rect_filled(bar, 0.0, color);
            }
        }
        Glyph::Prev | Glyph::Next => {
            let dir = if matches!(glyph, Glyph::Next) {
                1.0
            } else {
                -1.0
            };
            painter.add(poly(vec![
                c + vec2(-0.8 * s * dir, -s),
                c + vec2(-0.8 * s * dir, s),
                c + vec2(0.7 * s * dir, 0.0),
            ]));
            let bar_x = c.x + 0.7 * s * dir;
            let bar = Rect::from_x_y_ranges(bar_x - 0.15 * s..=bar_x + 0.15 * s, c.y - s..=c.y + s);
            painter.rect_filled(bar, 0.0, color);
        }
        Glyph::Shuffle => {
            let line = Stroke::new(1.5, color);
            for flip in [1.0, -1.0] {
                let y = 0.6 * s * flip;
                painter.add(Shape::line(
                    vec![
                        c + vec2(-s, y),
                        c + vec2(-0.35 * s, y),
                        c + vec2(0.35 * s, -y),
                        c + vec2(0.85 * s, -y),
                    ],
                    line,
                ));
                arrow_head(painter, c + vec2(s, -y), 1.0, s, color);
            }
        }
        Glyph::Repeat { one } => {
            let (x, y) = (0.9 * s, 0.55 * s);
            painter.add(Shape::line(
                vec![
                    c + vec2(0.55 * x, -y),
                    c + vec2(-x, -y),
                    c + vec2(-x, y),
                    c + vec2(x, y),
                    c + vec2(x, -0.1 * y),
                ],
                Stroke::new(1.5, color),
            ));
            arrow_head(painter, c + vec2(0.85 * x, -y), 1.0, s, color);
            if one {
                painter.text(
                    c,
                    Align2::CENTER_CENTER,
                    "1",
                    FontId::proportional(0.9 * s),
                    color,
                );
            }
        }
        Glyph::Volume { muted: silent } => {
            let body = Rect::from_min_max(c + vec2(-s, -0.35 * s), c + vec2(-0.5 * s, 0.35 * s));
            painter.rect_filled(body, 0.0, color);
            painter.add(poly(vec![
                c + vec2(-0.5 * s, -0.35 * s),
                c + vec2(0.0, -0.8 * s),
                c + vec2(0.0, 0.8 * s),
                c + vec2(-0.5 * s, 0.35 * s),
            ]));
            let line = Stroke::new(1.5, color);
            if silent {
                let x = c + vec2(0.65 * s, 0.0);
                let d = 0.3 * s;
                painter.line_segment([x + vec2(-d, -d), x + vec2(d, d)], line);
                painter.line_segment([x + vec2(-d, d), x + vec2(d, -d)], line);
            } else {
                for radius in [0.45 * s, 0.85 * s] {
                    let arc = (-3..=3)
                        .map(|k| {
                            let a = k as f32 * 0.22;
                            c + radius * vec2(a.cos(), a.sin())
                        })
                        .collect();
                    painter.add(Shape::line(arc, line));
                }
            }
        }
    }
    if active {
        painter.circle_filled(pos2(c.x, rect.bottom() + 1.0), 2.0, RED);
    }
    resp.clicked()
}

/// Hairline progress bar with a knob on hover. Returns the response and the
/// pointer position as a 0..=1 fraction while it interacts with the bar.
fn line_bar(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &str,
    fraction: f32,
    enabled: bool,
) -> (egui::Response, Option<f32>) {
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(rect, ui.scope_id().with(id), sense);
    let painter = ui.painter();
    let y = rect.center().y;
    let x = rect.left() + rect.width() * fraction.clamp(0.0, 1.0);
    painter.hline(rect.x_range(), y, Stroke::new(2.0, muted(0.25)));
    painter.hline(
        rect.left()..=x,
        y,
        Stroke::new(2.0, if enabled { RED } else { muted(0.4) }),
    );
    if enabled && (resp.hovered() || resp.dragged()) {
        painter.circle_filled(pos2(x, y), 6.0, ASH);
    }
    let pointer = resp
        .interact_pointer_pos()
        .filter(|_| enabled && rect.width() > 0.0)
        .map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
    (resp, pointer)
}

/// Single-line text truncated with "…" to fit `rect`. Returns the text bounds.
fn paint_text(
    painter: &Painter,
    rect: Rect,
    text: &str,
    font: FontId,
    color: Color32,
    align: Align2,
) -> Rect {
    let mut job = LayoutJob::single_section(text.to_owned(), TextFormat::simple(font, color));
    job.wrap = TextWrapping::truncate_at_width(rect.width().max(0.0));
    let galley = painter.layout_job(job);
    let bounds = align.align_size_within_rect(galley.size(), rect);
    painter.galley(bounds.min, galley, color);
    bounds
}
