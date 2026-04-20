use ta::Next;
use ta::indicators::RateOfChange as TaRoc;

pub fn compute_roc(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let Ok(mut roc) = TaRoc::new(period) else {
        return vec![None; closes.len()];
    };
    closes.iter().map(|c| Some(roc.next(*c as f64) as f32)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_values() {
        let closes: Vec<f32> = (1..=30).map(|i| i as f32 * 1.5).collect();
        let result = compute_roc(&closes, 12);
        assert_eq!(result.len(), closes.len());
        assert!(result.iter().all(|v| v.is_some()));
    }

    #[test]
    fn empty() {
        let result = compute_roc(&[], 12);
        assert!(result.is_empty());
    }

    #[test]
    fn invalid_period() {
        let closes = vec![1.0f32, 2.0, 3.0];
        let result = compute_roc(&closes, 0);
        assert_eq!(result.len(), closes.len());
        assert!(result.iter().all(|v| v.is_none()));
    }
}
