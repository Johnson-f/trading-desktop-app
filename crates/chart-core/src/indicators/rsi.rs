/// Wilder-smoothed Relative Strength Index.
///
/// First RSI value appears at index `period` (needs `period` deltas, hence
/// `period + 1` closes). If `period < 2` or `n <= period`, returns all `None`.
/// An `avg_loss` of zero is treated as an all-gains window and returns 100.
pub fn compute_rsi(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if period < 2 || n <= period {
        return out;
    }
    let mut gain_sum = 0.0f32;
    let mut loss_sum = 0.0f32;
    for i in 1..=period {
        let diff = closes[i] - closes[i - 1];
        if diff >= 0.0 {
            gain_sum += diff;
        } else {
            loss_sum += -diff;
        }
    }
    let mut avg_gain = gain_sum / period as f32;
    let mut avg_loss = loss_sum / period as f32;
    out[period] = Some(rsi_from(avg_gain, avg_loss));
    let p = period as f32;
    for i in (period + 1)..n {
        let diff = closes[i] - closes[i - 1];
        let gain = if diff > 0.0 { diff } else { 0.0 };
        let loss = if diff < 0.0 { -diff } else { 0.0 };
        avg_gain = (avg_gain * (p - 1.0) + gain) / p;
        avg_loss = (avg_loss * (p - 1.0) + loss) / p;
        out[i] = Some(rsi_from(avg_gain, avg_loss));
    }
    out
}

fn rsi_from(avg_gain: f32, avg_loss: f32) -> f32 {
    if avg_loss <= 0.0 {
        return 100.0;
    }
    let rs = avg_gain / avg_loss;
    100.0 - 100.0 / (1.0 + rs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsi_none_prefix_length_equals_period() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let rsi = compute_rsi(&closes, 3);
        assert!(rsi[0].is_none());
        assert!(rsi[1].is_none());
        assert!(rsi[2].is_none());
        assert!(rsi[3].is_some());
    }

    #[test]
    fn rsi_all_gains_returns_100() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let rsi = compute_rsi(&closes, 3);
        for v in rsi.iter().flatten() {
            assert!((v - 100.0).abs() < 1e-3);
        }
    }

    #[test]
    fn rsi_returns_empty_when_insufficient_data() {
        let closes = vec![1.0, 2.0];
        let rsi = compute_rsi(&closes, 3);
        assert!(rsi.iter().all(|v| v.is_none()));
    }

    #[test]
    fn rsi_period_below_two_returns_none() {
        let closes = vec![1.0, 2.0, 3.0];
        let rsi = compute_rsi(&closes, 1);
        assert!(rsi.iter().all(|v| v.is_none()));
    }
}
