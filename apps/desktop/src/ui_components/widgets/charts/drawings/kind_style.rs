//! Per-kind style extensions. `DrawingStyle` owns the look every drawing shares
//! (color, width, dash, opacity, extend); `KindStyle` is a typed enum of
//! knobs that only make sense for a specific tool. Tools that don't need one
//! leave it at `KindStyle::None`.

use serde::{Deserialize, Serialize};
use zaned_chart_core::Rgba;

/// Number of fib ratios currently rendered by `FibRetracement`. Keep in sync
/// with the `RATIOS` const in `kinds/fib_retracement.rs`.
pub const FIB_RATIO_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum KindStyle {
    None,
    /// Rectangle / Range Measure fill toggle.
    FilledRect {
        fill_enabled: bool,
        fill_alpha: u8,
    },
    /// Fib Retracement — one bit per ratio + label toggle.
    Fib {
        /// Bit `i` on = ratio `i` is rendered.
        ratios_mask: u8,
        show_labels: bool,
    },
    /// Long / Short position zone colors.
    Position {
        profit_color: Rgba,
        loss_color: Rgba,
        entry_color: Rgba,
        show_label: bool,
    },
    /// Pitchfork fill between the outer tines.
    Pitchfork {
        fill_enabled: bool,
        fill_alpha: u8,
    },
    /// Range Measure info label toggle.
    LabeledRect {
        show_label: bool,
    },
}

impl Default for KindStyle {
    fn default() -> Self {
        Self::None
    }
}

impl KindStyle {
    pub const DEFAULT_RECT: Self = Self::FilledRect {
        fill_enabled: true,
        fill_alpha: 40,
    };

    pub const DEFAULT_FIB: Self = Self::Fib {
        ratios_mask: (1 << FIB_RATIO_COUNT) - 1,
        show_labels: true,
    };

    pub const DEFAULT_PITCHFORK: Self = Self::Pitchfork {
        fill_enabled: true,
        fill_alpha: 22,
    };

    pub const DEFAULT_LABELED_RECT: Self = Self::LabeledRect { show_label: true };

    pub fn default_position() -> Self {
        Self::Position {
            profit_color: Rgba([52, 168, 83, 255]),
            loss_color: Rgba([220, 68, 55, 255]),
            entry_color: Rgba([200, 200, 210, 255]),
            show_label: true,
        }
    }

    /// True iff a given fib ratio index is enabled.
    pub fn fib_ratio_enabled(&self, idx: usize) -> bool {
        match self {
            Self::Fib { ratios_mask, .. } => (*ratios_mask >> idx) & 1 == 1,
            _ => true,
        }
    }
}
