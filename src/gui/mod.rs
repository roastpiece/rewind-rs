use egui::Context;
use winit::window::Window;

pub struct Gui {
    picked_path: Option<String>,
    replay: Option<crate::models::osu_replay::osu_replay::OsuReplay>,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            picked_path: None,
            replay: None,
        }
    }

    pub fn render(&mut self, context: &Context) {
        egui::CentralPanel::default().show(&context, |ui| {
            ui.heading("Hello, egui!");
            ui.label("This is a simple egui window.");

            if ui.button("Click me!")
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
                ui.label("Replay data:");
                ui.label(format!("Player: {}", replay.player_name));
                ui.label(format!("Score: {}", replay.score));
                ui.label(format!("Max combo: {}", replay.max_combo));
                ui.label(format!("Misses: {}", replay.count_miss));
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
