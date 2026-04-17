/// Exponential Moving Average over `period` using α = 2 / (period + 1).
///
/// Seeded at index `period - 1` with the SMA of the first `period` closes.
/// Entries before the seed are `None`. If `period == 0` or `closes.len() < period`,
/// returns all `None`.
pub fn compute_ema(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if period == 0 || n < period {
        return out;
    }
    let alpha = 2.0f32 / (period as f32 + 1.0);
    let mut sum = 0.0f32;
    for c in &closes[..period] {
        sum += *c;
    }
    let mut prev = sum / period as f32;
    out[period - 1] = Some(prev);
    for i in period..n {
        prev = alpha * closes[i] + (1.0 - alpha) * prev;
        out[i] = Some(prev);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_has_none_prefix_then_smooths() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let ema = compute_ema(&closes, 3);
        assert_eq!(ema.len(), 6);
        assert!(ema[0].is_none());
        assert!(ema[1].is_none());
        assert!((ema[2].unwrap() - 2.0).abs() < 1e-5);
        assert!((ema[3].unwrap() - 3.0).abs() < 1e-5);
        assert!((ema[4].unwrap() - 4.0).abs() < 1e-5);
        assert!((ema[5].unwrap() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn ema_empty_when_data_shorter_than_period() {
        let closes = vec![1.0, 2.0];
        let ema = compute_ema(&closes, 5);
        assert!(ema.iter().all(|v| v.is_none()));
    }

    #[test]
    fn ema_period_zero_returns_all_none() {
        let closes = vec![1.0, 2.0, 3.0];
        let ema = compute_ema(&closes, 0);
        assert!(ema.iter().all(|v| v.is_none()));
    }
}
