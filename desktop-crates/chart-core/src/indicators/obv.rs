pub fn compute_obv(closes: &[f32], volumes: &[f32]) -> Vec<Option<f32>> {
    let n = closes.len().min(volumes.len());
    if n == 0 {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(n);
    let mut obv: f64 = 0.0;
    result.push(Some(0.0f32));
    for i in 1..n {
        if closes[i] > closes[i - 1] {
            obv += volumes[i] as f64;
        } else if closes[i] < closes[i - 1] {
            obv -= volumes[i] as f64;
        }
        result.push(Some(obv as f32));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_closes_increase_obv() {
        let closes = vec![1.0f32, 2.0, 3.0, 4.0];
        let volumes = vec![100.0f32, 200.0, 300.0, 400.0];
        let result = compute_obv(&closes, &volumes);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Some(0.0));
        assert_eq!(result[1], Some(200.0));
        assert_eq!(result[2], Some(500.0));
        assert_eq!(result[3], Some(900.0));
    }

    #[test]
    fn falling_closes_decrease_obv() {
        let closes = vec![4.0f32, 3.0, 2.0, 1.0];
        let volumes = vec![100.0f32, 200.0, 300.0, 400.0];
        let result = compute_obv(&closes, &volumes);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Some(0.0));
        assert_eq!(result[1], Some(-200.0));
        assert_eq!(result[2], Some(-500.0));
        assert_eq!(result[3], Some(-900.0));
    }

    #[test]
    fn flat_closes_no_change() {
        let closes = vec![5.0f32, 5.0, 5.0, 5.0];
        let volumes = vec![100.0f32, 200.0, 300.0, 400.0];
        let result = compute_obv(&closes, &volumes);
        assert_eq!(result.len(), 4);
        for v in result.iter().flatten() {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn empty() {
        let result = compute_obv(&[], &[]);
        assert!(result.is_empty());
    }
}
