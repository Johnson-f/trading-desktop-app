use ta::Next;
use ta::indicators::MovingAverageConvergenceDivergence as TaMacd;

/// Returns three series: (macd_line, signal_line, histogram).
pub fn compute_macd(
    closes: &[f32],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let Ok(mut macd) = TaMacd::new(fast, slow, signal) else {
        let empty = vec![None; closes.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut macd_line = Vec::with_capacity(closes.len());
    let mut signal_line = Vec::with_capacity(closes.len());
    let mut histogram = Vec::with_capacity(closes.len());
    for c in closes {
        let out = macd.next(*c as f64);
        macd_line.push(Some(out.macd as f32));
        signal_line.push(Some(out.signal as f32));
        histogram.push(Some(out.histogram as f32));
    }
    (macd_line, signal_line, histogram)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_three_series() {
        let closes: Vec<f32> = (1..=50).map(|i| i as f32 * 1.5).collect();
        let (macd_line, signal_line, histogram) = compute_macd(&closes, 12, 26, 9);
        assert_eq!(macd_line.len(), closes.len());
        assert_eq!(signal_line.len(), closes.len());
        assert_eq!(histogram.len(), closes.len());
    }

    #[test]
    fn histogram_is_macd_minus_signal() {
        let closes: Vec<f32> = (1..=50).map(|i| i as f32 * 2.0).collect();
        let (macd_line, signal_line, histogram) = compute_macd(&closes, 12, 26, 9);
        for i in 0..closes.len() {
            if let (Some(m), Some(s), Some(h)) = (macd_line[i], signal_line[i], histogram[i]) {
                let expected = m - s;
                assert!(
                    (h - expected).abs() < 1e-3,
                    "histogram[{i}]: {h} != {m} - {s} = {expected}"
                );
            }
        }
    }

    #[test]
    fn empty() {
        let (a, b, c) = compute_macd(&[], 12, 26, 9);
        assert!(a.is_empty());
        assert!(b.is_empty());
        assert!(c.is_empty());
    }

    #[test]
    fn invalid_params() {
        // fast=0 causes ta to return an error
        let closes = vec![1.0f32, 2.0, 3.0];
        let (a, b, c) = compute_macd(&closes, 0, 26, 9);
        assert_eq!(a.len(), closes.len());
        assert_eq!(b.len(), closes.len());
        assert_eq!(c.len(), closes.len());
        assert!(a.iter().all(|v| v.is_none()));
    }
}
