use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

/// Granularity at which raw daily candles are aggregated into higher-
/// timeframe bars. `Daily` is a no-op (raw data passes through unchanged).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    Daily,
    Weekly,
    Monthly,
}

impl Timeframe {
    pub fn short_label(self) -> &'static str {
        match self {
            Timeframe::Daily => "1D",
            Timeframe::Weekly => "1W",
            Timeframe::Monthly => "1M",
        }
    }

    pub fn long_label(self) -> &'static str {
        match self {
            Timeframe::Daily => "Daily",
            Timeframe::Weekly => "Weekly",
            Timeframe::Monthly => "Monthly",
        }
    }

    pub const ALL: &'static [Timeframe] =
        &[Timeframe::Daily, Timeframe::Weekly, Timeframe::Monthly];
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CandleInstance {
    pub index: f32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
}

#[derive(serde::Deserialize)]
pub struct JsonCandle {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

#[derive(Clone)]
pub struct CandleData {
    pub instances: Vec<CandleInstance>,
    pub dates: Vec<String>,
}

impl CandleData {
    pub fn from_json(json_candles: &[JsonCandle]) -> Self {
        let instances: Vec<CandleInstance> = json_candles
            .iter()
            .enumerate()
            .map(|(i, c)| CandleInstance {
                index: i as f32,
                open: c.open as f32,
                high: c.high as f32,
                low: c.low as f32,
                close: c.close as f32,
                volume: c.volume as f32,
            })
            .collect();
        let dates = json_candles.iter().map(|c| c.date.clone()).collect();
        Self { instances, dates }
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Given a date string (YYYY-MM-DD), return the index of the aggregated
    /// candle that contains or immediately follows that date. Aggregated dates
    /// store the *last* date of each bucket, so `partition_point` finds the
    /// first bucket whose end-date >= `date`.
    pub fn index_for_date(&self, date: &str) -> Option<usize> {
        if self.dates.is_empty() {
            return None;
        }
        let idx = self.dates.partition_point(|d| d.as_str() < date);
        Some(idx.min(self.dates.len() - 1))
    }

    /// Returns the date string at the given index, or `None` if out of range.
    pub fn date_for_index(&self, idx: usize) -> Option<String> {
        self.dates.get(idx).cloned()
    }

    /// Like `index_for_date`, but on a miss returns the index of the nearest
    /// available date (by absolute difference in `YYYY-MM-DD` lex order, which
    /// is also chronological order). Returns `None` only when the data is empty.
    pub fn nearest_index_for_date(&self, date: &str) -> Option<usize> {
        if self.dates.is_empty() {
            return None;
        }
        let pos = self.dates.partition_point(|d| d.as_str() < date);
        if pos == 0 {
            return Some(0);
        }
        if pos == self.dates.len() {
            return Some(self.dates.len() - 1);
        }
        let after = &self.dates[pos];
        let before = &self.dates[pos - 1];
        if after.as_str() == date {
            return Some(pos);
        }
        let d_before = days_between_yyyy_mm_dd(before, date);
        let d_after = days_between_yyyy_mm_dd(date, after);
        if d_before <= d_after {
            Some(pos - 1)
        } else {
            Some(pos)
        }
    }

    /// Roll up daily candles into calendar-aligned higher-timeframe bars.
    /// Unlike `bucketed` (which groups by count), this respects ISO weeks and
    /// calendar months. Date strings are expected in `YYYY-MM-DD` format;
    /// un-parseable rows fall into their own single-candle bucket.
    ///
    /// `Timeframe::Daily` returns a trivial clone.
    pub fn aggregated(&self, timeframe: Timeframe) -> Self {
        if matches!(timeframe, Timeframe::Daily) || self.instances.is_empty() {
            return Self {
                instances: self.instances.clone(),
                dates: self.dates.clone(),
            };
        }

        // Group consecutive entries sharing the same bucket key. Since the
        // input is ordered chronologically, we only need to compare against
        // the previous key — no full hash map required.
        let mut instances: Vec<CandleInstance> = Vec::new();
        let mut dates: Vec<String> = Vec::new();
        let mut current_key: Option<i64> = None;
        let mut bucket_start: usize = 0;

        for i in 0..self.instances.len() {
            let key = bucket_key(&self.dates[i], timeframe);
            if current_key != Some(key) {
                if current_key.is_some() {
                    let last = i - 1;
                    push_aggregate(
                        &mut instances,
                        &mut dates,
                        &self.instances[bucket_start..=last],
                        &self.dates[last],
                    );
                }
                current_key = Some(key);
                bucket_start = i;
            }
        }
        // Flush the final open bucket.
        if !self.instances.is_empty() {
            let last = self.instances.len() - 1;
            push_aggregate(
                &mut instances,
                &mut dates,
                &self.instances[bucket_start..=last],
                &self.dates[last],
            );
        }

        Self { instances, dates }
    }

    /// Produce a downsampled view where every `bucket_size` raw candles are
    /// aggregated into one "higher-timeframe" candle (open = first, close =
    /// last, high = max, low = min, volume = sum). The returned `CandleData`
    /// preserves the invariant `instances[i].index == i` so every caller that
    /// indexes by position keeps working.
    ///
    /// `bucket_size` of 0 or 1 returns a trivial clone.
    pub fn bucketed(&self, bucket_size: usize) -> Self {
        if bucket_size <= 1 {
            return Self {
                instances: self.instances.clone(),
                dates: self.dates.clone(),
            };
        }

        let n = self.instances.len();
        let bucket_count = n.div_ceil(bucket_size);
        let mut instances = Vec::with_capacity(bucket_count);
        let mut dates = Vec::with_capacity(bucket_count);

        for bucket_idx in 0..bucket_count {
            let start = bucket_idx * bucket_size;
            let end = (start + bucket_size).min(n);
            let slice = &self.instances[start..end];
            // Safe: start < end because start < n and bucket_size >= 1.
            let first = slice[0];
            let last = slice[slice.len() - 1];
            let mut high = first.high;
            let mut low = first.low;
            let mut volume = 0.0;
            for c in slice {
                if c.high > high {
                    high = c.high;
                }
                if c.low < low {
                    low = c.low;
                }
                volume += c.volume;
            }
            instances.push(CandleInstance {
                index: bucket_idx as f32,
                open: first.open,
                high,
                low,
                close: last.close,
                volume,
            });
            dates.push(self.dates[end - 1].clone());
        }

        Self { instances, dates }
    }
}

/// Append one aggregated bar to the output vectors, summarising an already-
/// grouped contiguous slice of raw candles.
fn push_aggregate(
    instances: &mut Vec<CandleInstance>,
    dates: &mut Vec<String>,
    slice: &[CandleInstance],
    last_date: &str,
) {
    if slice.is_empty() {
        return;
    }
    let first = slice[0];
    let last = slice[slice.len() - 1];
    let mut high = first.high;
    let mut low = first.low;
    let mut volume = 0.0;
    for c in slice {
        if c.high > high {
            high = c.high;
        }
        if c.low < low {
            low = c.low;
        }
        volume += c.volume;
    }
    let idx = instances.len() as f32;
    instances.push(CandleInstance {
        index: idx,
        open: first.open,
        high,
        low,
        close: last.close,
        volume,
    });
    dates.push(last_date.to_string());
}

/// Exact day-difference between two `YYYY-MM-DD` strings via Julian Day Numbers.
/// Returns `i64::MAX` if either string fails to parse — callers only use this
/// for "which is closer" comparisons so an unparseable date will simply lose.
fn days_between_yyyy_mm_dd(a: &str, b: &str) -> i64 {
    let (Some((ya, ma, da)), Some((yb, mb, db))) = (parse_ymd(a), parse_ymd(b)) else {
        return i64::MAX;
    };
    (jdn(ya, ma, da) - jdn(yb, mb, db)).abs()
}

/// Calendar bucket key for a date string and a target timeframe. Returns a
/// unique `i64` per (year, month) for monthly, or per ISO-week-start for
/// weekly. Un-parseable dates get a key based on their position in the vec
/// (effectively a no-aggregation fallback for that row).
fn bucket_key(date: &str, timeframe: Timeframe) -> i64 {
    match parse_ymd(date) {
        Some((y, m, d)) => match timeframe {
            Timeframe::Daily => jdn(y, m, d),
            // Monthly: (year * 12 + month) — monotonic and unique per month.
            Timeframe::Monthly => (y as i64) * 12 + (m as i64 - 1),
            // Weekly: JDN / 7. Integer division naturally groups Mon..Sun
            // into one bucket because JDN mod 7 = 0 on Mondays (verified
            // against 2000-01-03 Mon, JDN 2451547; 2451547 = 7 × 350221).
            Timeframe::Weekly => jdn(y, m, d) / 7,
        },
        None => {
            // Stable fallback so un-parseable rows don't collapse together
            // accidentally: use the raw pointer mod something stable. We
            // don't have the index here, so just hash the string.
            let mut h: i64 = 0;
            for b in date.as_bytes() {
                h = h.wrapping_mul(31).wrapping_add(*b as i64);
            }
            h
        }
    }
}

/// Parse an ISO-like `YYYY-MM-DD` into components. Tolerates leading chars
/// beyond the date (e.g. `YYYY-MM-DDTHH:MM:SS`) by only reading the first 10.
fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let bytes = s.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year: i32 = std::str::from_utf8(&bytes[0..4]).ok()?.parse().ok()?;
    let month: u32 = std::str::from_utf8(&bytes[5..7]).ok()?.parse().ok()?;
    let day: u32 = std::str::from_utf8(&bytes[8..10]).ok()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

/// Julian Day Number for a proleptic Gregorian `YYYY-MM-DD`. Useful as a
/// linear integer day axis for week bucketing.
fn jdn(year: i32, month: u32, day: u32) -> i64 {
    let a = (14 - month as i32) / 12;
    let y = year + 4800 - a;
    let m = month as i32 + 12 * a - 3;
    (day as i64) + ((153 * m as i64 + 2) / 5) + 365 * y as i64 + (y as i64) / 4 - (y as i64) / 100
        + (y as i64) / 400
        - 32045
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(values: &[(f32, f32, f32, f32, f32)], dates: &[&str]) -> CandleData {
        let instances = values
            .iter()
            .enumerate()
            .map(|(i, (o, h, l, c, v))| CandleInstance {
                index: i as f32,
                open: *o,
                high: *h,
                low: *l,
                close: *c,
                volume: *v,
            })
            .collect();
        let dates = dates.iter().map(|s| s.to_string()).collect();
        CandleData { instances, dates }
    }

    #[test]
    fn bucket_size_one_is_identity() {
        let d = raw(
            &[(1.0, 2.0, 0.5, 1.5, 100.0), (1.5, 3.0, 1.0, 2.5, 200.0)],
            &["d1", "d2"],
        );
        let b = d.bucketed(1);
        assert_eq!(b.len(), 2);
        assert_eq!(b.instances[0].open, 1.0);
        assert_eq!(b.instances[1].close, 2.5);
        assert_eq!(b.dates, vec!["d1".to_string(), "d2".to_string()]);
    }

    #[test]
    fn bucket_size_zero_is_identity() {
        let d = raw(&[(1.0, 2.0, 0.5, 1.5, 100.0)], &["d1"]);
        let b = d.bucketed(0);
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn bucket_aggregates_ohlcv_correctly() {
        // 5 raw candles bucketed by 5 → 1 bucket.
        let d = raw(
            &[
                (10.0, 12.0, 9.0, 11.0, 100.0),
                (11.0, 13.0, 10.5, 12.5, 150.0),
                (12.5, 14.0, 12.0, 13.0, 200.0),
                (13.0, 13.5, 11.0, 11.5, 120.0),
                (11.5, 12.0, 10.0, 10.5, 180.0),
            ],
            &["d1", "d2", "d3", "d4", "d5"],
        );
        let b = d.bucketed(5);
        assert_eq!(b.len(), 1);
        let c = b.instances[0];
        assert_eq!(c.index, 0.0);
        assert_eq!(c.open, 10.0); // first
        assert_eq!(c.close, 10.5); // last
        assert_eq!(c.high, 14.0); // max
        assert_eq!(c.low, 9.0); // min
        assert!((c.volume - 750.0).abs() < 1e-3);
        assert_eq!(b.dates, vec!["d5".to_string()]);
    }

    #[test]
    fn bucket_leaves_tail_partial_bucket() {
        // 7 raw candles bucketed by 3 → 3 buckets of sizes 3, 3, 1.
        let d = raw(
            &[
                (1.0, 2.0, 0.5, 1.5, 10.0),
                (1.5, 2.5, 1.0, 2.0, 20.0),
                (2.0, 3.0, 1.5, 2.5, 30.0),
                (2.5, 3.5, 2.0, 3.0, 40.0),
                (3.0, 4.0, 2.5, 3.5, 50.0),
                (3.5, 4.5, 3.0, 4.0, 60.0),
                (4.0, 5.0, 3.5, 4.5, 70.0),
            ],
            &["d1", "d2", "d3", "d4", "d5", "d6", "d7"],
        );
        let b = d.bucketed(3);
        assert_eq!(b.len(), 3);
        assert_eq!(b.instances[0].open, 1.0);
        assert_eq!(b.instances[0].close, 2.5);
        assert_eq!(b.instances[2].open, 4.0);
        assert_eq!(b.instances[2].close, 4.5);
        assert_eq!(b.instances[2].high, 5.0);
        assert_eq!(b.instances[2].low, 3.5);
        assert_eq!(
            b.dates,
            vec!["d3".to_string(), "d6".to_string(), "d7".to_string()]
        );
    }

    #[test]
    fn bucket_preserves_index_invariant() {
        // instances[i].index == i must hold for downstream indexing code.
        let d = raw(
            &[
                (1.0, 2.0, 0.5, 1.5, 10.0),
                (1.5, 2.5, 1.0, 2.0, 20.0),
                (2.0, 3.0, 1.5, 2.5, 30.0),
                (2.5, 3.5, 2.0, 3.0, 40.0),
            ],
            &["d1", "d2", "d3", "d4"],
        );
        let b = d.bucketed(2);
        for (i, inst) in b.instances.iter().enumerate() {
            assert_eq!(inst.index as usize, i);
        }
    }

    #[test]
    fn daily_aggregation_is_identity() {
        let d = raw(
            &[(1.0, 2.0, 0.5, 1.5, 100.0), (1.5, 3.0, 1.0, 2.5, 200.0)],
            &["2026-03-02", "2026-03-03"],
        );
        let agg = d.aggregated(Timeframe::Daily);
        assert_eq!(agg.len(), 2);
        assert_eq!(agg.dates, d.dates);
    }

    #[test]
    fn monthly_aggregates_by_calendar_month() {
        // Mix of dates across two months — Jan has 3 days, Feb has 2.
        let d = raw(
            &[
                (10.0, 12.0, 9.0, 11.0, 100.0),
                (11.0, 13.0, 10.0, 12.0, 110.0),
                (12.0, 14.0, 11.0, 13.0, 120.0),
                (13.0, 15.0, 12.0, 14.0, 200.0),
                (14.0, 16.0, 13.0, 15.0, 210.0),
            ],
            &[
                "2026-01-05",
                "2026-01-12",
                "2026-01-26",
                "2026-02-02",
                "2026-02-09",
            ],
        );
        let agg = d.aggregated(Timeframe::Monthly);
        assert_eq!(agg.len(), 2);
        // January bucket
        assert_eq!(agg.instances[0].open, 10.0);
        assert_eq!(agg.instances[0].close, 13.0);
        assert_eq!(agg.instances[0].high, 14.0);
        assert_eq!(agg.instances[0].low, 9.0);
        assert!((agg.instances[0].volume - 330.0).abs() < 1e-3);
        assert_eq!(agg.dates[0], "2026-01-26"); // last day in Jan bucket
        // February bucket
        assert_eq!(agg.instances[1].open, 13.0);
        assert_eq!(agg.instances[1].close, 15.0);
        assert_eq!(agg.instances[1].high, 16.0);
        assert_eq!(agg.instances[1].low, 12.0);
        assert!((agg.instances[1].volume - 410.0).abs() < 1e-3);
        assert_eq!(agg.dates[1], "2026-02-09");
    }

    #[test]
    fn weekly_aggregates_mon_through_sun_into_one_bucket() {
        // 2026-03-02 = Monday; 2026-03-08 = Sunday → same bucket.
        // 2026-03-09 = Monday → new bucket.
        let d = raw(
            &[
                (1.0, 2.0, 0.5, 1.5, 10.0),
                (1.5, 2.5, 1.0, 2.0, 20.0),
                (2.0, 3.0, 1.5, 2.5, 30.0),
                (3.0, 4.0, 2.5, 3.5, 40.0),
            ],
            &[
                "2026-03-02", // Mon
                "2026-03-06", // Fri
                "2026-03-09", // Mon (new week)
                "2026-03-13", // Fri
            ],
        );
        let agg = d.aggregated(Timeframe::Weekly);
        assert_eq!(agg.len(), 2);
        assert_eq!(agg.dates[0], "2026-03-06"); // last day in week 1
        assert_eq!(agg.dates[1], "2026-03-13"); // last day in week 2
        // First week: 2 days rolled up.
        assert_eq!(agg.instances[0].open, 1.0);
        assert_eq!(agg.instances[0].close, 2.0);
        assert!((agg.instances[0].volume - 30.0).abs() < 1e-3);
    }

    #[test]
    fn aggregated_preserves_index_invariant() {
        let d = raw(
            &[
                (1.0, 2.0, 0.5, 1.5, 10.0),
                (2.0, 3.0, 1.5, 2.5, 20.0),
                (3.0, 4.0, 2.5, 3.5, 30.0),
            ],
            &["2026-01-05", "2026-02-05", "2026-03-05"],
        );
        let agg = d.aggregated(Timeframe::Monthly);
        assert_eq!(agg.len(), 3);
        for (i, inst) in agg.instances.iter().enumerate() {
            assert_eq!(inst.index as usize, i);
        }
    }

    #[test]
    fn date_for_index_returns_date_at_position() {
        let candles = vec![
            JsonCandle {
                date: "2026-01-01".into(),
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: 1.0,
                volume: 0,
            },
            JsonCandle {
                date: "2026-01-02".into(),
                open: 2.0,
                high: 2.0,
                low: 2.0,
                close: 2.0,
                volume: 0,
            },
            JsonCandle {
                date: "2026-01-03".into(),
                open: 3.0,
                high: 3.0,
                low: 3.0,
                close: 3.0,
                volume: 0,
            },
        ];
        let d = CandleData::from_json(&candles);
        assert_eq!(d.date_for_index(0).as_deref(), Some("2026-01-01"));
        assert_eq!(d.date_for_index(2).as_deref(), Some("2026-01-03"));
        assert_eq!(d.date_for_index(99), None);
    }

    #[test]
    fn nearest_index_for_date_handles_misses() {
        let candles = vec![
            JsonCandle {
                date: "2026-01-05".into(),
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: 1.0,
                volume: 0,
            },
            JsonCandle {
                date: "2026-01-12".into(),
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: 1.0,
                volume: 0,
            },
            JsonCandle {
                date: "2026-01-19".into(),
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: 1.0,
                volume: 0,
            },
        ];
        let d = CandleData::from_json(&candles);
        // exact hit
        assert_eq!(d.nearest_index_for_date("2026-01-12"), Some(1));
        // before first
        assert_eq!(d.nearest_index_for_date("2026-01-01"), Some(0));
        // after last
        assert_eq!(d.nearest_index_for_date("2026-12-31"), Some(2));
        // between — picks closer
        assert_eq!(d.nearest_index_for_date("2026-01-08"), Some(0)); // closer to 2026-01-05 than to 2026-01-12
        assert_eq!(d.nearest_index_for_date("2026-01-10"), Some(1)); // closer to 2026-01-12 than to 2026-01-05
        // empty
        let empty = CandleData::from_json(&[]);
        assert_eq!(empty.nearest_index_for_date("2026-01-01"), None);
    }
}
