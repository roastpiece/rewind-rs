use std::{fs::File, io::BufReader, path::PathBuf, time::SystemTime};

use egui::Context;
use rodio::{source::Buffered, Decoder, OutputStream, Sink, Source};
use winit::window::Window;

use crate::{graphics::object::Renderable, models::{osu_map::{ApproachRate, HitType, OverallDifficulty}, osu_replay::Keys}};

pub struct Gui {
    osu_data: Option<OsuData>,
    slider: u64,
    system_time: SystemTime,
    replay_data: Option<LoadedReplayData>,
    errors: Vec<String>,
}

struct OsuData {
    path: PathBuf,
    beatmaps: osu_db::listing::Listing,
}

struct LoadedReplayData {
    replay: crate::models::osu_replay::OsuReplay,
    beatmap: crate::models::osu_map::OsuMap,
    replay_path: String,
    #[allow(dead_code)] // need to store ref
    audio_output: OutputStream,
    audio_stream_handle: rodio::OutputStreamHandle,
    audio_song_sink: Sink,
    audio_song_source: Buffered<Decoder<BufReader<File>>>,
    hit_sound_source: Buffered<Decoder<BufReader<File>>>,

    offset: f64,
    playback_speed: f64,
    playing: bool,
    play_time: f64,

    misses: Vec<Miss>,

    hit_object_index: usize,
    next_hit_object_to_hit_index: usize,
    last_hit_object_index: Option<usize>,
    last_checked_cursor_index: usize,

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
            slider: 0,
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
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("osu! database", &["db"])
                    .pick_file()
                {
                    self.errors.clear();
                    let path = path.display().to_string();
                    let beatmaps = osu_db::listing::Listing::from_file(&path);
                    match beatmaps {
                        Err(e) => self.errors.push(format!("Failed to load osu! database from {}.\n{}", path, e)),
                        Ok(beatmaps) => self.osu_data = Some(OsuData { path: path.into(), beatmaps }),
                    }
                }
            };

            if let Some(OsuData {
                path,
                beatmaps,
            }) = &self.osu_data {
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
                        let replay =
                            crate::models::osu_replay::OsuReplay::from_file(&replay_path);

                        if let Some(beatmap_listing) = beatmaps.beatmaps.iter().find(|b| b.hash == Some(replay.beatmap_hash.clone())) {
                            let (_stream, handle) = OutputStream::try_default().unwrap();

                            let osu_beatmap_path = path
                                .parent().unwrap()
                                .join("Songs")
                                .join(beatmap_listing.folder_name.as_ref().unwrap());

                            let audio_path = osu_beatmap_path.join(beatmap_listing.audio.as_ref().unwrap());

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


                            let osu_file_path = osu_beatmap_path.join(beatmap_listing.file_name.as_ref().unwrap());


                            let beatmap = crate::models::osu_map::OsuMap::from_file(&osu_file_path);

                            // How to calculate offset?
                            let offset = -1.75 + beatmap.hit_objects[0].time as f64 / 1000.0;

                            self.replay_data = Some(LoadedReplayData {
                                replay,
                                beatmap,
                                replay_path: replay_path.display().to_string(),
                                audio_output: _stream,
                                audio_stream_handle: handle,
                                audio_song_sink: sink,
                                audio_song_source: song_source,
                                hit_sound_source,
                                offset,
                                playback_speed: 1.0,
                                playing: false,
                                play_time: 0.0,
                                hit_object_index: 0,
                                next_hit_object_to_hit_index: 0,
                                misses: Vec::new(),
                                pause_on_miss: false,
                                last_hit_object_index: None,
                                last_checked_cursor_index: 0,
                            });
                        } else {
                            self.errors.push(format!("Failed to find beatmap with hash {}.", replay.beatmap_hash));
                        }
                    }
                };

                if self.errors.len() > 0 {
                    ui.label("Errors");
                    for error in &mut self.errors {
                        ui.add_enabled(false, egui::TextEdit::multiline(error));
                    }
                }

                if let Some(LoadedReplayData {
                    replay,
                    beatmap,
                    replay_path,
                    audio_stream_handle,
                    audio_song_sink,
                    audio_song_source,
                    hit_sound_source,
                    ref mut offset,
                    ref mut playback_speed,
                    ref mut playing,
                    ref mut play_time,
                    ref mut hit_object_index,
                    ref mut next_hit_object_to_hit_index,
                    ref mut misses,
                    ref mut pause_on_miss,
                    ref mut last_hit_object_index,
                    ref mut last_checked_cursor_index,
                    ..
                }) = &mut self.replay_data
                {
                    ui.label(format!("Picked path: {}", replay_path));

                    let audio_offset = if *offset < 0.0 { 0.0 } else { *offset };

                    let replay_offset = if *offset > 0.0 { 0.0 } else { -*offset };

                    ui.label(format!("Beatmap: {}", replay.beatmap_hash));
                    if ui.button("Play/Pause").clicked() {
                        *playing = !*playing;
                        if *playing {
                            let source = audio_song_source.clone().skip_duration(
                                std::time::Duration::from_secs_f64(*play_time + audio_offset),
                            );

                            audio_song_sink.set_speed(*playback_speed as f32);
                            audio_song_sink.append(source);
                            audio_song_sink.play();
                        } else {
                            audio_song_sink.pause();
                            audio_song_sink.clear();
                        }
                    };

                    ui.label(format!("Playing: {}", playing));
                    ui.label(format!("Play Time:\t{}", play_time));
                    ui.label(format!(
                        "Audio Time:\t{}",
                        audio_song_sink.get_pos().as_secs_f64()
                    ));

                    ui.add(egui::Slider::new(offset, -5.0..=5.0).text("Offset"));
                    ui.add(
                        egui::Slider::new(playback_speed, 0.1..=2.0)
                            .text("Playback speed")
                            .step_by(0.05),
                    );
                    ui.checkbox(pause_on_miss, "Pause on miss");
                    ui.label(format!("Offset: {}", offset));
                    ui.label(format!("Audio offset: {}", audio_offset));
                    ui.label(format!("Replay offset: {}", replay_offset));

                    ui.label("Replay data:");
                    ui.label(format!("Player: {}", replay.player_name));
                    ui.label(format!("Score: {}", replay.score));
                    ui.label(format!("Max combo: {}", replay.max_combo));
                    ui.label(format!("Misses: {}", replay.count_miss));

                    if ui.button("play hit sound").clicked() {
                        audio_stream_handle
                            .play_raw(hit_sound_source.clone().convert_samples())
                            .unwrap();
                    }

                    // add slider with full screen width

                    if *playing {
                        *play_time += delta_time * *playback_speed;

                        if *play_time + replay_offset
                            > replay.replay_data[replay.replay_data.len() - 1].total_time as f64
                                / 1000.0
                        {
                            *playing = false;
                            audio_song_sink.pause();
                        }

                        while *playing
                            && *play_time + replay_offset
                                > replay.replay_data[self.slider as usize].total_time as f64
                                    / 1000.0
                        {
                            self.slider += 1;
                        }

                        while *playing {
                            if let Some(object) = beatmap.hit_objects.get(*hit_object_index) {
                                if object.time as f64 / 1000.0 > *play_time {
                                    break;
                                }
                                *hit_object_index += 1;
                            } else {
                                break;
                            }
                        }
                    }

                    let ApproachRate {preempt, fade_in, ..} = beatmap.difficulty.approach_rate;

                    ui.spacing_mut().slider_width = ui.available_width() - 100.0;
                    if ui
                        .add_sized(
                            [ui.available_width(), 20.0],
                            egui::Slider::new(
                                &mut self.slider,
                                0..=(replay.replay_data.len() - 1) as u64,
                            ),
                        )
                        .changed()
                    {
                        *play_time =
                            replay.replay_data[self.slider as usize].total_time as f64 / 1000.0;
                        let source = audio_song_source.clone().skip_duration(
                            std::time::Duration::from_secs_f64(*play_time + audio_offset),
                        );

                        *last_checked_cursor_index = 0;
                        *last_hit_object_index = None;

                        *next_hit_object_to_hit_index = 0;
                        while let Some(object) = beatmap.hit_objects.get(*next_hit_object_to_hit_index) {
                            if object.time as f64 / 1000.0 >= *play_time - preempt {
                                break;
                            }
                            *next_hit_object_to_hit_index += 1;
                        }
                        *hit_object_index = *next_hit_object_to_hit_index;

                        audio_song_sink.clear();
                        audio_song_sink.append(source);
                        if *playing {
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
                        hit_window_50,
                        ..
                    } = beatmap.difficulty.overall_difficulty;

                    let next_hit_object_to_hit = &beatmap.hit_objects.get(*next_hit_object_to_hit_index);
                    let relative_hit_error = if let Some(object) = next_hit_object_to_hit {
                        *play_time - object.time as f64 / 1000.0
                    } else {
                        0.0
                    };

                    {
                        
                        let first = if relative_hit_error > hit_window_50 {
                            if let Some(last_index) = *last_hit_object_index {
                                if last_index != *next_hit_object_to_hit_index {
                                    misses.push(Miss {
                                        time: *play_time,
                                        hit_object_index: *next_hit_object_to_hit_index,
                                        cursor_position: (
                                            replay.replay_data[self.slider as usize].x as f64,
                                            replay.replay_data[self.slider as usize].y as f64,
                                        ),
                                    });
                                }
                            }
                            *next_hit_object_to_hit_index
                        } else {
                            *next_hit_object_to_hit_index
                        };
                        let last = {
                            let mut last = first;
                            while let Some(hit_object) = beatmap.hit_objects.get(last) {
                                if hit_object.time as f64 / 1000.0 > *play_time + preempt {
                                    break;
                                }
                                last += 1;
                            }
                            last
                        };

                        ui.label(format!("First: {} Last: {} Preempt: {} FadeIn: {}", first, last, preempt, fade_in));
                        ui.label(format!("Current: {} Time: {}", *hit_object_index, beatmap.hit_objects.get(*hit_object_index).map(|o| o.time as f64 / 1000.0).unwrap_or(-1.0)));
                        ui.label(format!("Current: {} Last Hit: {} Next Hit: {}", *hit_object_index, last_hit_object_index.unwrap_or(0), *next_hit_object_to_hit_index));
                        ui.label(format!("Misses: {}", misses.len()));

                        for i in (first..=last).rev() {
                            if let Some(hit_object) = &beatmap.hit_objects.get(i) {
                                hit_object.render(ui, beatmap, *play_time, scale, offset);
                            }
                        }
                    }

                    {
                        let first = if self.slider > 20 {
                            (self.slider - 20) as usize
                        } else {
                            0
                        };
                        let last = self.slider as usize;
                        for cursor_index in first..=last {
                            // draw arrow from last to current replay data
                            if cursor_index > 0 {
                                let last_data = &replay.replay_data[cursor_index - 1];
                                let current_data = &replay.replay_data[cursor_index];

                                let color = egui::Color32::from_white_alpha(
                                    ((cursor_index - first) as f32 / (last - first) as f32 * 255.0) as u8,
                                );

                                ui.painter().line_segment(
                                    [
                                        egui::Pos2::new(last_data.x as f32, last_data.y as f32) * scale
                                            + offset,
                                        egui::Pos2::new(current_data.x as f32, current_data.y as f32)
                                            * scale
                                            + offset,
                                    ],
                                    egui::Stroke::new(1.0, color),
                                );

                                fn is_key_down(keys: i32, key: Keys) -> bool {
                                    keys & key as i32 != 0
                                }

                                let is_k1_pressed = is_key_down(current_data.keys, Keys::K1)
                                    && !is_key_down(last_data.keys, Keys::K1);
                                let is_k2_pressed = is_key_down(current_data.keys, Keys::K2)
                                    && !is_key_down(last_data.keys, Keys::K2);

                                if is_k1_pressed || is_k2_pressed {
                                    ui.painter().circle_filled(
                                        egui::Pos2::new(current_data.x as f32, current_data.y as f32)
                                            * scale
                                            + offset,
                                        5.0,
                                        egui::Color32::from_rgba_premultiplied(255, 0, 255, 255),
                                    );

                                    if cursor_index == self.slider as usize {
                                        if cursor_index != *last_checked_cursor_index {
                                            let mut hit_object_index = *next_hit_object_to_hit_index;
                                            while let Some(object) = beatmap.hit_objects.get(hit_object_index) {
                                                let distance_to_object = ((current_data.x as f64 - object.x as f64).powi(2) + (current_data.y as f64 - object.y as f64).powi(2)).sqrt();
                                                let size = 54.4 - 4.48 * beatmap.difficulty.circle_size;
                                                let time_diff = *play_time - object.time as f64 / 1000.0;

                                                // todo add grace period when hitting way too early
                                                if time_diff.abs() > hit_window_50 {
                                                    break;
                                                }

                                                match object.hit_type {
                                                    HitType::Circle | HitType::Slider(_) =>
                                                        if distance_to_object > size {
                                                            if time_diff.abs() < hit_window_50 {
                                                                if *pause_on_miss {
                                                                    *playing = false;
                                                                    audio_song_sink.pause();
                                                                }
                                                                misses.push(Miss {
                                                                    time: *play_time,
                                                                    hit_object_index: *next_hit_object_to_hit_index,
                                                                    cursor_position: (
                                                                        current_data.x as f64,
                                                                        current_data.y as f64,
                                                                    ),
                                                                });
                                                            }
                                                        } else {
                                                            audio_stream_handle
                                                                .play_raw(hit_sound_source.clone().convert_samples())
                                                                .unwrap();
                                                            *last_hit_object_index = Some(hit_object_index);
                                                            *last_checked_cursor_index = cursor_index;
                                                            *next_hit_object_to_hit_index = hit_object_index + 1;
                                                            break;
                                                        }
                                                    HitType::Spinner(_) => {}
                                                }
                                                hit_object_index += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    {
                        for miss in misses {
                            let time_diff = *play_time - miss.time;
                            if time_diff.abs() > 3.0 {
                                continue;
                            }
                            let size = 54.4 - 4.48 * beatmap.difficulty.circle_size;
                            if let Some(missed_object) = &beatmap.hit_objects.get(miss.hit_object_index) {
                                ui.painter().circle_stroke(
                                    egui::Pos2::new(missed_object.x as f32, missed_object.y as f32)
                                        * scale
                                        + offset,
                                    size as f32 * scale,
                                    egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 0, 0, 255)),
                                );

                                // draw line from circle to cursor
                                ui.painter().line_segment(
                                    [
                                        egui::Pos2::new(missed_object.x as f32, missed_object.y as f32)
                                            * scale
                                            + offset,
                                        egui::Pos2::new(miss.cursor_position.0 as f32, miss.cursor_position.1 as f32)
                                            * scale
                                            + offset,
                                    ],
                                    egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 0, 0, 255)),
                                );
                            }

                            ui.painter().circle_filled(
                                egui::Pos2::new(miss.cursor_position.0 as f32, miss.cursor_position.1 as f32)
                                    * scale
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
