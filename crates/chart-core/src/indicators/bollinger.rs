use ta::Next;
use ta::indicators::BollingerBands as TaBollinger;

/// Returns three series: (upper, middle, lower).
pub fn compute_bollinger(
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let Ok(mut bb) = TaBollinger::new(period, multiplier as f64) else {
        let empty = vec![None; closes.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut upper = Vec::with_capacity(closes.len());
    let mut middle = Vec::with_capacity(closes.len());
    let mut lower = Vec::with_capacity(closes.len());
    for c in closes {
        let out = bb.next(*c as f64);
        upper.push(Some(out.upper as f32));
        middle.push(Some(out.average as f32));
        lower.push(Some(out.lower as f32));
    }
    (upper, middle, lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_three_series() {
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (upper, middle, lower) = compute_bollinger(&closes, 20, 2.0);
        assert_eq!(upper.len(), 30);
        assert_eq!(middle.len(), 30);
        assert_eq!(lower.len(), 30);
    }

    #[test]
    fn upper_above_lower() {
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (upper, _middle, lower) = compute_bollinger(&closes, 20, 2.0);
        for (u, l) in upper.iter().zip(lower.iter()) {
            if let (Some(u), Some(l)) = (u, l) {
                assert!(u >= l, "upper {u} should be >= lower {l}");
            }
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let (upper, middle, lower) = compute_bollinger(&[], 20, 2.0);
        assert!(upper.is_empty());
        assert!(middle.is_empty());
        assert!(lower.is_empty());
    }

    #[test]
    fn invalid_period_returns_all_none() {
        let closes = vec![1.0f32, 2.0, 3.0];
        // period=0 is invalid for ta::BollingerBands
        let (upper, middle, lower) = compute_bollinger(&closes, 0, 2.0);
        assert!(upper.iter().all(|v| v.is_none()));
        assert!(middle.iter().all(|v| v.is_none()));
        assert!(lower.iter().all(|v| v.is_none()));
    }
}
