use std::{env, fs::File, io::BufReader, path::PathBuf, time::SystemTime};

use egui::Context;
use rodio::{source::Buffered, Decoder, OutputStream, Sink, Source};
use winit::window::Window;

use crate::{
    graphics::object::Renderable,
    models::{
        osu_map::{ApproachRate, HitObject, HitType, OverallDifficulty, Spinner},
        osu_replay::Keys,
    },
};

pub struct Gui {
    osu_data: Option<OsuData>,
    system_time: SystemTime,
    replay_data: Option<ReplayPlaybackData>,
    errors: Vec<String>,
}

struct OsuData {
    path: PathBuf,
    beatmaps: osu_db::listing::Listing,
}

struct ReplayPlaybackData {
    pub(crate) replay: crate::models::osu_replay::OsuReplay,
    pub(crate) beatmap: crate::models::osu_map::OsuMap,
    replay_path: String,
    #[allow(dead_code)] // need to store ref
    audio_output: OutputStream,

    offset: f64,
    playback_status: PlaybackStatus,
}

struct PlaybackStatus {
    playback_speed: f64,
    playing: bool,
    play_time: f64,
    replay_data_index: usize,
    audio_song_source: Buffered<Decoder<BufReader<File>>>,
    hit_sound_source: Buffered<Decoder<BufReader<File>>>,
    audio_stream_handle: rodio::OutputStreamHandle,
    audio_song_sink: Sink,

    misses: Vec<Miss>,
    volume: f64,

    hit_object_index: usize,
    next_hit_object_to_hit_index: usize,
    last_hit_object_index: Option<usize>,
    last_checked_cursor_index: usize,
    last_missed_hit_object: Option<usize>,
    last_maybe_missed_hit_object: Option<usize>,

    pause_on_miss: bool,
}

struct Miss {
    time: f64,
    hit_object_index: usize,
    cursor_position: (f64, f64),
}

impl Gui {
    pub fn new() -> Self {
        Self {
            osu_data: None,
            system_time: SystemTime::now(),
            replay_data: None,
            errors: Vec::new(),
        }
    }

    pub fn render(&mut self, context: &Context) {
        egui::CentralPanel::default().show(&context, |ui| {
            let delta_time = SystemTime::now()
                .duration_since(self.system_time)
                .unwrap()
                .as_secs_f64();
            self.system_time = SystemTime::now();

            ui.heading("Hello, egui!");
            ui.label("This is a simple egui window.");

            if ui
                .button("Load osu!")
                .on_hover_ui(|ui| {
                    ui.label("Hello!");
                })
                .clicked()
            {
                let osu_path = match env::consts::OS {
                    "windows" => {
                        let path = PathBuf::from("%localappdata%")
                            .join("AppData")
                            .join("Local");

                        Some(path)
                    }
                    "macos" => {
                        let path = PathBuf::from("~")
                            .join("Library")
                            .join("Application Support");

                        Some(path)
                    }
                    "linux" => {
                        let path = PathBuf::from(format!("/home/{}", env::var("USER").unwrap()))
                            .join(".local")
                            .join("share")
                            .join("osu-wine");

                        Some(path)
                    }
                    _ => None,
                }
                .map(|path| path.join("osu!").join("osu!.db"));

                println!("{:?}", osu_path);
                println!("{:?}", env::consts::OS);
                println!("{:?}", osu_path.clone().unwrap().exists());

                let dialog = rfd::FileDialog::new().set_file_name("osu!.db");

                let osu_db_path = if let Some(osu_path) = osu_path {
                    if osu_path.exists() {
                        Some(osu_path)
                    } else {
                        dialog.clone().pick_file()
                    }
                } else {
                    dialog.clone().pick_file()
                };

                if let Some(path) = osu_db_path {
                    self.errors.clear();
                    let path = path.display().to_string();
                    let beatmaps = osu_db::listing::Listing::from_file(&path);
                    match beatmaps {
                        Err(e) => self.errors.push(format!(
                            "Failed to load osu! database from {}.\n{}",
                            path, e
                        )),
                        Ok(beatmaps) => {
                            self.osu_data = Some(OsuData {
                                path: path.into(),
                                beatmaps,
                            })
                        }
                    }
                }
            };

            if let Some(OsuData { path, beatmaps }) = &self.osu_data {
                ui.label(format!("osu!.db path: {}", path.display()));

                if ui
                    .button("Load replay")
                    .on_hover_ui(|ui| {
                        ui.label("Hello!");
                    })
                    .clicked()
                {
                    if let Some(replay_path) = rfd::FileDialog::new()
                        .add_filter("osu! replay", &["osr"])
                        .pick_file()
                    {
                        self.errors.clear();
                        let replay = crate::models::osu_replay::OsuReplay::from_file(&replay_path);

                        if let Some(beatmap_listing) = beatmaps
                            .beatmaps
                            .iter()
                            .find(|b| b.hash == Some(replay.beatmap_hash.clone()))
                        {
                            let (_stream, handle) = OutputStream::try_default().unwrap();

                            let osu_beatmap_path = path
                                .parent()
                                .unwrap()
                                .join("Songs")
                                .join(beatmap_listing.folder_name.as_ref().unwrap());

                            let audio_path =
                                osu_beatmap_path.join(beatmap_listing.audio.as_ref().unwrap());

                            let song_source = {
                                let file = BufReader::new(File::open(audio_path).unwrap());
                                Decoder::new(file).unwrap().buffered()
                            };
                            let sink = Sink::try_new(&handle).unwrap();
                            sink.pause();

                            let hit_sound_source = {
                                let file = BufReader::new(File::open("hit.wav").unwrap());
                                Decoder::new(file).unwrap().buffered()
                            };

                            let osu_file_path =
                                osu_beatmap_path.join(beatmap_listing.file_name.as_ref().unwrap());

                            let beatmap = crate::models::osu_map::OsuMap::from_file(&osu_file_path);

                            // How to calculate offset?
                            let offset = -1.75 + beatmap.hit_objects[0].time as f64 / 1000.0;

                            let status = PlaybackStatus {
                                playback_speed: 1.0,
                                playing: false,
                                play_time: 0.0,
                                replay_data_index: 0,
                                audio_song_source: song_source,
                                hit_sound_source,
                                audio_stream_handle: handle,
                                audio_song_sink: sink,
                                misses: Vec::new(),
                                hit_object_index: 0,
                                next_hit_object_to_hit_index: 0,
                                last_hit_object_index: None,
                                last_missed_hit_object: None,
                                last_maybe_missed_hit_object: None,
                                last_checked_cursor_index: 0,
                                pause_on_miss: false,
                                volume: 1.0,
                            };

                            self.replay_data = Some(ReplayPlaybackData {
                                replay,
                                beatmap,
                                replay_path: replay_path.display().to_string(),
                                audio_output: _stream,
                                offset,
                                playback_status: status,
                            });
                        } else {
                            self.errors.push(format!(
                                "Failed to find beatmap with hash {}.",
                                replay.beatmap_hash
                            ));
                        }
                    }
                };

                if self.errors.len() > 0 {
                    ui.label("Errors");
                    for error in &mut self.errors {
                        ui.add_enabled(false, egui::TextEdit::multiline(error));
                    }
                }

                if let Some(playback) = &mut self.replay_data {
                    let ReplayPlaybackData {
                        ref replay_path,
                        ref replay,
                        offset,
                        playback_status: ref mut status,
                        ..
                    } = *playback;

                    ui.label(format!("Picked path: {}", replay_path));

                    let audio_offset = if offset < 0.0 { 0.0 } else { offset };
                    let replay_offset = if offset > 0.0 { 0.0 } else { -offset };

                    ui.label(format!("Beatmap: {}", replay.beatmap_hash));
                    if ui.button("Play/Pause").clicked() {
                        status.playing = !status.playing;
                        if status.playing {
                            status.play(audio_offset);
                        } else {
                            status.pause();
                        }
                    };

                    ui.label(format!("Playing: {}", status.playing));
                    ui.label(format!("Play Time:\t{}", status.play_time));
                    ui.label(format!(
                        "Audio Time:\t{}",
                        status.audio_song_sink.get_pos().as_secs_f64()
                    ));

                    if ui.add(egui::Slider::new(&mut status.volume, 0.0..=1.0).text("Volume"))
                        .changed()
                    {
                        status.audio_song_sink.set_volume(status.volume as f32);
                    }

                    ui.add(egui::Slider::new(&mut playback.offset, -5.0..=5.0).text("Offset"));
                    ui.add(
                        egui::Slider::new(&mut status.playback_speed, 0.1..=2.0)
                            .text("Playback speed")
                            .step_by(0.05),
                    );
                    ui.checkbox(&mut status.pause_on_miss, "Pause on miss");
                    ui.label(format!("Offset: {}", offset));
                    ui.label(format!("Audio offset: {}", audio_offset));
                    ui.label(format!("Replay offset: {}", replay_offset));

                    ui.label("Replay data:");
                    ui.label(format!("Player: {}", playback.replay.player_name));
                    ui.label(format!("Score: {}", playback.replay.score));
                    ui.label(format!("Max combo: {}", playback.replay.max_combo));
                    ui.label(format!("Misses: {}", playback.replay.count_miss));

                    // add slider with full screen width

                    if status.playing {
                        status.play_time += delta_time * status.playback_speed;

                        if status.play_time + replay_offset
                            > playback.replay.replay_data[playback.replay.replay_data.len() - 1]
                                .total_time as f64
                                / 1000.0
                        {
                            status.pause();
                        }

                        while status.playing
                            && status.play_time + replay_offset
                                > playback.replay.replay_data[status.replay_data_index].total_time
                                    as f64
                                    / 1000.0
                        {
                            status.replay_data_index += 1;
                        }

                        while status.playing {
                            if let Some(object) =
                                playback.beatmap.hit_objects.get(status.hit_object_index)
                            {
                                if object.time as f64 / 1000.0 > status.play_time {
                                    break;
                                }
                                status.hit_object_index += 1;
                            } else {
                                break;
                            }
                        }
                    }

                    let ApproachRate {
                        preempt, fade_in, ..
                    } = playback.beatmap.difficulty.approach_rate;

                    ui.spacing_mut().slider_width = ui.available_width() - 100.0;
                    if ui
                        .add_sized(
                            [ui.available_width(), 20.0],
                            egui::Slider::new(
                                &mut status.replay_data_index,
                                0..=(playback.replay.replay_data.len() - 1),
                            ),
                        )
                        .changed()
                    {
                        status.play_time = playback.replay.replay_data[status.replay_data_index]
                            .total_time as f64
                            / 1000.0;
                        let source = status.audio_song_source.clone().skip_duration(
                            std::time::Duration::from_secs_f64(status.play_time + audio_offset),
                        );

                        status.last_checked_cursor_index = 0;
                        status.last_hit_object_index = None;

                        status.next_hit_object_to_hit_index = 0;
                        while let Some(object) = playback
                            .beatmap
                            .hit_objects
                            .get(status.next_hit_object_to_hit_index)
                        {
                            if object.time as f64 / 1000.0 >= status.play_time - preempt {
                                break;
                            }
                            status.next_hit_object_to_hit_index += 1;
                        }
                        status.hit_object_index = status.next_hit_object_to_hit_index;
                        status.misses.clear();

                        let audio_song_sink = &status.audio_song_sink;
                        audio_song_sink.clear();
                        audio_song_sink.append(source);
                        if status.playing {
                            audio_song_sink.play();
                        }
                    }

                    let offset = egui::Vec2::new(50.0, ui.cursor().min.y + 50.0);
                    let scale = (ui.available_height() - 100.0) / 384.0;

                    ui.painter().rect(
                        egui::Rect::from_min_size(
                            egui::Pos2::new(offset.x, offset.y),
                            egui::Vec2::new(512.0, 384.0) * scale,
                        ),
                        0.0,
                        egui::Color32::from_black_alpha(255),
                        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(255)),
                    );

                    let OverallDifficulty {
                        hit_window_100,
                        hit_window_50,
                        hit_window_300,
                        ..
                    } = playback.beatmap.difficulty.overall_difficulty;

                    ui.label(format!(
                        "Hit windows: 300: {} 100: {} 50: {}",
                        hit_window_300, hit_window_100, hit_window_50
                    ));

                    let next_hit_object_to_hit = playback
                        .beatmap
                        .hit_objects
                        .get(status.next_hit_object_to_hit_index);
                    let relative_hit_error = if let Some(object) = next_hit_object_to_hit {
                        match object.hit_type {
                            HitType::Spinner(Spinner {
                                end_time
                            }) => {
                                status.play_time - end_time as f64 / 1000.0
                            },
                            _ => status.play_time - object.time as f64 / 1000.0
                        }
                    } else {
                        0.0
                    };

                    if let Some(next_hit_object_to_hit) = next_hit_object_to_hit {
                        match next_hit_object_to_hit.hit_type {
                            HitType::Spinner(_) if relative_hit_error > 0.0 => {
                                status.successful_hit(
                                    status.next_hit_object_to_hit_index,
                                    next_hit_object_to_hit,
                                );
                            }
                            _ => (),
                        }
                    }

                    if relative_hit_error > hit_window_50 {
                        match status.last_missed_hit_object {
                            Some(last_index) => {
                                if last_index < status.next_hit_object_to_hit_index {
                                    match status.last_maybe_missed_hit_object {
                                        Some(index) => {
                                            if index != status.next_hit_object_to_hit_index {
                                                status.miss_timeout(&playback.replay.replay_data[status.replay_data_index]);
                                                status.next_hit_object_to_hit_index += 1;
                                            } else {
                                                status.last_maybe_missed_hit_object = None;
                                                status.next_hit_object_to_hit_index += 1;
                                            }
                                        },
                                        None => {
                                            status.miss_timeout(&playback.replay.replay_data[status.replay_data_index]);
                                            status.next_hit_object_to_hit_index += 1;
                                        }
                                    }
                                }
                            }
                            None => {
                                status.miss_timeout(&playback.replay.replay_data[status.replay_data_index]);
                                status.next_hit_object_to_hit_index += 1;
                            }
                        }
                    }

                    {
                        let first = status.next_hit_object_to_hit_index;
                        let last = {
                            let mut last = first;
                            while let Some(hit_object) = playback.beatmap.hit_objects.get(last) {
                                if hit_object.time as f64 / 1000.0 > status.play_time + preempt {
                                    break;
                                }
                                last += 1;
                            }
                            last
                        };

                        ui.label(format!(
                            "First: {} Last: {} Preempt: {} FadeIn: {}",
                            first, last, preempt, fade_in
                        ));
                        ui.label(format!(
                            "Current: {} Time: {} Type {}",
                            status.hit_object_index,
                            playback
                                .beatmap
                                .hit_objects
                                .get(status.hit_object_index)
                                .map(|o| o.time as f64 / 1000.0)
                                .unwrap_or(-1.0),
                            next_hit_object_to_hit
                                .map(|o| o.hit_type.to_string())
                                .unwrap_or("Unknown".to_string())
                        ));

                        ui.label(format!(
                            "Current: {} Last Hit: {} Next Hit: {} Last Miss: {}",
                            status.hit_object_index,
                            status.last_hit_object_index.unwrap_or(0),
                            status.next_hit_object_to_hit_index,
                            status.last_missed_hit_object.unwrap_or(0)
                        ));
                        ui.label(format!("Misses: {}", status.misses.len()));

                        for i in (first..=last).rev() {
                            if let Some(hit_object) = playback.beatmap.hit_objects.get(i) {
                                hit_object.render(
                                    ui,
                                    &playback.beatmap,
                                    status.play_time,
                                    scale,
                                    offset,
                                );
                                ui.painter().text(
                                    egui::Pos2::new(
                                        hit_object.x as f32 * scale + offset.x,
                                        hit_object.y as f32 * scale + offset.y,
                                    ),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}", i),
                                    egui::FontId::default(),
                                    egui::Color32::from_white_alpha(255),
                                );
                            }
                        }
                    }

                    {
                        fn is_key_down(keys: i32, key: Keys) -> bool {
                            keys & key as i32 != 0
                        }

                        if let Some(cursor) = playback.replay.replay_data.get(status.replay_data_index) {
                            let last_keys = if status.replay_data_index > 0 {
                                playback
                                    .replay
                                    .replay_data
                                    .get(status.replay_data_index - 1)
                                    .map(|data| data.keys)
                                    .unwrap_or(0)
                            } else {
                                0
                            };

                            let is_k1_pressed = is_key_down(cursor.keys, Keys::K1)
                                && !is_key_down(last_keys, Keys::K1);
                            let is_k2_pressed = is_key_down(cursor.keys, Keys::K2)
                                && !is_key_down(last_keys, Keys::K2);

                            if (is_k1_pressed || is_k2_pressed)
                                && status.replay_data_index != status.last_checked_cursor_index
                            {
                                let mut hit_object_index = status.next_hit_object_to_hit_index;
                                let mut maybe_misses = Vec::new();

                                while let Some(object) =
                                    playback.beatmap.hit_objects.get(hit_object_index)
                                {
                                    let distance_to_object = ((cursor.x as f64 - object.x as f64)
                                        .powi(2)
                                        + (cursor.y as f64 - object.y as f64).powi(2))
                                    .sqrt();
                                    let size =
                                        54.4 - 4.48 * playback.beatmap.difficulty.circle_size;
                                    let time_diff = status.play_time - object.time as f64 / 1000.0;

                                    match object.hit_type {
                                        HitType::Circle | HitType::Slider(_) => {
                                            if distance_to_object > size {
                                                if time_diff.abs() < hit_window_50 {
                                                    if status.pause_on_miss {
                                                        status.pause();
                                                    }
                                                    maybe_misses.push(status.miss_aim(cursor));
                                                    status.last_checked_cursor_index = status.replay_data_index;
                                                } else {
                                                    if time_diff > 0.0 {
                                                        continue;
                                                    } else {
                                                        status.misses.append(&mut maybe_misses);
                                                        break;
                                                    }
                                                }
                                            } else {
                                                // too early but not way to early
                                                if time_diff < 0.0
                                                    && time_diff.abs() > hit_window_50
                                                    && time_diff.abs() < hit_window_50 * 2.0
                                                {
                                                    status.miss_timing(cursor);
                                                    status.last_checked_cursor_index = status.replay_data_index;
                                                    break;
                                                } else {
                                                    status.successful_hit(hit_object_index, object);
                                                    status.last_checked_cursor_index = status.replay_data_index;
                                                    break;
                                                }
                                            }
                                        }
                                        HitType::Spinner(_) => {}
                                    }
                                    hit_object_index += 1;
                                }
                            }
                        }

                        let first = if status.replay_data_index > 20 {
                            (status.replay_data_index - 20) as usize
                        } else {
                            0
                        };
                        let last = status.replay_data_index as usize;
                        for cursor_index in first..=last {
                            // draw arrow from last to current replay data
                            if cursor_index > 0 {
                                let last_data = &playback.replay.replay_data[cursor_index - 1];
                                let current_data = &playback.replay.replay_data[cursor_index];

                                let color = egui::Color32::from_white_alpha(
                                    ((cursor_index - first) as f32 / (last - first) as f32 * 255.0)
                                        as u8,
                                );

                                ui.painter().line_segment(
                                    [
                                        egui::Pos2::new(last_data.x as f32, last_data.y as f32)
                                            * scale
                                            + offset,
                                        egui::Pos2::new(
                                            current_data.x as f32,
                                            current_data.y as f32,
                                        ) * scale
                                            + offset,
                                    ],
                                    egui::Stroke::new(1.0, color),
                                );

                                let is_k1_pressed = is_key_down(current_data.keys, Keys::K1)
                                    && !is_key_down(last_data.keys, Keys::K1);
                                let is_k2_pressed = is_key_down(current_data.keys, Keys::K2)
                                    && !is_key_down(last_data.keys, Keys::K2);

                                if is_k1_pressed || is_k2_pressed {
                                    ui.painter().circle_filled(
                                        egui::Pos2::new(
                                            current_data.x as f32,
                                            current_data.y as f32,
                                        ) * scale
                                            + offset,
                                        5.0,
                                        egui::Color32::from_rgba_premultiplied(255, 0, 255, 255),
                                    );
                                }
                            }
                        }
                    }

                    {
                        for miss in &status.misses {
                            let time_diff = status.play_time - miss.time;
                            if time_diff.abs() > 3.0 {
                                continue;
                            }
                            let size = 54.4 - 4.48 * playback.beatmap.difficulty.circle_size;
                            if let Some(missed_object) =
                                playback.beatmap.hit_objects.get(miss.hit_object_index)
                            {
                                ui.painter().circle_stroke(
                                    egui::Pos2::new(missed_object.x as f32, missed_object.y as f32)
                                        * scale
                                        + offset,
                                    size as f32 * scale,
                                    egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgba_premultiplied(255, 0, 0, 255),
                                    ),
                                );

                                // draw line from circle to cursor
                                ui.painter().line_segment(
                                    [
                                        egui::Pos2::new(
                                            missed_object.x as f32,
                                            missed_object.y as f32,
                                        ) * scale
                                            + offset,
                                        egui::Pos2::new(
                                            miss.cursor_position.0 as f32,
                                            miss.cursor_position.1 as f32,
                                        ) * scale
                                            + offset,
                                    ],
                                    egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgba_premultiplied(255, 0, 0, 255),
                                    ),
                                );
                            }

                            ui.painter().circle_filled(
                                egui::Pos2::new(
                                    miss.cursor_position.0 as f32,
                                    miss.cursor_position.1 as f32,
                                ) * scale
                                    + offset,
                                5.0,
                                egui::Color32::from_rgba_premultiplied(255, 0, 0, 255),
                            );
                        }
                    }
                }
            }
        });
    }

    pub(crate) fn handle_event(
        &self,
        egui_winit_state: &mut egui_winit::State,
        window: &Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        let response = egui_winit_state.on_window_event(window, event);
        if response.repaint {
            window.request_redraw();
        }
        response.consumed
    }
}

impl PlaybackStatus {
    fn successful_hit(&mut self, index: usize, _hit_object: &HitObject) {
        self.audio_stream_handle
            .play_raw(self.hit_sound_source.clone().amplify(self.volume as f32).convert_samples())
            .unwrap();

        self.next_hit_object_to_hit_index = index + 1;
        self.last_hit_object_index = Some(index);
    }

    fn play(&mut self, audio_offset: f64) {
        let source =
            self.audio_song_source
                .clone()
                .skip_duration(std::time::Duration::from_secs_f64(
                    self.play_time + audio_offset,
                ));
        self.playing = true;

        self.audio_song_sink.set_speed(self.playback_speed as f32);
        self.audio_song_sink.append(source);
        self.audio_song_sink.play();
    }

    fn pause(&mut self) {
        self.playing = false;
        self.audio_song_sink.pause();
        self.audio_song_sink.clear();
    }

    fn miss(&mut self, cursor: &crate::models::osu_replay::ReplayData) {
        if self.pause_on_miss {
            self.pause();
        }
        let time_diff = self.play_time - cursor.total_time as f64 / 1000.0;
        println!("timing: {}", time_diff);

        self.misses.push(Miss {
            time: self.play_time,
            hit_object_index: self.next_hit_object_to_hit_index,
            cursor_position: (cursor.x as f64, cursor.y as f64),
        });
    }

    fn miss_timing(&mut self, cursor: &crate::models::osu_replay::ReplayData) {
        println!("missed on timing");
        self.last_missed_hit_object = Some(self.next_hit_object_to_hit_index);
        self.miss(cursor);
    }

    fn miss_aim(&mut self, cursor: &crate::models::osu_replay::ReplayData) -> Miss {
        println!("missed on aim");
        self.last_maybe_missed_hit_object = Some(self.next_hit_object_to_hit_index);
        Miss {
            time: self.play_time,
            hit_object_index: self.next_hit_object_to_hit_index,
            cursor_position: (cursor.x as f64, cursor.y as f64),
        }
    }

    fn miss_timeout(&mut self, cursor: &crate::models::osu_replay::ReplayData) {
        println!("missed on timeout");
        self.last_missed_hit_object = Some(self.next_hit_object_to_hit_index);
        self.miss(cursor);
    }
}
