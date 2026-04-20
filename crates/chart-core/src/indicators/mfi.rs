use ta::Next;
use ta::indicators::MoneyFlowIndex as TaMfi;
use super::util::build_data_items;

pub fn compute_mfi(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    volumes: &[f32],
    period: usize,
) -> Vec<Option<f32>> {
    let items = build_data_items(highs, lows, closes, Some(volumes));
    let Ok(mut mfi) = TaMfi::new(period) else {
        return vec![None; items.len()];
    };
    items.iter().map(|item| Some(mfi.next(item) as f32)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_values() {
        let highs: Vec<f32> = (1..=30).map(|i| i as f32 * 1.1).collect();
        let lows: Vec<f32> = (1..=30).map(|i| i as f32 * 0.9).collect();
        let closes: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let volumes: Vec<f32> = vec![1000.0; 30];
        let result = compute_mfi(&highs, &lows, &closes, &volumes, 14);
        assert_eq!(result.len(), 30);
        assert!(result.iter().all(|v| v.is_some()));
    }

    #[test]
    fn values_in_0_100() {
        let highs: Vec<f32> = (1..=50).map(|i| i as f32 * 1.2).collect();
        let lows: Vec<f32> = (1..=50).map(|i| i as f32 * 0.8).collect();
        let closes: Vec<f32> = (1..=50).map(|i| i as f32).collect();
        let volumes: Vec<f32> = (1..=50).map(|i| (i * 100) as f32).collect();
        let result = compute_mfi(&highs, &lows, &closes, &volumes, 14);
        for v in result.iter().flatten() {
            assert!(*v >= 0.0 && *v <= 100.0, "MFI out of range: {}", v);
        }
    }

    #[test]
    fn empty() {
        let result = compute_mfi(&[], &[], &[], &[], 14);
        assert!(result.is_empty());
    }

    #[test]
    fn invalid_period() {
        let highs = vec![1.0f32, 2.0, 3.0];
        let lows = vec![0.5f32, 1.0, 1.5];
        let closes = vec![1.0f32, 1.5, 2.0];
        let volumes = vec![100.0f32; 3];
        let result = compute_mfi(&highs, &lows, &closes, &volumes, 0);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|v| v.is_none()));
    }
}
