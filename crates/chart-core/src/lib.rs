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
pub use indicators::{compute_adx, compute_alma, compute_atr, compute_bollinger, compute_cci, compute_chandelier, compute_ema, compute_hma, compute_ichimoku, compute_keltner, compute_macd, compute_mfi, compute_obv, compute_parabolic_sar, compute_pivot_points, compute_ppo, compute_roc, compute_rsi, compute_sma, compute_stochastic, compute_supertrend, compute_vma, compute_vwap, compute_williams_r, compute_wma_indicator};
pub use params::{ParamField, ParamKind, ParamSchema, ParamValue, ParamValues};
pub use series::{ComputedSeries, InputSpec, LegendEntry};
