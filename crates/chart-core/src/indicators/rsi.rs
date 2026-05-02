use ta::Next;
use ta::indicators::RelativeStrengthIndex;

/// Relative Strength Index. Thin adapter over `ta::RelativeStrengthIndex`:
/// feeds every close through the streaming indicator and wraps each output in
/// `Some`. ta's RSI returns 50.0 as the initial value (no deltas seen yet)
/// and converges to Wilder-smoothed values within ~period bars.
///
/// If `ta::RelativeStrengthIndex::new(period)` rejects the period (period must
/// be >= 1), the function returns all `None`.
pub fn compute_rsi(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let Ok(mut rsi) = RelativeStrengthIndex::new(period) else {
        return vec![None; closes.len()];
    };
    closes
        .iter()
        .map(|c| Some(rsi.next(*c as f64) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsi_first_value_is_fifty() {
        // No deltas seen yet → ta reports 50.
        let closes = vec![10.0, 11.0, 12.0];
        let rsi = compute_rsi(&closes, 3);
        assert!((rsi[0].unwrap() - 50.0).abs() < 1e-3);
    }

    #[test]
    fn rsi_monotonic_gains_trend_toward_100() {
        // Strictly rising series → RSI climbs toward 100 without ever exceeding it.
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let rsi = compute_rsi(&closes, 14);
        let last = rsi.last().copied().flatten().unwrap();
        assert!(
            last > 90.0,
            "expected RSI > 90 on pure-gains series, got {last}"
        );
        assert!(last <= 100.0);
    }

    #[test]
    fn rsi_empty_input_returns_empty() {
        let rsi = compute_rsi(&[], 14);
        assert!(rsi.is_empty());
    }

    #[test]
    fn rsi_period_zero_returns_all_none() {
        let closes = vec![1.0, 2.0, 3.0];
        let rsi = compute_rsi(&closes, 0);
        assert!(rsi.iter().all(|v| v.is_none()));
    }

    #[test]
    fn rsi_every_bar_has_a_value() {
        let closes = vec![1.0, 2.0, 3.0];
        let rsi = compute_rsi(&closes, 14);
        assert!(rsi.iter().all(|v| v.is_some()));
    }
}
