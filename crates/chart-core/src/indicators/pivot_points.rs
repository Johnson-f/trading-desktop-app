use super::util::session_boundaries;

/// pivot_type: 0=Standard, 1=Fibonacci, 2=Woodie.
/// Returns 7 series: (pivot, r1, r2, r3, s1, s2, s3).
pub fn compute_pivot_points(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    dates: &[String],
    pivot_type: i32,
) -> (Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>, Vec<Option<f32>>) {
    let n = highs.len().min(lows.len()).min(closes.len());
    let boundaries = session_boundaries(dates);
    let mut pivot = vec![None; n];
    let mut r1 = vec![None; n];
    let mut r2 = vec![None; n];
    let mut r3 = vec![None; n];
    let mut s1 = vec![None; n];
    let mut s2 = vec![None; n];
    let mut s3 = vec![None; n];

    for sess_idx in 1..boundaries.len() {
        let prev_start = boundaries[sess_idx - 1];
        let prev_end = boundaries[sess_idx];

        let mut ph = f32::MIN;
        let mut pl = f32::MAX;
        for i in prev_start..prev_end {
            if i < n {
                if highs[i] > ph { ph = highs[i]; }
                if lows[i] < pl { pl = lows[i]; }
            }
        }
        let pc = if prev_end > 0 && prev_end - 1 < n { closes[prev_end - 1] } else { continue };

        let (p, r1v, r2v, r3v, s1v, s2v, s3v) = match pivot_type {
            1 => { // Fibonacci
                let p = (ph + pl + pc) / 3.0;
                let range = ph - pl;
                (p, p + range * 0.382, p + range * 0.618, p + range, p - range * 0.382, p - range * 0.618, p - range)
            }
            2 => { // Woodie
                let p = (ph + pl + 2.0 * pc) / 4.0;
                (p, 2.0 * p - pl, p + (ph - pl), ph + 2.0 * (p - pl), 2.0 * p - ph, p - (ph - pl), pl - 2.0 * (ph - p))
            }
            _ => { // Standard
                let p = (ph + pl + pc) / 3.0;
                (p, 2.0 * p - pl, p + (ph - pl), ph + 2.0 * (p - pl), 2.0 * p - ph, p - (ph - pl), pl - 2.0 * (ph - p))
            }
        };

        let curr_end = if sess_idx + 1 < boundaries.len() { boundaries[sess_idx + 1] } else { n };
        for i in boundaries[sess_idx]..curr_end.min(n) {
            pivot[i] = Some(p);
            r1[i] = Some(r1v);
            r2[i] = Some(r2v);
            r3[i] = Some(r3v);
            s1[i] = Some(s1v);
            s2[i] = Some(s2v);
            s3[i] = Some(s3v);
        }
    }

    (pivot, r1, r2, r3, s1, s2, s3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dates(n: usize, days: usize) -> Vec<String> {
        let mut dates = Vec::new();
        for d in 0..days {
            let bars_per_day = n / days;
            for _ in 0..bars_per_day {
                dates.push(format!("2024-01-{:02}", d + 1));
            }
        }
        // Fill remainder
        while dates.len() < n {
            dates.push(format!("2024-01-{:02}", days));
        }
        dates
    }

    #[test]
    fn standard_pivot_basic() {
        // 2 days of data, 4 bars each
        let highs = vec![10.0f32; 8];
        let lows = vec![5.0f32; 8];
        let closes = vec![7.0f32; 8];
        let dates = make_dates(8, 2);
        let (pivot, r1, _, _, s1, _, _) =
            compute_pivot_points(&highs, &lows, &closes, &dates, 0);
        // First session has no pivots (no previous session)
        assert!(pivot[0].is_none());
        // Second session should have pivot values
        let p = pivot[4].unwrap();
        let expected_p = (10.0 + 5.0 + 7.0) / 3.0;
        assert!((p - expected_p).abs() < 0.01, "p={} expected={}", p, expected_p);
        let r1v = r1[4].unwrap();
        let expected_r1 = 2.0 * expected_p - 5.0;
        assert!((r1v - expected_r1).abs() < 0.01);
        let s1v = s1[4].unwrap();
        let expected_s1 = 2.0 * expected_p - 10.0;
        assert!((s1v - expected_s1).abs() < 0.01);
    }

    #[test]
    fn no_levels_for_first_session() {
        let highs = vec![10.0f32; 8];
        let lows = vec![5.0f32; 8];
        let closes = vec![7.0f32; 8];
        let dates = make_dates(8, 2);
        let (pivot, _, _, _, _, _, _) =
            compute_pivot_points(&highs, &lows, &closes, &dates, 0);
        // First 4 bars (session 1) should have no pivot
        for i in 0..4 {
            assert!(pivot[i].is_none(), "bar {} should be None", i);
        }
    }

    #[test]
    fn empty() {
        let (pivot, r1, r2, r3, s1, s2, s3) =
            compute_pivot_points(&[], &[], &[], &[], 0);
        assert!(pivot.is_empty());
        assert!(r1.is_empty());
        assert!(r2.is_empty());
        assert!(r3.is_empty());
        assert!(s1.is_empty());
        assert!(s2.is_empty());
        assert!(s3.is_empty());
    }
}
