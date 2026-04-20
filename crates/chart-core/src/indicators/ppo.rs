use ta::Next;
use ta::indicators::PercentagePriceOscillator as TaPpo;

/// Returns three series: (ppo_line, signal_line, histogram).
pub fn compute_ppo(
    closes: &[f32],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let Ok(mut ppo) = TaPpo::new(fast, slow, signal) else {
        let empty = vec![None; closes.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut ppo_line = Vec::with_capacity(closes.len());
    let mut signal_line = Vec::with_capacity(closes.len());
    let mut histogram = Vec::with_capacity(closes.len());
    for c in closes {
        let out = ppo.next(*c as f64);
        ppo_line.push(Some(out.ppo as f32));
        signal_line.push(Some(out.signal as f32));
        histogram.push(Some(out.histogram as f32));
    }
    (ppo_line, signal_line, histogram)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_three_series() {
        let closes: Vec<f32> = (1..=50).map(|i| i as f32 * 1.5).collect();
        let (ppo_line, signal_line, histogram) = compute_ppo(&closes, 12, 26, 9);
        assert_eq!(ppo_line.len(), closes.len());
        assert_eq!(signal_line.len(), closes.len());
        assert_eq!(histogram.len(), closes.len());
    }

    #[test]
    fn histogram_is_ppo_minus_signal() {
        let closes: Vec<f32> = (1..=50).map(|i| i as f32 * 2.0).collect();
        let (ppo_line, signal_line, histogram) = compute_ppo(&closes, 12, 26, 9);
        for i in 0..closes.len() {
            if let (Some(p), Some(s), Some(h)) = (ppo_line[i], signal_line[i], histogram[i]) {
                let expected = p - s;
                assert!(
                    (h - expected).abs() < 1e-3,
                    "histogram[{i}]: {h} != {p} - {s} = {expected}"
                );
            }
        }
    }
}
