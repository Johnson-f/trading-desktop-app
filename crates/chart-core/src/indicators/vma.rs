use ta::Next;
use ta::indicators::SimpleMovingAverage;

/// Volume Moving Average — SMA of volumes. Thin adapter over
/// `ta::SimpleMovingAverage`. ta returns a growing-window mean until the
/// window fills (at bar `period - 1`), then the rolling mean of the last
/// `period` samples.
///
/// If `ta::SimpleMovingAverage::new(period)` rejects the period (period must
/// be >= 1), the function returns all `None`.
pub fn compute_vma(volumes: &[f32], period: usize) -> Vec<Option<f32>> {
    let Ok(mut sma) = SimpleMovingAverage::new(period) else {
        return vec![None; volumes.len()];
    };
    volumes
        .iter()
        .map(|v| Some(sma.next(*v as f64) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vma_partial_window_then_rolling_mean() {
        // ta returns the mean of whatever it has seen so far until the window
        // fills at bar `period - 1`.
        let vols = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let vma = compute_vma(&vols, 3);
        assert!((vma[0].unwrap() - 10.0).abs() < 1e-5); // mean of {10}
        assert!((vma[1].unwrap() - 15.0).abs() < 1e-5); // mean of {10,20}
        assert!((vma[2].unwrap() - 20.0).abs() < 1e-5); // mean of {10,20,30}
        assert!((vma[3].unwrap() - 30.0).abs() < 1e-5); // mean of {20,30,40}
        assert!((vma[4].unwrap() - 40.0).abs() < 1e-5); // mean of {30,40,50}
    }

    #[test]
    fn vma_empty_input_returns_empty() {
        let vma = compute_vma(&[], 5);
        assert!(vma.is_empty());
    }

    #[test]
    fn vma_period_zero_returns_all_none() {
        let vols = vec![10.0, 20.0];
        let vma = compute_vma(&vols, 0);
        assert!(vma.iter().all(|v| v.is_none()));
    }

    #[test]
    fn vma_every_bar_has_a_value() {
        let vols = vec![10.0, 20.0];
        let vma = compute_vma(&vols, 5);
        assert!(vma.iter().all(|v| v.is_some()));
    }
}
