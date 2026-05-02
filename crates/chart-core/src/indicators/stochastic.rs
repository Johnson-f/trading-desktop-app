use super::util::build_data_items;
use ta::Next;
use ta::indicators::{ExponentialMovingAverage, FastStochastic};

/// Returns two series: (%K, %D).
///
/// %K is the raw fast stochastic computed from HLC data.
/// %D is an EMA of %K (the smoothed signal line).
pub fn compute_stochastic(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
    let items = build_data_items(highs, lows, closes, None);
    let (Ok(mut fast_stoch), Ok(mut ema)) = (
        FastStochastic::new(period),
        ExponentialMovingAverage::new(period),
    ) else {
        let empty = vec![None; items.len()];
        return (empty.clone(), empty);
    };
    let mut k_series = Vec::with_capacity(items.len());
    let mut d_series = Vec::with_capacity(items.len());
    for item in &items {
        let k = fast_stoch.next(item) as f32;
        let d = ema.next(k as f64) as f32;
        k_series.push(Some(k));
        d_series.push(Some(d));
    }
    (k_series, d_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_two_series() {
        let highs: Vec<f32> = (10..=40).map(|i| i as f32 + 2.0).collect();
        let lows: Vec<f32> = (10..=40).map(|i| i as f32 - 2.0).collect();
        let closes: Vec<f32> = (10..=40).map(|i| i as f32).collect();
        let (k, d) = compute_stochastic(&highs, &lows, &closes, 14);
        assert_eq!(k.len(), closes.len());
        assert_eq!(d.len(), closes.len());
        assert!(k.iter().all(|v| v.is_some()));
        assert!(d.iter().all(|v| v.is_some()));
    }

    #[test]
    fn values_in_0_100_range() {
        let highs: Vec<f32> = (10..=60).map(|i| i as f32 + 5.0).collect();
        let lows: Vec<f32> = (10..=60).map(|i| i as f32 - 5.0).collect();
        let closes: Vec<f32> = (10..=60).map(|i| i as f32).collect();
        let (k, d) = compute_stochastic(&highs, &lows, &closes, 14);
        for v in k.iter().flatten() {
            assert!(*v >= 0.0 && *v <= 100.0, "%K out of range: {v}");
        }
        for v in d.iter().flatten() {
            assert!(*v >= 0.0 && *v <= 100.0, "%D out of range: {v}");
        }
    }

    #[test]
    fn empty_returns_empty() {
        let (k, d) = compute_stochastic(&[], &[], &[], 14);
        assert!(k.is_empty());
        assert!(d.is_empty());
    }

    #[test]
    fn invalid_period_returns_none_series() {
        let highs = vec![10.0f32, 11.0, 12.0];
        let lows = vec![8.0f32, 9.0, 10.0];
        let closes = vec![9.0f32, 10.0, 11.0];
        let (k, d) = compute_stochastic(&highs, &lows, &closes, 0);
        assert_eq!(k.len(), closes.len());
        assert_eq!(d.len(), closes.len());
        assert!(k.iter().all(|v| v.is_none()));
        assert!(d.iter().all(|v| v.is_none()));
    }
}
