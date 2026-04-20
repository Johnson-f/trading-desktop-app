pub fn compute_parabolic_sar(
    highs: &[f32],
    lows: &[f32],
    accel_start: f32,
    accel_max: f32,
) -> Vec<Option<f32>> {
    let n = highs.len().min(lows.len());
    if n < 2 {
        return vec![None; n];
    }

    let mut sar = vec![None; n];
    let mut is_long = highs[1] > highs[0];
    let mut af = accel_start;
    let mut ep = if is_long { highs[0] } else { lows[0] };
    let mut current_sar = if is_long { lows[0] } else { highs[0] };

    sar[0] = Some(current_sar);

    for i in 1..n {
        let prev_sar = current_sar;
        current_sar = prev_sar + af * (ep - prev_sar);

        if is_long {
            current_sar = current_sar.min(lows[i - 1]);
            if i >= 2 { current_sar = current_sar.min(lows[i - 2]); }

            if lows[i] < current_sar {
                is_long = false;
                current_sar = ep;
                ep = lows[i];
                af = accel_start;
            } else {
                if highs[i] > ep {
                    ep = highs[i];
                    af = (af + accel_start).min(accel_max);
                }
            }
        } else {
            current_sar = current_sar.max(highs[i - 1]);
            if i >= 2 { current_sar = current_sar.max(highs[i - 2]); }

            if highs[i] > current_sar {
                is_long = true;
                current_sar = ep;
                ep = highs[i];
                af = accel_start;
            } else {
                if lows[i] < ep {
                    ep = lows[i];
                    af = (af + accel_start).min(accel_max);
                }
            }
        }
        sar[i] = Some(current_sar);
    }
    sar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_values() {
        let highs: Vec<f32> = (1..=20).map(|x| x as f32 + 1.0).collect();
        let lows: Vec<f32> = (1..=20).map(|x| x as f32).collect();
        let result = compute_parabolic_sar(&highs, &lows, 0.02, 0.2);
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn all_values_are_some_for_n_ge_2() {
        let highs = vec![10.0f32; 10];
        let lows = vec![5.0f32; 10];
        let result = compute_parabolic_sar(&highs, &lows, 0.02, 0.2);
        assert_eq!(result.len(), 10);
        assert!(result.iter().all(|v| v.is_some()));
    }

    #[test]
    fn empty() {
        let result = compute_parabolic_sar(&[], &[], 0.02, 0.2);
        assert!(result.is_empty());
    }

    #[test]
    fn single_bar() {
        let result = compute_parabolic_sar(&[10.0], &[5.0], 0.02, 0.2);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_none());
    }
}
