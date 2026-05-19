use ta::Next;
use ta::indicators::ExponentialMovingAverage;

/// Exponential Moving Average. Thin adapter over `ta::ExponentialMovingAverage`:
/// feeds every close through the streaming indicator and wraps each output in
/// `Some`. ta seeds at the first sample (not SMA-of-first-N) and emits a value
/// for every bar, so there is no leading `None` prefix.
///
/// If `ta::ExponentialMovingAverage::new(period)` rejects the period (period
/// must be >= 1), the function returns all `None`.
pub fn compute_ema(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let Ok(mut ema) = ExponentialMovingAverage::new(period) else {
        return vec![None; closes.len()];
    };
    closes
        .iter()
        .map(|c| Some(ema.next(*c as f64) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_matches_ta_seeding_from_first_value() {
        // ta seeds EMA at the first sample, then α-smooths with α = 2/(p+1).
        // period=3 → α=0.5; closes=[1,2,3,4,5,6].
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let ema = compute_ema(&closes, 3);
        assert_eq!(ema.len(), 6);
        assert!((ema[0].unwrap() - 1.0).abs() < 1e-5);
        assert!((ema[1].unwrap() - 1.5).abs() < 1e-5);
        assert!((ema[2].unwrap() - 2.25).abs() < 1e-5);
        assert!((ema[3].unwrap() - 3.125).abs() < 1e-5);
        assert!((ema[4].unwrap() - 4.0625).abs() < 1e-5);
        assert!((ema[5].unwrap() - 5.03125).abs() < 1e-5);
    }

    #[test]
    fn ema_empty_input_returns_empty() {
        let ema = compute_ema(&[], 5);
        assert!(ema.is_empty());
    }

    #[test]
    fn ema_period_zero_returns_all_none() {
        let closes = vec![1.0, 2.0, 3.0];
        let ema = compute_ema(&closes, 0);
        assert!(ema.iter().all(|v| v.is_none()));
    }

    #[test]
    fn ema_every_bar_has_a_value() {
        // Unlike the prior hand-rolled impl, ta emits a value from bar 0 — no
        // leading `None` prefix.
        let closes = vec![1.0, 2.0, 3.0, 4.0];
        let ema = compute_ema(&closes, 10);
        assert!(ema.iter().all(|v| v.is_some()));
    }
}
