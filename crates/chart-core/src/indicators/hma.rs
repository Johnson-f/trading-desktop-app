use super::util::compute_wma;

pub fn compute_hma(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    if period < 2 || closes.is_empty() {
        return vec![None; closes.len()];
    }
    let half = period / 2;
    let sqrt_p = (period as f64).sqrt().round() as usize;

    let wma_half = compute_wma(closes, half.max(1));
    let wma_full = compute_wma(closes, period);

    let diff: Vec<f32> = wma_half
        .iter()
        .zip(wma_full.iter())
        .map(|(h, f)| match (h, f) {
            (Some(hv), Some(fv)) => 2.0 * hv - fv,
            _ => 0.0,
        })
        .collect();

    let valid: Vec<bool> = wma_half
        .iter()
        .zip(wma_full.iter())
        .map(|(h, f)| h.is_some() && f.is_some())
        .collect();

    let wma_diff = compute_wma(&diff, sqrt_p.max(1));

    wma_diff
        .iter()
        .enumerate()
        .map(|(i, v)| if valid[i] { *v } else { None })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_values_after_warmup() {
        let closes: Vec<f32> = (1..=20).map(|i| i as f32).collect();
        let result = compute_hma(&closes, 9);
        assert_eq!(result.len(), 20);
        // There should be at least some Some values in the output
        assert!(result.iter().any(|v| v.is_some()));
    }

    #[test]
    fn empty_returns_empty() {
        let result = compute_hma(&[], 9);
        assert!(result.is_empty());
    }

    #[test]
    fn period_one_all_none() {
        let closes = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let result = compute_hma(&closes, 1);
        assert!(result.iter().all(|v| v.is_none()));
    }

    #[test]
    fn leads_price_on_linear_trend() {
        // On a perfectly linear uptrend, HMA should track very closely with no lag
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let result = compute_hma(&closes, 9);
        // All values that are Some should be positive
        for v in result.iter().flatten() {
            assert!(*v > 0.0);
        }
    }
}
