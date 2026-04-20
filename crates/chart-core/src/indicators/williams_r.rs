use super::util::{rolling_high, rolling_low};

pub fn compute_williams_r(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> Vec<Option<f32>> {
    let rh = rolling_high(highs, period);
    let rl = rolling_low(lows, period);
    let n = closes.len().min(rh.len()).min(rl.len());
    (0..n)
        .map(|i| match (rh[i], rl[i]) {
            (Some(hh), Some(ll)) => {
                let range = hh - ll;
                if range.abs() < 1e-10 {
                    Some(-50.0)
                } else {
                    Some((hh - closes[i]) / range * -100.0)
                }
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_in_negative_100_to_0() {
        let highs: Vec<f32> = (1..=50).map(|i| i as f32 + 5.0).collect();
        let lows: Vec<f32> = (1..=50).map(|i| i as f32 - 5.0).collect();
        let closes: Vec<f32> = (1..=50).map(|i| i as f32).collect();
        let result = compute_williams_r(&highs, &lows, &closes, 14);
        for v in result.iter().flatten() {
            assert!(
                *v >= -100.0 && *v <= 0.0,
                "Williams %R out of range: {v}"
            );
        }
    }

    #[test]
    fn valid_after_warmup() {
        let highs: Vec<f32> = (1..=30).map(|i| i as f32 + 2.0).collect();
        let lows: Vec<f32> = (1..=30).map(|i| i as f32 - 2.0).collect();
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let period = 14;
        let result = compute_williams_r(&highs, &lows, &closes, period);
        assert_eq!(result.len(), closes.len());
        // First period-1 bars should be None
        for i in 0..(period - 1) {
            assert!(result[i].is_none(), "expected None at index {i}");
        }
        // After warmup should have values
        for i in (period - 1)..result.len() {
            assert!(result[i].is_some(), "expected Some at index {i}");
        }
    }

    #[test]
    fn empty() {
        let result = compute_williams_r(&[], &[], &[], 14);
        assert!(result.is_empty());
    }
}
