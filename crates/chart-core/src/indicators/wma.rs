use super::util::compute_wma;

pub fn compute_wma_indicator(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    compute_wma(closes, period)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wma_delegates_correctly() {
        // period=3, divisor=6
        // index 2: weights 1,2,3 → (1·1 + 2·2 + 3·3)/6 = 14/6
        let closes = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let result = compute_wma_indicator(&closes, 3);
        assert_eq!(result.len(), 5);
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!((result[2].unwrap() - 14.0 / 6.0).abs() < 1e-5);
    }

    #[test]
    fn wma_returns_values() {
        let closes: Vec<f32> = (1..=20).map(|i| i as f32).collect();
        let result = compute_wma_indicator(&closes, 5);
        assert_eq!(result.len(), 20);
        // After warmup (period-1=4 bars), all remaining bars should have values
        assert!(result[4..].iter().all(|v| v.is_some()));
    }

    #[test]
    fn wma_empty_returns_empty() {
        let result = compute_wma_indicator(&[], 3);
        assert!(result.is_empty());
    }

    #[test]
    fn wma_period_zero_returns_all_none() {
        let closes = vec![1.0f32, 2.0, 3.0];
        let result = compute_wma_indicator(&closes, 0);
        assert!(result.iter().all(|v| v.is_none()));
    }
}
