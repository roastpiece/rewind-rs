use std::{fs::File, io::BufReader, time::SystemTime};

use egui::Context;
use rodio::{source::Buffered, Decoder, OutputStream, Sink, Source};
use winit::window::Window;

use crate::models::osu_replay::Keys;

pub struct Gui {
    osu_path: Option<String>,
    slider: u64,
    system_time: SystemTime,
    replay_data: Option<LoadedReplayData>,
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

    last_hit_sound_played_index: usize,
    offset: f64,
    playback_speed: f64,
    playing: bool,
    play_time: f64,
    hit_object_index: usize,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            osu_path: None,
            slider: 0,
            system_time: SystemTime::now(),
            replay_data: None,
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
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.osu_path = Some(path.display().to_string());
                }
            };

            if let Some(osu_path) = &self.osu_path {
                ui.label(format!("Osu! path: {}", osu_path));

                if ui
                    .button("Load replay")
                    .on_hover_ui(|ui| {
                        ui.label("Hello!");
                    })
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        let replay_path = Some(path.display().to_string());
                        let replay =
                            crate::models::osu_replay::OsuReplay::from_file(&path);

                        let (_stream, handle) = OutputStream::try_default().unwrap();
                        let song_source = {
                            let file = BufReader::new(File::open("audio.mp3").unwrap());
                            Decoder::new(file).unwrap().buffered()
                        };
                        let sink = Sink::try_new(&handle).unwrap();
                        sink.pause();

                        let hit_sound_source = {
                            let file = BufReader::new(File::open("hit.wav").unwrap());
                            Decoder::new(file).unwrap().buffered()
                        };

                        // How to calculate offset?
                        let offset = -0.80;

                        let beatmap = crate::models::osu_map::OsuMap::from_file("diff.osu");

                        println!("{:?}", beatmap.difficulty);

                        self.replay_data = Some(LoadedReplayData {
                            replay,
                            beatmap,
                            replay_path: replay_path.unwrap(),
                            audio_output: _stream,
                            audio_stream_handle: handle,
                            audio_song_sink: sink,
                            audio_song_source: song_source,
                            hit_sound_source,
                            last_hit_sound_played_index: 0,
                            offset,
                            playback_speed: 1.0,
                            playing: false,
                            play_time: 0.0,
                            hit_object_index: 0,
                        });
                    }
                };

                if let Some(LoadedReplayData {
                    replay,
                    beatmap,
                    replay_path,
                    audio_output: _,
                    audio_stream_handle,
                    audio_song_sink,
                    audio_song_source,
                    hit_sound_source,
                    ref mut last_hit_sound_played_index,
                    ref mut offset,
                    ref mut playback_speed,
                    ref mut playing,
                    ref mut play_time,
                    ref mut hit_object_index,
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

                        while *playing && *play_time > beatmap.hit_objects[*hit_object_index].time as f64 / 1000.0 {
                            *hit_object_index += 1;
                        }
                    }

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
                        *last_hit_sound_played_index = 0;

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

                    {
                        let ar = beatmap.difficulty.approach_rate;
                        let (preempt,fade_in) = if ar < 5.0 {
                            (
                                (1200.0 + 600.0 * (5.0 - ar) / 5.0) / 1000.0,
                                (800.0 + 400.0 * (5.0 - ar) / 5.0) / 1000.0,
                            )
                        } else if ar == 5.0 {
                            (1.2, 0.8)
                        }else {
                            (
                                (1200.0 - 750.0 * (ar - 5.0) / 5.0) / 1000.0,
                                (800.0 - 500.0 * (ar - 5.0) / 5.0) / 1000.0,
                            )
                        };
                        
                        let first = *hit_object_index;
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
                        ui.label(format!("Current: {} Time: {}", *hit_object_index, beatmap.hit_objects[*hit_object_index].time as f64 / 1000.0));

                        for i in (first..=last).rev() {
                            let hit_object = &beatmap.hit_objects[i];
                            let time = hit_object.time as f64 / 1000.0;
                            let time_to_hit = time - *play_time;
                            let opacity = if time <= *play_time + fade_in {
                                255
                            } else if time <= *play_time + preempt {
                                let time = time - (*play_time + fade_in);
                                let opacity = time / (preempt - fade_in);
                                let opacity = 255.0 - opacity * 255.0;

                                if opacity < 0.0 {
                                    0
                                } else {
                                    opacity as u8
                                }
                            } else { 0 };

                            let color = egui::Color32::from_white_alpha(opacity);
                            let x = hit_object.x as f32;
                            let y = hit_object.y as f32;
                            let size = (54.4 - 4.48 * beatmap.difficulty.circle_size) as f32 * scale;

                            // hit circle
                            ui.painter().circle(
                                egui::Pos2::new(x * scale + offset.x, y * scale + offset.y),
                                size,
                                egui::Color32::from_white_alpha(0),
                                egui::Stroke::new(3.0, color),
                            );

                            let approach_size_multiplier = 1.0 + 3.0 * (1.0 - (preempt - time_to_hit) / preempt);

                            // approach circle
                            ui.painter().circle(
                                egui::Pos2::new(x * scale + offset.x, y * scale + offset.y),
                                size * approach_size_multiplier as f32,
                                egui::Color32::from_white_alpha(0),
                                egui::Stroke::new(1.0, color),
                            );
                        }
                    }

                    {
                        let first = if self.slider > 20 {
                            (self.slider - 20) as usize
                        } else {
                            0
                        };
                        let last = self.slider as usize;
                        for i in first..=last {
                            // draw arrow from last to current replay data
                            if i > 0 {
                                let last_data = &replay.replay_data[i - 1];
                                let current_data = &replay.replay_data[i];

                                let color = egui::Color32::from_white_alpha(
                                    ((i - first) as f32 / (last - first) as f32 * 255.0) as u8,
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

                                    if i == self.slider as usize {
                                        if i != *last_hit_sound_played_index {
                                            audio_stream_handle
                                                .play_raw(hit_sound_source.clone().convert_samples())
                                                .unwrap();
                                            *last_hit_sound_played_index = i;
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
