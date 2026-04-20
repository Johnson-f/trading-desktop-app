use ta::Next;
use ta::indicators::KeltnerChannel as TaKeltner;
use super::util::build_data_items;

/// Returns three series: (upper, middle, lower).
pub fn compute_keltner(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut kc) = TaKeltner::new(period, multiplier as f64) else {
        let empty = vec![None; items.len()];
        return (empty.clone(), empty.clone(), empty);
    };
    let mut upper = Vec::with_capacity(items.len());
    let mut middle = Vec::with_capacity(items.len());
    let mut lower = Vec::with_capacity(items.len());
    for item in &items {
        let out = kc.next(item);
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
        let highs: Vec<f32> = (1..=30).map(|i| i as f32 + 1.0).collect();
        let lows: Vec<f32> = (1..=30).map(|i| i as f32 - 1.0).collect();
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (upper, middle, lower) = compute_keltner(&highs, &lows, &closes, 20, 2.0);
        assert_eq!(upper.len(), 30);
        assert_eq!(middle.len(), 30);
        assert_eq!(lower.len(), 30);
    }

    #[test]
    fn upper_above_lower() {
        let highs: Vec<f32> = (1..=30).map(|i| i as f32 + 1.0).collect();
        let lows: Vec<f32> = (1..=30).map(|i| i as f32 - 1.0).collect();
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let (upper, _middle, lower) = compute_keltner(&highs, &lows, &closes, 20, 2.0);
        for (u, l) in upper.iter().zip(lower.iter()) {
            if let (Some(u), Some(l)) = (u, l) {
                assert!(u >= l, "upper {u} should be >= lower {l}");
            }
        }
    }
}
