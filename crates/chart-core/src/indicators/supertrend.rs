use super::util::build_data_items;
use ta::Next;
use ta::indicators::AverageTrueRange;

/// Returns two series: (supertrend_value, direction).
/// Direction: 1.0 = uptrend, -1.0 = downtrend.
pub fn compute_supertrend(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
    multiplier: f32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
    let n = highs.len().min(lows.len()).min(closes.len());
    if n == 0 || period == 0 {
        return (vec![None; n], vec![None; n]);
    }

    let items = build_data_items(highs, lows, closes, None);
    let Ok(mut atr_ind) = AverageTrueRange::new(period) else {
        return (vec![None; n], vec![None; n]);
    };

    let mut atr_vals = Vec::with_capacity(n);
    for item in &items {
        atr_vals.push(atr_ind.next(item) as f32);
    }

    let mut st = vec![None; n];
    let mut dir = vec![None; n];
    let mut upper_band = vec![0.0f32; n];
    let mut lower_band = vec![0.0f32; n];

    for i in 0..n {
        let hl2 = (highs[i] + lows[i]) / 2.0;
        let basic_upper = hl2 + multiplier * atr_vals[i];
        let basic_lower = hl2 - multiplier * atr_vals[i];

        upper_band[i] =
            if i > 0 && (basic_upper < upper_band[i - 1] || closes[i - 1] > upper_band[i - 1]) {
                basic_upper
            } else if i > 0 {
                upper_band[i - 1]
            } else {
                basic_upper
            };

        lower_band[i] =
            if i > 0 && (basic_lower > lower_band[i - 1] || closes[i - 1] < lower_band[i - 1]) {
                basic_lower
            } else if i > 0 {
                lower_band[i - 1]
            } else {
                basic_lower
            };

        if i < period {
            continue;
        }

        if i == period {
            if closes[i] <= upper_band[i] {
                st[i] = Some(upper_band[i]);
                dir[i] = Some(-1.0);
            } else {
                st[i] = Some(lower_band[i]);
                dir[i] = Some(1.0);
            }
            continue;
        }

        let prev_dir = dir[i - 1].unwrap_or(-1.0);
        if prev_dir > 0.0 {
            if closes[i] < lower_band[i] {
                st[i] = Some(upper_band[i]);
                dir[i] = Some(-1.0);
            } else {
                st[i] = Some(lower_band[i]);
                dir[i] = Some(1.0);
            }
        } else {
            if closes[i] > upper_band[i] {
                st[i] = Some(lower_band[i]);
                dir[i] = Some(1.0);
            } else {
                st[i] = Some(upper_band[i]);
                dir[i] = Some(-1.0);
            }
        }
    }

    (st, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_data(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut closes = Vec::with_capacity(n);
        for i in 0..n {
            let base = 100.0 + (i as f32) * 0.2;
            highs.push(base + 1.0);
            lows.push(base - 1.0);
            closes.push(base);
        }
        (highs, lows, closes)
    }

    #[test]
    fn produces_two_series() {
        let (h, l, c) = make_data(30);
        let (st, dir) = compute_supertrend(&h, &l, &c, 10, 3.0);
        assert_eq!(st.len(), 30);
        assert_eq!(dir.len(), 30);
        assert!(st.iter().any(|v| v.is_some()));
        assert!(dir.iter().any(|v| v.is_some()));
    }

    #[test]
    fn direction_is_1_or_neg1() {
        let (h, l, c) = make_data(30);
        let (_st, dir) = compute_supertrend(&h, &l, &c, 10, 3.0);
        for d in dir.iter().flatten() {
            assert!(
                (*d - 1.0).abs() < 1e-6 || (*d + 1.0).abs() < 1e-6,
                "Direction should be 1.0 or -1.0, got {}",
                d
            );
        }
    }

    #[test]
    fn empty() {
        let (st, dir) = compute_supertrend(&[], &[], &[], 10, 3.0);
        assert!(st.is_empty());
        assert!(dir.is_empty());
    }
}
