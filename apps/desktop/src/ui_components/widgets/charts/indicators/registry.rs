use super::kinds::{ema, rsi, volume};
use super::params::ParamSchema;
use super::trait_def::{Indicator, RenderTarget};

pub struct IndicatorDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Compact label used on the shortcut toolbar (e.g. "EMA", "VOL").
    pub short_name: &'static str,
    pub target: RenderTarget,
    pub params: ParamSchema,
    pub likes: u32,
    pub description: &'static str,
    pub factory: fn() -> Box<dyn Indicator>,
}

static DEFS: &[IndicatorDef] = &[
    IndicatorDef {
        id: ema::ID,
        name: ema::NAME,
        short_name: "EMA",
        target: RenderTarget::MainOverlay,
        params: ema::SCHEMA,
        likes: ema::LIKES,
        description: "Exponential Moving Average. Weights recent prices more heavily than older ones, making it more responsive to new information than a simple moving average.",
        factory: ema::factory,
    },
    IndicatorDef {
        id: rsi::ID,
        name: rsi::NAME,
        short_name: "RSI",
        target: RenderTarget::SubPane,
        params: rsi::SCHEMA,
        likes: rsi::LIKES,
        description: "Relative Strength Index. Oscillates between 0 and 100. Readings above 70 often indicate overbought conditions; below 30, oversold.",
        factory: rsi::factory,
    },
    IndicatorDef {
        id: volume::ID,
        name: volume::NAME,
        short_name: "VOL",
        target: RenderTarget::SubPane,
        params: volume::SCHEMA,
        likes: volume::LIKES,
        description: "Trading volume per bar with a simple moving average overlay (VMA). Up bars use the up color, down bars the down color.",
        factory: volume::factory,
    },
];

pub fn all() -> &'static [IndicatorDef] {
    DEFS
}

pub fn get(id: &str) -> Option<&'static IndicatorDef> {
    DEFS.iter().find(|d| d.id == id)
}
