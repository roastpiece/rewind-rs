use egui::Context;
use winit::window::Window;

pub struct Gui {
    color: egui::Color32,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            color: egui::Color32::from_rgb(0, 0, 0),
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
                    self.color = egui::Color32::from_rgb(255, 0, 0);
                }
        });

        // draw box
        let painter = context.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new(0)));
        painter.circle_filled(egui::Pos2::new(100.0,100.0),50.0, self.color);
    }

    pub(crate) fn handle_event(&self, egui_winit_state: &mut egui_winit::State, window: &Window, event: &winit::event::WindowEvent) -> bool {
        let response = egui_winit_state.on_window_event(window, event);
        if response.repaint {
            window.request_redraw();
        }
        response.consumed
    }
}
