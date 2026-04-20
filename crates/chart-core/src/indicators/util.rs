use ta::DataItem;

/// Build `ta::DataItem` structs from parallel OHLCV slices. Uses the shortest
/// slice length. If `volumes` is `None`, volume defaults to 0.0.
pub fn build_data_items(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    volumes: Option<&[f32]>,
) -> Vec<DataItem> {
    let n = highs.len().min(lows.len()).min(closes.len());
    let n = volumes.map_or(n, |v| n.min(v.len()));
    (0..n)
        .map(|i| {
            let vol = volumes.map_or(0.0, |v| v[i] as f64);
            DataItem::builder()
                .open(closes[i] as f64)
                .high(highs[i] as f64)
                .low(lows[i] as f64)
                .close(closes[i] as f64)
                .volume(vol)
                .build()
                .unwrap()
        })
        .collect()
}

/// Rolling maximum over a sliding window of `period` bars.
/// Returns `None` for indices where fewer than `period` bars are available.
pub fn rolling_high(data: &[f32], period: usize) -> Vec<Option<f32>> {
    data.iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                None
            } else {
                let start = i + 1 - period;
                data[start..=i].iter().copied().reduce(f32::max)
            }
        })
        .collect()
}

/// Rolling minimum over a sliding window of `period` bars.
/// Returns `None` for indices where fewer than `period` bars are available.
pub fn rolling_low(data: &[f32], period: usize) -> Vec<Option<f32>> {
    data.iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                None
            } else {
                let start = i + 1 - period;
                data[start..=i].iter().copied().reduce(f32::min)
            }
        })
        .collect()
}

/// Weighted Moving Average. Weights increase linearly: bar `i` in the window
/// gets weight `i + 1`. Returns `None` until `period` bars are available.
/// A `period` of 0 returns all `None`.
pub fn compute_wma(data: &[f32], period: usize) -> Vec<Option<f32>> {
    if period == 0 {
        return vec![None; data.len()];
    }
    let divisor: f32 = (period * (period + 1)) as f32 / 2.0;
    data.iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                None
            } else {
                let start = i + 1 - period;
                let sum: f32 = data[start..=i]
                    .iter()
                    .enumerate()
                    .map(|(w, v)| (w + 1) as f32 * v)
                    .sum();
                Some(sum / divisor)
            }
        })
        .collect()
}

/// Return the index of the first bar of every trading session (day).
/// Dates are expected as ISO-8601 strings; only the first 10 characters
/// (the `YYYY-MM-DD` portion) are compared. Index 0 is always included.
/// Returns an empty `Vec` if `dates` is empty.
pub fn session_boundaries(dates: &[String]) -> Vec<usize> {
    if dates.is_empty() {
        return Vec::new();
    }
    let mut boundaries = vec![0usize];
    for i in 1..dates.len() {
        let prev = &dates[i - 1].get(..10).unwrap_or(&dates[i - 1]);
        let curr = &dates[i].get(..10).unwrap_or(&dates[i]);
        if prev != curr {
            boundaries.push(i);
        }
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta::{Close, High, Low, Volume};

    // ── build_data_items ──────────────────────────────────────────────────────

    #[test]
    fn build_data_items_zips_hlc() {
        let highs = vec![10.0f32, 20.0, 30.0];
        let lows = vec![1.0f32, 2.0, 3.0];
        let closes = vec![5.0f32, 15.0, 25.0];
        let items = build_data_items(&highs, &lows, &closes, None);
        assert_eq!(items.len(), 3);
        assert!((items[0].high() - 10.0).abs() < 1e-6);
        assert!((items[0].low() - 1.0).abs() < 1e-6);
        assert!((items[0].close() - 5.0).abs() < 1e-6);
        assert!((items[0].volume() - 0.0).abs() < 1e-6);
        assert!((items[2].high() - 30.0).abs() < 1e-6);
    }

    #[test]
    fn build_data_items_with_volume() {
        let highs = vec![10.0f32];
        let lows = vec![1.0f32];
        let closes = vec![5.0f32];
        let volumes = vec![1000.0f32];
        let items = build_data_items(&highs, &lows, &closes, Some(&volumes));
        assert_eq!(items.len(), 1);
        assert!((items[0].volume() - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn build_data_items_mismatched_lengths_uses_shortest() {
        let highs = vec![10.0f32, 20.0, 30.0];
        let lows = vec![1.0f32, 2.0];
        let closes = vec![5.0f32, 15.0, 25.0, 35.0];
        let items = build_data_items(&highs, &lows, &closes, None);
        // shortest is lows with length 2
        assert_eq!(items.len(), 2);
    }

    // ── rolling_high / rolling_low ────────────────────────────────────────────

    #[test]
    fn rolling_high_basic() {
        let data = vec![1.0f32, 3.0, 2.0, 5.0, 4.0];
        let result = rolling_high(&data, 3);
        assert_eq!(result.len(), 5);
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!((result[2].unwrap() - 3.0).abs() < 1e-6);
        assert!((result[3].unwrap() - 5.0).abs() < 1e-6);
        assert!((result[4].unwrap() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn rolling_low_basic() {
        let data = vec![5.0f32, 3.0, 4.0, 1.0, 2.0];
        let result = rolling_low(&data, 3);
        assert_eq!(result.len(), 5);
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!((result[2].unwrap() - 3.0).abs() < 1e-6);
        assert!((result[3].unwrap() - 1.0).abs() < 1e-6);
        assert!((result[4].unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rolling_period_larger_than_data_all_none() {
        let data = vec![1.0f32, 2.0, 3.0];
        let high = rolling_high(&data, 10);
        let low = rolling_low(&data, 10);
        assert!(high.iter().all(|v| v.is_none()));
        assert!(low.iter().all(|v| v.is_none()));
    }

    // ── compute_wma ───────────────────────────────────────────────────────────

    #[test]
    fn wma_basic() {
        // period=3, divisor=6
        // index 2: weights 1,2,3 → (1·1 + 2·2 + 3·3)/6 = 14/6
        // index 3: weights 1,2,3 → (1·2 + 2·3 + 3·4)/6 = 20/6
        let data = vec![1.0f32, 2.0, 3.0, 4.0];
        let result = compute_wma(&data, 3);
        assert_eq!(result.len(), 4);
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!((result[2].unwrap() - 14.0 / 6.0).abs() < 1e-5);
        assert!((result[3].unwrap() - 20.0 / 6.0).abs() < 1e-5);
    }

    #[test]
    fn wma_period_one_is_identity() {
        let data = vec![3.0f32, 7.0, 2.0];
        let result = compute_wma(&data, 1);
        assert_eq!(result.len(), 3);
        assert!((result[0].unwrap() - 3.0).abs() < 1e-6);
        assert!((result[1].unwrap() - 7.0).abs() < 1e-6);
        assert!((result[2].unwrap() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn wma_empty_returns_empty() {
        let result = compute_wma(&[], 3);
        assert!(result.is_empty());
    }

    #[test]
    fn wma_period_zero_returns_all_none() {
        let data = vec![1.0f32, 2.0, 3.0];
        let result = compute_wma(&data, 0);
        assert!(result.iter().all(|v| v.is_none()));
    }

    // ── session_boundaries ────────────────────────────────────────────────────

    #[test]
    fn session_boundaries_finds_day_changes() {
        let dates: Vec<String> = vec![
            "2024-01-01T09:00:00".to_string(),
            "2024-01-01T09:30:00".to_string(),
            "2024-01-02T09:00:00".to_string(),
            "2024-01-02T09:30:00".to_string(),
            "2024-01-03T09:00:00".to_string(),
        ];
        let bounds = session_boundaries(&dates);
        assert_eq!(bounds, vec![0, 2, 4]);
    }

    #[test]
    fn session_boundaries_daily_data_every_bar_is_session() {
        let dates: Vec<String> = vec![
            "2024-01-01".to_string(),
            "2024-01-02".to_string(),
            "2024-01-03".to_string(),
        ];
        let bounds = session_boundaries(&dates);
        assert_eq!(bounds, vec![0, 1, 2]);
    }

    #[test]
    fn session_boundaries_empty() {
        let bounds = session_boundaries(&[]);
        assert!(bounds.is_empty());
    }
}
