use super::util::build_data_items;
use ta::Next;
use ta::indicators::AverageTrueRange as TaAtr;

pub fn compute_atr(highs: &[f32], lows: &[f32], closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut atr) = TaAtr::new(period) else {
        return vec![None; items.len()];
    };
    items
        .iter()
        .map(|item| Some(atr.next(item) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_data(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut closes = Vec::with_capacity(n);
        for i in 0..n {
            let base = 100.0 + (i as f32) * 0.5;
            highs.push(base + 2.0);
            lows.push(base - 2.0);
            closes.push(base);
        }
        (highs, lows, closes)
    }

    #[test]
    fn produces_values() {
        let (h, l, c) = make_data(30);
        let result = compute_atr(&h, &l, &c, 14);
        assert_eq!(result.len(), 30);
        assert!(result.iter().any(|v| v.is_some()));
    }

    #[test]
    fn positive_values() {
        let (h, l, c) = make_data(30);
        let result = compute_atr(&h, &l, &c, 14);
        for v in result.iter().flatten() {
            assert!(*v > 0.0, "ATR value should be positive, got {}", v);
        }
    }

    #[test]
    fn empty() {
        let result = compute_atr(&[], &[], &[], 14);
        assert!(result.is_empty());
    }

    #[test]
    fn invalid_period() {
        let (h, l, c) = make_data(10);
        let result = compute_atr(&h, &l, &c, 0);
        // period 0 causes TaAtr::new to fail — should return all None
        assert_eq!(result.len(), 10);
        assert!(result.iter().all(|v| v.is_none()));
    }
}
