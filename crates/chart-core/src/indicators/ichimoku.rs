use super::util::{rolling_high, rolling_low};

/// Returns five series: (tenkan, kijun, senkou_a, senkou_b, chikou).
/// senkou_a/senkou_b are displaced forward by kijun_period bars (series is longer than input).
/// chikou is displaced backward (same length as input).
pub fn compute_ichimoku(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    tenkan_period: usize,
    kijun_period: usize,
    senkou_b_period: usize,
) -> (
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
    Vec<Option<f32>>,
) {
    let n = highs.len().min(lows.len()).min(closes.len());
    let displacement = kijun_period;

    let th = rolling_high(highs, tenkan_period);
    let tl = rolling_low(lows, tenkan_period);
    let tenkan: Vec<Option<f32>> = th
        .iter()
        .zip(tl.iter())
        .map(|(h, l)| match (h, l) {
            (Some(hv), Some(lv)) => Some((hv + lv) / 2.0),
            _ => None,
        })
        .collect();

    let kh = rolling_high(highs, kijun_period);
    let kl = rolling_low(lows, kijun_period);
    let kijun: Vec<Option<f32>> = kh
        .iter()
        .zip(kl.iter())
        .map(|(h, l)| match (h, l) {
            (Some(hv), Some(lv)) => Some((hv + lv) / 2.0),
            _ => None,
        })
        .collect();

    // Senkou A: (tenkan + kijun) / 2, displaced forward
    let total_len = n + displacement;
    let mut senkou_a = vec![None; total_len];
    for i in 0..n {
        senkou_a[i + displacement] = match (tenkan.get(i), kijun.get(i)) {
            (Some(Some(t)), Some(Some(k))) => Some((t + k) / 2.0),
            _ => None,
        };
    }

    // Senkou B: rolling high/low of senkou_b_period, displaced forward
    let sh = rolling_high(highs, senkou_b_period);
    let sl = rolling_low(lows, senkou_b_period);
    let mut senkou_b = vec![None; total_len];
    for i in 0..n {
        senkou_b[i + displacement] = match (sh.get(i), sl.get(i)) {
            (Some(Some(hv)), Some(Some(lv))) => Some((hv + lv) / 2.0),
            _ => None,
        };
    }

    // Chikou: close displaced backward
    let mut chikou = vec![None; n];
    for i in displacement..n {
        chikou[i - displacement] = Some(closes[i]);
    }

    (tenkan, kijun, senkou_a, senkou_b, chikou)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_five_series() {
        let highs = vec![10.0f32; 30];
        let lows = vec![5.0f32; 30];
        let closes = vec![7.0f32; 30];
        let (tenkan, kijun, senkou_a, senkou_b, chikou) =
            compute_ichimoku(&highs, &lows, &closes, 9, 26, 52);
        assert_eq!(tenkan.len(), 30);
        assert_eq!(kijun.len(), 30);
        assert_eq!(senkou_a.len(), 56); // 30 + 26
        assert_eq!(senkou_b.len(), 56);
        assert_eq!(chikou.len(), 30);
    }

    #[test]
    fn senkou_series_are_longer_than_input() {
        let highs = vec![10.0f32; 60];
        let lows = vec![5.0f32; 60];
        let closes = vec![7.0f32; 60];
        let (_, _, senkou_a, senkou_b, _) = compute_ichimoku(&highs, &lows, &closes, 9, 26, 52);
        assert!(senkou_a.len() > 60);
        assert!(senkou_b.len() > 60);
        assert_eq!(senkou_a.len(), 86);
        assert_eq!(senkou_b.len(), 86);
    }

    #[test]
    fn tenkan_shorter_period_than_kijun() {
        let highs: Vec<f32> = (1..=20).map(|x| x as f32 + 5.0).collect();
        let lows: Vec<f32> = (1..=20).map(|x| x as f32).collect();
        let closes: Vec<f32> = (1..=20).map(|x| x as f32 + 2.5).collect();
        let (tenkan, kijun, _, _, _) = compute_ichimoku(&highs, &lows, &closes, 3, 9, 26);
        // tenkan becomes Some after period-1 = 2 bars
        assert!(tenkan[1].is_none());
        assert!(tenkan[2].is_some());
        // kijun becomes Some after period-1 = 8 bars
        assert!(kijun[7].is_none());
        assert!(kijun[8].is_some());
    }

    #[test]
    fn empty() {
        let (tenkan, kijun, senkou_a, senkou_b, chikou) =
            compute_ichimoku(&[], &[], &[], 9, 26, 52);
        assert!(tenkan.is_empty());
        assert!(kijun.is_empty());
        assert_eq!(senkou_a.len(), 26); // 0 + 26
        assert_eq!(senkou_b.len(), 26);
        assert!(chikou.is_empty());
    }
}
