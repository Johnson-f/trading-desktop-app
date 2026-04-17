use std::collections::HashMap;

use crate::color::Rgba;

/// Declares which series an indicator's `compute` needs as input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSpec {
    Closes,
    Opens,
    Highs,
    Lows,
    Volumes,
}

#[derive(Default)]
pub struct ComputedSeries {
    pub series: HashMap<&'static str, Vec<Option<f32>>>,
}

#[derive(Clone, Debug)]
pub struct LegendEntry {
    pub label: String,
    pub color: Rgba,
}
