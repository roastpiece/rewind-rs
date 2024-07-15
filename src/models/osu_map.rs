#![allow(dead_code)]
use core::fmt;
use std::{fmt::{Display, Formatter}, fs::File, io::Read, path::Path};

#[derive(Debug)]
pub struct OsuMap {
    pub(crate) difficulty: Difficulty,
    pub(crate) hit_objects: Vec<HitObject>,
}

#[derive(Debug)]
pub struct HitObject {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) time: u64,
    pub(crate) hit_type: HitType,
    pub(crate) new_combo: bool,
}

#[derive(Debug)]
pub enum HitType {
    Circle,
    Slider(Slider),
    Spinner(Spinner),
}

impl Display for HitType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            HitType::Circle => write!(f, "Circle"),
            HitType::Slider(_) => write!(f, "Slider"),
            HitType::Spinner(_) => write!(f, "Spinner"),
        }
    }
}

#[derive(Debug)]
pub struct Slider {
    pub(crate) curve_type: SliderCurveType,
    pub(crate) curve_points: Vec<(f64, f64)>,
    pub(crate) repeat: u32,
    pub(crate) pixel_length: f64,
    pub(crate) edge_sounds: Vec<u32>,
    pub(crate) edge_sets: Vec<String>,
}

#[derive(Debug)]
pub enum SliderCurveType {
    Linear,
    PerfectCircle,
    Bezier,
    Catmull,
}

#[derive(Debug)]
pub struct Spinner {
    pub(crate) end_time: u64,
}

enum HitTypeBits {
    Circle = 1,
    Slider = 2,
    NewCombo = 4,
    Spinner = 8,
}

#[derive(Debug, Default)]
pub struct OverallDifficulty {
    pub(crate) value: f64,
    pub(crate) hit_window_300: f64,
    pub(crate) hit_window_100: f64,
    pub(crate) hit_window_50: f64,
}

#[derive(Debug, Default)]
pub struct ApproachRate {
    pub(crate) value: f64,
    pub(crate) preempt: f64,
    pub(crate) fade_in: f64,
}

#[derive(Default, Debug)]
pub struct Difficulty {
    pub(crate) hit_point_drain_rate: f64,
    pub(crate) circle_size: f64,
    pub(crate) overall_difficulty: OverallDifficulty,
    pub(crate) approach_rate: ApproachRate,
    pub(crate) slider_multiplier: f64,
    pub(crate) slider_tick_rate: f64,
}

impl TryFrom<u8> for HitTypeBits {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value & HitTypeBits::Circle as u8 > 0 {
            Ok(HitTypeBits::Circle)
        } else if value & HitTypeBits::Slider as u8 > 0 {
            Ok(HitTypeBits::Slider)
        } else if value & HitTypeBits::Spinner as u8 > 0 {
            Ok(HitTypeBits::Spinner)
        } else {
            Err(())
        }
    }
}

impl OsuMap {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let mut file = File::open(path).unwrap();
        let mut data = String::new();
        file.read_to_string(&mut data).unwrap();

        let difficulty = data.lines()
            .skip_while(|line| !line.starts_with("[Difficulty]"))
            .skip(1)
            .take_while(|line| !line.is_empty())
            .fold(Difficulty::default(), |diff, line| {
                let mut parts = line.split(':');
                let key = parts.next().unwrap();
                let value = parts.next().unwrap().trim().parse().unwrap();
                match key {
                    "HPDrainRate" => Difficulty { hit_point_drain_rate: value, ..diff },
                    "CircleSize" => Difficulty { circle_size: value, ..diff },
                    "OverallDifficulty" => Difficulty { overall_difficulty: OverallDifficulty {
                        value,
                        hit_window_300: (80.0 - 6.0 * value) / 1000.0,
                        hit_window_100: (140.0 - 8.0 * value) / 1000.0,
                        hit_window_50: (200.0 - 10.0 * value) / 1000.0,
                    }, ..diff },
                    "ApproachRate" => {
                        let (preempt,fade_in) = if value < 5.0 {
                            (
                                (1200.0 + 600.0 * (5.0 - value) / 5.0) / 1000.0,
                                (800.0 + 400.0 * (5.0 - value) / 5.0) / 1000.0,
                            )
                        } else if value == 5.0 {
                            (1.2, 0.8)
                        } else {
                            (
                                (1200.0 - 750.0 * (value - 5.0) / 5.0) / 1000.0,
                                (800.0 - 500.0 * (value - 5.0) / 5.0) / 1000.0,
                            )
                        };
                        Difficulty {
                            approach_rate: ApproachRate {
                                value,
                                preempt,
                                fade_in,
                            },
                            ..diff 
                        }
                    },
                    "SliderMultiplier" => Difficulty { slider_multiplier: value, ..diff },
                    "SliderTickRate" => Difficulty { slider_tick_rate: value, ..diff },
                    _ => unreachable!(),
                }
            });

        let hit_objects = data.lines()
            .skip_while(|line| !line.starts_with("[HitObjects]"))
            .skip(1)
            .take_while(|line| !line.is_empty())
            .map(|line| {
                let mut parts = line.split(',');
                let x = parts.next().unwrap().parse().unwrap();
                let y = parts.next().unwrap().parse().unwrap();
                let time = parts.next().unwrap().parse().unwrap();

                let hit_type = HitTypeBits::try_from(
                    parts.next().unwrap()
                    .parse::<u8>().unwrap()
                ).unwrap();

                // skip hitsound
                let _ = parts.next().unwrap();

                let hit_type = match hit_type {
                    HitTypeBits::Circle => HitType::Circle,
                    HitTypeBits::Slider => {
                        let mut slider_parts = parts.next().unwrap().split('|');
                        let curve_type = match slider_parts.next().unwrap() {
                            "L" => SliderCurveType::Linear,
                            "P" => SliderCurveType::PerfectCircle,
                            "B" => SliderCurveType::Bezier,
                            "C" => SliderCurveType::Catmull,
                            _ => unreachable!(),
                        };
                        let curve_points = {
                            let mut curve_points = Vec::new();
                            while let Some(curve_point) = slider_parts.next() {
                                let mut point_parts = curve_point.split(':');
                                let x = point_parts.next().unwrap().parse().unwrap();
                                let y = point_parts.next().unwrap().parse().unwrap();
                                curve_points.push((x, y));
                            }
                            curve_points
                        };

                        let repeat = parts.next().unwrap().parse().unwrap();
                        let pixel_length = parts.next().unwrap().parse().unwrap();
                        let edge_sounds = parts.next().unwrap()
                            .split('|')
                            .map(|sound| sound.parse().unwrap())
                            .collect();
                        let edge_sets = parts.next().unwrap()
                            .split('|')
                            .map(|set| set.parse().unwrap())
                            .collect();
                        HitType::Slider(Slider {
                            curve_type,
                            curve_points,
                            repeat,
                            pixel_length,
                            edge_sounds,
                            edge_sets,
                        })
                    },
                    HitTypeBits::Spinner => {
                        let end_time = parts.next().unwrap().parse().unwrap();
                        HitType::Spinner(Spinner { end_time })
                    },
                    _ => unreachable!(),
                };

                HitObject { x, y, time, hit_type, new_combo: false }
            }).collect();

        OsuMap { difficulty, hit_objects }
    }
}
