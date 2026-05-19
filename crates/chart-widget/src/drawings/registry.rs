use super::kinds::{
    extended, fib_retracement, horizontal_line, horizontal_ray, long_position, parallel_channel,
    pitchfork, polyline, range_measure, ray, rectangle, short_position, trend_channel, trend_line,
    vertical_line,
};
use super::trait_def::DrawingTool;

pub struct DrawingToolDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub factory: fn() -> Box<dyn DrawingTool>,
}

static DEFS: &[DrawingToolDef] = &[
    DrawingToolDef {
        id: trend_line::ID,
        name: trend_line::NAME,
        icon: trend_line::ICON,
        factory: trend_line::factory,
    },
    DrawingToolDef {
        id: horizontal_line::ID,
        name: horizontal_line::NAME,
        icon: horizontal_line::ICON,
        factory: horizontal_line::factory,
    },
    DrawingToolDef {
        id: vertical_line::ID,
        name: vertical_line::NAME,
        icon: vertical_line::ICON,
        factory: vertical_line::factory,
    },
    DrawingToolDef {
        id: extended::ID,
        name: extended::NAME,
        icon: extended::ICON,
        factory: extended::factory,
    },
    DrawingToolDef {
        id: ray::ID,
        name: ray::NAME,
        icon: ray::ICON,
        factory: ray::factory,
    },
    DrawingToolDef {
        id: horizontal_ray::ID,
        name: horizontal_ray::NAME,
        icon: horizontal_ray::ICON,
        factory: horizontal_ray::factory,
    },
    DrawingToolDef {
        id: fib_retracement::ID,
        name: fib_retracement::NAME,
        icon: fib_retracement::ICON,
        factory: fib_retracement::factory,
    },
    DrawingToolDef {
        id: rectangle::ID,
        name: rectangle::NAME,
        icon: rectangle::ICON,
        factory: rectangle::factory,
    },
    DrawingToolDef {
        id: polyline::ID,
        name: polyline::NAME,
        icon: polyline::ICON,
        factory: polyline::factory,
    },
    DrawingToolDef {
        id: parallel_channel::ID,
        name: parallel_channel::NAME,
        icon: parallel_channel::ICON,
        factory: parallel_channel::factory,
    },
    DrawingToolDef {
        id: trend_channel::ID,
        name: trend_channel::NAME,
        icon: trend_channel::ICON,
        factory: trend_channel::factory,
    },
    DrawingToolDef {
        id: pitchfork::ID,
        name: pitchfork::NAME,
        icon: pitchfork::ICON,
        factory: pitchfork::factory,
    },
    DrawingToolDef {
        id: range_measure::ID,
        name: range_measure::NAME,
        icon: range_measure::ICON,
        factory: range_measure::factory,
    },
    DrawingToolDef {
        id: long_position::ID,
        name: long_position::NAME,
        icon: long_position::ICON,
        factory: long_position::factory,
    },
    DrawingToolDef {
        id: short_position::ID,
        name: short_position::NAME,
        icon: short_position::ICON,
        factory: short_position::factory,
    },
];

pub fn all() -> &'static [DrawingToolDef] {
    DEFS
}

pub fn get(id: &str) -> Option<&'static DrawingToolDef> {
    DEFS.iter().find(|d| d.id == id)
}
