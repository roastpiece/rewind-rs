use std::{fs::File, io::BufReader, time::SystemTime};

use egui::Context;
use rodio::{source::Buffered, Decoder, OutputStream, Sink, Source};
use winit::window::Window;

use crate::models::osu_replay::osu_replay::Keys;

pub struct Gui {
    replay_path: Option<String>,
    osu_path: Option<String>,
    replay: Option<crate::models::osu_replay::osu_replay::OsuReplay>,
    slider: u64,
    playing: bool,
    play_time: f64,
    system_time: SystemTime,

    audio_output: Option<OutputStream>,
    audio_stream_handle: Option<rodio::OutputStreamHandle>,
    audio_song_sink: Option<Sink>,
    hit_sound_source: Option<Buffered<Decoder<BufReader<File>>>>,
    last_hit_sound_played_index: usize,
    offset: f64,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            replay_path: None,
            replay: None,
            osu_path: None,
            slider: 0,
            playing: false,
            system_time: SystemTime::now(),
            play_time: 0.0,
            audio_output: None,
            audio_stream_handle: None,
            audio_song_sink: None,
            hit_sound_source: None,
            last_hit_sound_played_index: 0,
            offset: 0.0,
        }
    }

    pub fn render(&mut self, context: &Context) {
        egui::CentralPanel::default().show(&context, |ui| {
            let delta_time = SystemTime::now().duration_since(self.system_time).unwrap().as_secs_f64();
            self.system_time = SystemTime::now();
            
            ui.heading("Hello, egui!");
            ui.label("This is a simple egui window.");

            if ui.button("Load osu!")
                .on_hover_ui(|ui| {
                    ui.label("Hello!");
                })
                .clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.osu_path = Some(path.display().to_string());
                    }
                };

            if let Some(osu_path) = &self.osu_path {
                ui.label(format!("Osu! path: {}", osu_path));

                if ui.button("Load replay")
                    .on_hover_ui(|ui| {
                        ui.label("Hello!");
                    })
                    .clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.replay_path = Some(path.display().to_string());
                            let replay = crate::models::osu_replay::osu_replay::OsuReplay::from_file(&path);
                            self.replay = Some(replay);
                            
                            let (_stream, handle) = OutputStream::try_default().unwrap();
                            self.audio_output = Some(_stream);
                            self.audio_stream_handle = Some(handle);
                            let song_sink = {
                                let file = BufReader::new(File::open("audio.mp3").unwrap());
                                let source = Decoder::new(file).unwrap();
                                let sink = Sink::try_new(&self.audio_stream_handle.as_ref().unwrap()).unwrap();
                                sink.pause();
                                sink.append(source);
                                sink
                            };
                            self.audio_song_sink = Some(song_sink);

                            let hit_sound_source = {
                                let file = BufReader::new(File::open("hit.wav").unwrap());
                                Decoder::new(file).unwrap().buffered()
                            };
                            self.hit_sound_source = Some(hit_sound_source);

                            // How to calculate offset?
                            self.offset = -0.80;
                        }
                    };

                if let Some(path) = &self.replay_path {
                    ui.label(format!("Picked path: {}", path));
                }

                if let Some(replay) = &self.replay {
                    let audio_offset = if self.offset < 0.0 {
                        0.0
                    } else {
                        self.offset
                    };

                    let replay_offset = if self.offset > 0.0 {
                        0.0
                    } else {
                        -self.offset
                    };

                    ui.label(format!("Beatmap: {}", replay.beatmap_hash));
                    if ui.button("Play/Pause")
                        .clicked() {
                            self.playing = !self.playing;

                            if self.playing {
                                self.audio_song_sink.as_ref().unwrap().try_seek(std::time::Duration::from_secs_f64(self.play_time + audio_offset)).unwrap();
                                self.audio_song_sink.as_ref().unwrap().play();
                            } else {
                                self.audio_song_sink.as_ref().unwrap().pause();
                            }
                    };

                    ui.label(format!("Playing: {}", self.playing));
                    ui.label(format!("Time: {}", self.play_time));

                    ui.add(egui::Slider::new(&mut self.offset, -5.0..=5.0).text("Offset"));
                    ui.label(format!("Offset: {}", self.offset));
                    ui.label(format!("Audio offset: {}", audio_offset));
                    ui.label(format!("Replay offset: {}", replay_offset));

                    ui.label("Replay data:");
                    ui.label(format!("Player: {}", replay.player_name));
                    ui.label(format!("Score: {}", replay.score));
                    ui.label(format!("Max combo: {}", replay.max_combo));
                    ui.label(format!("Misses: {}", replay.count_miss));

                    if ui.button("play hit sound").clicked() {
                        if let Some(hit_sound_source) = &self.hit_sound_source {
                            self.audio_stream_handle.as_ref().unwrap().play_raw(hit_sound_source.clone().convert_samples()).unwrap();
                        }
                    }

                    // add slider with full screen width

                    if self.playing {
                        self.play_time += delta_time;

                        if self.play_time + replay_offset > replay.replay_data[replay.replay_data.len() - 1].total_time as f64 / 1000.0 {
                            self.playing = false;
                            self.audio_song_sink.as_ref().unwrap().pause();
                        }

                        while self.playing && self.play_time + replay_offset > replay.replay_data[self.slider as usize].total_time as f64 / 1000.0 {
                            self.slider += 1;
                        }
                    }


                    ui.spacing_mut().slider_width = ui.available_width() - 100.0;
                    if ui.add_sized(
                        [ui.available_width(), 20.0],
                        egui::Slider::new(&mut self.slider, 0..=(replay.replay_data.len()-1) as u64),
                    ).changed() {
                        self.play_time = replay.replay_data[self.slider as usize].total_time as f64 / 1000.0;
                        self.audio_song_sink.as_ref().unwrap().try_seek(std::time::Duration::from_secs_f64(self.play_time + audio_offset)).unwrap();
                    }

                    let first = if self.slider > 20 {(self.slider - 20) as usize} else {0};
                    let last = self.slider as usize;

                    let offset = egui::Vec2::new(50.0, ui.cursor().min.y + 50.0);
                    let scale = (ui.available_height() - 100.0) / 384.0;

                    ui.painter().rect(
                        egui::Rect::from_min_size(
                            egui::Pos2::new(offset.x, offset.y),
                            egui::Vec2::new(512.0, 384.0) * scale
                        ),
                        0.0,
                        egui::Color32::from_black_alpha(255),
                        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(255))
                    );

                    for i in first..=last {
                        // draw arrow from last to current replay data
                        if i > 0 {
                            let last_data = &replay.replay_data[i - 1];
                            let current_data = &replay.replay_data[i];

                            let color = egui::Color32::from_white_alpha(((i - first) as f32 / (last - first) as f32 * 255.0) as u8);

                            ui.painter().line_segment(
                                [
                                    egui::Pos2::new(last_data.x as f32, last_data.y as f32) * scale + offset,
                                    egui::Pos2::new(current_data.x as f32, current_data.y as f32) * scale + offset,
                                ],
                                egui::Stroke::new(1.0, color)
                            );

                            fn is_key_down(keys: i32, key: Keys) -> bool {
                                keys & key as i32 != 0
                            }

                            let is_k1_pressed = is_key_down(current_data.keys, Keys::K1) && !is_key_down(last_data.keys, Keys::K1);
                            let is_k2_pressed = is_key_down(current_data.keys, Keys::K2) && !is_key_down(last_data.keys, Keys::K2);

                            if is_k1_pressed || is_k2_pressed {
                                ui.painter().circle_filled(
                                    egui::Pos2::new(current_data.x as f32, current_data.y as f32) * scale + offset,
                                    5.0,
                                    color
                                );

                                if i == self.slider as usize {
                                    if let Some(hit_sound_source) = &self.hit_sound_source {
                                        if i != self.last_hit_sound_played_index {
                                            self.audio_stream_handle.as_ref().unwrap().play_raw(hit_sound_source.clone().convert_samples()).unwrap();
                                            self.last_hit_sound_played_index = i;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    pub(crate) fn handle_event(&self, egui_winit_state: &mut egui_winit::State, window: &Window, event: &winit::event::WindowEvent) -> bool {
        let response = egui_winit_state.on_window_event(window, event);
        if response.repaint {
            window.request_redraw();
        }
        response.consumed
    }
}
