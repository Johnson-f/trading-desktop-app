//! Pure chart primitives: data types, params, indicator series, and compute
//! functions. No egui/wgpu deps — everything here is reusable from any
//! renderer or headless consumer (server, tests, future mobile).

pub mod candle;
pub mod color;
pub mod indicators;
pub mod params;
pub mod series;

pub use candle::{CandleData, CandleInstance, JsonCandle, Timeframe};
pub use color::Rgba;
pub use indicators::{compute_ema, compute_rsi, compute_vma};
pub use params::{ParamField, ParamKind, ParamSchema, ParamValue, ParamValues};
pub use series::{ComputedSeries, InputSpec, LegendEntry};
