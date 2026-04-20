use ta::Next;
use ta::indicators::ChandelierExit as TaChandelier;
use super::util::build_data_items;

/// Returns two series: (long_exit, short_exit).
pub fn compute_chandelier(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut ce) = TaChandelier::new(period, multiplier as f64) else {
        let empty = vec![None; items.len()];
        return (empty.clone(), empty);
    };
    let mut long_exit = Vec::with_capacity(items.len());
    let mut short_exit = Vec::with_capacity(items.len());
    for item in &items {
        let out = ce.next(item);
        long_exit.push(Some(out.long as f32));
        short_exit.push(Some(out.short as f32));
    }
    (long_exit, short_exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_two_series() {
        let highs = vec![10.0f32; 30];
        let lows = vec![5.0f32; 30];
        let closes = vec![7.0f32; 30];
        let (long, short) = compute_chandelier(&highs, &lows, &closes, 22, 3.0);
        assert_eq!(long.len(), 30);
        assert_eq!(short.len(), 30);
        assert!(long.iter().all(|v| v.is_some()));
        assert!(short.iter().all(|v| v.is_some()));
    }

    #[test]
    fn long_above_short_generally() {
        // With upward-trending prices, long exit should be below short exit
        // (long = max - atr*mult, short = min + atr*mult)
        let highs: Vec<f32> = (1..=30).map(|x| x as f32 + 5.0).collect();
        let lows: Vec<f32> = (1..=30).map(|x| x as f32).collect();
        let closes: Vec<f32> = (1..=30).map(|x| x as f32 + 2.5).collect();
        let (long, short) = compute_chandelier(&highs, &lows, &closes, 10, 3.0);
        // At least the last value should have long < short (normal CE behavior)
        let last_long = long.last().and_then(|v| *v).unwrap();
        let last_short = short.last().and_then(|v| *v).unwrap();
        assert!(last_long < last_short, "long={} short={}", last_long, last_short);
    }

    #[test]
    fn empty() {
        let (long, short) = compute_chandelier(&[], &[], &[], 22, 3.0);
        assert!(long.is_empty());
        assert!(short.is_empty());
    }
}
