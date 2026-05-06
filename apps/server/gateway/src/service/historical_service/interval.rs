//! Validated bucket specification for historical bar aggregation.
//!
//! Pure types and validation logic — no I/O. The two emit-SQL methods
//! (`source_table`, `bucket_expr`) are deterministic and exhaustively
//! covered by unit tests so the resolver layer can rely on them
//! without round-tripping through ClickHouse.

use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarUnit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BucketSpec {
    pub unit: BarUnit,
    pub count: u32,
}

impl BucketSpec {
    /// Validate `count` against the per-unit upper bound. Returns an
    /// error message suitable for surfacing to GraphQL clients.
    pub fn new(unit: BarUnit, count: i32) -> Result<Self> {
        if count < 1 {
            bail!("count must be >= 1");
        }
        let max = match unit {
            BarUnit::Minute => 240,
            BarUnit::Hour => 4,
            BarUnit::Day => 365,
            BarUnit::Week => 52,
            BarUnit::Month => 12,
        };
        if count as u32 > max {
            bail!("count out of range for unit {:?} (max {})", unit, max);
        }
        Ok(Self { unit, count: count as u32 })
    }

    /// Source table for this bucket size. Sub-day buckets read 1m bars;
    /// day-and-above buckets read 1d bars.
    pub fn source_table(&self) -> &'static str {
        match self.unit {
            BarUnit::Minute | BarUnit::Hour => "bars_1m",
            BarUnit::Day | BarUnit::Week | BarUnit::Month => "bars_1d",
        }
    }

    /// SQL expression that buckets `ts` into bucket-start values.
    /// Always references the column literally as `ts`.
    pub fn bucket_expr(&self) -> String {
        let n = self.count;
        match (self.unit, n) {
            (BarUnit::Minute, n) => format!("toStartOfInterval(ts, INTERVAL {n} MINUTE)"),
            (BarUnit::Hour, n) => format!("toStartOfInterval(ts, INTERVAL {n} HOUR)"),
            (BarUnit::Day, 1) => "toStartOfDay(ts)".to_string(),
            (BarUnit::Day, n) => format!("toStartOfInterval(ts, INTERVAL {n} DAY)"),
            (BarUnit::Week, 1) => "toStartOfWeek(ts, 1)".to_string(),
            (BarUnit::Week, n) => format!("toStartOfInterval(ts, INTERVAL {n} WEEK)"),
            (BarUnit::Month, 1) => "toStartOfMonth(ts)".to_string(),
            (BarUnit::Month, n) => format!("toStartOfInterval(ts, INTERVAL {n} MONTH)"),
        }
    }
}

/// Validate a symbol string for safe use in queries. Allows ASCII
/// alphanumeric plus `.` and `-`; max 16 chars; non-empty.
pub fn validate_symbol(symbol: &str) -> Result<()> {
    if symbol.is_empty() {
        bail!("symbol must not be empty");
    }
    if symbol.len() > 16 {
        bail!("symbol must be at most 16 characters");
    }
    if !symbol.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        bail!("symbol contains invalid characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_table_routing() {
        assert_eq!(BucketSpec::new(BarUnit::Minute, 1).unwrap().source_table(), "bars_1m");
        assert_eq!(BucketSpec::new(BarUnit::Hour, 4).unwrap().source_table(), "bars_1m");
        assert_eq!(BucketSpec::new(BarUnit::Day, 1).unwrap().source_table(), "bars_1d");
        assert_eq!(BucketSpec::new(BarUnit::Week, 1).unwrap().source_table(), "bars_1d");
        assert_eq!(BucketSpec::new(BarUnit::Month, 1).unwrap().source_table(), "bars_1d");
    }

    #[test]
    fn bucket_expr_minute_and_hour() {
        assert_eq!(
            BucketSpec::new(BarUnit::Minute, 1).unwrap().bucket_expr(),
            "toStartOfInterval(ts, INTERVAL 1 MINUTE)"
        );
        assert_eq!(
            BucketSpec::new(BarUnit::Minute, 5).unwrap().bucket_expr(),
            "toStartOfInterval(ts, INTERVAL 5 MINUTE)"
        );
        assert_eq!(
            BucketSpec::new(BarUnit::Hour, 1).unwrap().bucket_expr(),
            "toStartOfInterval(ts, INTERVAL 1 HOUR)"
        );
        assert_eq!(
            BucketSpec::new(BarUnit::Hour, 4).unwrap().bucket_expr(),
            "toStartOfInterval(ts, INTERVAL 4 HOUR)"
        );
    }

    #[test]
    fn bucket_expr_day_uses_helper_for_count_1() {
        assert_eq!(
            BucketSpec::new(BarUnit::Day, 1).unwrap().bucket_expr(),
            "toStartOfDay(ts)"
        );
        assert_eq!(
            BucketSpec::new(BarUnit::Day, 5).unwrap().bucket_expr(),
            "toStartOfInterval(ts, INTERVAL 5 DAY)"
        );
    }

    #[test]
    fn bucket_expr_week_and_month_use_helpers_for_count_1() {
        assert_eq!(
            BucketSpec::new(BarUnit::Week, 1).unwrap().bucket_expr(),
            "toStartOfWeek(ts, 1)"
        );
        assert_eq!(
            BucketSpec::new(BarUnit::Week, 2).unwrap().bucket_expr(),
            "toStartOfInterval(ts, INTERVAL 2 WEEK)"
        );
        assert_eq!(
            BucketSpec::new(BarUnit::Month, 1).unwrap().bucket_expr(),
            "toStartOfMonth(ts)"
        );
        assert_eq!(
            BucketSpec::new(BarUnit::Month, 3).unwrap().bucket_expr(),
            "toStartOfInterval(ts, INTERVAL 3 MONTH)"
        );
    }

    #[test]
    fn count_lower_bound_is_one() {
        assert!(BucketSpec::new(BarUnit::Minute, 0).is_err());
        assert!(BucketSpec::new(BarUnit::Minute, -1).is_err());
        assert!(BucketSpec::new(BarUnit::Minute, 1).is_ok());
    }

    #[test]
    fn count_upper_bounds_per_unit() {
        assert!(BucketSpec::new(BarUnit::Minute, 240).is_ok());
        assert!(BucketSpec::new(BarUnit::Minute, 241).is_err());
        assert!(BucketSpec::new(BarUnit::Hour, 4).is_ok());
        assert!(BucketSpec::new(BarUnit::Hour, 5).is_err());
        assert!(BucketSpec::new(BarUnit::Day, 365).is_ok());
        assert!(BucketSpec::new(BarUnit::Day, 366).is_err());
        assert!(BucketSpec::new(BarUnit::Week, 52).is_ok());
        assert!(BucketSpec::new(BarUnit::Week, 53).is_err());
        assert!(BucketSpec::new(BarUnit::Month, 12).is_ok());
        assert!(BucketSpec::new(BarUnit::Month, 13).is_err());
    }

    #[test]
    fn validate_symbol_accepts_common_shapes() {
        assert!(validate_symbol("AAPL").is_ok());
        assert!(validate_symbol("BRK.B").is_ok());
        assert!(validate_symbol("BF-A").is_ok());
        assert!(validate_symbol("A").is_ok());
    }

    #[test]
    fn validate_symbol_rejects_bad_input() {
        assert!(validate_symbol("").is_err());
        assert!(validate_symbol("ÄBC").is_err());
        assert!(validate_symbol("abc def").is_err());
        assert!(validate_symbol("AAAAAAAAAAAAAAAAA").is_err()); // 17 chars
        assert!(validate_symbol("'; DROP TABLE bars; --").is_err());
    }
}
