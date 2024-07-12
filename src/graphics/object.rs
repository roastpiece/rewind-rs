use crate::models::osu_map::{ApproachRate, HitObject, HitType};

pub trait Renderable {
    fn render(&self, ctx: &mut egui::Ui, beatmap: &crate::models::osu_map::OsuMap, play_time: f64, scale: f32, offset: egui::Vec2);
}

impl Renderable for HitObject {
    fn render(&self, ctx: &mut egui::Ui, beatmap: &crate::models::osu_map::OsuMap, play_time: f64, scale: f32, offset: egui::Vec2) {
        match self.hit_type {
            HitType::Circle => render_circle(self, ctx, beatmap, play_time, scale, offset),
            HitType::Slider(_) => render_slider(self, ctx, beatmap, play_time, scale, offset),
            _ => {}
        }
    }
}

fn render_circle(hit_object: &HitObject, ui: &mut egui::Ui, beatmap: &crate::models::osu_map::OsuMap, play_time: f64, scale: f32, offset: egui::Vec2) {
    let time = hit_object.time as f64 / 1000.0;
    let ApproachRate {preempt, fade_in, ..} = beatmap.difficulty.approach_rate;

    let time_to_hit = time - play_time;
    let opacity = if time <= play_time + fade_in {
        255
    } else if time <= play_time + preempt {
        let time = time - (play_time + fade_in);
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

fn render_slider(hit_object: &HitObject, ui: &mut egui::Ui, beatmap: &crate::models::osu_map::OsuMap, play_time: f64, scale: f32, offset: egui::Vec2) {
    render_circle(hit_object, ui, beatmap, play_time, scale, offset);
}
