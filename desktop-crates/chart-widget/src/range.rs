/// Data window selection. Maps to a number of trailing candles shown in the
/// chart's viewport. `Max` = show everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    D1,
    D5,
    M1,
    M3,
    M6,
    YTD,
    Y1,
    Y5,
    Max,
}

impl Range {
    pub const ALL: &'static [Range] = &[
        Range::D1,
        Range::D5,
        Range::M1,
        Range::M3,
        Range::M6,
        Range::YTD,
        Range::Y1,
        Range::Y5,
        Range::Max,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Range::D1 => "1D",
            Range::D5 => "5D",
            Range::M1 => "1M",
            Range::M3 => "3M",
            Range::M6 => "6M",
            Range::YTD => "YTD",
            Range::Y1 => "1Y",
            Range::Y5 => "5Y",
            Range::Max => "MAX",
        }
    }

    /// Approximate trailing-candle count assuming 1 candle = 1 trading day.
    /// `Max` and `YTD` return `None` (handled by the caller).
    pub fn trailing_candles(self) -> Option<usize> {
        match self {
            Range::D1 => Some(1),
            Range::D5 => Some(5),
            Range::M1 => Some(21),
            Range::M3 => Some(63),
            Range::M6 => Some(126),
            Range::YTD => None,
            Range::Y1 => Some(252),
            Range::Y5 => Some(252 * 5),
            Range::Max => None,
        }
    }
}
