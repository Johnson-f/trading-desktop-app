use super::util::session_boundaries;

pub fn compute_vwap(
    highs: &[f32],
    lows: &[f32],
    closes: &[f32],
    volumes: &[f32],
    dates: &[String],
) -> Vec<Option<f32>> {
    let n = highs
        .len()
        .min(lows.len())
        .min(closes.len())
        .min(volumes.len());
    if n == 0 {
        return Vec::new();
    }
    let boundaries = session_boundaries(dates);
    let mut result = vec![None; n];
    let mut cum_tp_vol: f64 = 0.0;
    let mut cum_vol: f64 = 0.0;
    let mut boundary_idx = 0;

    for i in 0..n {
        if boundary_idx + 1 < boundaries.len() && i >= boundaries[boundary_idx + 1] {
            boundary_idx += 1;
            cum_tp_vol = 0.0;
            cum_vol = 0.0;
        }
        let typical_price = (highs[i] + lows[i] + closes[i]) as f64 / 3.0;
        cum_tp_vol += typical_price * volumes[i] as f64;
        cum_vol += volumes[i] as f64;
        if cum_vol > 0.0 {
            result[i] = Some((cum_tp_vol / cum_vol) as f32);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_vwap_calculation() {
        let highs = vec![10.0f32, 11.0, 12.0];
        let lows = vec![8.0f32, 9.0, 10.0];
        let closes = vec![9.0f32, 10.0, 11.0];
        let volumes = vec![100.0f32, 200.0, 300.0];
        let dates: Vec<String> = vec![
            "2024-01-01T09:00:00".to_string(),
            "2024-01-01T09:30:00".to_string(),
            "2024-01-01T10:00:00".to_string(),
        ];
        let result = compute_vwap(&highs, &lows, &closes, &volumes, &dates);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|v| v.is_some()));
        // All on same day, so vwap is cumulative
        // bar0: tp=(10+8+9)/3=9.0, cum_tp_vol=900, cum_vol=100 → 9.0
        assert!((result[0].unwrap() - 9.0).abs() < 1e-3);
        // bar1: tp=(11+9+10)/3=10.0, cum_tp_vol=900+2000=2900, cum_vol=300 → 9.667
        assert!((result[1].unwrap() - (2900.0 / 300.0) as f32).abs() < 1e-3);
    }

    #[test]
    fn resets_on_session_boundary() {
        let highs = vec![10.0f32, 11.0, 20.0, 21.0];
        let lows = vec![8.0f32, 9.0, 18.0, 19.0];
        let closes = vec![9.0f32, 10.0, 19.0, 20.0];
        let volumes = vec![100.0f32, 200.0, 100.0, 200.0];
        let dates: Vec<String> = vec![
            "2024-01-01T09:00:00".to_string(),
            "2024-01-01T09:30:00".to_string(),
            "2024-01-02T09:00:00".to_string(),
            "2024-01-02T09:30:00".to_string(),
        ];
        let result = compute_vwap(&highs, &lows, &closes, &volumes, &dates);
        assert_eq!(result.len(), 4);
        // Day 2 bar0 should only reflect that bar's data
        let day2_bar0_tp = (20.0f32 + 18.0 + 19.0) / 3.0;
        assert!((result[2].unwrap() - day2_bar0_tp).abs() < 1e-3);
    }

    #[test]
    fn empty() {
        let result = compute_vwap(&[], &[], &[], &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn zero_volume() {
        let highs = vec![10.0f32, 11.0];
        let lows = vec![8.0f32, 9.0];
        let closes = vec![9.0f32, 10.0];
        let volumes = vec![0.0f32, 0.0];
        let dates: Vec<String> = vec!["2024-01-01".to_string(), "2024-01-01".to_string()];
        let result = compute_vwap(&highs, &lows, &closes, &volumes, &dates);
        assert_eq!(result.len(), 2);
        // Zero volume → None
        assert!(result[0].is_none());
        assert!(result[1].is_none());
    }
}
