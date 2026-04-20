use ta::Next;
use ta::indicators::SimpleMovingAverage;

pub fn compute_sma(closes: &[f32], period: usize) -> Vec<Option<f32>> {
    let Ok(mut sma) = SimpleMovingAverage::new(period) else {
        return vec![None; closes.len()];
    };
    closes
        .iter()
        .map(|c| Some(sma.next(*c as f64) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_rolling_mean() {
        // period=3; rolling mean of [1,2,3,4,5]
        // ta SMA seeds at first value and accumulates until period bars seen:
        // bar0: 1/1=1, bar1: (1+2)/2=1.5, bar2: (1+2+3)/3=2, bar3: (2+3+4)/3=3, bar4: (3+4+5)/3=4
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = compute_sma(&closes, 3);
        assert_eq!(result.len(), 5);
        assert!((result[2].unwrap() - 2.0).abs() < 1e-4);
        assert!((result[3].unwrap() - 3.0).abs() < 1e-4);
        assert!((result[4].unwrap() - 4.0).abs() < 1e-4);
    }

    #[test]
    fn sma_empty() {
        let result = compute_sma(&[], 3);
        assert!(result.is_empty());
    }

    #[test]
    fn sma_period_zero() {
        let closes = vec![1.0, 2.0, 3.0];
        let result = compute_sma(&closes, 0);
        assert!(result.iter().all(|v| v.is_none()));
    }

    #[test]
    fn sma_every_bar_has_value() {
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = compute_sma(&closes, 3);
        assert!(result.iter().all(|v| v.is_some()));
    }
}
