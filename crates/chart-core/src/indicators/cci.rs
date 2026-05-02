use super::util::build_data_items;
use ta::Next;
use ta::indicators::CommodityChannelIndex as TaCci;

pub fn compute_cci(highs: &[f32], lows: &[f32], closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut cci) = TaCci::new(period) else {
        return vec![None; items.len()];
    };
    items
        .iter()
        .map(|item| Some(cci.next(item) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_values() {
        let highs: Vec<f32> = (10..=40).map(|i| i as f32 + 2.0).collect();
        let lows: Vec<f32> = (10..=40).map(|i| i as f32 - 2.0).collect();
        let closes: Vec<f32> = (10..=40).map(|i| i as f32).collect();
        let result = compute_cci(&highs, &lows, &closes, 20);
        assert_eq!(result.len(), closes.len());
        assert!(result.iter().all(|v| v.is_some()));
    }

    #[test]
    fn empty() {
        let result = compute_cci(&[], &[], &[], 20);
        assert!(result.is_empty());
    }

    #[test]
    fn invalid_period() {
        let highs = vec![10.0f32, 11.0, 12.0];
        let lows = vec![8.0f32, 9.0, 10.0];
        let closes = vec![9.0f32, 10.0, 11.0];
        let result = compute_cci(&highs, &lows, &closes, 0);
        assert_eq!(result.len(), closes.len());
        assert!(result.iter().all(|v| v.is_none()));
    }
}
