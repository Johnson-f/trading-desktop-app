//! Pure SQL string builder for the `bars` query. Emits a single
//! parameterized statement; placeholders for `symbol`, `from`, `to`,
//! `limit` are supplied via the `clickhouse` crate's bind layer at
//! call time.

use crate::service::historical_service::interval::BucketSpec;

/// Build the SELECT SQL for fetching aggregated bars. The result
/// contains four `?` placeholders, in this order: symbol, from, to,
/// limit.
pub fn build_select_sql(bucket: &BucketSpec) -> String {
    let bucket_expr = bucket.bucket_expr();
    let table = bucket.source_table();
    format!(
        "SELECT \
            {bucket_expr} AS ts, \
            argMin(open, ts) AS open, \
            max(high) AS high, \
            min(low) AS low, \
            argMax(close, ts) AS close, \
            sum(volume) AS volume \
         FROM {table} FINAL \
         WHERE symbol = ? AND ts >= ? AND ts < ? \
         GROUP BY ts \
         ORDER BY ts DESC \
         LIMIT ? \
         SETTINGS do_not_merge_across_partitions_select_final = 1"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::historical_service::interval::BarUnit;

    #[test]
    fn renders_minute_query() {
        let sql = build_select_sql(&BucketSpec::new(BarUnit::Minute, 5).unwrap());
        assert!(sql.contains("toStartOfInterval(ts, INTERVAL 5 MINUTE) AS ts"));
        assert!(sql.contains("FROM bars_1m FINAL"));
        assert!(sql.contains("argMin(open, ts) AS open"));
        assert!(sql.contains("argMax(close, ts) AS close"));
        assert!(sql.contains("max(high) AS high"));
        assert!(sql.contains("min(low) AS low"));
        assert!(sql.contains("sum(volume) AS volume"));
        assert!(sql.contains("WHERE symbol = ? AND ts >= ? AND ts < ?"));
        assert!(sql.contains("GROUP BY ts"));
        assert!(sql.contains("ORDER BY ts DESC"));
        assert!(sql.contains("LIMIT ?"));
        assert!(sql.contains("SETTINGS do_not_merge_across_partitions_select_final = 1"));
    }

    #[test]
    fn renders_day_query_against_bars_1d() {
        let sql = build_select_sql(&BucketSpec::new(BarUnit::Day, 1).unwrap());
        assert!(sql.contains("toStartOfDay(ts) AS ts"));
        assert!(sql.contains("FROM bars_1d FINAL"));
    }

    #[test]
    fn renders_week_query_uses_monday_anchor() {
        let sql = build_select_sql(&BucketSpec::new(BarUnit::Week, 1).unwrap());
        assert!(sql.contains("toStartOfWeek(ts, 1) AS ts"));
        assert!(sql.contains("FROM bars_1d FINAL"));
    }

    #[test]
    fn renders_month_query_uses_helper() {
        let sql = build_select_sql(&BucketSpec::new(BarUnit::Month, 1).unwrap());
        assert!(sql.contains("toStartOfMonth(ts) AS ts"));
    }

    #[test]
    fn placeholder_count_is_four() {
        let sql = build_select_sql(&BucketSpec::new(BarUnit::Minute, 1).unwrap());
        assert_eq!(sql.matches('?').count(), 4);
    }
}
