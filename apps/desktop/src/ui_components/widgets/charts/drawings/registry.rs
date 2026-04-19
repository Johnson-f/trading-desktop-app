use super::kinds::{
    extended, fib_retracement, horizontal_line, horizontal_ray, ray, trend_line, vertical_line,
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
];

pub fn all() -> &'static [DrawingToolDef] {
    DEFS
}

pub fn get(id: &str) -> Option<&'static DrawingToolDef> {
    DEFS.iter().find(|d| d.id == id)
}
