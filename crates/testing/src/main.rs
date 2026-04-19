use markets::{Interval, Ticker, TimeRange};
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ticker = Ticker::new("AAPL").await?;

    let chart = ticker.chart(Interval::OneDay, TimeRange::Max).await?;

    let candles: Vec<serde_json::Value> = chart
        .candles
        .iter()
        .map(|c| {
            let date = chrono::DateTime::from_timestamp(c.timestamp, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| c.timestamp.to_string());

            serde_json::json!({
                "date": date,
                "open": c.open,
                "high": c.high,
                "low": c.low,
                "close": c.close,
                "volume": c.volume,
            })
        })
        .collect();

    let json = serde_json::to_string_pretty(&candles)?;
    fs::write("AAPL.json", &json)?;

    println!("Wrote {} candles to AAPL.json", candles.len());

    Ok(())
}
