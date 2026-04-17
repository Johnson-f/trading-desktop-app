/// Simple moving average of volumes over `period`.
///
/// First `period - 1` entries are `None`; after that, `vma[i]` is the mean of
/// `volumes[i - (period - 1)..=i]`. If `period == 0` or `volumes.len() < period`,
/// returns all `None`.
pub fn compute_vma(volumes: &[f32], period: usize) -> Vec<Option<f32>> {
    let n = volumes.len();
    let mut out = vec![None; n];
    if period == 0 || n < period {
        return out;
    }
    let mut sum: f32 = volumes[..period].iter().sum();
    out[period - 1] = Some(sum / period as f32);
    for i in period..n {
        sum += volumes[i] - volumes[i - period];
        out[i] = Some(sum / period as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vma_has_none_prefix_then_sma() {
        let vols = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let vma = compute_vma(&vols, 3);
        assert!(vma[0].is_none());
        assert!(vma[1].is_none());
        assert!((vma[2].unwrap() - 20.0).abs() < 1e-5);
        assert!((vma[3].unwrap() - 30.0).abs() < 1e-5);
        assert!((vma[4].unwrap() - 40.0).abs() < 1e-5);
    }

    #[test]
    fn vma_empty_when_data_shorter_than_period() {
        let vols = vec![10.0, 20.0];
        let vma = compute_vma(&vols, 5);
        assert!(vma.iter().all(|v| v.is_none()));
    }
}
