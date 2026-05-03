mod manager;
mod params;
mod registry;
mod trait_def;

pub mod kinds;

pub use manager::{ActiveIndicator, IndicatorEvent, IndicatorManager};
pub use params::{ParamField, ParamKind, ParamSchema, ParamValue, ParamValues};
pub use registry::{IndicatorDef, all, get};
pub use trait_def::{Indicator, RenderTarget};
pub use zaned_chart_core::{ComputedSeries, LegendEntry};
