use chrono::{DateTime, Datelike, TimeZone, Utc};

/// Compute the next instant when the clock reaches `hour_utc:00:00 UTC`
/// strictly after `now`. Used to schedule the next daily sync.
pub fn next_run_at(now: DateTime<Utc>, hour_utc: u32) -> DateTime<Utc> {
    let today = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), hour_utc, 0, 0)
        .single()
        .expect("valid hour");
    if today > now {
        today
    } else {
        today + chrono::Duration::days(1)
    }
}

/// Sleep until `next_run_at(now, hour_utc)`. Cancellation-safe — caller
/// should `select!` this against a shutdown signal.
pub async fn wait_until_next_run(hour_utc: u32) {
    let now = Utc::now();
    let target = next_run_at(now, hour_utc);
    let dur = (target - now).to_std().unwrap_or(std::time::Duration::ZERO);
    tracing::info!(hour_utc, ?dur, %target, "sleeping until next run");
    tokio::time::sleep(dur).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_today_if_hour_not_yet_passed() {
        let now = Utc.with_ymd_and_hms(2026, 5, 3, 1, 30, 0).unwrap();
        let next = next_run_at(now, 2);
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 5, 3, 2, 0, 0).unwrap());
    }

    #[test]
    fn rolls_to_tomorrow_if_hour_already_passed() {
        let now = Utc.with_ymd_and_hms(2026, 5, 3, 5, 0, 0).unwrap();
        let next = next_run_at(now, 2);
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 5, 4, 2, 0, 0).unwrap());
    }

    #[test]
    fn rolls_to_tomorrow_if_exactly_at_hour() {
        let now = Utc.with_ymd_and_hms(2026, 5, 3, 2, 0, 0).unwrap();
        let next = next_run_at(now, 2);
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 5, 4, 2, 0, 0).unwrap());
    }
}
