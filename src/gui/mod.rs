use std::time::SystemTime;

use egui::Context;
use winit::window::Window;

use crate::models::osu_replay::osu_replay::Keys;

pub struct Gui {
    picked_path: Option<String>,
    replay: Option<crate::models::osu_replay::osu_replay::OsuReplay>,
    slider: u64,
    playing: bool,
    play_time: f64,
    system_time: SystemTime,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            picked_path: None,
            replay: None,
            slider: 0,
            playing: false,
            system_time: SystemTime::now(),
            play_time: 0.0,
        }
    }

    pub fn render(&mut self, context: &Context) {
        egui::CentralPanel::default().show(&context, |ui| {
            let delta_time = SystemTime::now().duration_since(self.system_time).unwrap().as_secs_f64();
            self.system_time = SystemTime::now();
            
            ui.heading("Hello, egui!");
            ui.label("This is a simple egui window.");

            if ui.button("Load replay")
                .on_hover_ui(|ui| {
                    ui.label("Hello!");
                })
                .clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.picked_path = Some(path.display().to_string());
                        self.replay = Some(crate::models::osu_replay::osu_replay::OsuReplay::from_file(&path));
                    }
                };

            if let Some(path) = &self.picked_path {
                ui.label(format!("Picked path: {}", path));
            }

            if let Some(replay) = &self.replay {
                if ui.button("Play/Pause")
                    .clicked() {
                        self.playing = !self.playing;
                };

                ui.label(format!("Plaing: {}", self.playing));

                ui.label("Replay data:");
                ui.label(format!("Player: {}", replay.player_name));
                ui.label(format!("Score: {}", replay.score));
                ui.label(format!("Max combo: {}", replay.max_combo));
                ui.label(format!("Misses: {}", replay.count_miss));

                // add slider with full screen width

                if self.playing {
                    self.play_time += delta_time;

                    if self.play_time > replay.replay_data[replay.replay_data.len() - 1].total_time as f64 / 1000.0 {
                        self.playing = false
                    }

                    while self.playing && self.play_time > replay.replay_data[self.slider as usize].total_time as f64 / 1000.0 {
                        self.slider += 1;
                    }
                }


                ui.spacing_mut().slider_width = ui.available_width() - 100.0;
                ui.add_sized(
                    [ui.available_width(), 20.0],
                    egui::Slider::new(&mut self.slider, 0..=replay.replay_data.len() as u64),
                );

                let first = if self.slider > 20 {(self.slider - 20) as usize} else {0};
                let last = self.slider as usize;

                let offset = egui::Vec2::new(0.0, ui.cursor().min.y);
                let scale = ui.available_height() / 384.0;

                for i in first..last {
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

                        fn is_key_down(keys: i32) -> bool {
                            (keys & (Keys::K1 as i32 | Keys::K2 as i32)) as i32 != 0
                        }

                        if is_key_down(current_data.keys) && !is_key_down(last_data.keys) {
                            ui.painter().circle_filled(
                                egui::Pos2::new(current_data.x as f32, current_data.y as f32) * scale + offset,
                                5.0,
                                color
                            );
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
