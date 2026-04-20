pub fn compute_alma(closes: &[f32], period: usize, offset: f32, sigma: f32) -> Vec<Option<f32>> {
    if period < 2 || closes.is_empty() || sigma <= 0.0 {
        return vec![None; closes.len()];
    }

    let m = offset * (period as f32 - 1.0);
    let s = period as f32 / sigma;

    let weights: Vec<f32> = (0..period)
        .map(|i| {
            let diff = i as f32 - m;
            (-diff * diff / (2.0 * s * s)).exp()
        })
        .collect();
    let weight_sum: f32 = weights.iter().sum();

    closes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 1 < period {
                return None;
            }
            let start = i + 1 - period;
            let val: f32 = closes[start..=i]
                .iter()
                .zip(weights.iter())
                .map(|(c, w)| c * w)
                .sum::<f32>()
                / weight_sum;
            Some(val)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alma_basic() {
        // 20 bars, period 9: first 8 should be None, bar index 8 (9th) should be Some
        let closes: Vec<f32> = (1..=20).map(|i| i as f32).collect();
        let result = compute_alma(&closes, 9, 0.85, 6.0);
        assert_eq!(result.len(), 20);
        for i in 0..8 {
            assert!(result[i].is_none(), "bar {} should be None", i);
        }
        assert!(result[8].is_some(), "bar 8 should be Some");
    }

    #[test]
    fn offset_one_weights_recent() {
        // With offset=1.0, weights peak at the last bar in the window (most recent)
        // The result should be close to the most recent close
        let closes = vec![1.0f32, 1.0, 1.0, 1.0, 10.0];
        let result = compute_alma(&closes, 5, 1.0, 6.0);
        assert_eq!(result.len(), 5);
        // The last bar (index 4) should be Some and heavily weighted toward 10.0
        let v = result[4].unwrap();
        assert!(v > 5.0, "with offset=1 and last bar=10, expected > 5, got {}", v);
    }

    #[test]
    fn empty_returns_empty() {
        let result = compute_alma(&[], 9, 0.85, 6.0);
        assert!(result.is_empty());
    }

    #[test]
    fn period_one_all_none() {
        let closes = vec![1.0f32, 2.0, 3.0];
        let result = compute_alma(&closes, 1, 0.85, 6.0);
        assert!(result.iter().all(|v| v.is_none()));
    }
}
