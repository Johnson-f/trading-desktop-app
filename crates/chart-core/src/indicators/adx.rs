/// Returns three series: (ADX, +DI, -DI).
pub fn compute_adx(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    period: usize,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let n = highs.len().min(lows.len()).min(closes.len());
    if n < 2 || period == 0 {
        let empty = vec![None; n];
        return (empty.clone(), empty.clone(), empty);
    }

    // Step 1: Calculate True Range, +DM, -DM
    let mut tr = vec![0.0f64; n];
    let mut plus_dm = vec![0.0f64; n];
    let mut minus_dm = vec![0.0f64; n];

    for i in 1..n {
        let h = highs[i] as f64;
        let l = lows[i] as f64;
        let pc = closes[i - 1] as f64;
        tr[i] = (h - l).max((h - pc).abs()).max((l - pc).abs());

        let up = h - highs[i - 1] as f64;
        let down = lows[i - 1] as f64 - l;
        plus_dm[i] = if up > down && up > 0.0 { up } else { 0.0 };
        minus_dm[i] = if down > up && down > 0.0 { down } else { 0.0 };
    }

    // Step 2: Wilder smoothing
    let smooth = |raw: &[f64]| -> Vec<f64> {
        let mut s = vec![0.0; n];
        if period >= n {
            return s;
        }
        let seed: f64 = raw[1..=period.min(n - 1)].iter().sum();
        s[period] = seed;
        for i in (period + 1)..n {
            s[i] = s[i - 1] - s[i - 1] / period as f64 + raw[i];
        }
        s
    };

    let atr_s = smooth(&tr);
    let plus_dm_s = smooth(&plus_dm);
    let minus_dm_s = smooth(&minus_dm);

    // Step 3: +DI, -DI, DX
    let mut di_plus = vec![None; n];
    let mut di_minus = vec![None; n];
    let mut dx = vec![0.0f64; n];

    for i in period..n {
        if atr_s[i] > 0.0 {
            let pdv = (plus_dm_s[i] / atr_s[i] * 100.0) as f32;
            let mdv = (minus_dm_s[i] / atr_s[i] * 100.0) as f32;
            di_plus[i] = Some(pdv);
            di_minus[i] = Some(mdv);
            let sum = pdv + mdv;
            dx[i] = if sum > 0.0 {
                ((pdv - mdv).abs() / sum * 100.0) as f64
            } else {
                0.0
            };
        }
    }

    // Step 4: ADX = Wilder-smoothed DX
    let mut adx = vec![None; n];
    let start = period * 2;
    if start < n {
        let seed: f64 = dx[period..start].iter().sum::<f64>() / period as f64;
        adx[start] = Some(seed as f32);
        for i in (start + 1)..n {
            if let Some(prev) = adx[i - 1] {
                let v = (prev as f64 * (period as f64 - 1.0) + dx[i]) / period as f64;
                adx[i] = Some(v as f32);
            }
        }
    }

    (adx, di_plus, di_minus)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_data(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut closes = Vec::with_capacity(n);
        for i in 0..n {
            let base = 100.0 + (i as f32) * 0.3;
            highs.push(base + 1.5);
            lows.push(base - 1.5);
            closes.push(base);
        }
        (highs, lows, closes)
    }

    #[test]
    fn produces_three_series() {
        let (h, l, c) = make_data(50);
        let (adx, di_plus, di_minus) = compute_adx(&h, &l, &c, 14);
        assert_eq!(adx.len(), 50);
        assert_eq!(di_plus.len(), 50);
        assert_eq!(di_minus.len(), 50);
        assert!(adx.iter().any(|v| v.is_some()));
        assert!(di_plus.iter().any(|v| v.is_some()));
        assert!(di_minus.iter().any(|v| v.is_some()));
    }

    #[test]
    fn adx_values_in_0_100() {
        let (h, l, c) = make_data(100);
        let (adx, di_plus, di_minus) = compute_adx(&h, &l, &c, 14);
        for v in adx.iter().flatten() {
            assert!(*v >= 0.0 && *v <= 100.0, "ADX out of range: {}", v);
        }
        for v in di_plus.iter().flatten() {
            assert!(*v >= 0.0, "+DI should be non-negative: {}", v);
        }
        for v in di_minus.iter().flatten() {
            assert!(*v >= 0.0, "-DI should be non-negative: {}", v);
        }
    }

    #[test]
    fn empty() {
        let (adx, di_plus, di_minus) = compute_adx(&[], &[], &[], 14);
        assert!(adx.is_empty());
        assert!(di_plus.is_empty());
        assert!(di_minus.is_empty());
    }

    #[test]
    fn invalid_period() {
        let (h, l, c) = make_data(20);
        let (adx, di_plus, di_minus) = compute_adx(&h, &l, &c, 0);
        assert_eq!(adx.len(), 20);
        assert!(adx.iter().all(|v| v.is_none()));
        assert!(di_plus.iter().all(|v| v.is_none()));
        assert!(di_minus.iter().all(|v| v.is_none()));
    }
}
